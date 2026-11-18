#!/usr/bin/env node
// Screenshots of the Map view in the real test exe, with an invented family and a local stand-in for GeoNames.
//   node scripts/maps-shots.mjs [out-dir]      (default: Saved/maps-shots)
// Needs the test exe (bash scripts/test.sh --e2e builds it). Starts a local server for ZZ.zip, runs the `maps` spec, stops the server.
import { spawn, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const out = path.resolve(process.argv[2] ?? path.join(repo, 'Saved', 'maps-shots'));
const fixture = path.join(out, 'fixture');
const made = spawnSync('python', ['-B', path.join(repo, 'scripts', 'make-maps-fixture.py'), fixture], { encoding: 'utf8' });
if (made.status !== 0) {
  console.error(made.stderr);
  process.exit(2);
}
const zip = fs.readFileSync(path.join(fixture, 'ZZ.zip'));
const server = http.createServer((req, res) => {
  if (req.url?.endsWith('/ZZ.zip')) {
    res.writeHead(200, { 'Content-Length': zip.length, 'Content-Type': 'application/zip' });
    res.end(req.method === 'HEAD' ? undefined : zip);
  } else {
    res.writeHead(404, { 'Content-Length': 0 });
    res.end();
  }
});
await new Promise((r) => server.listen(0, '127.0.0.1', r));
const base = `http://127.0.0.1:${server.address().port}`;
const wdio = path.join(repo, 'app', 'node_modules', '@wdio', 'cli', 'bin', 'wdio.js');
const child = spawn(process.execPath, [wdio, 'run', 'e2e-tauri/wdio.conf.ts', '--spec', path.join(repo, 'app', 'e2e-tauri', 'specs', 'maps.e2e.ts')], {
  cwd: path.join(repo, 'app'),
  stdio: 'inherit',
  env: {
    ...process.env,
    GT_E2E_GED: path.join(fixture, 'maps.ged'),
    GT_E2E_SHOTS: out,
    GT_GAZETTEER_BASE: base,
    GT_E2E_DRIVER_PORT: '',
    GT_E2E_NATIVE_PORT: '',
    GT_E2E_AGENT_PORT: '',
    GT_E2E_WORK: '',
    GT_E2E_CONFIG_DIR: '',
  },
});
const code = await new Promise((r) => child.on('exit', r));
server.close();
process.exit(code ?? 1);
