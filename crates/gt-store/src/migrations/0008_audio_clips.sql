-- Schema v8: a link to a stretch of a sound recording (a clip), the time counterpart of a picture
-- region. All four NULL on a link to the whole media, as on every older row. clip_id stays the same
-- when the clip's times or label change. Times are milliseconds from the start of the recording,
-- clip_start_ms < clip_end_ms. Not exported to GEDCOM.
-- An older app refuses a v8 project, so the columns are never silently dropped by a save.
ALTER TABLE media_link ADD COLUMN clip_id TEXT;
ALTER TABLE media_link ADD COLUMN clip_start_ms INTEGER;
ALTER TABLE media_link ADD COLUMN clip_end_ms INTEGER;
ALTER TABLE media_link ADD COLUMN clip_label TEXT;
