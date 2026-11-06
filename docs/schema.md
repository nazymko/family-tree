# Project database schema (version 5)

A gene-tree project is a folder:

```
MyFamily.genetree/
  tree.db     SQLite database (WAL mode)
  media/      original files, named by content hash, never modified
  thumbs/     regenerable thumbnails
  backups/    timestamped copies of tree.db
```

`tree.db` is plain SQLite and can be read with any SQLite tool. The schema version is stored in `PRAGMA user_version` (currently `5`), and migrations are applied in order when a project is opened. A project written by a newer version of the app is refused.

## Conventions

- **Ids** are UUID v7 strings in `id` columns. **Timestamps** are Unix milliseconds UTC (integers).
- **Booleans** are integers `0` or `1`.
- Every row table has these four columns, described once here:
  - `id`: the record's UUID (text primary key).
  - `private`: `1` if the record is marked private and must be hidden from privacy-aware exports.
  - `changed`: when the record was last changed (Unix ms), or null if unknown.
  - `unknown_json`: GEDCOM data the app does not understand, kept as a JSON array of verbatim nodes (tag, payload, children, in original order) so it can be written back unchanged. Null when there is none.
- **Dates** appear as a group of four columns with a prefix, written `<p>_raw`, `<p>_json`, `<p>_sort_min`, `<p>_sort_max`:
  - `<p>_raw`: the date text as the user typed or imported it.
  - `<p>_json`: the parsed date (calendar, qualifier, precision, parse status) as JSON.
  - `<p>_sort_min`, `<p>_sort_max`: the earliest and latest day number the date can mean, used for sorting and range search (a ranges or dual-dated value needs both).
- **Enumerations** (kind, role, relationship, and so on) are stored as lowercase snake_case text such as `birth` or `civil_union`. A free-form value that is not in the app's list is stored as `other:<text>`.
- `target_type` / `target_id` pairs are polymorphic references with no database foreign key. `target_type` is one of `person`, `name`, `family`, `child_link`, `event`, `place`, `source`, `repository`, `citation`, `media`, `note`.
- `pos` is the position of the row inside its parent, to keep the original order.
- `seq` is the position of a top-level record inside the whole tree. Every top-level record table (`person`, `family`, `event`, `place`, `source`, `repository`, `citation`, `media`, `note`) has `seq INTEGER NOT NULL`; saving writes the in-memory order as `seq` and loading orders by it, so order survives `VACUUM` (row ids do not). `seq` is indexed (schema 2): an edit gives a new record `MAX(seq) + 1` and an edited record keeps its `seq`.
- Foreign keys are enforced (`PRAGMA foreign_keys = ON`) and every foreign key column has an index.

## Project metadata

### `meta`
Key/value settings of the project.
- `key`: setting name. The keys `app_version` (the app version that created the project) and `created_ms` (creation time) always exist. Optional keys: `living_max_age` (years after birth until a person counts as deceased, default 100), `home_person` (UUID of the person the app opens on) and `last_import_id` (id of the import that created the project, cleared by the first edit). The key `fts_scheme` (`2`) says the full-text rows use the `seq`-based row ids; it is written in the same transaction that rebuilds them, and a project without it is rebuilt on open.
- `value`: setting value as text.

### `tree_header`
At most one row (`id` is always 1): facts about the file the tree was imported from.
- `id`: always `1`.
- `producer`: name of the program that produced the imported file.
- `source_version`: that program's version.
- `unknown_json`: unrecognised header data, as described above.

## People and families

### `person`
One row per person. Names, events and links are separate rows.
- `sex`: `male`, `female`, `intersex`, `unknown`, or `other:<text>`.
- `living_override`: `living` or `deceased` when the user forces the status; null to compute it.
- `living_cached`: the computed living flag (`1` living, `0` not), a cache that can be rebuilt. Null when not yet computed.
- `sibling_marker`: reserved (schema 3, default `0`); not written yet.

### `name`
One row per name assertion of a person (a person can have many names).
- `person_id`: the person it belongs to.
- `pos`: order among the person's names.
- `kind`: `birth`, `married`, `aka`, `religious`, `nickname`, `immigrant`, `professional`, or `other:<text>`.
- `preferred`: `1` for the name shown by default.
- `proof`: `proven`, `disproven`, or null.
- `confidence`: a confidence number (small integer, higher is more certain), or null.
- `prefix`: title before the name, such as "Dr.".
- `given`: given names.
- `call`: the given name the person is called by.
- `surname_prefix`: particle before the surname ("van", "de").
- `surnames_json`: JSON array of surnames.
- `patronymic`: patronymic or matronymic part.
- `suffix`: suffix such as "Jr.".
- `nickname`: nickname part.
- `lang`: language code of the name.
- `date_raw`, `date_json`, `date_sort_min`, `date_sort_max`: the date the name was in use.
- `raw`: the full name as originally written, kept for faithful export.

### `family`
A union of up to two partners (either may be unknown) and the anchor for children.
- `partner1_id`, `partner2_id`: the partners (people), or null.
- `kind`: `married`, `civil_union`, `unmarried`, `unknown`, or `other:<text>`.

### `child_link`
Links a child to a family and says how the child is related to each partner.
- `family_id`: the family the child belongs to.
- `person_id`: the child.
- `pos`: order among the family's children.
- `rel1`: relationship to partner 1 (`birth`, `adopted`, `step`, `foster`, `guardian`, `sealed`, `unknown`, `other:<text>`).
- `rel2`: relationship to partner 2, same values.
- `preferred`: `1` if this is the child's preferred family.
- `assertion_preferred`: `1` if this link is the preferred assertion among competing ones (distinct from `preferred`).
- `proof`: `proven`, `disproven`, or null.
- `confidence`: confidence number, or null.

## Events

### `event`
A fact with a date and place. Family events (marriage and so on) are events whose participant is the family.
- `kind`: event type such as `birth`, `death`, `marriage`, `residence`, or `custom:<text>`.
- `date_raw`, `date_json`, `date_sort_min`, `date_sort_max`: when it happened (`date_sort_min` is indexed).
- `place_id`: where it happened.
- `description`: free-text detail (for example the occupation).
- `age`: age text as recorded.
- `cause`: cause text (for example cause of death).
- `negative`: `1` when the event is recorded as "did not happen".
- `preferred`: `1` for the preferred assertion of its kind.
- `proof`: `proven`, `disproven`, or null.
- `confidence`: confidence number, or null.

### `participant`
Who took part in an event.
- `event_id`: the event.
- `pos`: order among the event's participants.
- `target_type`: `person` or `family`.
- `target_id`: the person or family id.
- `role`: `principal`, `witness`, `godparent`, `clergy`, `officiator`, `informant`, `parent`, `father`, `mother`, `child`, `spouse`, `husband`, `wife`, `friend`, `neighbor`, `multiple`, or `other:<text>`.

## Places

### `place`
A place in a hierarchy.
- `parent_id`: the enclosing place (for example the county of a town).
- `kind`: place type text (city, parish, ...).
- `lat`, `lon`: coordinates in decimal degrees.
- `raw`: the place text as originally written.

### `place_name`
The names a place has had, in several languages and periods.
- `place_id`: the place.
- `pos`: order among the place's names.
- `name`: the name text.
- `lang`: language code.
- `date_raw`, `date_json`, `date_sort_min`, `date_sort_max`: the period the name was used.

## Sources and citations

### `source`
A book, record set or other source.
- `title`, `author`, `publication`, `abbreviation`: bibliographic data.
- `text`: transcribed or descriptive text.

### `repository`
An archive or library.
- `name`: its name.
- `address`: postal address text.
- `urls_json`: JSON array of web addresses.

### `repo_ref`
Where a source is kept.
- `source_id`: the source.
- `pos`: order among the source's repositories.
- `repository_id`: the repository.
- `call_number`: the source's call number there.

### `citation`
A reference to a place inside a source, attached to records through `citation_link`.
- `source_id`: the source cited.
- `page`: page or locator text.
- `date_raw`, `date_json`, `date_sort_min`, `date_sort_max`: the date of the cited entry.
- `text`: quoted or transcribed text.
- `quality`: 0 to 3 quality rating, or null.
- `ev_original`: whether the cited item is an original record (`1`, `0`, or null if unknown).
- `ev_primary`: whether the information is primary (`1`, `0`, or null).
- `ev_direct`: whether the evidence is direct (`1`, `0`, or null).

## Media and notes

### `media`
A file in `media/` (or elsewhere) with its description.
- `path`: path of the file, relative to the project folder.
- `original_name`: the file's name before import.
- `mime`: media type such as `image/jpeg`.
- `title`: title text.
- `date_raw`, `date_json`, `date_sort_min`, `date_sort_max`: date of the item.
- `description`: free-text description.
- `sha256`: hex SHA-256 of the file, used to detect duplicates.

### `note`
A note or story, written in Markdown. Links to people and places use `genetree://person/<uuid>` addresses.
- `kind`: `general`, `story`, `research`, `transcription`, `conclusion`, or `other:<text>`.
- `text`: the Markdown text.
- `lang`: language code.
- `shared`: `1` if the note is shared between several records rather than owned by one.

## Polymorphic links and external ids

### `citation_link`
Attaches a citation to any record or assertion.
- `target_type`, `target_id`: the record carrying the citation.
- `pos`: order among that record's citations.
- `citation_id`: the citation.

### `note_link`
Attaches a note to any record or assertion.
- `target_type`, `target_id`: the record carrying the note.
- `pos`: order among that record's notes.
- `note_id`: the note.

### `media_link`
Attaches a media item to any record, optionally to a region of the picture (for example one face).
- `target_type`, `target_id`: the record carrying the media.
- `pos`: order among that record's media.
- `media_id`: the media item.
- `primary`: `1` if this is the record's main picture.
- `region_x`, `region_y`, `region_w`, `region_h`: the region as fractions of the picture from 0 to 1 (left, top, width, height); all null for the whole picture.

### `ext_id`
Identifiers of a record in other systems.
- `target_type`, `target_id`: the record.
- `pos`: order among that record's identifiers.
- `kind`: `xref`, `uid`, `familysearch`, `ancestry_apid`, `exid`, or `other`.
- `scheme`: for `exid` the identifier type or scheme; for `other` the kind name; otherwise null.
- `value`: the identifier.

## Derived data and history

### `name_phonetic`
Phonetic search keys computed from names (can be rebuilt).
- `id`: row id.
- `name_id`: the name.
- `algo`: the algorithm, such as `soundex` or `daitch_mokotoff`.
- `code`: the key.

### `change_log`
History of edits, used for undo and the per-record history. Rows get increasing integer ids; one user action writes one row per changed record, all sharing a `group_id`.
- `id`: increasing sequence number.
- `ts`: when the change was made (Unix ms).
- `target_type`, `target_id`: the record that changed (`target_type` is the record's table name, such as `person`).
- `op`: the operation: `create`, `update` or `delete` (rows written by schema 1 may say `insert`).
- `before_json`: the record as it was before the change, as JSON; null for a create.
- `group_id`: the UUID shared by every row of one edit; null for rows written before schema 2.
- `label`: the user-facing name of the edit, such as `Edit name`; null for rows written before schema 2.
- `after_json`: the record as it is after the change, as JSON; null for a delete.
- `before_seq`: for a delete, the record's `seq`; undoing the delete restores it when still free, so record order survives delete and undo.
- `reverts_group`: for an undo, the `group_id` of the edit it reverts; otherwise null.
- `actor_kind`: who made the edit, `user`, `agent` or `copy` (a relative's edit taken in by merging their copy of the tree, schema 5); null for rows written before schema 3.
- `actor_id`: for an agent, the UUID of its connection; otherwise null.
- `actor_name`: for an agent, its name at that time (kept so a revoked connection stays named); for `copy`, whose copy it was ("Olena"); otherwise null.
- `batch_id`: the `agent_batch` the edit belongs to, if any.

### `agent_batch`
A named run of agent edits, such as one imported file (schema 3). A merge of a relative's copy is a batch too (schema 5): `actor_name` is whose copy, `source_file` the copy's folder or file name, `report` the counts.
- `id`: UUID.
- `title`: what the run is.
- `source_file`: the file the run works from, if any.
- `actor_id`, `actor_name`: the agent that started it.
- `started_ts`, `ended_ts`: Unix ms; `ended_ts` is null while the batch is open.
- `report`: the agent's closing summary; null until the batch is closed.

### `change_person`
The people a change group is about, one row per group and person (schema 3). The change log shows them for every edit; only agent edits count as unreviewed.
- `group_id`, `person_id`: together the key.
- `reviewed_ts`: when the user reviewed the change for this person (Unix ms); null until then. An edit that was later undone no longer counts as unreviewed.

### `merged_group`
The ledger of a merge (schema 5): one row for each change group taken from a relative's copy, written in the same transaction as the merge itself. A group in the ledger counts as known, like a group already in this tree's own log, so merging the same copy again finds nothing new and the next round takes only what came after.
- `source_group`: the group id in the copy's change log (primary key).
- `merge_group`: the group this tree's own log wrote for it; null when nothing needed writing.
- `batch_id`: the `agent_batch` of the merge.
- `source_name`: whose copy it was.
- `source_ts`: when the relative made the group, on their clock (Unix ms). Shown to the owner, never used to decide anything.
- `merged_ts`: when it was merged (Unix ms).

### `merge_alias`
Reserved (schema 5): will map a person's id in a relative's copy to the same person under another id here, when the owner joins the two. Nothing writes it yet.
- `their_id`: the id in the copy (primary key).
- `our_id`: the id in this tree.

## Search

Both tables are rebuilt on every save and can be regenerated; they hold no data of their own. Edits rewrite only the rows of the records they touch. A row's `rowid` is `seq * 4 + k` (`k` is 0 for a person, 1 for a place, 2 for a source and 3 for a note), so one record's row can be replaced without scanning.

### `search_words`
FTS5 table (`unicode61 remove_diacritics 2`): word and prefix search, accent-insensitive.
- `kind`: `person`, `place`, `source` or `note` (not indexed).
- `id`: the record's UUID (not indexed).
- `label`: the display name or title.
- `body`: all other searchable text (every name variant and the places of a person's events, a source's author and text, and so on).

### `search_trigram`
FTS5 table (`trigram`): substring search, needed for Chinese, Japanese and Korean text that has no spaces.
- `kind`, `id`: as above.
- `text`: the label, a newline, then the body.

