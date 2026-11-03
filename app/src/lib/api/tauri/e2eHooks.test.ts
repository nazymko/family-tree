import { afterEach, describe, expect, it } from 'vitest';
import { clearMocks, mockIPC, mockWindows } from '@tauri-apps/api/mocks';
import { installE2eHooks } from './e2eHooks';
import { TauriPlatform } from './TauriPlatform';

afterEach(() => {
  clearMocks();
  delete window.__gtE2e;
});

describe('e2e hooks', () => {
  it('stay off (and expose nothing) when the backend answers null or has no such command', async () => {
    mockWindows('main');
    mockIPC(() => null);
    expect(await installE2eHooks()).toBeNull();
    expect(window.__gtE2e).toBeUndefined();
    mockIPC(() => {
      throw new Error('command e2e_hooks not found');
    });
    expect(await installE2eHooks()).toBeNull();
    expect(window.__gtE2e).toBeUndefined();
  });

  it('answer dialogs from the queue and emit drops when on', async () => {
    mockWindows('main');
    mockIPC((cmd) => (cmd === 'e2e_hooks' ? { dialogResults: ['C:/a.ged'] } : null));
    const platform = new TauriPlatform();
    await expect.poll(() => window.__gtE2e).toBeDefined();
    window.__gtE2e!.pushDialog('C:/out');
    window.__gtE2e!.pushDialog('x.jpg|y.png');
    expect(await platform.pickFile({ title: 't' })).toBe('C:/a.ged');
    expect(await platform.pickFolder('t')).toBe('C:/out');
    expect(await platform.pickFiles({ title: 't' })).toEqual(['x.jpg', 'y.png']);
    expect(await platform.pickFolder('t')).toBeNull();

    const got: string[][] = [];
    const off = platform.onFilesDropped((p) => got.push(p));
    await new Promise((r) => setTimeout(r, 0));
    window.__gtE2e!.drop(['a.jpg', 'b.exe']);
    off();
    window.__gtE2e!.drop(['late.jpg']);
    expect(got).toEqual([['a.jpg', 'b.exe']]);
  });
});
