#!/usr/bin/env node
// Runs the window specs (app/e2e-tauri/specs/*.e2e.ts) several at a time, each in its own fully isolated wdio run
// (own drivers and ports, own settings folder, own WebView2 profile, own agent-access port; see wdio.conf.ts), so they
// also run next to the owner's own Gene Tree. Normally started by `bash scripts/test.sh --e2e`.
//
//   node scripts/e2e-parallel.mjs [--jobs N] [--keep] [spec names...]
//
// A spec name is the file name without `.e2e.ts` (flows, date-picker, ...); none means all of them.
// Environment: GT_E2E_SHOTS keeps every screenshot in that folder; GT_E2E_JOBS is the default for --jobs.
// Exit code: 0 only when every spec passed. Ctrl+C (or SIGTERM) stops every child it started, drivers and apps included.
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { exitOnSignals, isStopping, runTask } from './lib/tasks.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const repo = path.resolve(here, '..');
const app = path.join(repo, 'app');
const specsDir = path.join(app, 'e2e-tauri', 'specs');
const wdioJs = path.join(app, 'node_modules', '@wdio', 'cli', 'bin', 'wdio.js');

// Slowest first, so the last one to start is a short one. People in the synthetic file: only the specs whose
// assertions need a big tree ask for 5,000; the rest get a smaller one (their import is the slowest step).
const ORDER = ['screenshots', 'flows', 'agent-access', 'merge-copy', 'voice-samples', 'selection-tree', 'arrange-around', 'one-scrollbar', 'date-picker', 'family-notes'];
const BIG = { flows: 5000, screenshots: 5000, 'agent-access': 5000 };
const SMALL = Number(process.env.GT_E2E_SMALL ?? 1000);

const args = process.argv.slice(2);
let jobs = Number(process.env.GT_E2E_JOBS) || 0;
let keep = false;
const wanted = [];
for (let i = 0; i < args.length; i++) {
  if (args[i] === '--jobs' || args[i] === '-j') jobs = Number(args[++i]);
  else if (args[i] === '--keep') keep = true;
  else wanted.push(args[i].replace(/\.e2e\.ts$/, '').replace(/^.*[\\/]/, ''));
}
const all = fs.readdirSync(specsDir).filter((f) => f.endsWith('.e2e.ts')).map((f) => f.replace(/\.e2e\.ts$/, ''));
for (const w of wanted) if (!all.includes(w)) die(`no such spec: ${w}. Specs: ${all.join(', ')}`);
const specs = (wanted.length ? wanted : all).sort((a, b) => (ORDER.indexOf(a) + 1 || 99) - (ORDER.indexOf(b) + 1 || 99));
if (!jobs) jobs = Math.max(2, Math.min(6, Math.floor(os.cpus().length / 2.5)));
jobs = Math.max(1, Math.min(jobs, specs.length));

function die(msg) {
  console.error(msg);
  process.exit(2);
}

const application = process.env.GT_E2E_APP ?? [path.join(repo, 'target', 'e2e', 'debug', 'gene-tree.exe'), path.join(repo, 'target', 'debug', 'gene-tree.exe'), path.join(repo, 'target', 'e2e', 'debug', 'gene-tree'), path.join(repo, 'target', 'debug', 'gene-tree')].find((p) => fs.existsSync(p));
if (!application || !fs.existsSync(application)) die('The test exe is missing. Build it first: bash scripts/test.sh --e2e');
if (!fs.existsSync(wdioJs)) die('app/node_modules is missing. Run: npm --prefix app ci');

const run = fs.mkdtempSync(path.join(os.tmpdir(), 'gt-e2e-run-'));
const logs = path.join(run, 'logs');
fs.mkdirSync(logs);
const shots = process.env.GT_E2E_SHOTS || path.join(run, 'shots');

// ---------------------------------------------------------------- stopping
exitOnSignals(130, 'Stopped: every running spec, with its drivers and apps, was killed.');

// ---------------------------------------------------------------- synthetic files (once per size)
const python = ['python3', 'python', 'py'].find((c) => spawnSync(process.platform === 'win32' ? 'where' : 'which', [c], { stdio: 'ignore' }).status === 0);
if (!python) die('Python is needed to write the synthetic GEDCOM');
const geds = new Map();
for (const s of specs) {
  const n = BIG[s] ?? SMALL;
  if (geds.has(n)) continue;
  const file = path.join(run, `synthetic-${n}.ged`);
  const made = spawnSync(python, ['-B', path.join(repo, 'scripts', 'make-synthetic-ged.py'), file, '--persons', String(n)], { encoding: 'utf8' });
  if (made.status !== 0) die(`make-synthetic-ged.py failed: ${made.stderr}`);
  geds.set(n, file);
}

// ---------------------------------------------------------------- the pool
const started = Date.now();
const results = [];
const queue = [...specs];
let running = 0;

console.log(`${specs.length} spec(s), ${jobs} at a time. App: ${application}`);
console.log(`Logs: ${logs}${process.env.GT_E2E_SHOTS ? '' : `   Screenshots: ${shots}`}`);

await new Promise((resolve) => {
  const next = () => {
    while (!isStopping() && running < jobs && queue.length) launch(queue.shift());
    if (!running && !queue.length) resolve();
  };
  const launch = (name) => {
    running++;
    runTask({
      name,
      file: process.execPath,
      args: [wdioJs, 'run', 'e2e-tauri/wdio.conf.ts', '--spec', path.join(specsDir, `${name}.e2e.ts`)],
      cwd: app,
      log: path.join(logs, `${name}.log`),
      env: {
        GT_E2E_APP: application,
        GT_E2E_GED: geds.get(BIG[name] ?? SMALL),
        GT_E2E_SHOTS: shots,
        GT_E2E_PARENT_PID: String(process.pid),
        // Each instance picks its own ports and folders; never inherit a neighbour's.
        GT_E2E_DRIVER_PORT: '',
        GT_E2E_NATIVE_PORT: '',
        GT_E2E_AGENT_PORT: '',
        GT_E2E_WORK: '',
        GT_E2E_CONFIG_DIR: '',
      },
    }).done.then((r) => {
      results.push({ name, ok: r.code === 0, seconds: r.seconds });
      console.log(`${r.code === 0 ? 'PASS' : 'FAIL'}  ${name}  ${r.seconds.toFixed(0)} s`);
      running--;
      next();
    });
  };
  next();
});

// ---------------------------------------------------------------- summary
const total = (Date.now() - started) / 1000;
const failed = results.filter((r) => !r.ok);
console.log('\n--- summary');
for (const r of results.sort((a, b) => a.name.localeCompare(b.name))) console.log(`${r.ok ? 'PASS' : 'FAIL'}  ${r.seconds.toFixed(0).padStart(4)} s  ${r.name}`);
console.log(`wall time ${total.toFixed(0)} s, sum of specs ${results.reduce((a, r) => a + r.seconds, 0).toFixed(0)} s, ${jobs} at a time`);
for (const r of failed) {
  const text = fs.readFileSync(path.join(logs, `${r.name}.log`), 'utf8').split(/\r?\n/);
  console.log(`\n=== ${r.name} failed. Last lines of ${path.join(logs, `${r.name}.log`)}:`);
  console.log(text.slice(-60).join('\n'));
}
if (!failed.length && !keep) {
  fs.rmSync(logs, { recursive: true, force: true });
  for (const f of geds.values()) for (const g of [f, `${f}.meta.json`]) fs.rmSync(g, { force: true });
  if (!process.env.GT_E2E_SHOTS) console.log(`Screenshots are in ${shots}`);
}
process.exit(failed.length ? 1 : 0);
