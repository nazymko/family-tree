/** Positions of every `[data-flip-key]` element in `container`, to hand to `flipMove` after the DOM changes. */
export function snapshotRects(container: HTMLElement): Map<string, DOMRect> {
  const m = new Map<string, DOMRect>();
  for (const el of container.querySelectorAll<HTMLElement>('[data-flip-key]')) {
    const key = el.dataset.flipKey;
    if (key) m.set(key, el.getBoundingClientRect());
  }
  return m;
}

/**
 * FLIP: animates elements with `data-flip-key` from where they were (`before`) to where they are now,
 * using the Web Animations API. Does nothing when `reduced` is true.
 */
export function flipMove(container: HTMLElement, before: Map<string, DOMRect>, ms: number, reduced: boolean): void {
  if (reduced || ms <= 0) return;
  for (const el of container.querySelectorAll<HTMLElement>('[data-flip-key]')) {
    const prev = before.get(el.dataset.flipKey ?? '');
    if (!prev || typeof el.animate !== 'function') continue;
    const now = el.getBoundingClientRect();
    const dx = prev.left - now.left;
    const dy = prev.top - now.top;
    if (Math.abs(dx) < 0.5 && Math.abs(dy) < 0.5) continue;
    el.animate([{ transform: `translate(${dx}px, ${dy}px)` }, { transform: 'none' }], {
      duration: ms,
      easing: 'cubic-bezier(0.2, 0, 0, 1)',
    });
  }
}
