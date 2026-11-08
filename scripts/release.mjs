#!/usr/bin/env node
/**
 * Versions and release notes for the release cycle (docs/release.md). Used by the GitHub workflows and by hand.
 *
 *   node scripts/release.mjs next [auto|patch|minor|major]   the next version; prints nothing when there is nothing to release
 *   node scripts/release.mjs version <X.Y.Z[-pre]>           writes that version into every manifest (CI does this in its copy)
 *   node scripts/release.mjs notes <tag>                     release notes: the commits between the tag before <tag> and <tag>
 *   node scripts/release.mjs current                         the version in the manifests
 *
 * A release is a tag `vX.Y.Z` and nothing else: no workflow commits to main or develop, so the owner's local branches
 * never fall behind GitHub. The version in the repo files is only the starting point for the first release.
 *
 * "auto" picks minor when a `feat` commit landed since the last release tag, otherwise patch. Commits follow
 * `type(scope): text`; feat, fix and perf go into the notes, everything else (tests, docs, merges, chores) does not.
 */
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const file = (p) => path.join(root, p);
const git = (...args) =>
  execFileSync('git', args, { cwd: root, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim();
const SEMVER = /^(\d+)\.(\d+)\.(\d+)(-[0-9A-Za-z.-]+)?$/;
const TAGS = 'v[0-9]*.[0-9]*.[0-9]*';

function current() {
  return JSON.parse(fs.readFileSync(file('app/src-tauri/tauri.conf.json'), 'utf8')).version;
}

/** The newest release tag reachable from `rev` (HEAD by default), or null. */
function lastTag(rev = 'HEAD') {
  try {
    return git('describe', '--tags', '--abbrev=0', '--match', TAGS, rev);
  } catch {
    return null;
  }
}

const KINDS = [
  ['feat', 'Added'],
  ['fix', 'Fixed'],
  ['perf', 'Faster'],
];

function parse(subject) {
  const m = /^(\w+)(?:\(([^)]*)\))?!?:\s*(.+)$/.exec(subject);
  return m ? { type: m[1], scope: m[2], text: m[3] } : null;
}

/** Parsed commit subjects in `range`, merges left out. */
function commits(range) {
  const out = git('log', '--no-merges', '--format=%s', range);
  return (out ? out.split('\n') : []).map(parse).filter(Boolean);
}

function next(bump = 'auto') {
  const tag = lastTag();
  if (!tag) return current(); // the first release ships the version already in the manifests
  const list = commits(`${tag}..HEAD`);
  if (!list.some((c) => KINDS.some(([t]) => t === c.type))) return null; // nothing a user would notice
  const m = SEMVER.exec(tag.slice(1));
  if (!m) throw new Error(`The tag ${tag} is not vX.Y.Z`);
  let [maj, min, pat] = [Number(m[1]), Number(m[2]), Number(m[3])];
  if (bump === 'auto') bump = list.some((c) => c.type === 'feat') ? 'minor' : 'patch';
  if (bump === 'major') [maj, min, pat] = [maj + 1, 0, 0];
  else if (bump === 'minor') [min, pat] = [min + 1, 0];
  else if (bump === 'patch') pat += 1;
  else throw new Error(`Unknown bump "${bump}" (auto, patch, minor or major)`);
  return `${maj}.${min}.${pat}`;
}

function setVersion(rel, re, version) {
  const p = file(rel);
  const before = fs.readFileSync(p, 'utf8');
  if (!re.test(before)) throw new Error(`No version found in ${rel}`);
  fs.writeFileSync(p, before.replace(re, (_, a, _v, b) => `${a}${version}${b}`));
}

function version(v) {
  if (!SEMVER.test(v)) throw new Error(`"${v}" is not X.Y.Z or X.Y.Z-pre`);
  // The workspace version (every crate inherits it): the first `version = "..."` under [workspace.package].
  setVersion('Cargo.toml', /(\[workspace\.package\][^[]*?\nversion\s*=\s*")([^"]+)(")/, v);
  setVersion('app/src-tauri/tauri.conf.json', /("version"\s*:\s*")([^"]+)(")/, v);
  setVersion('app/package.json', /("version"\s*:\s*")([^"]+)(")/, v);
}

function notes(tag) {
  const prev = lastTag(`${tag}^`);
  const header = [];
  if (!prev) {
    header.push('The first public release. The README says what Family Tree does and how to install it.');
    return header.join('\n');
  }
  const list = commits(`${prev}..${tag}`);
  const parts = [];
  for (const [type, title] of KINDS) {
    const items = list.filter((c) => c.type === type);
    if (!items.length) continue;
    parts.push(`### ${title}`, '');
    for (const c of items) parts.push(`- ${c.scope ? `**${c.scope}**: ` : ''}${c.text}`);
    parts.push('');
  }
  parts.push(`**All changes**: https://github.com/nazymko/family-tree/compare/${prev}...${tag}`);
  return parts.join('\n');
}

const [cmd, arg] = process.argv.slice(2);
if (cmd === 'next') {
  const v = next(arg);
  if (v) console.log(v);
} else if (cmd === 'version') version(arg);
else if (cmd === 'notes') console.log(notes(arg));
else if (cmd === 'current') console.log(current());
else {
  console.error('usage: node scripts/release.mjs next [auto|patch|minor|major] | version <X.Y.Z> | notes <tag> | current');
  process.exit(2);
}
