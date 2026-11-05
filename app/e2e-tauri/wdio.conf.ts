/**
 * WebdriverIO + tauri-driver against the real debug build of the desktop app.
 *
 *   npm --prefix app run tauri -- build --debug --no-bundle
 *   npm --prefix app run e2e
 *
 * Needs `tauri-driver` (cargo install tauri-driver --locked) and, on Windows, a `msedgedriver` that matches the installed
 * WebView2 runtime (found on PATH, in $GT_E2E_EDGEDRIVER, or in <repo>/.tools/msedgedriver). On Linux it needs
 * `WebKitWebDriver` (webkit2gtk-driver). macOS has no WebDriver for WKWebView, so the suite runs on Windows and Linux only.
 *
 * The app is started with GT_E2E=1 (test hooks in debug builds only), an isolated settings folder and the synthetic
 * GEDCOM that onPrepare writes with scripts/make-synthetic-ged.py.
 */
import { spawn, spawnSync, type ChildProcess } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const repo = path.resolve(here, '..', '..');
const exe = process.platform === 'win32' ? 'gene-tree.exe' : 'gene-tree';
const application = path.join(repo, 'target', 'debug', exe);
const persons = process.env.GT_E2E_PERSONS ?? '5000';

let driver: ChildProcess | undefined;

function find(command: string): string | undefined {
  const probe = spawnSync(process.platform === 'win32' ? 'where' : 'which', [command], { encoding: 'utf8' });
  return probe.status === 0 ? probe.stdout.split(/\r?\n/)[0].trim() : undefined;
}

function nativeDriver(): string | undefined {
  if (process.env.GT_E2E_EDGEDRIVER) return process.env.GT_E2E_EDGEDRIVER;
  if (process.platform === 'win32') {
    const local = path.join(repo, '.tools', 'msedgedriver', 'msedgedriver.exe');
    return fs.existsSync(local) ? local : find('msedgedriver');
  }
  return find('WebKitWebDriver');
}

function python(): string {
  for (const c of ['python3', 'python', 'py']) if (find(c)) return c;
  throw new Error('Python is needed to write the synthetic GEDCOM');
}

export const config: WebdriverIO.Config = {
  runner: 'local',
  hostname: '127.0.0.1',
  port: 4444,
  path: '/',
  specs: [path.join(here, 'specs', 'flows.e2e.ts'), path.join(here, 'specs', 'screenshots.e2e.ts'), path.join(here, 'specs', 'agent-access.e2e.ts')],
  maxInstances: 1,
  capabilities: [
    {
      browserName: 'wry',
      'wdio:enforceWebDriverClassic': true,
      'tauri:options': { application },
    } as WebdriverIO.Capabilities,
  ],
  logLevel: 'warn',
  waitforTimeout: 15000,
  strictSelectors: false,
  connectionRetryTimeout: 60000,
  connectionRetryCount: 2,
  framework: 'mocha',
  reporters: ['spec'],
  mochaOpts: { ui: 'bdd', timeout: 240000 },

  onPrepare() {
    if (!fs.existsSync(application)) {
      throw new Error(`${application} is missing. Build it first: npm --prefix app run tauri -- build --debug --no-bundle`);
    }
    const work = fs.mkdtempSync(path.join(os.tmpdir(), 'gt-e2e-'));
    const ged = path.join(work, 'synthetic.ged');
    const made = spawnSync(python(), ['-B', path.join(repo, 'scripts', 'make-synthetic-ged.py'), ged, '--persons', persons], { encoding: 'utf8' });
    if (made.status !== 0) throw new Error(`make-synthetic-ged.py failed: ${made.stderr}`);
    // Everything below is inherited by tauri-driver and then by the app it starts.
    process.env.GT_E2E = '1';
    process.env.GT_E2E_WORK = work;
    process.env.GT_E2E_GED = ged;
    process.env.GT_E2E_CONFIG_DIR = path.join(work, 'config');
    // The app starts in Ukrainian; the specs read English unless one switches the language itself.
    fs.mkdirSync(process.env.GT_E2E_CONFIG_DIR, { recursive: true });
    fs.writeFileSync(path.join(process.env.GT_E2E_CONFIG_DIR, 'settings.json'), JSON.stringify({ language: 'en' }));
    if (!process.env.GT_E2E_SHOTS) process.env.GT_E2E_SHOTS = path.join(work, 'shots');

    const native = nativeDriver();
    if (!native) throw new Error('No native WebDriver found (msedgedriver on Windows, WebKitWebDriver on Linux)');
    const bin = path.join(os.homedir(), '.cargo', 'bin', process.platform === 'win32' ? 'tauri-driver.exe' : 'tauri-driver');
    driver = spawn(fs.existsSync(bin) ? bin : 'tauri-driver', ['--native-driver', native], { stdio: [null, process.stdout, process.stderr] });
    driver.on('error', (e) => console.error('tauri-driver failed to start:', e));
    return new Promise<void>((resolve) => setTimeout(resolve, 2000));
  },

  onComplete() {
    driver?.kill();
    const work = process.env.GT_E2E_WORK;
    if (work && !process.env.GT_E2E_KEEP) fs.rmSync(work, { recursive: true, force: true });
  },
};
