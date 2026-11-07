/**
 * WebdriverIO + tauri-driver against the real debug build of the desktop app.
 *
 *   bash scripts/test.sh --e2e            (builds the test exe if stale, runs every spec, several at a time)
 *   bash scripts/test.sh --e2e flows date-picker
 *
 * or by hand, one spec after another in this process:
 *
 *   npm --prefix app run tauri -- build --debug --no-bundle     (with CARGO_TARGET_DIR=target/e2e, see scripts/test.sh)
 *   npm --prefix app run e2e
 *
 * Needs `tauri-driver` (cargo install tauri-driver --locked) and, on Windows, a `msedgedriver` that matches the installed
 * WebView2 runtime (found on PATH, in $GT_E2E_EDGEDRIVER, or in <repo>/.tools/msedgedriver, also in the main checkout when
 * this is a git worktree). On Linux it needs `WebKitWebDriver` (webkit2gtk-driver). macOS has no WebDriver for WKWebView,
 * so the suite runs on Windows and Linux only.
 *
 * ONE RUN OF THIS FILE IS ONE FULLY ISOLATED INSTANCE, so several can run at the same time (scripts/e2e-parallel.mjs
 * starts one per spec) and next to the owner's own running Gene Tree:
 *   - its own tauri-driver and native driver, on ports the OS gave out (not 4444/4445);
 *   - its own work folder, with its own settings folder (GT_E2E_CONFIG_DIR: settings, recents, key hashes, the AI key),
 *     its own WebView2 data folder (WEBVIEW2_USER_DATA_FOLDER) and its own copy of the project folders the specs make;
 *   - its own agent-access port (GT_E2E_AGENT_PORT);
 *   - a fresh settings folder and WebView2 folder for EVERY spec file, so no spec depends on another and any spec can
 *     run alone.
 * The app starts with GT_E2E=1 (test hooks in debug builds only; with it the single-instance plugin is not registered,
 * and the keychain and the shared temp folder are not touched).
 */
import { spawn, spawnSync, type ChildProcess } from 'node:child_process';
import fs from 'node:fs';
import net from 'node:net';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const repo = path.resolve(here, '..', '..');
const exe = process.platform === 'win32' ? 'gene-tree.exe' : 'gene-tree';
/** The e2e build lives in its own target folder, so `cargo test` and `clippy` never overwrite it. */
const application = process.env.GT_E2E_APP ?? defaultApplication();
const persons = process.env.GT_E2E_PERSONS ?? '5000';

function defaultApplication(): string {
  const own = path.join(repo, 'target', 'e2e', 'debug', exe);
  return fs.existsSync(own) ? own : path.join(repo, 'target', 'debug', exe);
}

/**
 * A free port picked at random from 41000-48999: below Windows' dynamic range (49152 and up), where the outgoing
 * connections of every program on the machine get their ports, and above the ports the Rust tests choose (20000-40000).
 * Asking the OS for port 0 gave a port from the dynamic range, and one agent-access run lost it to someone else's
 * connection between picking and listening.
 */
async function freePort(): Promise<number> {
  for (;;) {
    const port = 41000 + Math.floor(Math.random() * 8000);
    const free = await new Promise<boolean>((resolve) => {
      const s = net.createServer();
      s.once('error', () => resolve(false));
      s.listen(port, '127.0.0.1', () => s.close(() => resolve(true)));
    });
    if (free) return port;
  }
}

/** The launcher picks the ports once and the workers it starts read them from the environment. */
async function envPort(name: string): Promise<number> {
  if (!process.env[name]) process.env[name] = String(await freePort());
  return Number(process.env[name]);
}

const driverPort = await envPort('GT_E2E_DRIVER_PORT');
const nativePort = await envPort('GT_E2E_NATIVE_PORT');
await envPort('GT_E2E_AGENT_PORT');

let driver: ChildProcess | undefined;

function find(command: string): string | undefined {
  const probe = spawnSync(process.platform === 'win32' ? 'where' : 'which', [command], { encoding: 'utf8' });
  return probe.status === 0 ? probe.stdout.split(/\r?\n/)[0].trim() : undefined;
}

/** The folder that holds `.tools`: this checkout, or the main checkout when this is a git worktree. */
function toolRoots(): string[] {
  const roots = [repo];
  const common = spawnSync('git', ['rev-parse', '--path-format=absolute', '--git-common-dir'], { cwd: repo, encoding: 'utf8' });
  if (common.status === 0) roots.push(path.dirname(common.stdout.trim()));
  return roots;
}

function nativeDriver(): string | undefined {
  if (process.env.GT_E2E_EDGEDRIVER) return process.env.GT_E2E_EDGEDRIVER;
  if (process.platform === 'win32') {
    for (const root of toolRoots()) {
      const local = path.join(root, '.tools', 'msedgedriver', 'msedgedriver.exe');
      if (fs.existsSync(local)) return local;
    }
    return find('msedgedriver');
  }
  return find('WebKitWebDriver');
}

function python(): string {
  for (const c of ['python3', 'python', 'py']) if (find(c)) return c;
  throw new Error('Python is needed to write the synthetic GEDCOM');
}

/** Every spec starts from the same state: English, no recents, no stored keys, a fresh WebView2 profile. */
function freshState() {
  const config = process.env.GT_E2E_CONFIG_DIR!;
  fs.rmSync(config, { recursive: true, force: true });
  fs.mkdirSync(config, { recursive: true });
  // The app starts in Ukrainian; the specs read English unless one switches the language itself.
  fs.writeFileSync(path.join(config, 'settings.json'), JSON.stringify({ language: 'en' }));
  fs.rmSync(process.env.WEBVIEW2_USER_DATA_FOLDER!, { recursive: true, force: true });
}

/** Kills the driver and the native driver it started (a process tree, by the PID this run started). */
function stopDriver() {
  if (!driver?.pid) return;
  if (process.platform === 'win32') spawnSync('taskkill', ['/PID', String(driver.pid), '/T', '/F'], { stdio: 'ignore' });
  else driver.kill();
  driver = undefined;
}

/**
 * When a script started this run (GT_E2E_PARENT_PID) and that script dies without cleaning up, nothing on Windows
 * stops its children. So the run checks its parent and takes its whole process tree (drivers, app) down with it.
 */
function watchParent() {
  const parent = Number(process.env.GT_E2E_PARENT_PID);
  if (!parent) return;
  const timer = setInterval(() => {
    try {
      process.kill(parent, 0);
    } catch {
      stopDriver();
      if (process.platform === 'win32') spawnSync('taskkill', ['/PID', String(process.pid), '/T', '/F'], { stdio: 'ignore' });
      process.exit(1);
    }
  }, 2000);
  timer.unref();
}

export const config: WebdriverIO.Config = {
  runner: 'local',
  hostname: '127.0.0.1',
  port: driverPort,
  path: '/',
  specs: [
    'flows',
    'screenshots',
    'agent-access',
    'voice-samples',
    'selection-tree',
    'arrange-around',
    'one-scrollbar',
    'date-picker',
    'merge-copy',
    'family-notes',
    'maiden-name',
  ].map((name) => path.join(here, 'specs', `${name}.e2e.ts`)),
  // One instance of this file runs its specs one after another; scripts/e2e-parallel.mjs runs several instances.
  maxInstances: 1,
  capabilities: [
    {
      browserName: 'wry',
      'wdio:enforceWebDriverClassic': true,
      'tauri:options': { application },
    } as WebdriverIO.Capabilities,
  ],
  logLevel: 'warn',
  waitforTimeout: 30000, // a machine busy with the gate runs a spec 2-3 times slower; a wait that is not needed costs nothing
  strictSelectors: false,
  connectionRetryTimeout: 60000,
  connectionRetryCount: 2,
  framework: 'mocha',
  reporters: ['spec'],
  mochaOpts: { ui: 'bdd', timeout: 240000 },

  onPrepare() {
    if (!fs.existsSync(application)) {
      throw new Error(`${application} is missing. Build it first: bash scripts/test.sh --e2e (or see the header of wdio.conf.ts)`);
    }
    // A caller (scripts/e2e-parallel.mjs) can hand over a ready file and work folder; then this run does not delete them.
    const given = process.env.GT_E2E_GED;
    const work = fs.mkdtempSync(path.join(os.tmpdir(), 'gt-e2e-'));
    const ged = given ?? path.join(work, 'synthetic.ged');
    if (!given) {
      const made = spawnSync(python(), ['-B', path.join(repo, 'scripts', 'make-synthetic-ged.py'), ged, '--persons', persons], { encoding: 'utf8' });
      if (made.status !== 0) throw new Error(`make-synthetic-ged.py failed: ${made.stderr}`);
    }
    // Everything below is inherited by tauri-driver and then by the app it starts.
    process.env.GT_E2E = '1';
    process.env.GT_E2E_WORK = work;
    process.env.GT_E2E_GED = ged;
    process.env.GT_E2E_CONFIG_DIR = path.join(work, 'config');
    process.env.WEBVIEW2_USER_DATA_FOLDER = path.join(work, 'webview');
    freshState();
    if (!process.env.GT_E2E_SHOTS) process.env.GT_E2E_SHOTS = path.join(work, 'shots');

    const native = nativeDriver();
    if (!native) throw new Error('No native WebDriver found (msedgedriver on Windows, WebKitWebDriver on Linux)');
    const bin = path.join(os.homedir(), '.cargo', 'bin', process.platform === 'win32' ? 'tauri-driver.exe' : 'tauri-driver');
    driver = spawn(
      fs.existsSync(bin) ? bin : 'tauri-driver',
      ['--port', String(driverPort), '--native-port', String(nativePort), '--native-driver', native],
      { stdio: [null, process.stdout, process.stderr] },
    );
    driver.on('error', (e) => console.error('tauri-driver failed to start:', e));
    watchParent();
    return new Promise<void>((resolve) => setTimeout(resolve, 2000));
  },

  // Runs in the worker before the session (and so the app) starts: every spec file gets a fresh settings folder.
  beforeSession() {
    freshState();
  },

  onComplete() {
    stopDriver();
    const work = process.env.GT_E2E_WORK;
    if (work && !process.env.GT_E2E_KEEP) fs.rmSync(work, { recursive: true, force: true });
  },
};

process.on('exit', stopDriver);
