-- Schema v2: granular edits. The change log gains the edit group, a label and the record
-- after the change (so a history entry can be redone as well as undone).
ALTER TABLE change_log ADD COLUMN group_id TEXT;
ALTER TABLE change_log ADD COLUMN label TEXT;
ALTER TABLE change_log ADD COLUMN after_json TEXT;
CREATE INDEX change_log_target ON change_log(target_id, ts);
CREATE INDEX change_log_group ON change_log(group_id);
-- A new record takes MAX(seq) + 1; without these indexes that is a table scan.
CREATE INDEX idx_person_seq ON person(seq);
CREATE INDEX idx_family_seq ON family(seq);
CREATE INDEX idx_event_seq ON event(seq);
CREATE INDEX idx_place_seq ON place(seq);
CREATE INDEX idx_source_seq ON source(seq);
CREATE INDEX idx_repository_seq ON repository(seq);
CREATE INDEX idx_citation_seq ON citation(seq);
CREATE INDEX idx_media_seq ON media(seq);
CREATE INDEX idx_note_seq ON note(seq);
