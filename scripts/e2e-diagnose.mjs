#!/usr/bin/env node
// Prints what is needed to understand a window-spec run that cannot start a WebDriver session (CI step `if: failure()`).
// It does what tauri-driver does, one piece at a time, with the output on screen:
//   1. versions: WebView2 runtime, msedgedriver / WebKitWebDriver, tauri-driver
//   2. the app alone for 15 s with the test environment: does it stay up, what does it print, how does it end
//   3. Windows only: msedgedriver alone, verbose, one session request with a 60 s limit, then its log
// Synthetic/empty data only. Everything it starts is stopped again (only the PIDs it started).
import { spawn, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const win = process.platform === 'win32';
const exe = win ? 'gene-tree.exe' : 'gene-tree';
const app = process.env.GT_E2E_APP ?? [path.join(root, 'target', 'e2e', 'debug', exe), path.join(root, 'target', 'debug', exe)].find((p) => fs.existsSync(p));
const head = (t) => console.log(`\n===== ${t}`);
const sh = (cmd, args) => {
  const r = spawnSync(cmd, args, { encoding: 'utf8', timeout: 30000 });
  return `${(r.stdout ?? '').trim()} ${(r.stderr ?? '').trim()}`.trim() || `(exit ${r.status}${r.error ? `, ${r.error.message}` : ''})`;
};
const kill = (pid) => (win ? spawnSync('taskkill', ['/PID', String(pid), '/T', '/F'], { stdio: 'ignore' }) : spawnSync('kill', ['-9', String(pid)]));
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const nativeDriver = (() => {
  if (process.env.GT_E2E_EDGEDRIVER) return process.env.GT_E2E_EDGEDRIVER;
  if (win) {
    const local = path.join(root, '.tools', 'msedgedriver', 'msedgedriver.exe');
    return fs.existsSync(local) ? local : sh('where', ['msedgedriver']).split(/\r?\n/)[0];
  }
  return sh('which', ['WebKitWebDriver']);
})();

head('versions');
console.log('app:', app, app && fs.existsSync(app) ? `${(fs.statSync(app).size / 1e6).toFixed(0)} MB` : 'MISSING');
console.log('native driver:', nativeDriver, '->', sh(nativeDriver, ['--version']));
console.log('tauri-driver:', sh(win ? 'where' : 'which', ['tauri-driver']));
if (win) {
  const ps = (c) => sh('powershell', ['-NoProfile', '-Command', c]);
  console.log('WebView2 runtime:', ps("(Get-ItemProperty 'HKLM:\\SOFTWARE\\WOW6432Node\\Microsoft\\EdgeUpdate\\Clients\\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}').pv"));
  console.log('elevated:', ps('([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)'));
  console.log('session id:', ps('[Diagnostics.Process]::GetCurrentProcess().SessionId'));
}

const work = fs.mkdtempSync(path.join(os.tmpdir(), 'gt-diag-'));
const env = { ...process.env, GT_E2E: '1', GT_E2E_CONFIG_DIR: path.join(work, 'config'), WEBVIEW2_USER_DATA_FOLDER: path.join(work, 'webview') };
fs.mkdirSync(env.GT_E2E_CONFIG_DIR, { recursive: true });
fs.writeFileSync(path.join(env.GT_E2E_CONFIG_DIR, 'settings.json'), JSON.stringify({ language: 'en' }));

head('the app alone, 15 s');
if (app && fs.existsSync(app)) {
  let out = '';
  const child = spawn(app, [], { env, stdio: ['ignore', 'pipe', 'pipe'] });
  child.stdout.on('data', (d) => (out += d));
  child.stderr.on('data', (d) => (out += d));
  let exited;
  child.on('exit', (code, signal) => (exited = `exit code ${code} signal ${signal}`));
  child.on('error', (e) => (exited = `could not start: ${e.message}`));
  await sleep(15000);
  console.log(exited ? `The app ENDED on its own: ${exited}` : 'The app is still running after 15 s (good).');
  console.log(out.trim().split(/\r?\n/).slice(-40).join('\n') || '(no output)');
  if (!exited && child.pid) kill(child.pid);
} else console.log('skipped: no app');

if (win && nativeDriver && fs.existsSync(nativeDriver) && app) {
  head('msedgedriver alone, one session request (60 s limit)');
  const port = 41000 + Math.floor(Math.random() * 8000);
  const log = path.join(work, 'msedgedriver.log');
  const drv = spawn(nativeDriver, [`--port=${port}`, '--verbose', `--log-path=${log}`], { env: { ...env, TAURI_WEBVIEW_AUTOMATION: 'true', TAURI_AUTOMATION: 'true' }, stdio: ['ignore', 'pipe', 'inherit'] });
  drv.stdout.resume();
  await sleep(2000);
  const body = { capabilities: { alwaysMatch: { browserName: 'webview2', 'ms:edgeOptions': { binary: app, args: [], webviewOptions: {} } } } };
  const t0 = Date.now();
  try {
    const res = await fetch(`http://127.0.0.1:${port}/session`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body), signal: AbortSignal.timeout(60000) });
    const text = await res.text();
    console.log(`answer after ${Date.now() - t0} ms: HTTP ${res.status} ${text.slice(0, 600)}`);
  } catch (e) {
    console.log(`NO ANSWER after ${Date.now() - t0} ms: ${e.message}`);
  }
  if (drv.pid) kill(drv.pid);
  await sleep(500);
  console.log('--- msedgedriver.log (last 80 lines)');
  console.log(fs.existsSync(log) ? fs.readFileSync(log, 'utf8').split(/\r?\n/).slice(-80).join('\n') : '(no log)');
  console.log('--- Windows Application event log, last 30 minutes, entries that mention the app or WebView2');
  console.log(sh('powershell', ['-NoProfile', '-Command', "Get-WinEvent -FilterHashtable @{LogName='Application'; StartTime=(Get-Date).AddMinutes(-30)} -ErrorAction SilentlyContinue | Where-Object { $_.Message -match 'gene-tree|msedgewebview2|WebView2' } | Select-Object -First 8 | ForEach-Object { $_.TimeCreated.ToString('s') + ' ' + $_.ProviderName + ' ' + (($_.Message -split \"`n\")[0..3] -join ' ') }"]));
}
fs.rmSync(work, { recursive: true, force: true });
