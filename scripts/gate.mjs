#!/usr/bin/env node
// The Gene Tree test gate. Start it through scripts/test.sh (or test.ps1); it is the same on every OS.
//
//   node scripts/gate.mjs                 everyday: Rust tests (nextest) and the frontend tests at the same time
//   node scripts/gate.mjs --full          before merge: fmt, clippy, Rust tests, heavy tier, doctests, vitest, svelte-check, eslint
//   node scripts/gate.mjs --quick         only what the changes since the merge-base with develop (and uncommitted work) can affect
//   node scripts/gate.mjs --e2e [spec..]  builds the test exe if it is stale, then runs the window specs several at a time
//
// Independent steps run at the same time, each to its own log. A failed step does not stop the others; the summary
// names every failure and prints its log, and the exit code is non-zero if anything failed.
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { exitOnSignals, mark, runTask, tail } from './lib/tasks.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '..');
const appDir = path.join(root, 'app');
const nm = path.join(appDir, 'node_modules');
const node = process.execPath;
const logDir = fs.mkdtempSync(path.join(os.tmpdir(), 'gt-gate-'));
const exe = process.platform === 'win32' ? 'gene-tree.exe' : 'gene-tree';
const e2eTarget = path.join(root, 'target', 'e2e');

const tools = {
  vitest: [node, path.join(nm, 'vitest', 'vitest.mjs')],
  svelteCheck: [node, path.join(nm, 'svelte-check', 'bin', 'svelte-check')],
  eslint: [node, path.join(nm, 'eslint', 'bin', 'eslint.js')],
  tauri: [node, path.join(nm, '@tauri-apps', 'cli', 'tauri.js')],
};

// ------------------------------------------------------------------------------------------------ arguments
const argv = process.argv.slice(2);
const mode = argv[0] === '--full' ? 'full' : argv[0] === '--quick' ? 'quick' : argv[0] === '--e2e' ? 'e2e' : argv.length === 0 ? 'everyday' : null;
if (!mode) {
  console.error('usage: bash scripts/test.sh [--quick | --full | --e2e [spec names...]]');
  process.exit(2);
}
const extra = argv.slice(1);

// ------------------------------------------------------------------------------------------------ helpers
const results = [];
const notes = [];
const t0 = Date.now();

function need() {
  if (!fs.existsSync(nm)) die('app/node_modules is missing. Run: npm --prefix app ci');
}
function die(msg) {
  console.error(msg);
  process.exit(1);
}
function haveNextest() {
  return spawnSync('cargo', ['nextest', '--version'], { stdio: 'ignore' }).status === 0;
}
function needNextest() {
  if (!haveNextest()) die('cargo-nextest is missing. Install it once with: cargo install cargo-nextest --locked');
}

/** Starts a step now; it is reported when it ends. `after` is a promise the step waits for (its inputs). */
function step(name, file, args, { cwd = root, env, after } = {}) {
  const log = path.join(logDir, `${name.replace(/[^a-z0-9]+/gi, '-').toLowerCase()}.log`);
  const run = async () => {
    if (after) {
      const prior = await after;
      if (prior.code !== 0) {
        const r = { name, code: 1, seconds: 0, log, skipped: true };
        fs.writeFileSync(log, `skipped: "${prior.name}" failed first (see its log)\n`);
        results.push(r);
        console.log(`SKIP  ${name}  (needs ${prior.name})`);
        return r;
      }
    }
    const handle = runTask({ name, file, args, cwd, env, log });
    const r = await handle.done;
    results.push(r);
    console.log(`${mark(r)}  ${r.seconds.toFixed(0).padStart(4)} s  ${name}`);
    return r;
  };
  return run();
}

function report(ran, skipped = []) {
  const total = (Date.now() - t0) / 1000;
  console.log('\n--- summary');
  for (const r of results.sort((a, b) => a.name.localeCompare(b.name))) console.log(`${mark(r)}  ${r.seconds.toFixed(1).padStart(6)} s  ${r.name}`);
  console.log(`wall time ${total.toFixed(0)} s, sum of steps ${results.reduce((a, r) => a + r.seconds, 0).toFixed(0)} s`);
  const failed = results.filter((r) => r.code !== 0);
  for (const r of failed) {
    console.log(`\n=== ${r.name} FAILED. Log: ${r.log}`);
    console.log(tail(r.log, 80));
  }
  if (ran.length || skipped.length) {
    console.log('');
    console.log(`RAN:     ${ran.join('; ') || '(nothing)'}`);
    console.log(`SKIPPED: ${skipped.join('; ') || '(nothing)'}`);
  }
  for (const n of notes) console.log(n);
  if (failed.length) {
    console.log(`\nFAILED steps: ${failed.map((r) => r.name).join('; ')}`);
    process.exit(1);
  }
  if (!failed.length) fs.rmSync(logDir, { recursive: true, force: true });
  console.log(mode === 'quick' ? 'Quick run passed. This is NOT the full gate: run `bash scripts/test.sh --full` before merging.' : 'All steps passed.');
  process.exit(0);
}

// ------------------------------------------------------------------------------------------------ modes
exitOnSignals();
console.log(`Logs: ${logDir}`);

if (mode === 'everyday') {
  need();
  needNextest();
  await Promise.all([
    step('Rust tests (nextest, everyday set)', 'cargo', ['nextest', 'run', '--workspace', '--no-fail-fast']),
    step('Frontend tests (vitest)', ...tools.vitest.slice(0, 1), [...tools.vitest.slice(1), 'run'], { cwd: appDir }),
  ]);
  report(['Rust tests (everyday set)', 'vitest (all)'], ['fmt, clippy, heavy tier, doctests, svelte-check, eslint: --full only']);
}

if (mode === 'full') {
  need();
  needNextest();
  await runFull();
  report(['fmt', 'clippy', 'Rust tests', 'heavy tier', 'doctests', 'vitest', 'svelte-check', 'eslint'], []);
}

if (mode === 'quick') {
  need();
  await runQuick();
}

if (mode === 'e2e') {
  need();
  await buildE2e();
  const code = await runE2e(extra);
  process.exit(code);
}

// ------------------------------------------------------------------------------------------------ full
async function runFull() {
  // Cargo takes a lock on the build folder, so a second cargo that needs to compile waits for the first. Compiling the
  // tests once is what lets nextest, the heavy tier and the doctests start together. Clippy in a folder of its own was
  // measured (124 s both ways after touching a core crate), so it shares target/ and no second 1.5 GB tree is kept.
  const build = step('Rust build (tests)', 'cargo', ['nextest', 'run', '--workspace', '--no-run']);
  const rust = build.then(() =>
    Promise.all([
      step('Rust tests (nextest, everyday set)', 'cargo', ['nextest', 'run', '--workspace', '--no-fail-fast'], { after: build }),
      step('Rust heavy tier (heavy_*)', 'cargo', ['nextest', 'run', '--workspace', '--no-fail-fast', '--run-ignored', 'only', '-E', 'test(/heavy_/)'], { after: build }),
      step('Rust doctests', 'cargo', ['test', '--doc', '--workspace'], { after: build }),
    ]),
  );
  const rest = [
    step('cargo fmt --check', 'cargo', ['fmt', '--all', '--', '--check']),
    step('cargo clippy', 'cargo', ['clippy', '-q', '--workspace', '--all-targets', '--', '-D', 'warnings']),
    step('Frontend tests (vitest)', tools.vitest[0], [tools.vitest[1], 'run'], { cwd: appDir }),
    step('svelte-check', tools.svelteCheck[0], [tools.svelteCheck[1], '--tsconfig', './tsconfig.json', '--fail-on-warnings'], { cwd: appDir }),
    step('eslint', tools.eslint[0], [tools.eslint[1], '.'], { cwd: appDir }),
  ];
  await Promise.all([rust, ...rest]);
}

// ------------------------------------------------------------------------------------------------ quick
function git(args) {
  const r = spawnSync('git', args, { cwd: root, encoding: 'utf8' });
  return r.status === 0 ? r.stdout : null;
}

function changedFiles() {
  // GT_QUICK_BASE compares with another commit instead of the merge-base with develop.
  let base = process.env.GT_QUICK_BASE ? git(['rev-parse', process.env.GT_QUICK_BASE])?.trim() : null;
  for (const ref of base ? [] : ['develop', 'origin/develop', 'main']) {
    const out = git(['merge-base', 'HEAD', ref]);
    if (out) {
      base = out.trim();
      break;
    }
  }
  if (!base) return null;
  const tracked = git(['diff', '--name-only', '--no-renames', base]) ?? '';
  const fresh = git(['ls-files', '--others', '--exclude-standard']) ?? '';
  return [...new Set([...tracked.split('\n'), ...fresh.split('\n')].map((f) => f.trim()).filter(Boolean))];
}

/** Workspace crate directories -> package names, read from each Cargo.toml. */
function crates() {
  const found = [];
  const add = (dir) => {
    const toml = path.join(root, dir, 'Cargo.toml');
    if (!fs.existsSync(toml)) return;
    const m = /^\s*name\s*=\s*"([^"]+)"/m.exec(fs.readFileSync(toml, 'utf8').split(/\[dependencies\]/)[0]);
    if (m) found.push({ dir: dir.replaceAll('\\', '/'), name: m[1] });
  };
  for (const d of fs.readdirSync(path.join(root, 'crates'), { withFileTypes: true })) if (d.isDirectory()) add(path.join('crates', d.name));
  add('app/src-tauri');
  return found;
}

async function runQuick() {
  const files = changedFiles();
  const ran = [];
  const skipped = [];
  if (!files) {
    notes.push('No merge-base with develop/main found: running the whole everyday set.');
    return fallbackAll();
  }
  console.log(`${files.length} changed file(s) since the merge-base with develop, plus uncommitted work.`);
  if (!files.length) {
    console.log('Nothing changed: nothing to run.');
    return report([], ['everything (nothing changed)']);
  }

  const pkgs = crates();
  const rustPackages = new Set();
  let rustAll = false;
  let rustTouched = false;
  let webAll = false;
  const webFiles = []; // app files that vitest/eslint know
  const lintFiles = [];
  let webTouched = false;
  const unsure = [];
  const noTest = [];

  for (const f of files) {
    const p = f.replaceAll('\\', '/');
    if (/^(docs|\.github)\/|\.md$|^LICENSE|^THIRD_PARTY\.md$|^fuzz\//.test(p)) continue; // text only
    if (/^(Cargo\.toml|Cargo\.lock|rust-toolchain\.toml|\.config\/|tests\/)/.test(p) || /(^|\/)build\.rs$/.test(p) || /(^|\/)tauri\.conf\.json$/.test(p) || /(^|\/)capabilities\//.test(p)) {
      rustAll = true;
      rustTouched = true;
      continue;
    }
    const owner = pkgs.find((c) => p.startsWith(c.dir + '/'));
    if (owner) {
      rustTouched = true;
      rustPackages.add(owner.name);
      if (/(^|\/)Cargo\.toml$/.test(p)) rustAll = true; // a dependency change reaches further than rdeps
      // Generated bindings are written by a Rust test and read by the whole frontend.
      if (/export_bindings/.test(p)) webAll = true;
      continue;
    }
    if (p.startsWith('app/')) {
      const rel = p.slice(4);
      if (/^(package\.json|package-lock\.json|vite\.config\.ts|svelte\.config\.js|tsconfig\.json|eslint\.config\.js)$/.test(rel) || /^src\/(test-setup\.ts|app\.css)$/.test(rel) || /^src\/lib\/i18n\//.test(rel) && !/\.test\.ts$/.test(rel) || /bindings/.test(rel) || /^[^/]+\.html$/.test(rel)) {
        webAll = true;
        webTouched = true;
        continue;
      }
      if (/^src\/.*\.(ts|svelte|js|css)$/.test(rel) || /^e2e-tauri\/.*\.ts$/.test(rel) || /^[^/]*\.(ts|js)$/.test(rel)) {
        webTouched = true;
        if (fs.existsSync(path.join(root, p))) {
          if (/^src\//.test(rel) && /\.(ts|svelte|js)$/.test(rel)) webFiles.push(rel);
          if (/\.(ts|svelte|js)$/.test(rel)) lintFiles.push(rel);
        }
        continue;
      }
      if (/^(e2e-tauri|src-tauri)\//.test(rel)) continue;
    }
    if (/^scripts\//.test(p) || /^\.gitattributes$|^\.gitignore$/.test(p)) {
      // Build and dev scripts: no unit test covers them (make-synthetic-ged.py is only used by the window specs).
      noTest.push(p);
      continue;
    }
    unsure.push(p);
  }
  if (unsure.length) {
    // Not a place the rules know: run everything on both sides, never less.
    rustAll = true;
    webAll = true;
    rustTouched = true;
    webTouched = true;
    notes.push(`Widened to everything because of: ${unsure.slice(0, 6).join(', ')}${unsure.length > 6 ? ', ...' : ''}`);
  }

  if (noTest.length) notes.push(`Not covered by any test here: ${noTest.slice(0, 6).join(', ')}${noTest.length > 6 ? ', ...' : ''}`);
  const jobs = [];
  if (rustTouched) {
    needNextest();
    const filter = rustAll ? null : [...rustPackages].map((n) => `rdeps(${n})`).join(' | ');
    ran.push(rustAll ? 'Rust: all crates (a workspace-wide file changed)' : `Rust: ${[...rustPackages].join(', ')} and their reverse dependencies`);
    jobs.push(step(rustAll ? 'Rust tests (all)' : 'Rust tests (changed crates + rdeps)', 'cargo', ['nextest', 'run', '--workspace', '--no-fail-fast', '--no-tests=pass', ...(filter ? ['-E', filter] : [])]));
    jobs.push(step('cargo fmt --check', 'cargo', ['fmt', '--all', '--', '--check']));
    ran.push('cargo fmt --check');
    skipped.push('clippy, heavy tier, doctests (--full only)');
  } else skipped.push('Rust tests (no Rust file changed)');

  if (webTouched) {
    if (webAll) {
      ran.push('vitest (all), svelte-check, eslint (all): a shared frontend file changed');
      jobs.push(step('Frontend tests (vitest, all)', tools.vitest[0], [tools.vitest[1], 'run'], { cwd: appDir }));
      jobs.push(step('svelte-check', tools.svelteCheck[0], [tools.svelteCheck[1], '--tsconfig', './tsconfig.json', '--fail-on-warnings'], { cwd: appDir }));
      jobs.push(step('eslint (all)', tools.eslint[0], [tools.eslint[1], '.'], { cwd: appDir }));
    } else {
      if (webFiles.length) {
        ran.push(`vitest related to ${webFiles.length} file(s)`);
        jobs.push(step('Frontend tests (vitest related)', tools.vitest[0], [tools.vitest[1], 'related', '--run', '--passWithNoTests', ...webFiles], { cwd: appDir }));
      } else skipped.push('vitest (no source file under app/src changed)');
      if (lintFiles.length) {
        ran.push(`eslint on ${lintFiles.length} file(s)`);
        jobs.push(step('eslint (changed files)', tools.eslint[0], [tools.eslint[1], ...lintFiles], { cwd: appDir }));
      }
      ran.push('svelte-check');
      jobs.push(step('svelte-check', tools.svelteCheck[0], [tools.svelteCheck[1], '--tsconfig', './tsconfig.json', '--fail-on-warnings'], { cwd: appDir }));
    }
    skipped.push('window specs (e2e): `bash scripts/test.sh --e2e`');
  } else skipped.push('vitest, svelte-check, eslint (no frontend file changed)');

  if (!jobs.length) console.log('Only text files changed: nothing to run.');
  await Promise.all(jobs);
  report(ran, skipped);
}

async function fallbackAll() {
  need();
  needNextest();
  await Promise.all([step('Rust tests (everyday set)', 'cargo', ['nextest', 'run', '--workspace', '--no-fail-fast']), step('Frontend tests (vitest)', tools.vitest[0], [tools.vitest[1], 'run'], { cwd: appDir })]);
  report(['everyday set'], ['everything else']);
}

// ------------------------------------------------------------------------------------------------ e2e
function newestInput() {
  let newest = 0;
  const skip = new Set(['node_modules', 'target', 'dist', '.git', 'tests', 'e2e-tauri', 'test-results', 'playwright-report']);
  const walk = (dir) => {
    for (const d of fs.readdirSync(dir, { withFileTypes: true })) {
      if (skip.has(d.name) && !(d.name === 'tests' && false)) continue;
      const p = path.join(dir, d.name);
      if (d.isDirectory()) walk(p);
      else if (/\.(rs|toml|lock|ts|js|svelte|css|html|json|png|ico|icns|md)$/.test(d.name) && !/\.(test|spec)\.ts$/.test(d.name)) newest = Math.max(newest, fs.statSync(p).mtimeMs);
    }
  };
  for (const d of ['crates', path.join('app', 'src'), path.join('app', 'src-tauri')]) walk(path.join(root, d));
  for (const f of ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', path.join('app', 'package.json'), path.join('app', 'vite.config.ts'), path.join('app', 'svelte.config.js'), path.join('app', 'index.html'), path.join('app', 'charts.html'), path.join('app', 'editors.html'), path.join('app', 'sources.html')]) {
    const p = path.join(root, f);
    if (fs.existsSync(p)) newest = Math.max(newest, fs.statSync(p).mtimeMs);
  }
  return newest;
}

async function buildE2e() {
  const built = path.join(e2eTarget, 'debug', exe);
  if (process.env.GT_E2E_APP) return;
  if (fs.existsSync(built) && fs.statSync(built).mtimeMs >= newestInput()) {
    console.log(`Test exe is up to date: ${built}`);
    return;
  }
  console.log('Building the test exe (debug, custom-protocol) into target/e2e, apart from the folder cargo test and clippy use ...');
  const r = await runTask({
    name: 'build test exe',
    file: tools.tauri[0],
    args: [tools.tauri[1], 'build', '--debug', '--no-bundle'],
    cwd: appDir,
    env: { CARGO_TARGET_DIR: e2eTarget },
    log: path.join(logDir, 'build-test-exe.log'),
  }).done;
  console.log(`${mark(r)}  ${r.seconds.toFixed(0).padStart(4)} s  build test exe`);
  if (r.code !== 0) {
    console.log(tail(r.log, 60));
    process.exit(1);
  }
}

function runE2e(specs) {
  return new Promise((resolve) => {
    runTask({ name: 'window specs', file: node, args: [path.join(here, 'e2e-parallel.mjs'), ...specs], cwd: root, log: null }).done.then((res) => resolve(res.code));
  });
}
