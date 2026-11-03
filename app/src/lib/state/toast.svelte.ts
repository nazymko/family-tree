export interface ToastItem {
  id: number;
  text: string;
  action?: { label: string; run: () => void };
  kind: 'ok' | 'warn' | 'error';
}

class Toasts {
  items = $state<ToastItem[]>([]);
  #next = 1;

  push(t: Omit<ToastItem, 'id' | 'kind'> & { kind?: ToastItem['kind'] }): number {
    const id = this.#next++;
    this.items = [...this.items.slice(-3), { kind: 'ok', ...t, id }];
    return id;
  }

  dismiss(id: number): void {
    this.items = this.items.filter((t) => t.id !== id);
  }

  clear(): void {
    this.items = [];
  }
}

export const toasts = new Toasts();
