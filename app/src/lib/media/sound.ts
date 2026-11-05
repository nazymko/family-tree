/** File extensions of the sound recordings the app attaches. */
export const AUDIO_EXTENSIONS = ['mp3', 'wav', 'ogg', 'oga', 'opus', 'm4a', 'aac', 'flac'];

const SOUND_WORDS = /audio|mp3|mpeg|wav|ogg|opus|m4a|aac|flac/i;

/**
 * Whether a media item is a sound recording. The stored type is usually `audio/...`, but a record that came in
 * from a file has free text ("mp3") or none, so the file name is read too. Being wrong costs a player that says
 * it cannot play the file; being wrong the other way would show a recording as a broken picture.
 */
export function isSound(m: { mime?: string | null; originalName?: string | null }): boolean {
  const mime = m.mime ?? '';
  if (mime.startsWith('image/') || mime.startsWith('video/') || mime === 'application/pdf') return false;
  if (SOUND_WORDS.test(mime)) return true;
  const ext = /\.([A-Za-z0-9]{1,8})$/.exec(m.originalName ?? '')?.[1]?.toLowerCase();
  return !!ext && AUDIO_EXTENSIONS.includes(ext);
}
