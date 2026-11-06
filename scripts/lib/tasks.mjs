// Shared by scripts/gate.mjs, scripts/e2e-parallel.mjs and scripts/ship.mjs: run commands side by side, each to its own
// log, and leave nothing running when the script stops.
//
// Stopping: Ctrl+C / SIGTERM kill every task started here, with its children (cargo, rustc, drivers, apps). A script
// killed HARD (Windows `kill` is a TerminateProcess, no handler runs) is covered by a small reaper process that holds a
// pipe to this script: when the pipe closes it kills the pids still registered. Only pids started here are touched.
import { spawn, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const live = new Map(); // pid -> ChildProcess
let reaper;
let stopping = false;

export function killTree(pid) {
  if (process.platform === 'win32') spawnSync('taskkill', ['/PID', String(pid), '/T', '/F'], { stdio: 'ignore' });
  else {
    try {
      process.kill(-pid, 'SIGKILL');
    } catch {
      try {
        process.kill(pid, 'SIGKILL');
      } catch {
        /* gone already */
      }
    }
  }
}

function tell(line) {
  try {
    reaper?.stdin.write(line + '\n');
  } catch {
    /* the reaper is gone; the signal handlers still work */
  }
}

function ensureReaper() {
  if (reaper) return;
  reaper = spawn(process.execPath, [path.join(here, 'reaper.mjs')], { stdio: ['pipe', 'ignore', 'ignore'], windowsHide: true, detached: process.platform !== 'win32' });
  reaper.on('error', () => {});
  reaper.stdin.on('error', () => {});
  reaper.unref();
}

/** Kills everything started with `runTask` that still runs. */
export function stopAll() {
  stopping = true;
  for (const pid of live.keys()) killTree(pid);
  live.clear();
}

export function isStopping() {
  return stopping;
}

/** Call once: Ctrl+C and SIGTERM stop every task, say so, and exit with `code`. */
export function exitOnSignals(code = 130, message = 'Stopped: every running task, with its drivers and apps, was killed.') {
  for (const sig of ['SIGINT', 'SIGTERM', 'SIGHUP', 'SIGBREAK']) {
    process.on(sig, () => {
      if (!stopping) {
        stopAll();
        console.error(`\n${message}`);
      }
      process.exit(code);
    });
  }
  process.on('exit', () => stopAll());
}

/**
 * Starts `file args` (no shell) with stdout and stderr going to `log`. Resolves, never rejects, to
 * { name, code, seconds, log }. `handle.kill()` stops it and its children.
 */
export function runTask({ name, file, args = [], cwd, env, log }) {
  ensureReaper();
  const t0 = Date.now();
  if (log) fs.mkdirSync(path.dirname(log), { recursive: true });
  const fd = log ? fs.openSync(log, 'w') : null; // no log: the task writes to this terminal
  let child;
  const handle = { name, log, kill: () => child?.pid && killTree(child.pid) };
  handle.done = new Promise((resolve) => {
    if (stopping) {
      if (fd !== null) fs.closeSync(fd);
      resolve({ name, code: 130, seconds: 0, log, killed: true });
      return;
    }
    const finish = (code, extra = '') => {
      if (child?.pid) {
        live.delete(child.pid);
        tell(`del ${child.pid}`);
      }
      if (fd !== null) {
        if (extra) fs.writeSync(fd, extra);
        fs.closeSync(fd);
      }
      resolve({ name, code, seconds: (Date.now() - t0) / 1000, log });
    };
    child = spawn(file, args, { cwd, env: { ...process.env, ...env }, stdio: fd === null ? 'inherit' : ['ignore', fd, fd], windowsHide: true, detached: process.platform !== 'win32' });
    child.on('error', (e) => finish(127, `\ncould not start ${file}: ${e.message}\n`));
    child.on('exit', (code, signal) => finish(code ?? (signal ? 137 : 1)));
    if (child.pid) {
      live.set(child.pid, child);
      tell(`add ${child.pid}`);
    }
  });
  return handle;
}

export const mark = (r) => (r.skipped ? 'SKIP' : r.code === 0 ? 'PASS' : 'FAIL');

/** Prints the last `lines` lines of a log. */
export function tail(log, lines = 80) {
  const text = fs.readFileSync(log, 'utf8').replace(/\x1b\[[0-9;]*m/g, '').split(/\r?\n/);
  return text.slice(-lines).join('\n');
}
