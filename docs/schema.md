# Project database schema (version 12)

A gene-tree project is a folder:

```
MyFamily.genetree/
  tree.db     SQLite database (WAL mode)
  media/      original files, named by content hash, never modified
  thumbs/     regenerable thumbnails
  backups/    timestamped copies of tree.db
```

`tree.db` is plain SQLite and can be read with any SQLite tool. The schema version is stored in `PRAGMA user_version` (currently `8`), and migrations are applied in order when a project is opened. A project written by a newer version of the app is refused.

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
- `kind`: event type such as `birth`, `death`, `missing` (went missing without trace; it does not count as a death), `marriage`, `residence`, or `custom:<text>`.
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

### `godparent`
Who stood godparent at a baptism or christening (schema 12). The rows belong to their event.
- `event_id`: the event.
- `pos`: order among the event's godparents.
- `person_id`: the godparent, when they are a person in the tree; no foreign key (a deleted person's row becomes their name).
- `name`: the godparent as the record writes them, when they are not a person in the tree. Exactly one of `person_id` and `name` is set.
- `role`: `godfather`, `godmother`, or `godparent` (not said which).
- `note`: what else the record says of them.

The schema 12 migration moves `participant` rows with the role `godparent` (or `other:Godfather` / `other:Godmother`) here, the role following the person's sex. GEDCOM: a person is an `ASSO` pointer on the event with `RELA Godfather` (5.5.1) or `ROLE GODP` + `PHRASE Godfather` (7.0); a name only is `ASSO @VOID@` with the name as its `PHRASE` (7.0) or, since 5.5.1 has no void pointer, a `NOTE Godfather: <name>` on the event with the godparent's note on the following lines. The import reads all of these back.

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
- `kind`: what kind of material it is (schema 10): a stable id (`civil_registry_extract`, `civil_certificate`, `metric_book`, `census`, `household_book`, `military_record`, `archive_document`, `personal_document`, `photo`, `gravestone`, `oral_testimony`, `book_or_article`, `website_or_database`, `genealogy_tree`, `research_analysis`, `other`), or `other:<label>` for a free label; null when nobody has said, as on every older row. An unknown id is read as a label.
- `issuer`, `doc_number`, `doc_date_json` (schema 10): the office that issued the document, the number it gave and the date on it (a serialised date, as `date_json` elsewhere). Null on older rows. Which archive and which case the source is kept in stays in `repo_ref` (repository and call number).

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
- `transcription_json`: what is written on the picture (schema 6), or null when nothing is. A JSON array of pieces in reading order, each `{"id", "area", "rotation", "text"}`: `area` is `{"x", "y", "w", "h"}`, fractions from 0 to 1 of the picture as it is displayed (after its EXIF orientation), from the top left corner, or null for text with no place on the picture (a whole transcript); `rotation` is how far the writing is turned clockwise, 0, 90, 180 or 270. On a sound recording the same array is a time-coded transcript: each piece has no `area` and may have `"time": {"start_ms", "end_ms"}` (milliseconds from the start of the recording, start before end; a piece whose times are not in that order loses them when the tree opens) and `"speaker"` (who says it, as written); both are left out when absent, and a recording has no transcript unless one is entered. The text belongs to the picture wherever it is attached. The joined full text is the pieces' texts, trimmed, with a blank line between them. A GEDCOM export does not carry it (GEDCOM has no place for it), and it is not in the search tables.
- `rotation`: how the picture is shown (schema 7): degrees clockwise, 0, 90, 180 or 270, on top of its EXIF orientation; null when nobody has turned it, which shows as 0. The transcription's areas stay on the picture upright by EXIF, before this turn, so turning a picture never moves them. A value that is not a quarter turn is read as 0 when the tree opens. A GEDCOM export does not carry it.
- `place_id`: where the media was made, a recording recorded or a picture taken (schema 9): the id of a `place`, the same places the events use; null when nobody has said, as on every older row. It has no foreign key; a place that is gone reads as none when the tree opens. A GEDCOM export does not carry it.
- `version_of`: when this file is another version of the same page or document, a copy in other quality (schema 11): the id of the `media` row that is the group's primary, shown and opened first; null on the primary itself and on every file with no other versions, as on every older row. It never names a row that has a `version_of` of its own. It has no foreign key; a primary that is gone reads as none when the tree opens, and a chain reads as its first primary. Files, transcriptions and links stay on each version. A GEDCOM export writes every version as an ordinary OBJE record.
- `map_layer_json`: when the picture is an old map laid on the Map (schema 13), its layer as JSON: `series`, `year`, `source` (text, each optional), `corners` (four `{lat_e6, lon_e6}` points in WGS 84 microdegrees: top left, top right, bottom right, bottom left of the picture; absent while the map is not placed yet), `opacity` (0 to 100), `visible`, and `pos` (drawing order, lowest first). Null on every other file and on every older row; JSON that does not read is taken as no layer. Taking a map off the Map sets it back to null and keeps the file. A GEDCOM export writes the layer as a NOTE under the OBJE record.

### `note`
A note or story, written in Markdown. Links to people and places use `genetree://person/<uuid>` addresses.
- `kind`: `general`, `story`, `research`, `transcription`, `conclusion`, or `other:<text>`.
- `text`: the Markdown text.
- `lang`: language code.
- `shared`: `1` if the note is shared between several records rather than owned by one.
- `story`: a readable retelling of `text` (schema 14), shown in the app's story view; null when the note has none, as on every older row. It never replaces `text`.

## Place records

Records about people who are not in the tree, for a place's list of events. A bulk side table beside the tree: not in the change log, not in undo, never rewritten by a save of the tree, and not in a GEDCOM export.

### `place_record`
One record of a church book, confession list, revision or census (schema 15).
- `id`: integer key.
- `place_id`: the `place` the record happened in. No foreign key: a place that is gone leaves its records unreachable, and undoing the place's removal brings them back.
- `kind`: `birth`, `marriage`, `death`, `confession`, `revision`, `census` or `other`.
- `date`: the date as the book gives it, partial dates allowed; `year`: the sortable year read from it (null when none).
- `book`, `page`, `rec_id`: the source. `rec_id` with `book` is unique when `rec_id` is not empty, so adding the same index twice adds nothing.
- `note`, `unsure` (`1` when the reading or the identification is doubtful).
- `search`: the participants' names in lower case, for the name search.
- `batch_id`: the agent batch that added it, when an agent did; `added_ms`: when (Unix ms).

### `place_record_person`
A person named in a record.
- `record_id`: the `place_record`; `ord`: position in the record.
- `role`, `given`, `patronymic`, `surname`, `as_written`, `estate`, `age`, `note`: as the book gives them.
- `person_id`: the person of the tree this participant is, when a relative; null otherwise. No foreign key.

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
Attaches a media item to any record, optionally to a region of the picture (for example one face) or to a clip: a stretch of a sound recording (schema 8). A clip lives on its link because it belongs to exactly one record: the same stretch told on two records is two links, and one recording cut into many clips is many links, so undo, history, merges and the agent's own-item rules work on clips as on any other link.
- `target_type`, `target_id`: the record carrying the media.
- `pos`: order among that record's media.
- `media_id`: the media item.
- `primary`: `1` if this is the record's main picture.
- `region_x`, `region_y`, `region_w`, `region_h`: the region as fractions of the picture from 0 to 1 (left, top, width, height); all null for the whole picture.
- `clip_id`, `clip_start_ms`, `clip_end_ms`, `clip_label`: a clip of a sound recording (schema 8), all null for the whole media. `clip_id` is a UUID that stays the same when the clip's times or label change; the times are milliseconds from the start of the recording, start before end, and within its length when the length is known; `clip_label` is an optional short name. A link whose clip cannot be read (no id, a time that is not a whole number of milliseconds, an end not after the start) is left out when the tree opens, with a note on stderr, rather than shown as the whole recording. A GEDCOM export does not carry clips (GEDCOM has no place for them): it writes the recording once, on its source, and not on the records its clips are attached to.

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

