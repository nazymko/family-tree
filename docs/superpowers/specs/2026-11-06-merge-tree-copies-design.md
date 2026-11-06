# Bringing relatives' edits back: merging copies of a tree

Status: proposal for the owner's review, 2026-11-06. Design only; no code has changed. "Confirmed" means read in the code (file named).
"Assumption" means not verifiable from the code. Names below are synthetic (Ivan Petrovych Koval, Olena Koval).

## Why

The owner keeps a family tree and sends a copy to relatives over the internet. The relatives edit their copies on their own
computers. The owner wants those edits back in the main tree, without retyping and without losing anything either side did.
Not live sync: a copy goes out, edits happen, a copy comes back, the owner merges it.

Assumptions about the situation (the owner should confirm; see open questions):

- A1. Several relatives, and several rounds over time with the same relative.
- A2. Relatives use this same app (Gene Tree) to edit; they have no other way to open a copy.
- A3. The copy that came back is a folder, or a zip of one, that the owner can reach on their disk.
- A4. The owner is the authority: relatives' edits are proposals the owner accepts, not equal partners (the feature works the same
  in every copy, so a relative can also pull the owner's changes; nothing here depends on that).

## The owner's flow (decided 2026-11-06)

The owner asked for "a button to archive the tree in zip and send, and then send back". So the first version includes it:

- **Send a copy** (Надіслати копію родоводу): one button makes a single `.zip` of a clean copy of the tree (`save_copy`, so no `backups/`
  and no open WAL) with its media, and shows the file in its folder. The person attaches it to a message by their own means: the app
  has no server and does not deliver anything. A plain warning says what is inside and shows the file's size, since mail and
  messengers limit size.
- **Open a received copy** (Відкрити отриману копію): on the relative's side, unpacks the zip into a folder they choose and opens it.
- **The relative sends it back** with the same Send a copy button.
- **Merge a copy of the tree…** (Об'єднати з копією родоводу…) accepts that zip as well as a folder. The zip is untrusted: limits on
  entries and sizes, no path outside the target, and it is unpacked to a scratch folder first.

This moves the single-file copy from phase 3 to phase 1. Sending stays unfiltered, with the warning: copies carry living people,
private records and history (owner's default, question 3).

## Goals

1. Choose a returned copy, see exactly what it would change, and apply it as one visible, undoable batch.
2. Additions and one-sided edits come across without questions. Anything where both sides touched the same thing is decided by the
   owner, side by side.
3. Works for the copies the owner has already sent, with nothing recorded at the time.
4. Merging the same copy twice adds nothing; merging several relatives' copies in turn works in any order.
5. Media files come across with their records.

## Non-goals

- Live sync, a server, accounts, or a sending service. The owner sends by their own means (mail, messenger, cloud drive).
- Merging two different trees that were never one (two separate GEDCOM imports). Refused, not guessed.
- Automatic merging of two records that are the same real person under different ids (a review step offers it; see phase 4).
- Line-by-line merging of note text. A note changed on both sides is a conflict.
- Merging app settings (`meta` keys such as `home_person`, `living_max_age`). The owner's stay.

## Anti-goals (a failure even if it "works")

- Any family data silently lost, overwritten, or resurrected after the owner deleted it.
- The owner's own unsent edits overwritten because the other side "looked newer" (clock time is never used to decide).
- A deletion applied without the owner seeing it. A guess presented as a fact: where the app cannot tell who changed something, it asks.
- A merge that changes anything between "Preview" and "Apply" other than what the preview showed.
- A merge that cannot be undone, or leaves a half-applied tree after a crash.
- A change that makes a living or private person more visible without the owner choosing it.
- A copy from the internet (untrusted file) that writes outside the project, crashes the app, or plants a broken tree.
- A second merge of the same copy duplicating people.

## Constraints

- Local app, no network code. Rust + Tauri 2 + Svelte 5. All writes go through the existing edit path (`Project::apply_changes`).
- The repo is public: tests use synthetic data only.
- Users are not technical: plain words, English and Ukrainian (a family tree is "родовід" in Ukrainian).
- Whatever the merge reads, it reads from a scratch copy; the folder the relative sent is never opened in place or modified.

## What exists today (confirmed in code)

| Fact | Where |
|---|---|
| A project is a folder: `tree.db` (SQLite, WAL), `media/<aa>/<sha256>.<ext>` (content-addressed), `thumbs/`, `backups/`, a lock file | `gt-store/src/project.rs`, `media.rs`, `docs/schema.md` |
| "Save a copy to a folder" exists: `VACUUM INTO` of the whole `tree.db` plus all media, into an empty folder. It is a folder, not a zip. The UI says it is unfiltered and "includes living people, private records and history; keep it to yourself". The copy has an empty `backups/`. | `gt-session/src/transfer.rs` (`save_copy`), `app/src/lib/i18n/en.ts` (`copyHint1/2`) |
| A relative opens a copy as an ordinary tree with "open project" (`open_project`). Opening migrates in place when the schema is older, after a pre-migration backup; a newer schema is refused (`NewerSchema`). | `project.rs` |
| Backups exist, including a `BackupReason::BeforeMerge` that nothing uses yet | `gt-store/src/backup.rs` |
| Ids are UUID v7 strings for every top-level record, and also for a person's names (`Name.id`). Sub-items without ids: child links (matched by `person`), event participants, place names, repo refs. | `docs/schema.md`, `gt-core/src/model.rs`, `ids.rs` |
| Nothing renumbers ids on load, save or copy. `seq` is only a local display order: an edit keeps it, a new record gets `MAX(seq)+1`; only a whole-tree import (`save_tree`) rewrites it. So `seq` must never be compared or merged. | `gt-store/src/edit.rs` (`write_record`), `save.rs` |
| Place ids from a GEDCOM import are derived from the place text, so two unrelated imports share place ids. Place ids cannot prove two trees are related; person ids and log group ids can. | `gt-gedcom/src/idgen.rs` |
| One writer for every edit, user or agent, undo or redo: `apply_changes`. It writes the record, keeps `seq`, refreshes search and living rows, and appends one `change_log` row per record with `before_json`, `after_json`, `group_id`, `label`, author columns and `batch_id`. | `edit.rs` |
| The change log is append-only: no code deletes or trims it. Undo adds an inverse group (`reverts_group`), it does not remove rows. A delete is logged with the record's full `before_json`. `AUTOINCREMENT` ids and all rows survive `VACUUM INTO`, so a copy carries the sender's complete log. | `0001_init.sql`, `0002_edit.sql`, `edit.rs`, grep of `DELETE FROM change_log` finds nothing |
| Gaps in the log: whole-tree import and CLI import (`save_tree`) write no log; rows from schema 1 have no `group_id`; schema 1-2 rows have no author (`actor_kind` null). Import leaves the log empty. | `transfer.rs`, `schema.md`, `authors.rs` (`has_untracked_groups`) |
| Authors: `Actor::User` (anonymous) or `Actor::Agent{id,name}`; named batches (`agent_batch`); a per-person review mark (`change_person.reviewed_ts`) that counts only agent groups as unreviewed. | `authors.rs`, `0003_authors.sql` |
| Undo of agent work from the log, fact by fact, also after a restart: `undo_group`, `undo_batch`. Both refuse groups not written by an agent. The Ctrl+Z stack holds the owner's edits only, 200 groups, lost on close. | `gt-session/src/undo.rs` |
| A three-way, field-level JSON merge already exists for undo (`revert.rs`): records compared as JSON without `common.changed`; arrays matched by `id` / `person`; a date and a surname list are one fact; it reports "conflict" and keeps the current value. It is exactly a merge of base, theirs and ours with roles swapped. | `gt-session/src/revert.rs` |
| Readable "what changed" rows per record, in words (names instead of ids) | `gt-session/src/field_diff.rs` |
| Search: full-text rows and phonetic keys (soundex, Daitch-Mokotoff) are maintained by `apply_changes`; `Project::search` also uses the phonetic table. | `edit.rs`, `search.rs` |
| Privacy: exports pass a filter (living, private) with a preview; "Save a copy" and backups do not. No duplicate detection exists yet (only in `docs/requirements.md`, which also says "no automatic merge, ever" for duplicates). | `gt-core/src/privacy.rs`, `requirements.md` §5 |
| GEDCOM/GEDZIP export keeps record ids (`_GTID`) but carries no change log. | `gt-gedcom`, `requirements.md` §4.2 |

## What is missing

1. Nothing marks a copy as a copy: no tree id, no record of what was sent when, no record of what was merged.
2. No merge engine, preview, conflict screen, or place to remember "this group of the relative's was already merged".
3. No author for "this edit came from a relative's copy" (`Actor` has only user and agent; `undo_batch` refuses non-agents).
4. No single-file copy (zip) for sending; the only way is a folder the owner zips by hand (taking `backups/` and a possibly open
   WAL with it, see Privacy).
5. No "same person under two ids" review.

## The base problem and the decision

A three-way merge needs the state of each record at the moment the copy was sent. Without it, "they changed it" and "I changed it"
look the same and the only honest answer is to ask about every difference.

### The key observation

A returned copy carries the owner's whole log up to the moment it was made (confirmed: `save_copy` is `VACUUM INTO` of everything),
followed by the relative's own groups. Each group has a UUID `group_id`. So, without any marker:

- Groups present in both logs are shared history. Groups only in the relative's log (and not in the ledger below) are **their new
  groups**: exactly their edits since the fork.
- For a record they touched, the base is the `before_json` of the first of their new groups on that record (or "absent" if that
  group is a create). Their result is the record as it is in their copy now. "Ours" is the record in the open tree now.
- The owner's own log is not needed for the base. The three-way compares base to theirs, and base to ours, field by field. Whether
  the owner changed a field is read from the owner's current record against the base, not from the owner's log, so an edit made
  and then undone (net no change) is correctly "not changed".
- No clock is involved. The relative's PC time is shown in words ("on 3 March, on Olena's computer") but never decides anything.

### Alternatives

| Option | Works for copies already sent | Needs history inside the copy | Weakness |
|---|---|---|---|
| A. Reconstruct from the log (above) | Yes, if the copy came from this app's "Save a copy" or a zipped project folder | Yes | Depends on log completeness; verified per record (below) |
| B. Snapshot at send: the owner's app keeps a copy of the tree (or a per-record hash list) under a `copy_id` written into the copy | No (none exist) | No | Disk per send; useless for already sent copies; a relative-to-relative chain has no snapshot |
| C. Marker only (`tree_id`, `copy_id`, log position at send) | No | Yes | A marker helps identify and label, but without a log or snapshot it cannot supply a base |

### Recommendation

Use A as the engine, with checks that make a wrong base impossible to apply silently. Add the markers of C going forward (phase 3)
for identification, naming and the lineage check, not as the base. Keep B only for the optional filtered copy (phase 5), the one
case where history cannot ship.

### Checks that make A safe (per record, before anything is proposed)

For each record R that the relative's new groups mention:

1. **Chain intact**: their rows for R, in log order, must link: each row's `before_json` equals the previous row's `after_json`
   (ignoring `common.changed`), and the last `after_json` equals R in their database now. If not, the log does not explain their
   copy (hand edit, bug, tampering, older app that did not log): R falls back to the no-base rule.
2. **Base plausible**: the base must decode with the current model (older rows lack newer fields, `sibling_marker` has a serde
   default). A row that does not decode sends R to the no-base rule.
3. **Unexplained difference**: if R differs between the two trees and neither side has a log row for it since the fork, the
   difference is unexplained and R goes to the no-base rule.

**No-base rule** (the safe fallback; never guesses): a record present on one side only is an addition **only if** it is theirs-only;
ours-only records are never treated as deleted. A record on both sides that differs is shown as a conflict, side by side, with no
automatic choice. Without a verified base, absence is never deletion.

### Copies already sent, with no marker

- Sent from this app (Save a copy, or a zip of the folder): they carry the log, so A applies in full. This is the expected case.
- Sent from an older app (schema 1: no `group_id`): no history; the no-base rule for every differing record. Safe, noisy.
- Sent as GEDCOM/GEDZIP and edited elsewhere or re-imported: ids may match (`_GTID`) but there is no log; no-base rule. If almost no
  person ids match, the merge is refused ("this does not look like a copy of your tree").
- If the owner restored an old backup after sending, or rebuilt the tree from a new import, the relative's log is no longer a
  prefix-compatible history of the open tree. Detected when shared group ids are few or none against many shared record ids:
  the app says so and uses the no-base rule. It never guesses a base.
- Unrelated trees: refused when there are no shared person ids and no shared group ids. Place ids do not count (see above).

### Merging the same copy twice, or several copies

A ledger in the owner's database, `merged_group(source_group, merge_group, source_name, merged_ts)`, is written in the same
transaction as the merge. A relative group already in the ledger is known, exactly like a group already in the owner's log. So:

- Same copy again: no new groups, nothing to do ("Nothing new in this copy").
- Same relative, next round: only groups after the last merge are new; the base for a record is the state after the previous groups,
  so what the owner declined last time is not asked again (the declined value stays: ours differs from base only where the owner
  said so, and their new edit is compared with that).
- Two relatives each add or edit: the second merge sees the first's result as "ours". A fact both changed to the same value is no
  conflict; to different values it is a conflict asked once.
- A group of Olena's that arrives via Maria's copy (Maria merged hers) is recognised by group id and skipped, not applied twice.
- If the owner undid a merged group, the ledger still counts it as seen: it is not re-proposed. (The preview offers "Show what was
  merged earlier and undone" only as a later extra.)

## Merge rules per record type

A record is JSON. Compare base (B), theirs (T), ours (O), ignoring `common.changed`. Reuse the `revert.rs` engine with roles swapped
(apply the change B to T onto O), extended to return a list of conflicting field paths instead of a single flag.

General rules:

- Only T changed a field (O equals B): take T. Only O changed: keep O. Both changed to the same value: that value.
  Both changed differently: conflict.
- Record exists in T only, with a create group: **addition**. Taken, together with the records it needs (see references).
- Deleted in T: never applied automatically in phase 1 (listed, unchecked). If O equals B it is a one-click "also remove here";
  if O changed it, it is a conflict "removed there, edited here", default keep.
- Deleted in O, changed in T: conflict, default keep **deleted** (the owner removed it on purpose); shown with what they changed.
- Anything that makes a person more visible (`private` 1 to 0, `living_override` set to `deceased` or cleared on a living person)
  is never taken automatically even when only T changed it: shown as a decision.
- Arrays (names, citations, notes, media links, children, participants, place names, repo refs) merge by item identity as in
  `revert.rs`: an item added on either side stays; an item removed on one side and untouched on the other is removed (listed);
  an item edited on one side and removed on the other is a conflict; reordering on both sides: ours wins.

| Record | Notes |
|---|---|
| Person | `sex`, `living_override`, `private`: scalars. `names` by `Name.id`, so Olena adding a name and the owner fixing another name of the same person both stay. The same name's `given` changed both ways is a conflict. A new preferred name on both sides leaves two preferred: conflict on `preferred`. `living_cached` is derived and recomputed, never merged. |
| Family | `partner1`, `partner2`, `kind` scalars. Both sides setting a different partner on the same family is a conflict. `children` matched by `person`; a child linked to two families by the two sides is kept in both (it is two real links) and flagged "now in two families"; two `preferred` links for one child is a conflict. |
| Event | Fields as scalars; date is one fact; `participants` by whole value (so two different witnesses added both stay). `place` pointing to a deleted place: see references. |
| Place | `parent` is a scalar (both sides changing it differently: conflict). After merging, the place parent chain is checked for a cycle. `names` by whole value, so added names union. Coordinates are one fact (lat and lon together). |
| Source, Repository, Citation | Fields as scalars. A citation edited on both sides is field level. `repo_refs` by repository id. |
| Note | `text` is one fact: a conflict offers **Keep mine / Take theirs / Keep both** (theirs appended under a line saying where it came from). `kind`, `lang`, `shared` as scalars. |
| Media | The record merges as a scalar set. Files: see below. Region links on a person are array items by `media`; the `primary` flag follows the one-preferred rule. |
| Voice samples | Media records (confirmed to exist in `gt-session/src/media.rs`); handled as media. They can be large; sizes shown in the preview. |

### References and integrity (checked on the merged result, before apply)

After computing the result in memory, validate the whole tree: every id referenced from a taken or kept record exists
(families to partners and children, events to participants and place, links to citations, notes and media, places to parents).

- A taken addition that needs a record that is not coming (for example their new person is a child in a family that the owner
  deleted) pulls the needed record in, shown as part of the same line, or is blocked and listed.
- A person deleted on one side and linked by the other side's new family or event: conflict, default keep the person.
- `revert.rs` already has `settle_kept` and `dangling_writes` for the same problem (what must stay so nothing dangles); reuse them.
- **Cycles**: a person who becomes their own ancestor (via merged child links), or a place that becomes its own parent, blocks the
  affected records and shows them as a conflict; nothing is applied that creates one.
- Conflicting partners/parents (two sides gave a child different families, or a person two different "birth" parents): both kept
  as separate assertions where the model allows (child links have `assertion` and `preferred`), flagged for a decision otherwise.

### Same real person under two ids

The id match cannot catch it: the owner adds "Petro Koval" and Olena adds the same man from memory. Both are creates, both are
kept (nothing is lost). After the plan is computed, for each person that is new on their side, the app proposes up to three
candidates among people not in their copy (including the owner's new ones), scored by:

- name and phonetic keys (the existing `name_phonetic` soundex and Daitch-Mokotoff and `Project::search`), birth year within a few
  years, same parents' or spouse's names, same birth place.
- shown as "Might be the same person?" in the preview as a note on the addition, never automatic.

Phase 4 lets the owner say "Same person": the two records are joined by choosing each differing field, their id is mapped to ours
(`merge_alias(their_id, our_id)` in the ledger so later rounds map the same way, and every reference in the incoming records is
rewritten), and the result is one undoable batch. Until then the owner uses the future duplicate-merge of the requirements.

### Media

- A media record that comes across brings its file only if missing: look up `media/<aa>/<sha256>.<ext>` in the owner's project; if
  absent, copy from the copy after verifying the bytes hash to the name. A mismatch or a missing file skips the record (shown) and
  copies nothing.
- `media.path` from the file is untrusted: it must be a lexical `media/` path with no `..`, drive or absolute part
  (use `is_lexically_under`, as the importer does); symlinks in the copy are ignored (as in `copy_media_tree`).
- Files are copied before the database change (content-addressed, so a stray file after a failure is harmless); a file is never
  deleted by a merge, even when the record is removed.

## Where it fits the existing model

- **Backup first**: `Project::backup(BeforeMerge, ..)` (the reason already exists); the merge aborts if the backup fails.
- **One batch**: a batch row (reusing `agent_batch`: title "Merged copy: <name>", source = the copy's folder name, report =
  the counts) and a group per unit (a person with the records that belong to them, a place, a source, a note), each with a
  readable label such as "From Olena's copy: edited Ivan Petrovych Koval". All groups are applied in **one SQLite transaction**
  (a new `apply_changes_many`) with the ledger rows, so a crash leaves all or nothing. The in-memory session tree is reloaded as
  after an undo.
- **Author**: a new actor kind `copy` (`Actor::Copy{ name }`, stored in `actor_kind`/`actor_name`), shown in the change log as
  "Olena's copy", with the relative's own time and label in the entry text. A migration (schema 5) adds `merged_group` and
  `merge_alias` and bumps `user_version`, so an older app that cannot read `actor_kind = copy` refuses the project instead of
  reporting corruption. Their log rows are **not** copied into the owner's log (they carry old values, possibly deleted private
  data, and would double-count); the owner's log keeps one entry per merge group, and `merged_group` ties it to the source
  group ids and times, so a person's history view can say "edited in Olena's copy, 3 March".
- **Review marks**: `change_person` rows are written for every person touched, as agents' edits are. The "unreviewed" filter
  (now agent only) is widened to `agent` and `copy`, so the owner can walk through everything a relative changed and mark it read.
- **Undo**: not on the Ctrl+Z stack (as `requirements.md` §150 says of merges) but as "Undo this merge" from the change log,
  using `undo_batch` generalised to `copy` groups. It works fact by fact and after a restart, and keeps what the owner changed
  since. The backup is the last resort ("restore the backup taken before the merge").
- **Search and living**: unchanged; `apply_changes` refreshes full-text rows, phonetic keys and `living_cached`. Tests assert it.
- **Concurrency**: the plan is computed against a version stamp of the open tree (highest `change_log.id`). Apply re-checks it; if the
  owner edited in between, the preview is recomputed and shown again. No edits (typing, agents over MCP) are accepted during apply.
- **Schema versions**: the copy is `VACUUM INTO`'d to a scratch folder and opened there. Older schema: migrated in the scratch copy
  only. Newer than this app: refused with "This copy was saved by a newer Gene Tree. Update the app first." Schema 1 copy: no
  history, no-base rule. The original folder is never changed, and its lock file is never needed (read-only open of the source).
- **Untrusted input**: limits on number of records, JSON size and depth, and media sizes before reading (as GEDZIP does), no path
  outside the project, a corrupt `tree.db` is a plain "cannot read this copy".

## The flow, in plain words

English first, Ukrainian in brackets.

**Entry point.** In the tree menu: **Merge a copy of the tree…** (Об'єднати з копією родоводу…). The app asks for the copy's folder
(or, from phase 3, its file), and for a name to credit ("Whose copy is this?" / «Чия це копія?», default the folder name).

**Checks, shown as one line each**: it is a Gene Tree copy; it is a copy of this tree; the app version can read it; "This copy
came from your tree and has the history needed to tell your edits from theirs" or, if not, the plain warning of which fallback
applies ("This copy has no history; everything that differs will be shown for you to decide, nothing will be removed").
Refusals say why and what to do.

**Preview, "What would change"** (Що зміниться). Nothing is written. Counts first, then lists, each line a person or record in words
(never ids), with "Open" to look at the person in the tree:

1. *Added in the copy* (Додано в копії): new people, families, events, places, sources, notes, photos. All checked by default.
   "Might be the same person?" notes sit here.
2. *Changed only in the copy* (Змінено тільки в копії): "Olena changed the birth date of Ivan Petrovych Koval from … to …". Checked.
3. *Changed in both* (Змінено в обох): needs a decision. Not applied until chosen.
4. *Removed in the copy* (Видалено в копії): unchecked; each says if it is also edited here.
5. *Left as it is*: a short line "N things you changed are not touched."

A banner at the top: "Nothing has been changed yet. A backup of your tree will be made first."

**Conflict screen** (Розбіжності). One decision at a time, with the person's name as the title. Two columns, **Yours**
(Ваше) and **In Olena's copy** (У копії Олени), plus the original value in grey ("Before either of you changed it"), field by field
in words (reuse `field_diff.rs`). Buttons: **Keep mine** (Залишити моє), **Take theirs** (Взяти з копії), and for notes **Keep both**
(Залишити обидва). "Do this for all remaining" is offered only for identical kinds of decision, never for deletions. Skipping a
decision leaves the owner's value and keeps the item on the list for next time.

**Apply** then shows "Applying…" and the **Result** (Готово): "Added 12 people, changed 5 records, kept 3 of yours, left 2 undecided.
Photos copied: 8." with buttons **Show changes** (opens the change log filtered to this merge), **Mark all as read**, and
**Undo this merge** (Скасувати це об'єднання). The change log lists it under "Olena's copy".

## Privacy of sending a copy

Confirmed today: "Save a copy" is unfiltered, and its own hint says it includes living people, private records and history. The
history is not obvious to a layman: it contains every earlier value, including text the owner later deleted or marked private.
A zipped project folder also takes `backups/` (every old database) and may take an open WAL, which is why the copy command makes
a clean snapshot instead. Exports (GEDCOM, GEDZIP) are filtered, but carry no log.

Proposal, only what is needed:

- Phase 1: nothing changes in sending. The merge works with what the app already produces.
- Phase 3: a **Send a copy** page that is `save_copy` (and a zip of it) with an honest plain warning of what is inside (living people,
  private records, the history of edits including old and removed text, and no filtering), a text suggestion to tell the owner to
  use it instead of zipping the project folder, and the copy's own `tree_id`, `copy_id`, name of the recipient.
- Phase 5, only if the owner wants relatives not to see living or private people: a **filtered copy** that withholds them and drops
  the history, with the withheld ids and a snapshot kept by the owner as the base (alternative B for that case). A merged-back
  filtered copy never treats a withheld record as deleted. That is a separate feature with its own design; the log-based merge
  does not work with it, which is the price.

## Failure handling

| Situation | Behaviour |
|---|---|
| Not a Gene Tree folder / corrupt / locked / unreadable | Plain message, nothing changed |
| Newer schema | Refuse, ask to update |
| Unrelated tree (no shared person or group ids) | Refuse |
| Related, history missing or inconsistent | Warn; no-base rule; no deletions |
| Chain check fails for one record | That record falls back to the no-base rule; the rest proceed |
| Owner edits between preview and apply | Recompute and show again |
| Backup fails | Abort |
| Media file missing/hash mismatch | Skip that photo and its record, list it |
| Crash in the middle | One transaction: all or nothing; files left are harmless |
| Merge produces a cycle or dangling reference | Blocked records listed as conflicts, rest applied |
| Same copy again | "Nothing new" |

## Test plan (synthetic data only)

Build with the session API so logs are real: tree T0 with Ivan Petrovych Koval, Olena Koval, their two children, a place
hierarchy, a source with citation, a note, a photo. Make copies A and B with `save_copy`, then:

1. **Round trip of two edited copies.** A: respells a name, adds a child and a note, deletes a sibling. B: changes the same person's
   birth date, adds a place and photo, adds the same grandfather Petro Koval that the owner also adds on T0. Owner: changes the
   birth date differently, edits the note. Merge A, then B. Assert every rule above on the result; then merge A again (zero changes),
   merge B again (zero), and merge A and B in the other order (same tree up to `seq` and `changed`).
2. **Convergence**: after the owner merged A and B, A merges the owner's copy and ends equal to the owner.
3. **Undo**: "Undo this merge" returns every record to its pre-merge state (compared as records), and a later owner edit survives it.
4. **Idempotence and ledger**: next-round edits by A only appear; a value the owner declined is not asked again.
5. **No silent loss**: property test over random single-field edits on T0, A and B; for every field the result is one of base,
   theirs, ours, or listed as conflict; none vanish. Deletion of a record never occurs unless chosen.
6. **Safety fallbacks**: copy with log stripped, schema 1 copy, tampered log (`after_json` not equal to the record), unrelated tree,
   copy from an unrelated import that shares place ids, restored-backup case, a person deleted on one side and linked on the other,
   a cycle through merged parents, a place parent cycle, a media file with the wrong hash, `media.path` of `../x`, a huge or
   malformed `tree.db`.
7. **Privacy rule**: `private` 1 to 0 and `living_override` loosening are never taken without a decision.
8. **Search and living**: after merge and after undo, search finds the right names (spelling variants by phonetic key), and
   `living_cached` is right.
9. **Stale preview**: an edit between preview and apply forces a new preview.
10. **UI**: the end-to-end test drives the conflict screen in English and Ukrainian (the existing e2e harness).

## Build order

0. **Phase 1a: the zip.** Send a copy, Open a received copy, the warning and size (see "The owner's flow").
1. **Phase 1: read-only preview plus safe apply.** Scratch open of a copy (folder or zip); the base from logs with the three checks and the no-base
   rule; plan with additions and one-sided changes; preview screen; apply of additions and one-sided changes only, in one
   transaction, with backup, ledger, `copy` author, and "Undo this merge". Conflicts, deletions and possible duplicates are
   **listed and not applied** (the owner can open the person and decide by hand). Safe alone: it never overwrites and never
   deletes, and is already useful for the copies out there.
2. **Phase 2: decisions.** Conflict screen, deletions (unchecked), notes "keep both", integrity pulls, cycle blocking, review marks
   for copy edits.
3. **Phase 3: copy identity.** `tree_id`/`copy_id`/recipient markers written by Send a copy, lineage check and nicer labels ("Copy for Olena, sent 12 Oct").
4. **Phase 4: same person.** Candidate scoring, the join with field choices, `merge_alias`.
5. **Phase 5: only if asked.** The filtered copy with snapshot base.

## Open questions for the owner

1. **How exactly did you send the copies already out?** (Save a copy, a zipped project folder, GEDCOM.) Recommended default: assume
   Save a copy or a zipped folder; GEDCOM-origin copies are handled by the safe fallback (everything differing is shown, nothing
   removed). If they were GEDCOM, expect many "changed in both" lines.
2. **Do relatives all use Gene Tree** (A2), and do they send back a folder or a zip? Default: yes, folder or zip; a single-file
   copy comes in phase 3.
3. **Are you comfortable that a copy contains living people, private records and the edit history?** (It does today.) Default: yes
   for trusted family, with the clear warning in phase 3; phase 5 only on request, because it changes how merging works.
4. **When a relative deletes someone, what should happen?** Default: listed, unchecked, never automatic.
5. **How should a relative's edits be marked in your change log?** Default: by the name you give at merge ("Olena's copy"),
   and shown as unreviewed until you mark them read.
6. **When both of you changed the same note, what is the usual wish?** Default: keep yours, with "Keep both" one click away.
7. **Two people who are the same under different ids:** keep both and warn (phase 1-3), join later (phase 4). Default: this.
