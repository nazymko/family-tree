/** One-at-a-time toast queue shared by the source, media and export views. Mount <ToastSlot /> once to show it. */
export interface Notice {
  message: string;
  actionLabel?: string;
  onaction?: () => void;
  /** When it returns false the action is hidden (e.g. the change is no longer the latest one). */
  actionValid?: () => boolean;
}

class Notices {
  current = $state<Notice | null>(null);
  #timer: ReturnType<typeof setTimeout> | undefined;

  show(n: Notice, ms = 7000): void {
    clearTimeout(this.#timer);
    this.current = n;
    if (ms > 0) this.#timer = setTimeout(() => this.dismiss(), ms);
  }

  dismiss(): void {
    clearTimeout(this.#timer);
    this.current = null;
  }
}

export const notices = new Notices();
