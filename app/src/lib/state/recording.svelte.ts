import type { Id } from '../api/types';

/** Which recording the full player shows (the Shell owns the dialog), and the clip to start at. */
class RecordingView {
  open = $state<{ media: Id; clip?: Id } | null>(null);

  show(media: Id, clip?: Id): void {
    this.open = { media, clip };
  }

  close(): void {
    this.open = null;
  }
}

export const recordingView = new RecordingView();
