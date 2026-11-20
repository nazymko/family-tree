/** The letters of pre-1918 Russian and Ukrainian orthography that a keyboard lacks, with the i18n key of each one's name. */
export const OLD_LETTERS: readonly { ch: string; name: 'yat' | 'i' | 'fita' | 'izhitsa' | 'yer' }[] = [
  { ch: 'Ѣ', name: 'yat' },
  { ch: 'ѣ', name: 'yat' },
  { ch: 'І', name: 'i' },
  { ch: 'і', name: 'i' },
  { ch: 'Ѳ', name: 'fita' },
  { ch: 'ѳ', name: 'fita' },
  { ch: 'Ѵ', name: 'izhitsa' },
  { ch: 'ѵ', name: 'izhitsa' },
  { ch: 'Ъ', name: 'yer' },
  { ch: 'ъ', name: 'yer' },
];

/** The text with `insert` put over the selection `start..end` (the caret when they are equal), and where the caret goes. */
export function insertAtCaret(value: string, start: number, end: number, insert: string): { value: string; caret: number } {
  const a = Math.max(0, Math.min(start, value.length));
  const b = Math.max(a, Math.min(end, value.length));
  return { value: value.slice(0, a) + insert + value.slice(b), caret: a + insert.length };
}
