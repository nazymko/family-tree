-- Schema v10: what kind of material a source is, and the document details: the issuing office, the
-- document number and the date on it. All NULL on every older row (an older tree opens unchanged).
-- kind holds a stable id (`metric_book`, `other:<label>`); doc_date_json a serialised GenDate.
-- An older app refuses a v10 project, so the columns are never silently dropped by a save.
ALTER TABLE source ADD COLUMN kind TEXT;
ALTER TABLE source ADD COLUMN issuer TEXT;
ALTER TABLE source ADD COLUMN doc_number TEXT;
ALTER TABLE source ADD COLUMN doc_date_json TEXT;
