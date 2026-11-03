export interface Shortcut {
  id: string;
  /** 'Mod+K', 'Mod+Shift+Z', 'Alt+ArrowLeft', 'g p' (sequence). 'Mod' is Ctrl on Windows/Linux and Cmd on macOS. */
  keys: string;
  label: string;
  run: () => void;
  when?: () => boolean;
  /** Fire even while focus is in a text field (Mod+K). */
  inInput?: boolean;
}

export type PlatformKind = 'mac' | 'other';

let platformOverride: PlatformKind | null = null;

/** Tests force a platform; pass null to go back to detection. */
export function setShortcutPlatform(p: PlatformKind | null): void {
  platformOverride = p;
}

export function currentPlatform(): PlatformKind {
  if (platformOverride) return platformOverride;
  if (typeof navigator === 'undefined') return 'other';
  return /Mac|iPhone|iPad/i.test(navigator.platform || navigator.userAgent) ? 'mac' : 'other';
}

const registry: Shortcut[] = [];
let installed = false;
let pending: { keys: string[]; at: number } | null = null;
const SEQUENCE_MS = 1000;

interface Combo {
  mod: boolean;
  shift: boolean;
  alt: boolean;
  key: string;
}

function parseCombo(spec: string): Combo {
  const parts = spec === '+' ? ['+'] : spec.split('+');
  const c: Combo = { mod: false, shift: false, alt: false, key: '' };
  for (const p of parts) {
    if (p === 'Mod') c.mod = true;
    else if (p === 'Shift') c.shift = true;
    else if (p === 'Alt') c.alt = true;
    else c.key = p;
  }
  return c;
}

function matches(spec: Combo, e: KeyboardEvent, plat: PlatformKind): boolean {
  const modDown = plat === 'mac' ? e.metaKey : e.ctrlKey;
  if (spec.mod !== modDown) return false;
  if (plat === 'mac' && e.ctrlKey) return false;
  if (spec.alt !== e.altKey) return false;
  // '?' needs Shift on most layouts, so shift is only compared for named keys and chords.
  const printable = spec.key.length === 1 && !spec.mod && !spec.alt;
  if (!printable && spec.shift !== e.shiftKey) return false;
  return e.key.toLowerCase() === spec.key.toLowerCase();
}

function isEditable(el: Element | null): boolean {
  if (!el) return false;
  const h = el as HTMLElement;
  if (h.isContentEditable) return true;
  const tag = el.tagName;
  if (tag === 'TEXTAREA' || tag === 'SELECT') return true;
  if (tag === 'INPUT') {
    const t = (el as HTMLInputElement).type;
    return !['checkbox', 'radio', 'button', 'submit', 'reset', 'range', 'color', 'file'].includes(t);
  }
  return false;
}

function onKeyDown(e: KeyboardEvent): void {
  if (e.defaultPrevented || e.isComposing) return;
  if (['Shift', 'Control', 'Alt', 'Meta'].includes(e.key)) return;
  const plat = currentPlatform();
  const editing = isEditable(document.activeElement);
  const live = [...registry].reverse();
  // While a modal dialog is open only shortcuts flagged `inInput` (Mod+K) still work.
  const modal = !!document.querySelector('[aria-modal="true"]');
  const usable = (s: Shortcut) =>
    (!editing || s.inInput === true) && (!modal || s.inInput === true) && (!s.when || s.when());
  const plain = !e.ctrlKey && !e.metaKey && !e.altKey;

  if (plain) {
    const now = Date.now();
    const base = pending && now - pending.at < SEQUENCE_MS ? pending.keys : [];
    const cand = [...base, e.key];
    const seqs = live.filter((s) => s.keys.includes(' ') && usable(s));
    const full = seqs.find((s) => s.keys === cand.join(' '));
    if (full) {
      pending = null;
      e.preventDefault();
      full.run();
      return;
    }
    const prefix = seqs.some((s) => {
      const steps = s.keys.split(' ');
      return steps.length > cand.length && cand.every((k, i) => steps[i] === k);
    });
    pending = prefix ? { keys: cand, at: now } : null;
    if (prefix) return;
  } else {
    pending = null;
  }

  for (const s of live) {
    if (s.keys.includes(' ') || !usable(s)) continue;
    if (matches(parseCombo(s.keys), e, plat)) {
      e.preventDefault();
      s.run();
      return;
    }
  }
}

/** Adds shortcuts; later registrations win over earlier ones for the same keys. Returns the unregister function. */
export function registerShortcuts(list: Shortcut[]): () => void {
  registry.push(...list);
  if (!installed && typeof window !== 'undefined') {
    window.addEventListener('keydown', onKeyDown);
    installed = true;
  }
  return () => {
    for (const s of list) {
      const i = registry.indexOf(s);
      if (i >= 0) registry.splice(i, 1);
    }
  };
}

const MAC_GLYPH: Record<string, string> = { Mod: '⌘', Shift: '⇧', Alt: '⌥', Ctrl: '⌃' };
const KEY_GLYPH: Record<string, string> = {
  ArrowLeft: '←',
  ArrowRight: '→',
  ArrowUp: '↑',
  ArrowDown: '↓',
  Escape: 'Esc',
};

function keyName(k: string): string {
  if (KEY_GLYPH[k]) return KEY_GLYPH[k];
  return k.length === 1 ? k.toUpperCase() : k;
}

export function formatKeys(keys: string, platform: PlatformKind): string {
  if (keys.includes(' ')) return keys.split(' ').map(keyName).join(' then ');
  const parts = keys === '+' ? ['+'] : keys.split('+');
  if (platform === 'mac') return parts.map((p) => MAC_GLYPH[p] ?? keyName(p)).join('');
  return parts.map((p) => (p === 'Mod' ? 'Ctrl' : keyName(p))).join('+');
}
