-- Schema v1. Documented in docs/schema.md; keep both in sync.
-- Every top-level record table (person, family, event, place, source, repository, citation, media, note) also has seq INTEGER NOT NULL: its position in the tree, so load order is stable.
-- Every row table: id TEXT (uuid v7), private 0/1, changed (unix ms), unknown_json (verbatim unknown GEDCOM subtrees).
-- A "date" is four columns: <p>_raw, <p>_json, <p>_sort_min, <p>_sort_max.

CREATE TABLE meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE tree_header (
    id             INTEGER PRIMARY KEY CHECK (id = 1),
    producer       TEXT,
    source_version TEXT,
    unknown_json   TEXT
);

-- Core ------------------------------------------------------------------

CREATE TABLE person (
    id              TEXT PRIMARY KEY,
    seq          INTEGER NOT NULL,
    private         INTEGER NOT NULL DEFAULT 0,
    changed         INTEGER,
    unknown_json    TEXT,
    sex             TEXT NOT NULL DEFAULT 'unknown',
    living_override TEXT,
    living_cached   INTEGER
);

CREATE TABLE name (
    id             TEXT PRIMARY KEY,
    private        INTEGER NOT NULL DEFAULT 0,
    changed        INTEGER,
    unknown_json   TEXT,
    person_id      TEXT NOT NULL REFERENCES person(id) ON DELETE CASCADE,
    pos            INTEGER NOT NULL DEFAULT 0,
    kind           TEXT NOT NULL DEFAULT 'birth',
    preferred      INTEGER NOT NULL DEFAULT 0,
    proof          TEXT,
    confidence     INTEGER,
    prefix         TEXT,
    given          TEXT,
    call           TEXT,
    surname_prefix TEXT,
    surnames_json  TEXT NOT NULL DEFAULT '[]',
    patronymic     TEXT,
    suffix         TEXT,
    nickname       TEXT,
    lang           TEXT,
    date_raw       TEXT,
    date_json      TEXT,
    date_sort_min  INTEGER,
    date_sort_max  INTEGER,
    raw            TEXT
);

CREATE TABLE family (
    id           TEXT PRIMARY KEY,
    seq          INTEGER NOT NULL,
    private      INTEGER NOT NULL DEFAULT 0,
    changed      INTEGER,
    unknown_json TEXT,
    partner1_id  TEXT REFERENCES person(id) ON DELETE SET NULL,
    partner2_id  TEXT REFERENCES person(id) ON DELETE SET NULL,
    kind         TEXT NOT NULL DEFAULT 'unknown'
);

CREATE TABLE child_link (
    id           TEXT PRIMARY KEY,
    private      INTEGER NOT NULL DEFAULT 0,
    changed      INTEGER,
    unknown_json TEXT,
    family_id    TEXT NOT NULL REFERENCES family(id) ON DELETE CASCADE,
    person_id    TEXT NOT NULL REFERENCES person(id) ON DELETE CASCADE,
    pos          INTEGER NOT NULL DEFAULT 0,
    rel1         TEXT NOT NULL DEFAULT 'birth',
    rel2         TEXT NOT NULL DEFAULT 'birth',
    preferred    INTEGER NOT NULL DEFAULT 0,
    assertion_preferred INTEGER NOT NULL DEFAULT 0,
    proof        TEXT,
    confidence   INTEGER
);

-- Places ----------------------------------------------------------------

CREATE TABLE place (
    id           TEXT PRIMARY KEY,
    seq          INTEGER NOT NULL,
    private      INTEGER NOT NULL DEFAULT 0,
    changed      INTEGER,
    unknown_json TEXT,
    parent_id    TEXT REFERENCES place(id) ON DELETE SET NULL,
    kind         TEXT,
    lat          REAL,
    lon          REAL,
    raw          TEXT
);

CREATE TABLE place_name (
    id            TEXT PRIMARY KEY,
    private       INTEGER NOT NULL DEFAULT 0,
    changed       INTEGER,
    unknown_json  TEXT,
    place_id      TEXT NOT NULL REFERENCES place(id) ON DELETE CASCADE,
    pos           INTEGER NOT NULL DEFAULT 0,
    name          TEXT NOT NULL,
    lang          TEXT,
    date_raw      TEXT,
    date_json     TEXT,
    date_sort_min INTEGER,
    date_sort_max INTEGER
);

-- Events ----------------------------------------------------------------

CREATE TABLE event (
    id            TEXT PRIMARY KEY,
    seq          INTEGER NOT NULL,
    private       INTEGER NOT NULL DEFAULT 0,
    changed       INTEGER,
    unknown_json  TEXT,
    kind          TEXT NOT NULL,
    date_raw      TEXT,
    date_json     TEXT,
    date_sort_min INTEGER,
    date_sort_max INTEGER,
    place_id      TEXT REFERENCES place(id) ON DELETE SET NULL,
    description   TEXT,
    age           TEXT,
    cause         TEXT,
    negative      INTEGER NOT NULL DEFAULT 0,
    preferred     INTEGER NOT NULL DEFAULT 0,
    proof         TEXT,
    confidence    INTEGER
);

-- target_id points at a person or a family (no foreign key: polymorphic).
CREATE TABLE participant (
    id           TEXT PRIMARY KEY,
    private       INTEGER NOT NULL DEFAULT 0,
    changed      INTEGER,
    unknown_json TEXT,
    event_id     TEXT NOT NULL REFERENCES event(id) ON DELETE CASCADE,
    pos          INTEGER NOT NULL DEFAULT 0,
    target_type  TEXT NOT NULL CHECK (target_type IN ('person', 'family')),
    target_id    TEXT NOT NULL,
    role         TEXT NOT NULL DEFAULT 'principal'
);

-- Sources ---------------------------------------------------------------

CREATE TABLE source (
    id           TEXT PRIMARY KEY,
    seq          INTEGER NOT NULL,
    private      INTEGER NOT NULL DEFAULT 0,
    changed      INTEGER,
    unknown_json TEXT,
    title        TEXT,
    author       TEXT,
    publication  TEXT,
    abbreviation TEXT,
    text         TEXT
);

CREATE TABLE repository (
    id           TEXT PRIMARY KEY,
    seq          INTEGER NOT NULL,
    private      INTEGER NOT NULL DEFAULT 0,
    changed      INTEGER,
    unknown_json TEXT,
    name         TEXT NOT NULL,
    address      TEXT,
    urls_json    TEXT NOT NULL DEFAULT '[]'
);

CREATE TABLE repo_ref (
    id            TEXT PRIMARY KEY,
    private       INTEGER NOT NULL DEFAULT 0,
    changed       INTEGER,
    unknown_json  TEXT,
    source_id     TEXT NOT NULL REFERENCES source(id) ON DELETE CASCADE,
    pos           INTEGER NOT NULL DEFAULT 0,
    repository_id TEXT NOT NULL REFERENCES repository(id) ON DELETE CASCADE,
    call_number   TEXT
);

CREATE TABLE citation (
    id            TEXT PRIMARY KEY,
    seq          INTEGER NOT NULL,
    private       INTEGER NOT NULL DEFAULT 0,
    changed       INTEGER,
    unknown_json  TEXT,
    source_id     TEXT REFERENCES source(id) ON DELETE SET NULL,
    page          TEXT,
    date_raw      TEXT,
    date_json     TEXT,
    date_sort_min INTEGER,
    date_sort_max INTEGER,
    text          TEXT,
    quality       INTEGER,
    ev_original   INTEGER,
    ev_primary    INTEGER,
    ev_direct     INTEGER
);

-- Media and notes -------------------------------------------------------

CREATE TABLE media (
    id            TEXT PRIMARY KEY,
    seq          INTEGER NOT NULL,
    private       INTEGER NOT NULL DEFAULT 0,
    changed       INTEGER,
    unknown_json  TEXT,
    path          TEXT NOT NULL,
    original_name TEXT,
    mime          TEXT,
    title         TEXT,
    date_raw      TEXT,
    date_json     TEXT,
    date_sort_min INTEGER,
    date_sort_max INTEGER,
    description   TEXT,
    sha256        TEXT
);

CREATE TABLE note (
    id           TEXT PRIMARY KEY,
    seq          INTEGER NOT NULL,
    private      INTEGER NOT NULL DEFAULT 0,
    changed      INTEGER,
    unknown_json TEXT,
    kind         TEXT NOT NULL DEFAULT 'general',
    text         TEXT NOT NULL,
    lang         TEXT,
    shared       INTEGER NOT NULL DEFAULT 0
);

-- Polymorphic links (target_id has no foreign key) -----------------------
-- target_type: person|name|family|child_link|event|place|source|repository|citation|media|note

CREATE TABLE citation_link (
    id           TEXT PRIMARY KEY,
    private      INTEGER NOT NULL DEFAULT 0,
    changed      INTEGER,
    unknown_json TEXT,
    target_type  TEXT NOT NULL,
    target_id    TEXT NOT NULL,
    pos          INTEGER NOT NULL DEFAULT 0,
    citation_id  TEXT NOT NULL REFERENCES citation(id) ON DELETE CASCADE
);

CREATE TABLE note_link (
    id           TEXT PRIMARY KEY,
    private      INTEGER NOT NULL DEFAULT 0,
    changed      INTEGER,
    unknown_json TEXT,
    target_type  TEXT NOT NULL,
    target_id    TEXT NOT NULL,
    pos          INTEGER NOT NULL DEFAULT 0,
    note_id      TEXT NOT NULL REFERENCES note(id) ON DELETE CASCADE
);

CREATE TABLE media_link (
    id           TEXT PRIMARY KEY,
    private      INTEGER NOT NULL DEFAULT 0,
    changed      INTEGER,
    unknown_json TEXT,
    target_type  TEXT NOT NULL,
    target_id    TEXT NOT NULL,
    pos          INTEGER NOT NULL DEFAULT 0,
    media_id     TEXT NOT NULL REFERENCES media(id) ON DELETE CASCADE,
    "primary"    INTEGER NOT NULL DEFAULT 0,
    region_x     REAL,
    region_y     REAL,
    region_w     REAL,
    region_h     REAL
);

CREATE TABLE ext_id (
    id           TEXT PRIMARY KEY,
    private      INTEGER NOT NULL DEFAULT 0,
    changed      INTEGER,
    unknown_json TEXT,
    target_type  TEXT NOT NULL,
    target_id    TEXT NOT NULL,
    pos          INTEGER NOT NULL DEFAULT 0,
    kind         TEXT NOT NULL,
    scheme       TEXT,
    value        TEXT NOT NULL
);

-- Derived and history ---------------------------------------------------

CREATE TABLE name_phonetic (
    id      TEXT PRIMARY KEY,
    name_id TEXT NOT NULL REFERENCES name(id) ON DELETE CASCADE,
    algo    TEXT NOT NULL,
    code    TEXT NOT NULL
);

CREATE TABLE change_log (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    ts          INTEGER NOT NULL,
    target_type TEXT NOT NULL,
    target_id   TEXT NOT NULL,
    op          TEXT NOT NULL,
    before_json TEXT
);

-- Indexes: every foreign key, every polymorphic target, event dates ----------

CREATE INDEX idx_name_person ON name(person_id);
CREATE INDEX idx_family_partner1 ON family(partner1_id);
CREATE INDEX idx_family_partner2 ON family(partner2_id);
CREATE INDEX idx_child_link_family ON child_link(family_id);
CREATE INDEX idx_child_link_person ON child_link(person_id);
CREATE INDEX idx_place_parent ON place(parent_id);
CREATE INDEX idx_place_name_place ON place_name(place_id);
CREATE INDEX idx_event_place ON event(place_id);
CREATE INDEX idx_event_date_sort_min ON event(date_sort_min);
CREATE INDEX idx_participant_event ON participant(event_id);
CREATE INDEX idx_participant_target ON participant(target_type, target_id);
CREATE INDEX idx_repo_ref_source ON repo_ref(source_id);
CREATE INDEX idx_repo_ref_repository ON repo_ref(repository_id);
CREATE INDEX idx_citation_source ON citation(source_id);
CREATE INDEX idx_citation_link_target ON citation_link(target_type, target_id);
CREATE INDEX idx_citation_link_citation ON citation_link(citation_id);
CREATE INDEX idx_note_link_target ON note_link(target_type, target_id);
CREATE INDEX idx_note_link_note ON note_link(note_id);
CREATE INDEX idx_media_link_target ON media_link(target_type, target_id);
CREATE INDEX idx_media_link_media ON media_link(media_id);
CREATE INDEX idx_ext_id_target ON ext_id(target_type, target_id);
CREATE INDEX idx_ext_id_value ON ext_id(kind, value);
CREATE INDEX idx_name_phonetic_name ON name_phonetic(name_id);
CREATE INDEX idx_name_phonetic_code ON name_phonetic(algo, code);
CREATE INDEX idx_change_log_target ON change_log(target_type, target_id);

-- Full-text search (rebuilt by save_tree) --------------------------------------
CREATE VIRTUAL TABLE search_words USING fts5(kind UNINDEXED, id UNINDEXED, label, body, tokenize='unicode61 remove_diacritics 2');
CREATE VIRTUAL TABLE search_trigram USING fts5(kind UNINDEXED, id UNINDEXED, text, tokenize='trigram');
