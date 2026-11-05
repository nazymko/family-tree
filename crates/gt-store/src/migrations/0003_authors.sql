-- Schema v3: who made each change. Every change-log row records its author and batch; the
-- people a change group is about are listed (with a review mark) in change_person.
ALTER TABLE change_log ADD COLUMN actor_kind TEXT;   -- 'user' | 'agent'; NULL = before tracking
ALTER TABLE change_log ADD COLUMN actor_id TEXT;
ALTER TABLE change_log ADD COLUMN actor_name TEXT;   -- kept so a revoked connection stays named
ALTER TABLE change_log ADD COLUMN batch_id TEXT;
-- (the index on group_id already exists since schema 2)
CREATE INDEX change_log_batch ON change_log(batch_id);
CREATE TABLE agent_batch(id TEXT PRIMARY KEY, title TEXT NOT NULL, source_file TEXT, actor_id TEXT NOT NULL,
  actor_name TEXT NOT NULL, started_ts INTEGER NOT NULL, ended_ts INTEGER, report TEXT);
CREATE TABLE change_person(group_id TEXT NOT NULL, person_id TEXT NOT NULL, reviewed_ts INTEGER,
  PRIMARY KEY(group_id, person_id));
CREATE INDEX change_person_person ON change_person(person_id);
ALTER TABLE person ADD COLUMN sibling_marker INTEGER NOT NULL DEFAULT 0;  -- written from Task 3
