-- Schema v14: a note can carry a readable retelling (`story`) beside its original text. The original
-- text is never changed by it. NULL on every older row. An older app refuses a v14 project, so the
-- column is never silently dropped by a save.
ALTER TABLE note ADD COLUMN story TEXT;
