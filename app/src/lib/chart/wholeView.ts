import type { Id } from '../api/types';
import type { Camera } from './camera';

/**
 * Where the whole-tree chart was looked at when its reader left it for the screen built around a person. Coming back
 * (the screen's Back button, or the shell's back arrow) puts the same person under the selection and the view where it
 * was, instead of starting again on the focus person.
 */
export interface WholeView {
  /** Which chart: the chosen people, whether their descendants are on it, and the tree direction. */
  key: string;
  person: Id;
  cam: Camera;
}

let remembered: WholeView | null = null;

export const wholeViewKey = (people: Id[], descendants: boolean, reversed: boolean): string =>
  `${people.join(',')}|${descendants ? 'd' : ''}|${reversed ? 'r' : ''}`;

export function rememberWholeView(v: WholeView): void {
  remembered = { ...v, cam: { ...v.cam } };
}

/** The remembered view when it belongs to this chart and this person; it stays, so Back, Forward and Back again all return to it. */
export function recallWholeView(key: string, person: Id | null): WholeView | null {
  return remembered && remembered.key === key && remembered.person === person ? remembered : null;
}

export function forgetWholeView(): void {
  remembered = null;
}
