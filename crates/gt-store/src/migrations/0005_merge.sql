-- Schema v5: merging a relative's copy of the tree. `merged_group` is the ledger of groups taken
-- from a copy (so the same copy merged again has nothing new, and a group that reached a copy
-- through another relative is recognised); `merge_alias` is reserved for joining the same person
-- under two ids (written by a later version). An older app refuses a v5 project (its
-- `change_log.actor_kind` may now be 'copy').
CREATE TABLE merged_group(
  source_group TEXT PRIMARY KEY,   -- the group id in the copy's change log
  merge_group  TEXT,               -- the owner's group written for it (NULL: nothing needed writing)
  batch_id     TEXT NOT NULL,      -- the merge (agent_batch.id)
  source_name  TEXT NOT NULL,      -- whose copy ("Olena's copy")
  source_ts    INTEGER,            -- when the relative made the group (their clock; shown only)
  merged_ts    INTEGER NOT NULL
);
CREATE INDEX merged_group_batch ON merged_group(batch_id);
CREATE TABLE merge_alias(their_id TEXT PRIMARY KEY, our_id TEXT NOT NULL);
