#!/usr/bin/env node
// Prints what is needed to understand a window-spec run that cannot start a WebDriver session (CI step `if: failure()`).
// It does what tauri-driver does, one piece at a time, with the output on screen:
//   1. versions: WebView2 runtime, msedgedriver / WebKitWebDriver, tauri-driver
//   2. the app alone for 15 s with the test environment: does it stay up, what does it print, how does it end
//   3. Windows only: policies, 5 experiments with the app alone (debug port / pipe / folders / flags), then msedgedriver alone
//      (with and without our WEBVIEW2_USER_DATA_FOLDER) with process tree, profile files and DevToolsActivePort printed
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

// ---- Windows evidence helpers (each one answers one hypothesis)
const ps = (c) => sh('powershell', ['-NoProfile', '-Command', c]);
const tree = (label) => {
  console.log(`--- process tree ${label}: gene-tree / msedgedriver / msedgewebview2 with command lines`);
  console.log(ps("Get-CimInstance Win32_Process | Where-Object { $_.Name -match 'gene-tree|msedgedriver|msedgewebview2|tauri-driver' } | ForEach-Object { '{0} parent={1} {2} :: {3}' -f $_.ProcessId, $_.ParentProcessId, $_.Name, $_.CommandLine }"));
};
const findIn = (d, name, depth = 0, out = []) => {
  try {
    for (const e of fs.readdirSync(d, { withFileTypes: true })) {
      const f = path.join(d, e.name);
      if (e.isDirectory() && depth < 4) findIn(f, name, depth + 1, out);
      else if (e.name === name) out.push(f);
    }
  } catch {}
  return out;
};
// Lists the files a folder holds (depth-limited) and prints DevToolsActivePort / the Chromium debug log when present.
const files = (label, dir) => {
  console.log(`--- files in ${label}: ${dir}`);
  if (!dir || !fs.existsSync(dir)) return console.log('(folder does not exist: WebView2 never used it)');
  const lines = [];
  const walk = (d, depth) => {
    for (const e of fs.readdirSync(d, { withFileTypes: true })) {
      const f = path.join(d, e.name);
      if (e.isDirectory()) {
        if (depth < 1 && !/^(Default|Crashpad|GrShaderCache|component_crx_cache|BrowserMetrics)$/.test(e.name)) walk(f, depth + 1);
        else lines.push(`${f}\\ (dir)`);
      } else {
        let n = '?';
        try { n = fs.statSync(f).size; } catch {}
        lines.push(`${f} ${n} B`);
      }
      if (lines.length > 60) return;
    }
  };
  try { walk(dir, 0); } catch (e) { lines.push(`(cannot list: ${e.message})`); }
  console.log(lines.join('\n') || '(empty)');
  for (const f of findIn(dir, 'DevToolsActivePort')) console.log(`DevToolsActivePort ${f}: ${JSON.stringify(fs.readFileSync(f, 'utf8'))}`);
  for (const f of findIn(dir, 'chrome_debug.log')) console.log(`chrome_debug.log ${f} (last 25 lines):\n${fs.readFileSync(f, 'utf8').split(/\r?\n/).slice(-25).join('\n')}`);
};
const driverTemps = () => {
  const t = os.tmpdir();
  // Only the profile folders msedgedriver made in the last 3 minutes (older ones are leftovers of other runs).
  return fs.readdirSync(t).filter((n) => /^scoped_dir/i.test(n)).map((n) => path.join(t, n)).filter((d) => { try { return Date.now() - fs.statSync(d).mtimeMs < 180000; } catch { return false; } }).slice(-3);
};
const probePort = async (port) => {
  try {
    const r = await fetch(`http://127.0.0.1:${port}/json/version`, { signal: AbortSignal.timeout(3000) });
    return `HTTP ${r.status} ${(await r.text()).slice(0, 200)}`;
  } catch (e) {
    return `no answer (${e.message})`;
  }
};

if (win && app && fs.existsSync(app)) {
  head('Windows policies and settings that can block remote debugging or change WebView2');
  for (const k of ['HKLM:\\SOFTWARE\\Policies\\Microsoft\\Edge', 'HKLM:\\SOFTWARE\\Policies\\Microsoft\\Edge\\WebView2', 'HKCU:\\SOFTWARE\\Policies\\Microsoft\\Edge', 'HKLM:\\SOFTWARE\\Policies\\Microsoft\\EdgeUpdate', 'HKLM:\\SOFTWARE\\Policies\\Microsoft\\Edge\\WebView2\\AdditionalBrowserArguments'])
    console.log(`${k}:`, ps(`if (Test-Path '${k}') { (Get-ItemProperty '${k}' | Format-List | Out-String).Trim() } else { '(absent)' }`));
  console.log('env that matters:', ps("Get-ChildItem Env: | Where-Object { $_.Name -match 'WEBVIEW2|TEMP|TMP|LOCALAPPDATA|^CI$|RUNNER_' } | ForEach-Object { $_.Name + '=' + $_.Value }"));
  console.log('Defender:', ps('(Get-MpComputerStatus -ErrorAction SilentlyContinue | Select-Object RealTimeProtectionEnabled,AntivirusEnabled | Out-String).Trim()'));
  console.log('Edge / WebView2 processes running before the experiments:', ps("(Get-Process msedge*,msedgewebview2 -ErrorAction SilentlyContinue | Measure-Object).Count"));

  // Experiments A-E: the app alone with the arguments msedgedriver passes, and WebView2's own answer on the debugging port.
  // If DevToolsActivePort / the port answer here, WebView2 itself is fine and the driver's handshake is the problem.
  const variants = [
    ['A: own user-data folder + --remote-debugging-port=0 (what the driver asks for)', { udf: true, args: '--remote-debugging-port=0 --enable-logging --log-level=0' }],
    ['B: own user-data folder + fixed port 9333', { udf: true, args: '--remote-debugging-port=9333', port: 9333 }],
    ['C: default user-data folder (no WEBVIEW2_USER_DATA_FOLDER) + fixed port 9334', { udf: false, args: '--remote-debugging-port=9334', port: 9334 }],
    ['D: own folder + fixed port + --no-sandbox --disable-gpu (elevation / GPU hypotheses)', { udf: true, args: '--remote-debugging-port=9335 --no-sandbox --disable-gpu', port: 9335 }],
    ['E: own folder + --remote-debugging-pipe (no port at all)', { udf: true, args: '--remote-debugging-pipe --enable-logging --log-level=0' }],
  ];
  for (const [label, v] of variants) {
    head(`app alone, experiment ${label}`);
    const folder = path.join(work, `udf-${label[0]}`);
    const e = { ...env, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: v.args };
    if (v.udf) e.WEBVIEW2_USER_DATA_FOLDER = folder;
    else delete e.WEBVIEW2_USER_DATA_FOLDER;
    const c = spawn(app, [], { env: e, stdio: 'ignore' });
    let ended;
    c.on('exit', (code) => (ended = code));
    await sleep(12000);
    console.log(ended !== undefined ? `app ENDED with ${ended}` : 'app running after 12 s');
    if (v.port) console.log(`http://127.0.0.1:${v.port}/json/version ->`, await probePort(v.port));
    if (label.startsWith('A')) tree('(experiment A, 12 s)');
    if (v.udf) files('WebView2 user data folder', folder);
    else console.log('(default folder: newest entries in %LOCALAPPDATA%)', ps("Get-ChildItem $env:LOCALAPPDATA -Directory | Sort-Object LastWriteTime -Descending | Select-Object -First 4 | ForEach-Object { $_.FullName + ' ' + $_.LastWriteTime.ToString('s') }"));
    if (c.pid) kill(c.pid);
    await sleep(1500);
  }

  // The real thing: msedgedriver, twice. First as tauri-driver calls it (our WEBVIEW2_USER_DATA_FOLDER in its environment),
  // then without that variable, to see whether our folder collides with the profile folder msedgedriver makes for itself.
  if (!nativeDriver || !fs.existsSync(nativeDriver)) console.log('msedgedriver not found: the driver experiments are skipped');
  else for (const [label, keepUdf] of [['with WEBVIEW2_USER_DATA_FOLDER in the environment (as in the specs)', true], ['WITHOUT WEBVIEW2_USER_DATA_FOLDER', false]]) {
    head(`msedgedriver alone, one session request, ${label}`);
    const port = 41000 + Math.floor(Math.random() * 8000);
    const log = path.join(work, `msedgedriver-${keepUdf}.log`);
    const denv = { ...env, TAURI_WEBVIEW_AUTOMATION: 'true', TAURI_AUTOMATION: 'true' };
    if (!keepUdf) delete denv.WEBVIEW2_USER_DATA_FOLDER;
    const drv = spawn(nativeDriver, [`--port=${port}`, '--verbose', `--log-path=${log}`], { env: denv, stdio: ['ignore', 'pipe', 'inherit'] });
    drv.stdout.resume();
    await sleep(2000);
    const body = { capabilities: { alwaysMatch: { browserName: 'webview2', 'ms:edgeOptions': { binary: app, args: [], webviewOptions: {} } } } };
    const t0 = Date.now();
    const request = fetch(`http://127.0.0.1:${port}/session`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body), signal: AbortSignal.timeout(keepUdf ? 60000 : 35000) })
      .then(async (res) => `answer after ${Date.now() - t0} ms: HTTP ${res.status} ${(await res.text()).slice(0, 600)}`)
      .catch((e) => `NO ANSWER after ${Date.now() - t0} ms: ${e.message}`);
    await sleep(12000); // the driver is waiting now: look at what exists
    tree('(12 s into the wait)');
    const full = fs.existsSync(log) ? fs.readFileSync(log, 'utf8') : '';
    const m = /WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = (.*)/.exec(full);
    console.log('FULL WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:', m ? m[1] : '(not logged yet)');
    const profiles = driverTemps();
    console.log('temp folders that look like a driver/browser profile:', profiles.join(' | ') || '(none)');
    for (const pdir of profiles) files('driver temp profile', pdir);
    if (keepUdf) files('our WEBVIEW2_USER_DATA_FOLDER', env.WEBVIEW2_USER_DATA_FOLDER);
    console.log(await request);
    if (drv.pid) kill(drv.pid);
    await sleep(500);
    console.log('--- msedgedriver log (last 30 lines)');
    console.log(fs.existsSync(log) ? fs.readFileSync(log, 'utf8').split(/\r?\n/).slice(-30).map((l) => l.slice(0, 700)).join('\n') : '(no log)');
    tree('(after the request ended: leftovers)');
  }
  console.log('--- Windows Application event log, last 30 minutes, entries that mention the app or WebView2');
  console.log(ps("Get-WinEvent -FilterHashtable @{LogName='Application'; StartTime=(Get-Date).AddMinutes(-30)} -ErrorAction SilentlyContinue | Where-Object { $_.Message -match 'gene-tree|msedgewebview2|WebView2' } | Select-Object -First 8 | ForEach-Object { $_.TimeCreated.ToString('s') + ' ' + $_.ProviderName + ' ' + (($_.Message -split \"`n\")[0..3] -join ' ') }"));
}
fs.rmSync(work, { recursive: true, force: true });
