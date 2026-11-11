/**
 * One thing plays at a time across the app: a voice sample, a clip or the full recording. Starting one pauses
 * every other player on the page, wherever it is.
 */
export function playOnly(me: HTMLMediaElement): void {
  for (const a of document.querySelectorAll<HTMLMediaElement>('audio, video')) if (a !== me && !a.paused) a.pause();
}
