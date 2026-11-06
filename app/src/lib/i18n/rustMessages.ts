/**
 * Test helper: finds the fixed English messages the Rust side can send to the screen.
 * It reads Rust source text, so a new message with no catalogue pattern is caught by `backendMessages.test.ts`.
 */

export interface RustSource {
  path: string;
  src: string;
}

export interface RustMessage {
  path: string;
  line: number;
  /** The message as Rust writes it; a `{}` or `{name}` stays in place and marks a value filled in at run time. */
  text: string;
}

export interface DynamicSite {
  path: string;
  line: number;
  expr: string;
}

/** The source up to its test module; test code is not shown to anybody. */
export function productionPart(src: string): string {
  const cut = src.search(/^#\[cfg\(test\)\]/m);
  return cut >= 0 ? src.slice(0, cut) : src;
}

const lineOf = (src: string, index: number) => src.slice(0, index).split('\n').length;

/** The text between the parenthesis at `open` and its partner, skipping strings. */
export function balanced(src: string, open: number): string {
  let depth = 0;
  for (let i = open; i < src.length; i++) {
    const c = src[i];
    if (c === '"') {
      i++;
      while (i < src.length && src[i] !== '"') i += src[i] === '\\' ? 2 : 1;
    } else if (c === '(' || c === '[' || c === '{') depth++;
    else if (c === ')' || c === ']' || c === '}') {
      depth--;
      if (depth === 0) return src.slice(open + 1, i);
    }
  }
  return src.slice(open + 1);
}

/** The arguments of a call, split at top-level commas. */
export function splitArgs(args: string): string[] {
  const out: string[] = [];
  let depth = 0;
  let start = 0;
  for (let i = 0; i < args.length; i++) {
    const c = args[i];
    if (c === '"') {
      i++;
      while (i < args.length && args[i] !== '"') i += args[i] === '\\' ? 2 : 1;
    } else if (c === '(' || c === '[' || c === '{') depth++;
    else if (c === ')' || c === ']' || c === '}') depth--;
    else if (c === ',' && depth === 0) {
      out.push(args.slice(start, i).trim());
      start = i + 1;
    }
  }
  const last = args.slice(start).trim();
  if (last) out.push(last);
  return out;
}

/** The text of a Rust string literal body: line continuations joined, escapes resolved, `{{` and `}}` folded. */
export function unescapeRust(body: string): string {
  return body
    .replace(/\\\r?\n\s*/g, '')
    .replace(/\\"/g, '"')
    .replace(/\\'/g, "'")
    .replace(/\\n/g, '\n')
    .replace(/\\\\/g, '\\')
    .replace(/\{\{/g, '')
    .replace(/\}\}/g, '')
    .replace(//g, '{')
    .replace(//g, '}');
}

/** `"text"` as the text, or null when the expression is not one plain literal. */
function literal(expr: string): string | null {
  const m = /^"((?:[^"\\]|\\[\s\S])*)"$/.exec(expr.trim());
  return m ? unescapeRust(m[1]) : null;
}

/** Named string constants of a file: `const STALE: &str = "...";`. */
export function constants(src: string): Map<string, string> {
  const out = new Map<string, string>();
  for (const m of src.matchAll(/\bconst (\w+): &(?:'static )?str\s*=\s*("(?:[^"\\]|\\[\s\S])*")\s*;/g)) {
    const text = literal(m[2]);
    if (text !== null) out.set(m[1], text);
  }
  return out;
}

/**
 * The message an argument expression stands for: a literal, a `format!` of one, or a named const.
 * Returns null for anything computed.
 */
export function messageOf(expr: string, consts: Map<string, string>): string | null {
  const e = expr.trim().replace(/,$/, '').replace(/^&/, '');
  const lit = literal(e);
  if (lit !== null) return lit;
  const fmt = /^format!\(/.exec(e);
  if (fmt) {
    const first = splitArgs(balanced(e, e.indexOf('(')))[0] ?? '';
    return literal(first);
  }
  // `&format!(...)`, `"..".to_string()`, `"..".into()`, `String::from("..")`, `.to_owned()`.
  const wrapped = /^&?(?:String::from\((.*)\)|(".*")\s*\.(?:to_string|into|to_owned)\(\))$/s.exec(e);
  if (wrapped) return messageOf(wrapped[1] ?? wrapped[2], consts);
  if (/^[A-Z][A-Z0-9_]*$/.test(e)) return consts.get(e) ?? null;
  return null;
}

/**
 * Functions whose single text parameter is the message of an error they build: `fn refuse(msg: &str) -> ApiError`.
 * A function counts only when its body hands the parameter straight to `ApiError::new`.
 */
export function helperNames(src: string): [name: string, argIndex: number][] {
  const out: [string, number][] = [];
  // `fn refuse(message: impl Into<String>) -> ApiError { ApiError::new(ErrorCode::Invalid, message) }`
  for (const m of src.matchAll(/\bfn (\w+)\s*(?:<[^>]*>)?\(([^)]*)\)\s*->\s*ApiError\s*\{/g)) {
    const params = m[2].split(',').filter((p) => p.trim());
    const names = params.map((p) => /^\s*(\w+)\s*:/.exec(p)?.[1] ?? '');
    const body = balanced(src, m.index + m[0].length - 1);
    const index = names.findIndex((n) => n && new RegExp(`ApiError::new\\(\\s*[\\w:]+,\\s*${n}(?:\\.into\\(\\))?\\s*,?\\s*\\)`).test(body));
    if (index >= 0) out.push([m[1], index]);
  }
  // `let invalid = |m: &str| ApiError::new(ErrorCode::Invalid, m);`
  for (const m of src.matchAll(/\blet (\w+) = \|(\w+)(?::[^|]*)?\|\s*ApiError::new\(\s*[\w:]+,\s*\2\s*\)/g)) out.push([m[1], 0]);
  return out;
}

export interface Extracted {
  messages: RustMessage[];
  /** Calls whose message is computed, so no fixed text can be read from them. */
  dynamic: DynamicSite[];
}

/** Every message handed to `ApiError::new` or to a helper that wraps it, in the production part of the given files. */
export function extractMessages(sources: RustSource[], formatFiles = /(?!)/): Extracted {
  const parts = sources.map((s) => ({ path: s.path, src: productionPart(s.src) }));
  const helpers = new Map<string, number>();
  for (const p of parts) for (const [name, index] of helperNames(p.src)) helpers.set(name, index);
  const callees: [string, number][] = [['ApiError::new', 1], ['ImportError::new', 1], ['ImportError::invalid', 0], ['AtdbError::NotAtdb', 0], ['fail', 2], ['dto', 2], ...helpers];
  const messages: RustMessage[] = [];
  const dynamic: DynamicSite[] = [];
  for (const { path, src } of parts) {
    const consts = constants(src);
    // In these files every `format!("...")` is a message (the store error is worded in a `match`, outside any call).
    if (formatFiles.test(path)) {
      for (const m of src.matchAll(/\bformat!\(\s*"((?:[^"\\]|\\[\s\S])*)"/g)) messages.push({ path, line: lineOf(src, m.index), text: unescapeRust(m[1]) });
    }
    // Sentences kept in constants that a screen shows as they are (the note under the GEDCOM 5.5.1 export).
    for (const [name, text] of consts) if (/^LOSSLESS_/.test(name)) messages.push({ path, line: lineOf(src, src.indexOf(name)), text });
    // The reason a file was not attached: `reason: "already attached".into()` (the other reasons go through `fail(...)`).
    for (const m of src.matchAll(/\breason:\s*"((?:[^"\\]|\\[\s\S])*)"\.into\(\)/g)) messages.push({ path, line: lineOf(src, m.index), text: unescapeRust(m[1]) });
    // `#[error("...")]` of a thiserror enum: the text `to_string()` gives, which several commands pass on.
    for (const m of src.matchAll(/#\[error\(\s*"((?:[^"\\]|\\[\s\S])*)"/g)) {
      const text = unescapeRust(m[1]);
      if (text !== '{message}') messages.push({ path, line: lineOf(src, m.index), text });
    }
    for (const [name, argIndex] of callees) {
      // `dto` builds a repair line only in the importer's report; field_diff.rs has another function of that name.
      if (name === 'dto' && !path.endsWith('transfer.rs')) continue;
      const re = new RegExp(`(?:(?<![\\w:])|(?<=(?:super|ops)::))${name}\\(`, 'g');
      for (let m = re.exec(src); m; m = re.exec(src)) {
        // The definition of a helper is not a call.
        if (/\bfn\s+$/.test(src.slice(Math.max(0, m.index - 4), m.index))) continue;
        const args = splitArgs(balanced(src, m.index + m[0].length - 1));
        // Another function of the same name with no text argument is not this helper.
        if (args.length <= argIndex) continue;
        const expr = args[argIndex];
        const text = messageOf(expr, consts);
        const line = lineOf(src, m.index);
        if (text === null) dynamic.push({ path, line, expr: expr.replace(/\s+/g, ' ') });
        else messages.push({ path, line, text });
      }
    }
  }
  return { messages, dynamic };
}
