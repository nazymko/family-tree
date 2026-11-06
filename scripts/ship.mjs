#!/usr/bin/env node
// Verifies this checkout and, only with --merge, ships it. Start it through scripts/ship.sh.
//
//   bash scripts/ship.sh                       gate (--full) and all window specs at the same time. Changes nothing.
//   bash scripts/ship.sh --merge "message"     the same, and when both are green: merge develop into main with that message,
//                                              release build, install into %LOCALAPPDATA%\GeneTree (scripts/install-app.ps1).
//
// - Stops at the first failure (the other run is killed) and leaves main alone. The merge is made on a detached scratch
//   checkout of main and `main` is moved only after the release build and the install have worked.
// - Refuses to start while another ship.sh runs (lock file with the PID of the running one; a lock whose process is gone is cleared).
// - Ctrl+C / SIGTERM kill everything this started (cargo, drivers, apps). If this script itself is killed hard, a small
//   reaper process (scripts/lib/reaper.mjs) kills them a moment later, so no orphan goes on to start another pipeline.
// - Never pushes, never touches the running copy of the app except to rename its exe (install-app.ps1).
//
// For testing this script itself, GT_SHIP_GATE, GT_SHIP_E2E, GT_SHIP_BUILD and GT_SHIP_INSTALL may hold a JSON array
// [file, ...args] that replaces the real command of that step.
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { exitOnSignals, mark, runTask, stopAll, tail } from './lib/tasks.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '..');
const node = process.execPath;
const exe = process.platform === 'win32' ? 'gene-tree.exe' : 'gene-tree';
const logDir = fs.mkdtempSync(path.join(os.tmpdir(), 'gt-ship-'));
const lockFile = process.env.GT_SHIP_LOCK ?? path.join(os.tmpdir(), 'gene-tree-ship.lock');

// ------------------------------------------------------------------------------------------------ arguments
const argv = process.argv.slice(2);
let message = null;
for (let i = 0; i < argv.length; i++) {
  if (argv[i] === '--merge') {
    message = argv[++i];
    if (!message || message.startsWith('--')) usage('--merge needs the merge commit message in quotes.');
  } else usage(`unknown argument: ${argv[i]}`);
}
function usage(why) {
  console.error(`${why}\nusage: bash scripts/ship.sh [--merge "message"]`);
  process.exit(2);
}

const cleanups = [];
function die(msg, code = 1) {
  console.error(msg);
  process.exit(code);
}
process.on('exit', () => {
  for (const c of cleanups.reverse()) {
    try {
      c();
    } catch {
      /* best effort */
    }
  }
});

// ------------------------------------------------------------------------------------------------ the lock
function alive(pid) {
  try {
    process.kill(pid, 0);
  } catch (e) {
    if (e.code !== 'EPERM') return false;
  }
  if (process.platform === 'win32') {
    // A recycled pid belongs to some other program; only a node process can be the other ship.sh.
    const out = spawnSync('tasklist', ['/FI', `PID eq ${pid}`, '/FO', 'CSV', '/NH'], { encoding: 'utf8' }).stdout ?? '';
    return /node/i.test(out);
  }
  return true;
}

function takeLock() {
  for (let attempt = 0; attempt < 2; attempt++) {
    try {
      fs.writeFileSync(lockFile, JSON.stringify({ pid: process.pid, started: new Date().toISOString(), checkout: root }), { flag: 'wx' });
      cleanups.push(() => {
        try {
          if (JSON.parse(fs.readFileSync(lockFile, 'utf8')).pid === process.pid) fs.rmSync(lockFile, { force: true });
        } catch {
          /* already gone */
        }
      });
      return;
    } catch (e) {
      if (e.code !== 'EEXIST') throw e;
      let other = null;
      try {
        other = JSON.parse(fs.readFileSync(lockFile, 'utf8'));
      } catch {
        /* unreadable: treat as stale */
      }
      if (other && other.pid !== process.pid && alive(other.pid)) {
        die(`Another ship.sh is already running (PID ${other.pid}, started ${other.started}, in ${other.checkout}). Wait for it or stop it first.`, 3);
      }
      console.log(`Clearing a stale lock${other ? ` (PID ${other.pid} is gone)` : ''}.`);
      fs.rmSync(lockFile, { force: true });
    }
  }
  die('Could not take the ship lock.');
}

// ------------------------------------------------------------------------------------------------ git helpers
function git(cwd, ...args) {
  const r = spawnSync('git', args, { cwd, encoding: 'utf8' });
  return { ok: r.status === 0, out: (r.stdout ?? '').trim(), err: (r.stderr ?? '').trim() };
}

// ------------------------------------------------------------------------------------------------ run
exitOnSignals(130, 'Stopped: the gate, the window specs and everything they started were killed. main was not touched.');
takeLock();
console.log(`ship: PID ${process.pid}, logs in ${logDir}`);

if (message !== null) {
  const status = git(root, 'status', '--porcelain');
  if (!status.ok || status.out) die('Refusing to merge: the working tree is not clean. Commit or set aside your changes first.');
  const head = git(root, 'rev-parse', 'HEAD').out;
  const develop = git(root, 'rev-parse', 'refs/heads/develop');
  if (!develop.ok || develop.out !== head) die('Refusing to merge: HEAD is not the tip of develop, so the gate would test something else than what is merged.');
  if (!git(root, 'rev-parse', '--verify', 'refs/heads/main').ok) die('There is no local branch main.');
  // main is moved with update-ref, which would leave a checkout of main out of step with its branch.
  if (/^branch refs\/heads\/main$/m.test(git(root, 'worktree', 'list', '--porcelain').out)) die('main is checked out in a worktree. Switch that checkout away from main first.');
}

const fromEnv = (name, fallback) => (process.env[name] ? JSON.parse(process.env[name]) : fallback);
const gateCmd = fromEnv('GT_SHIP_GATE', [node, path.join(here, 'gate.mjs'), '--full']);
const e2eCmd = fromEnv('GT_SHIP_E2E', [node, path.join(here, 'gate.mjs'), '--e2e']);

const started = Date.now();
const runs = [
  { label: 'gate (--full)', cmd: gateCmd, log: path.join(logDir, 'gate.log') },
  { label: 'window specs', cmd: e2eCmd, log: path.join(logDir, 'e2e.log') },
].map((r) => ({ ...r, handle: runTask({ name: r.label, file: r.cmd[0], args: r.cmd.slice(1), cwd: root, log: r.log }) }));

const first = await new Promise((resolve) => {
  let open = runs.length;
  for (const r of runs) {
    r.handle.done.then((res) => {
      console.log(`${mark(res)}  ${res.seconds.toFixed(0).padStart(4)} s  ${r.label}`);
      if (res.code !== 0) resolve(r);
      else if (--open === 0) resolve(null);
    });
  }
});
if (first) {
  stopAll();
  console.log(`\n=== ${first.label} FAILED. Last lines of ${first.log}:`);
  console.log(tail(first.log, 80));
  console.log(`\nStopped at the first failure (${((Date.now() - started) / 1000).toFixed(0)} s). main was not touched.`);
  process.exit(1);
}
console.log(`\nBoth green after ${((Date.now() - started) / 1000).toFixed(0)} s.`);

if (message === null) {
  console.log('No --merge given: nothing was changed. To ship: bash scripts/ship.sh --merge "message"');
  process.exit(0);
}

// ------------------------------------------------------------------------------------------------ merge, build, install
const common = path.dirname(git(root, 'rev-parse', '--path-format=absolute', '--git-common-dir').out);
const oldMain = git(root, 'rev-parse', 'refs/heads/main').out;
const scratch = path.join(os.tmpdir(), `gt-ship-main-${process.pid}`);
const add = git(root, 'worktree', 'add', '--detach', scratch, oldMain);
if (!add.ok) die(`Could not make the scratch checkout of main: ${add.err}`);
cleanups.push(() => {
  const link = path.join(scratch, 'app', 'node_modules');
  try {
    fs.rmdirSync(link); // a junction: removes the link, never the folder it points at
  } catch {
    /* not made */
  }
  git(root, 'worktree', 'remove', '--force', scratch);
  git(root, 'worktree', 'prune');
});
const merge = git(scratch, 'merge', '--no-ff', 'refs/heads/develop', '-m', message);
if (!merge.ok) {
  git(scratch, 'merge', '--abort');
  die(`The merge of develop into main did not go through (conflicts?). main was not touched.\n${merge.err || merge.out}`);
}
const merged = git(scratch, 'rev-parse', 'HEAD').out;
console.log(`Merged develop into a scratch copy of main: ${merged.slice(0, 7)}`);

fs.symlinkSync(path.join(common, 'app', 'node_modules'), path.join(scratch, 'app', 'node_modules'), 'junction');
const shipTarget = path.join(common, 'target', 'ship');
const buildCmd = fromEnv('GT_SHIP_BUILD', [node, path.join(scratch, 'app', 'node_modules', '@tauri-apps', 'cli', 'tauri.js'), 'build', '--no-bundle']);
const build = await runTask({ name: 'release build', file: buildCmd[0], args: buildCmd.slice(1), cwd: path.join(scratch, 'app'), env: { CARGO_TARGET_DIR: shipTarget }, log: path.join(logDir, 'build.log') }).done;
console.log(`${mark(build)}  ${build.seconds.toFixed(0).padStart(4)} s  release build`);
if (build.code !== 0) {
  console.log(tail(build.log, 60));
  die('The release build failed. main was not touched.');
}

const installCmd = fromEnv('GT_SHIP_INSTALL', ['pwsh', '-NoProfile', '-File', path.join(scratch, 'scripts', 'install-app.ps1'), '-Source', path.join(shipTarget, 'release', exe)]);
const install = await runTask({ name: 'install', file: installCmd[0], args: installCmd.slice(1), cwd: scratch, log: path.join(logDir, 'install.log') }).done;
console.log(`${mark(install)}  ${install.seconds.toFixed(0).padStart(4)} s  install`);
console.log(tail(install.log, 10));
if (install.code !== 0) die('The install failed. main was not touched.');

// Only now does main move, and only if nobody moved it meanwhile.
const move = git(root, 'update-ref', '-m', `ship: ${message}`, 'refs/heads/main', merged, oldMain);
if (!move.ok) die(`main moved while shipping, so it was not updated: ${move.err}. The merged commit is ${merged}.`);
console.log(`main is now ${merged.slice(0, 7)} (was ${oldMain.slice(0, 7)}). Not pushed.`);
process.exit(0);
