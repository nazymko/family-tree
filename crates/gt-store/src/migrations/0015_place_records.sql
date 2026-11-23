-- Schema v15: records about people who are NOT in the tree, taken from church books, confession
-- lists, revisions and censuses, kept per place so the place can show every event that happened
-- there. A bulk table beside the tree (tens of thousands of rows): not part of the change log and
-- not in the undo stacks. `place_id` has no foreign key (a place that is gone leaves its records
-- unreachable, and undoing the place's removal brings them back). An older app refuses a v15
-- project, so the tables are never silently dropped by a save.
CREATE TABLE place_record (
    id        INTEGER PRIMARY KEY,
    place_id  TEXT NOT NULL,
    kind      TEXT NOT NULL,
    date      TEXT NOT NULL DEFAULT '',
    year      INTEGER,
    book      TEXT NOT NULL DEFAULT '',
    page      TEXT NOT NULL DEFAULT '',
    rec_id    TEXT NOT NULL DEFAULT '',
    note      TEXT NOT NULL DEFAULT '',
    unsure    INTEGER NOT NULL DEFAULT 0,
    search    TEXT NOT NULL DEFAULT '',
    batch_id  TEXT,
    added_ms  INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_place_record_place ON place_record(place_id, year);
CREATE UNIQUE INDEX idx_place_record_source ON place_record(book, rec_id) WHERE rec_id <> '';

CREATE TABLE place_record_person (
    id          INTEGER PRIMARY KEY,
    record_id   INTEGER NOT NULL REFERENCES place_record(id) ON DELETE CASCADE,
    ord         INTEGER NOT NULL,
    role        TEXT NOT NULL DEFAULT '',
    given       TEXT NOT NULL DEFAULT '',
    patronymic  TEXT NOT NULL DEFAULT '',
    surname     TEXT NOT NULL DEFAULT '',
    as_written  TEXT NOT NULL DEFAULT '',
    estate      TEXT NOT NULL DEFAULT '',
    age         TEXT NOT NULL DEFAULT '',
    note        TEXT NOT NULL DEFAULT '',
    person_id   TEXT
);
CREATE INDEX idx_place_record_person_record ON place_record_person(record_id);
CREATE INDEX idx_place_record_person_person ON place_record_person(person_id) WHERE person_id IS NOT NULL;
