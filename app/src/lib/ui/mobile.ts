import type { Platform } from '../api/api';

/** On a phone or tablet: no folder pickers, no file manager, trees live in the app's own storage. */
export const onMobile = (p: Platform): boolean => !!p.isMobile;

/**
 * Hands a file the app just made to the person (the system save / share sheet) on a phone or tablet; nothing to do
 * on a computer, where the person chose the place beforehand. False when they backed out.
 */
export async function handOver(p: Platform, path: string): Promise<boolean> {
  return p.isMobile && p.deliver ? p.deliver(path) : true;
}

/** The "show in folder" action of a notice, left out where there is no folder to show. */
export function revealAction(p: Platform, path: string, label: string): { actionLabel?: string; onaction?: () => void } {
  return p.isMobile ? {} : { actionLabel: label, onaction: () => void p.reveal(path) };
}

/** A folder name for a tree: the name the person typed, without characters a folder name cannot hold. */
export function folderNameFor(name: string): string {
  const clean = Array.from(name.trim(), (c) => (c < ' ' || '\\/:*?"<>|'.includes(c) ? '_' : c))
    .join('')
    .replace(/^\.+/, '_');
  return clean || 'tree';
}
