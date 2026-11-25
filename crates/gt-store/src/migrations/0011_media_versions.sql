-- Schema v11: versions of one page in several files (copies in other quality). `version_of` is the
-- id of the media record that is the group's primary, shown and opened first; NULL on the primary
-- and on every file with no other versions, as on every older row. No foreign key (SQLite cannot
-- drop such a column again); a primary that is gone reads as none when the tree opens.
-- A GEDCOM export writes every version as an ordinary OBJE record.
-- An older app refuses a v11 project, so the column is never silently dropped by a save.
ALTER TABLE media ADD COLUMN version_of TEXT;
