import { toasts } from '../state/toast.svelte';

/** A one-off message from a view that does not go through `apply` (or that opted out of the default toast). */
export interface Notice {
  message: string;
  actionLabel?: string;
  onaction?: () => void;
  /** When it returns false the action is hidden (e.g. the change is no longer the latest one). */
  actionValid?: () => boolean;
}

/**
 * Notices are shown by the shell's single toast host (`state/toast`); `current` mirrors the latest one for
 * tests and for the dev harness, which mounts <ToastSlot /> instead of the shell.
 */
class Notices {
  current = $state<Notice | null>(null);
  #timer: ReturnType<typeof setTimeout> | undefined;
  /** The harness shows `current` itself; the app forwards to the toast host. */
  forward = true;

  show(n: Notice, ms = 7000): void {
    clearTimeout(this.#timer);
    this.current = n;
    if (this.forward) {
      toasts.push({
        text: n.message,
        action: n.actionLabel && n.onaction ? { label: n.actionLabel, run: n.onaction } : undefined,
      });
    }
    if (ms > 0) this.#timer = setTimeout(() => this.dismiss(), ms);
  }

  dismiss(): void {
    clearTimeout(this.#timer);
    this.current = null;
  }
}

export const notices = new Notices();
