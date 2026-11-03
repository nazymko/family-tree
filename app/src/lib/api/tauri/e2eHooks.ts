import { invoke } from '@tauri-apps/api/core';

/**
 * Test-only seam for the end-to-end suite (WebdriverIO cannot click a native dialog or drag a file out of the OS).
 *
 * The Rust command `e2e_hooks` exists only in debug builds and answers only when the environment variable `GT_E2E=1`
 * is set. In a release build the invoke is rejected ("command not found"), so `install()` resolves to `null` and
 * nothing is exposed on `window`. When on, the suite drives the dialogs through `window.__gtE2e`.
 */
export interface E2eBridge {
  /** Queue the answer of the next dialog (a path; several files for `pickFiles` are joined with `|`). */
  pushDialog(result: string): void;
  /** Emit a drop of these files, as if dragged onto the window. */
  drop(paths: string[]): void;
}

declare global {
  interface Window {
    __gtE2e?: E2eBridge;
  }
}

export class E2eHooks implements E2eBridge {
  private queue: string[];
  private droppers = new Set<(paths: string[]) => void>();

  constructor(initial: string[]) {
    this.queue = [...initial];
  }

  pushDialog(result: string): void {
    this.queue.push(result);
  }

  nextDialog(): string | null {
    return this.queue.shift() ?? null;
  }

  onDrop(cb: (paths: string[]) => void): () => void {
    this.droppers.add(cb);
    return () => this.droppers.delete(cb);
  }

  drop(paths: string[]): void {
    for (const cb of [...this.droppers]) cb(paths);
  }
}

/** Resolves to the hooks when the backend has them switched on, else `null`. Never throws. */
export async function installE2eHooks(): Promise<E2eHooks | null> {
  try {
    const r = await invoke<{ dialogResults: string[] } | null>('e2e_hooks');
    if (!r) return null;
    const hooks = new E2eHooks(r.dialogResults);
    window.__gtE2e = hooks;
    return hooks;
  } catch {
    return null;
  }
}
