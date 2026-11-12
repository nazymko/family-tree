-- Schema v9: where a media record was made (a sound recording was recorded, a picture taken): a
-- place from the same places table the events use. NULL when nobody has said, as on every older
-- row. No foreign key (SQLite cannot drop such a column again); a place that is gone reads as none.
-- Not exported to GEDCOM.
-- An older app refuses a v9 project, so the column is never silently dropped by a save.
ALTER TABLE media ADD COLUMN place_id TEXT;
