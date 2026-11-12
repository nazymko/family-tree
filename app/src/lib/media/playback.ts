/**
 * One thing plays at a time across the app: a voice sample, a clip or the full recording. Starting one pauses
 * every other player on the page, wherever it is.
 */
export function playOnly(me: HTMLMediaElement): void {
  for (const a of document.querySelectorAll<HTMLMediaElement>('audio, video')) if (a !== me && !a.paused) a.pause();
}

/**
 * Calls `onEnd` when `a` reaches `endMs`, to the millisecond. `timeupdate` fires only about four times a second,
 * so a listener on it lets a clip run on for up to 250 ms. Instead a timer is set for the time left (at the
 * current playback rate) while the element plays, and set again after a seek, a rate change or a start; it is
 * cleared on pause. The time left is read from the element each time, so a timer that fires early just waits again.
 * Returns the function that stops watching (call it when the clip is left, and on destroy). `onEnd` runs once.
 */
export function watchEnd(a: HTMLMediaElement, endMs: number, onEnd: () => void): () => void {
  let timer: ReturnType<typeof setTimeout> | undefined;
  let done = false;
  const clear = () => {
    if (timer !== undefined) clearTimeout(timer);
    timer = undefined;
  };
  const arm = () => {
    clear();
    if (done || a.paused) return;
    const left = endMs - a.currentTime * 1000;
    if (left <= 1) {
      stop();
      onEnd();
      return;
    }
    timer = setTimeout(arm, Math.max(1, left / Math.max(0.0625, a.playbackRate || 1)));
  };
  const events = ['play', 'playing', 'seeked', 'ratechange'] as const;
  const stopEvents = ['pause', 'seeking', 'ended', 'emptied'] as const;
  function stop() {
    done = true;
    clear();
    for (const e of events) a.removeEventListener(e, arm);
    for (const e of stopEvents) a.removeEventListener(e, clear);
  }
  for (const e of events) a.addEventListener(e, arm);
  for (const e of stopEvents) a.addEventListener(e, clear);
  arm();
  return stop;
}
