import { describe, expect, it } from 'vitest';
import { cleanForStory, storyOf } from './storyText';

// Invented people and shelf marks only.
describe('cleanForStory', () => {
  it('removes_archive_shelf_marks_in_every_form', () => {
    expect(cleanForStory('Іван Тестенко народився 1890 (ДАХеО 1-2-3).')).toBe('Іван Тестенко народився 1890.');
    expect(cleanForStory('Запис у книзі ДАХмО 10-2-345 і ЦДІАК 11-22-33.')).toBe('Запис у книзі і.');
    expect(cleanForStory('Дивись ф. 259, оп. 51, спр. 14, арк. 5зв, там він.')).toBe('Дивись, там він.');
  });

  it('removes_pages_sheets_record_numbers_and_file_names', () => {
    expect(cleanForStory('Хрещений Петро Тестенко (PDF с. 29, арк. 5, ж. № 38).')).toBe('Хрещений Петро Тестенко.');
    expect(cleanForStory('Акт № 47: народилася Марія Тестенко.')).toBe('народилася Марія Тестенко.');
    expect(cleanForStory('Скан IMG_0042.jpg і page-3.PDF показують запис.')).toBe('Скан і показують запис.');
    expect(cleanForStory('Див. с.29 та стор. 30-31.')).toBe('Див. та.');
  });

  it('removes_hypothesis_tags_and_familysearch_ids_and_the_brackets_they_leave', () => {
    expect(cleanForStory('Може бути той самий Іван (H7), див. ГІПОТЕЗИ H24 і P29.')).toBe('Може бути той самий Іван, див. і.');
    expect(cleanForStory('Знайдено (DGS 4810397, i782) у списку.')).toBe('Знайдено у списку.');
    expect(cleanForStory('Source: https://www.familysearch.org/ark:/61903/1:1:ABCD-123 done')).toBe('Source: done');
  });

  it('keeps_dates_names_and_doubts', () => {
    const text = 'Іван Тестенко, народився 12-05-1890 у Лисках, можливо 1891? Родина Hunt (бл. 1850).';
    expect(cleanForStory(text)).toBe(text);
  });

  it('tidies_spaces_and_punctuation_and_keeps_line_breaks', () => {
    expect(cleanForStory('Перший  рядок ( ДАХеО 1-2-3 ) , кінець.\n\n\n\nДругий рядок H7 ;')).toBe('Перший рядок, кінець.\n\nДругий рядок;');
    expect(cleanForStory('Джерело:\nНародився 1890.')).toBe('Народився 1890.');
  });

  it('is_a_no_op_on_text_without_references', () => {
    expect(cleanForStory('Він жив у селі.')).toBe('Він жив у селі.');
    expect(cleanForStory('')).toBe('');
  });
});

describe('page prefix', () => {
  it('leaves_no_dangling_PDF_prefix', () => {
    expect(cleanForStory('Запис (PDF-с. 437).')).toBe('Запис.');
    expect(cleanForStory('Запис PDF – с. 437 тут')).toBe('Запис тут');
    expect(cleanForStory('Запис PDF, с. 437 тут')).toBe('Запис тут');
    expect(cleanForStory('Запис (127-1012-1719, PDF с. 618)')).not.toMatch(/PDF/);
  });
});

describe('storyOf', () => {
  it('prefers_the_written_story', () => {
    expect(storyOf({ text: 'Born 1890 (ДАХеО 1-2-3).', story: ' Born in 1890. ' })).toEqual({ text: 'Born in 1890.', cleaned: false });
  });

  it('keeps_a_research_note_with_its_references_when_there_is_no_story', () => {
    const text = 'Архів 1, ф. 1, оп. 2, спр. 3, PDF с. 10, ч. 5';
    expect(storyOf({ text, story: null })).toEqual({ text, cleaned: false });
    expect(storyOf({ text: 'ЦДІАК 127-1012-1603, PDF-с. 437, ч. 15' }).text).toBe('ЦДІАК 127-1012-1603, PDF-с. 437, ч. 15');
  });
});
