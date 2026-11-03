import { afterEach, describe, expect, it, vi } from 'vitest';
import { formatKeys, registerShortcuts, setShortcutPlatform } from './shortcuts';

const press = (key: string, init: KeyboardEventInit = {}, target: EventTarget = document.body) =>
  target.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...init }));

afterEach(() => setShortcutPlatform(null));

describe('shortcuts', () => {
  it('mod_maps_to_ctrl_on_windows_cmd_on_mac', () => {
    const run = vi.fn();
    const off = registerShortcuts([{ id: 'k', keys: 'Mod+K', label: 'Search', run }]);
    setShortcutPlatform('other');
    press('k', { metaKey: true });
    expect(run).not.toHaveBeenCalled();
    press('k', { ctrlKey: true });
    expect(run).toHaveBeenCalledTimes(1);
    setShortcutPlatform('mac');
    press('k', { ctrlKey: true });
    expect(run).toHaveBeenCalledTimes(1);
    press('k', { metaKey: true });
    expect(run).toHaveBeenCalledTimes(2);
    off();
  });

  it('sequence_g_p_navigates', () => {
    const run = vi.fn();
    const off = registerShortcuts([{ id: 'gp', keys: 'g p', label: 'Go to people', run }]);
    press('g');
    expect(run).not.toHaveBeenCalled();
    press('p');
    expect(run).toHaveBeenCalledTimes(1);
    press('p');
    expect(run).toHaveBeenCalledTimes(1);
    off();
  });

  it('ignored_inside_text_input', () => {
    const run = vi.fn();
    const search = vi.fn();
    const off = registerShortcuts([
      { id: 'help', keys: '?', label: 'Help', run },
      { id: 'k', keys: 'Mod+K', label: 'Search', run: search, inInput: true },
    ]);
    const input = document.createElement('input');
    document.body.append(input);
    input.focus();
    press('?', { shiftKey: true }, input);
    expect(run).not.toHaveBeenCalled();
    press('k', { ctrlKey: true }, input);
    expect(search).toHaveBeenCalledTimes(1);
    input.blur();
    press('?', { shiftKey: true });
    expect(run).toHaveBeenCalledTimes(1);
    input.remove();
    off();
  });

  it('unregister_removes', () => {
    const run = vi.fn();
    const off = registerShortcuts([{ id: 'x', keys: 'Mod+Shift+Z', label: 'Redo', run }]);
    press('Z', { ctrlKey: true, shiftKey: true });
    expect(run).toHaveBeenCalledTimes(1);
    off();
    press('Z', { ctrlKey: true, shiftKey: true });
    expect(run).toHaveBeenCalledTimes(1);
  });

  it('later_registration_wins_and_when_gates', () => {
    const a = vi.fn();
    const b = vi.fn();
    const offA = registerShortcuts([{ id: 'a', keys: 'Mod+E', label: 'A', run: a }]);
    const offB = registerShortcuts([{ id: 'b', keys: 'Mod+E', label: 'B', run: b, when: () => false }]);
    press('e', { ctrlKey: true });
    expect(a).toHaveBeenCalledTimes(1);
    expect(b).not.toHaveBeenCalled();
    offA();
    offB();
  });

  it('format_keys', () => {
    expect(formatKeys('Mod+Shift+Z', 'mac')).toBe('⌘⇧Z');
    expect(formatKeys('Mod+Shift+Z', 'other')).toBe('Ctrl+Shift+Z');
    expect(formatKeys('Alt+ArrowLeft', 'other')).toBe('Alt+←');
    expect(formatKeys('g p', 'other')).toBe('G then P');
  });
});
