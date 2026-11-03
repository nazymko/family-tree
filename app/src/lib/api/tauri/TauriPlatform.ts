import { open, save } from '@tauri-apps/plugin-dialog';
import { revealItemInDir } from '@tauri-apps/plugin-opener';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import type { Platform } from '../api';

const filters = (extensions?: string[]) => (extensions?.length ? [{ name: 'Files', extensions }] : undefined);

/** OS dialogs, drag-drop and "open with the system" through the Tauri plugins. */
export class TauriPlatform implements Platform {
  async pickFile(opts: { title: string; extensions?: string[] }): Promise<string | null> {
    const r = await open({ title: opts.title, multiple: false, directory: false, filters: filters(opts.extensions) });
    return typeof r === 'string' ? r : null;
  }

  async pickFiles(opts: { title: string; extensions?: string[] }): Promise<string[]> {
    const r = await open({ title: opts.title, multiple: true, directory: false, filters: filters(opts.extensions) });
    return Array.isArray(r) ? r : [];
  }

  async pickFolder(title: string): Promise<string | null> {
    const r = await open({ title, multiple: false, directory: true });
    return typeof r === 'string' ? r : null;
  }

  async pickSavePath(opts: { title: string; defaultName: string; extensions: string[] }): Promise<string | null> {
    return save({ title: opts.title, defaultPath: opts.defaultName, filters: filters(opts.extensions) });
  }

  onFilesDropped(cb: (paths: string[]) => void, onHover?: (over: boolean) => void): () => void {
    let cancelled = false;
    let unlisten: (() => void) | null = null;
    void getCurrentWebview()
      .onDragDropEvent((event) => {
        const p = event.payload;
        if (p.type === 'enter' || p.type === 'over') onHover?.(true);
        else if (p.type === 'leave') onHover?.(false);
        else if (p.type === 'drop') {
          onHover?.(false);
          cb(p.paths);
        }
      })
      .then((fn) => {
        // The caller may have unsubscribed before the listener was registered.
        if (cancelled) fn();
        else unlisten = fn;
      })
      .catch(() => {});
    return () => {
      cancelled = true;
      unlisten?.();
      unlisten = null;
    };
  }

  reveal(path: string): Promise<void> {
    return revealItemInDir(path);
  }
}
