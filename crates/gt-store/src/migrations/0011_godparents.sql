-- Schema v11: who stood godparent at a baptism or christening. A godparent is a person in the
-- tree (person_id) or only a name as the record writes it (name); exactly one of the two is set.
-- role is `godfather`, `godmother` or `godparent` (not said which). The rows belong to their event.
CREATE TABLE godparent (
    id         TEXT PRIMARY KEY,
    event_id   TEXT NOT NULL REFERENCES event(id) ON DELETE CASCADE,
    pos        INTEGER NOT NULL DEFAULT 0,
    person_id  TEXT,
    name       TEXT,
    role       TEXT NOT NULL,
    note       TEXT
);
CREATE INDEX idx_godparent_event ON godparent(event_id);
CREATE INDEX idx_godparent_person ON godparent(person_id);

-- Godparents an import kept as event participants become godparents. The role follows the
-- written role, else the person's sex.
INSERT INTO godparent(id, event_id, pos, person_id, name, role, note)
SELECT p.id, p.event_id, p.pos, p.target_id, NULL,
       CASE
           WHEN lower(p.role) = 'other:godfather' THEN 'godfather'
           WHEN lower(p.role) = 'other:godmother' THEN 'godmother'
           WHEN per.sex = 'male' THEN 'godfather'
           WHEN per.sex = 'female' THEN 'godmother'
           ELSE 'godparent'
       END,
       NULL
FROM participant p
JOIN person per ON per.id = p.target_id
WHERE p.target_type = 'person'
  AND lower(p.role) IN ('godparent', 'other:godfather', 'other:godmother');

DELETE FROM participant
WHERE target_type = 'person'
  AND lower(role) IN ('godparent', 'other:godfather', 'other:godmother')
  AND target_id IN (SELECT id FROM person);
