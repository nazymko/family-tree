import type { Platform } from '../api';

/**
 * Fake OS dialogs. Every call is recorded in `calls`; results come from `next` (set it before the
 * action under test) or a sensible default. `simulateDrop` fires the registered drop callbacks.
 */
export class MockPlatform implements Platform {
  calls: { method: string; args: unknown[] }[] = [];
  next: { file?: string | null; files?: string[]; folder?: string | null; save?: string | null } = {};
  private dropCallbacks = new Set<{ cb: (paths: string[]) => void; onHover?: (over: boolean) => void }>();

  async pickFile(opts: { title: string; extensions?: string[] }): Promise<string | null> {
    this.calls.push({ method: 'pickFile', args: [opts] });
    return this.next.file !== undefined ? this.next.file : 'C:/Mock/Family tree.ged';
  }

  async pickFiles(opts: { title: string; extensions?: string[] }): Promise<string[]> {
    this.calls.push({ method: 'pickFiles', args: [opts] });
    return this.next.files ?? ['C:/Mock/Photos/wedding.jpg'];
  }

  async pickFolder(title: string): Promise<string | null> {
    this.calls.push({ method: 'pickFolder', args: [title] });
    return this.next.folder !== undefined ? this.next.folder : 'C:/Mock/Genealogy';
  }

  async pickSavePath(opts: { title: string; defaultName: string; extensions: string[] }): Promise<string | null> {
    this.calls.push({ method: 'pickSavePath', args: [opts] });
    return this.next.save !== undefined ? this.next.save : `C:/Mock/${opts.defaultName}`;
  }

  onFilesDropped(cb: (paths: string[]) => void, onHover?: (over: boolean) => void): () => void {
    const entry = { cb, onHover };
    this.dropCallbacks.add(entry);
    return () => this.dropCallbacks.delete(entry);
  }

  async openExternally(path: string): Promise<void> {
    this.calls.push({ method: 'openExternally', args: [path] });
  }

  async reveal(path: string): Promise<void> {
    this.calls.push({ method: 'reveal', args: [path] });
  }

  simulateHover(over: boolean): void {
    for (const d of this.dropCallbacks) d.onHover?.(over);
  }

  simulateDrop(paths: string[]): void {
    for (const d of this.dropCallbacks) {
      d.onHover?.(false);
      d.cb(paths);
    }
  }
}
