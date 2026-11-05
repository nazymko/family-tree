# Agent access over MCP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** An MCP server inside the app lets a local agent read the whole open tree and make additive, attributed, reviewable, undoable changes, and a log screen shows who changed what.

**Architecture:** `gt-store` records an author with every change (schema 3). `gt-session` gains per-fact undo, a log API and an enforcement gate that checks the result of every agent write. A new crate `gt-mcp` holds the protocol, tools, rules and a small local HTTP server, generic over a host trait. `app/src-tauri` runs it and owns connections and keys; Svelte adds Settings, the log screen, badges and the sibling marker.

**Tech Stack:** Rust 1.97, rusqlite, `tiny_http` (new, in `gt-mcp` only), `sha2`, `getrandom`, Tauri 2, Svelte 5, ts-rs bindings, vitest, wdio.

**Spec:** `docs/superpowers/specs/2026-11-05-mcp-agent-access-design.md`. Read it first; every task's requirements include it.

## Global Constraints

- Real family data never enters code, tests, commits, screenshots committed, or docs. Synthetic names only (Ivan Petrovych Koval, Olena Koval). Do not open files in the user's Downloads.
- `unsafe_code = "forbid"`, `clippy::unwrap_used = "deny"`.
- In Ukrainian strings a family tree is родовід, never дерево; бабуся and дідусь in kinship.
- Every new UI string goes in both `app/src/lib/i18n` languages.
- Agents can never change whether an existing person counts as alive or dead, directly or indirectly.
- Server binds `127.0.0.1` only. Secret keys are never logged and the plain key is never stored; only its SHA-256 goes to the OS keychain.
- Gates from `app/`: `npx --no-install svelte-check --tsconfig ./tsconfig.json --fail-on-warnings`, `npx --no-install vitest run`, `npx --no-install eslint .`. From root: `cargo fmt --all`, `cargo clippy -q --workspace --all-targets -- -D warnings`, `cargo test -q --workspace`. Kill `gene-tree.exe` first (`taskkill //IM gene-tree.exe //F`): it locks the exe.
- After changing commands or DTOs: `UPDATE_BINDINGS=1 cargo test -p gt-session --test export_bindings`. `core.autocrlf=true`, so discard EOL-only noise (`git diff --ignore-cr-at-eol --stat`).
- New `EditCommand` variants need: fixture counts (currently 27) in `crates/gt-session/tests/ops.rs` and `app/src/lib/api/mock/MockApi.test.ts`; fixtures in `app/src/lib/api/__fixtures__/commands.json` and `commands.ts`; MockApi LABELS and handler; `i18n/backend.ts` label; generator in `crates/gt-session/tests/refresh.rs`.
- Commit trailer: `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. Work on branch `lane/mcp-agent-access`.

## Review Focus

1. Agent writes while the owner has an editor open: the owner's Save must not blank a field the agent just filled (Task 8 test `stale_editor_blocks_save`).
2. The owner closes the tree or opens another mid-run: tools answer "no tree is open" or refuse the unknown batch, nothing lands in the wrong tree (Task 5 test `batch_of_other_tree_refused`).
3. Port already taken, or app restarted with access on: Settings shows the problem, the app still works; access resumes on restart (Task 6 test `port_taken_reports_error`).
4. Undoing the same batch twice, or a batch part of which was already undone or reviewed: nothing breaks and nothing is undone twice (Task 2 test `undo_twice_is_harmless`).
5. Cyrillic and apostrophes in names, an unparsable date string, or a huge request body: names round-trip exactly, a bad date comes back as a clear refusal, an oversized body gets 413 (Task 5 tests `cyrillic_name_roundtrip`, `bad_date_refused`, `oversized_body_413`).

---

### Task 1: Author, batch and people records in the store

**Files:**
- Create: `crates/gt-store/src/migrations/0003_authors.sql`
- Modify: `crates/gt-store/src/migrate.rs`, `crates/gt-store/src/project.rs` (`SCHEMA_VERSION = 3`), `crates/gt-store/src/edit.rs`, `crates/gt-store/src/lib.rs`, `crates/gt-store/tests/edit.rs`, `crates/gt-session/src/undo.rs` (pass `LogMeta`), `docs/schema.md`
- Test: `crates/gt-store/tests/authors.rs`

**Interfaces:**
- Produces, in `gt_store`:
  - `enum Actor { User, Agent { id: Uuid, name: String } }`
  - `struct LogMeta { actor: Actor, batch: Option<Uuid>, people: Vec<Uuid> }`
  - `Project::apply_changes(&mut self, cs: &ChangeSet, living: &[(Uuid, bool)], meta: &LogMeta)`
  - `Project::group_changes(&self, group: Uuid) -> Result<Vec<Change>, StoreError>` (log order; decodes `before_json`/`after_json`)
  - `struct LogGroup { group: Uuid, ts: i64, label: String, actor: Option<Actor>, batch: Option<Uuid>, reverts: Option<Uuid>, reverted: bool, people: Vec<LogPerson>, changes: Vec<Change> }`, `struct LogPerson { id: Uuid, reviewed: Option<i64> }`
  - `struct LogFilter { actor_id: Option<Uuid>, actor_kind: Option<String>, batch: Option<Uuid>, person: Option<Uuid>, unreviewed_only: bool }`
  - `Project::log_page(&self, f: &LogFilter, offset: usize, limit: usize) -> Result<(Vec<LogGroup>, u32), StoreError>` newest first; `Project::log_group(&self, group: Uuid) -> Result<Option<LogGroup>, StoreError>`
  - `Project::mark_reviewed(&mut self, scope: ReviewScope, ts: i64)` with `enum ReviewScope { Person(Uuid), Batch(Uuid), Group(Uuid) }`
  - `Project::unreviewed_people(&self) -> Result<HashSet<Uuid>, StoreError>`: agent rows, not reverted, `reviewed_ts IS NULL`
  - `struct Batch { id, title, source_file: Option<String>, actor_id, actor_name, started: i64, ended: Option<i64>, report: Option<String> }`; `batch_create`, `batch_close(id, report, ts)`, `batch_get(id)`, `batch_list()`
  - `Project::last_writers(&self, target: Uuid) -> Result<Vec<(Option<Uuid>, ChangeOp)>, StoreError>`: writer id (`None` for user or before tracking) per row, oldest first; `Project::rows_of(&self, target: Uuid) -> Result<Vec<Change>, StoreError>` for ownership of embedded items.

- [ ] **Step 1: Write the failing tests** in `crates/gt-store/tests/authors.rs`: `v2_upgrades_and_reads_before_tracking` (build a v2 project from MIGRATIONS[..2] with one log row, open, assert `actor == None` and nothing lost); `author_and_people_recorded`; `review_per_person_clears_only_that_person`; `review_batch_clears_everyone`; `reverted_group_has_no_badge`; `log_filters_by_author_batch_unreviewed`; `group_changes_roundtrip_records`.
- [ ] **Step 2: Run** `cargo test -p gt-store --test authors`. Expected: does not compile.
- [ ] **Step 3: Write `0003_authors.sql`:**

```sql
ALTER TABLE change_log ADD COLUMN actor_kind TEXT;   -- 'user' | 'agent'; NULL = before tracking
ALTER TABLE change_log ADD COLUMN actor_id TEXT;
ALTER TABLE change_log ADD COLUMN actor_name TEXT;   -- kept so a revoked connection stays named
ALTER TABLE change_log ADD COLUMN batch_id TEXT;
CREATE INDEX change_log_group ON change_log(group_id);
CREATE INDEX change_log_batch ON change_log(batch_id);
CREATE TABLE agent_batch(id TEXT PRIMARY KEY, title TEXT NOT NULL, source_file TEXT, actor_id TEXT NOT NULL,
  actor_name TEXT NOT NULL, started_ts INTEGER NOT NULL, ended_ts INTEGER, report TEXT);
CREATE TABLE change_person(group_id TEXT NOT NULL, person_id TEXT NOT NULL, reviewed_ts INTEGER,
  PRIMARY KEY(group_id, person_id));
CREATE INDEX change_person_person ON change_person(person_id);
ALTER TABLE person ADD COLUMN sibling_marker INTEGER NOT NULL DEFAULT 0;  -- written from Task 3
```

- [ ] **Step 4: Implement the API; `Session::commit` passes `LogMeta { actor: Actor::User, batch: None, people: vec![] }` for now.** A group's people go in `change_person` only for agent groups. Update existing callers in `tests/edit.rs`.
- [ ] **Step 5: Run** `cargo test -p gt-store && cargo test -p gt-session`. Expected: PASS. Document the new columns and tables in `docs/schema.md`.
- [ ] **Step 6: Commit** `feat(store): record who made each change`.

### Task 2: Per-fact undo, attribution and the log in the session

**Files:**
- Create: `crates/gt-session/src/revert.rs`, `crates/gt-session/src/log.rs`
- Modify: `crates/gt-session/src/undo.rs`, `crates/gt-session/src/lib.rs`, `crates/gt-session/src/command.rs`, `app/src-tauri/src/commands.rs` (no behaviour change yet)
- Test: `crates/gt-session/tests/revert.rs`, `crates/gt-session/tests/log.rs`

**Interfaces:**
- Consumes: Task 1.
- Produces:
  - `revert::revert_change(ch: &Change, current: Option<&Record>, kept: &HashSet<Uuid>) -> Reverted`, `enum Reverted { Write(Option<Record>), Unchanged, Conflict(Option<Record>) }` (the `Conflict` payload is the partial merge, keeping what could not be undone)
  - `Session::people_of(&self, cs: &ChangeSet) -> Vec<Uuid>`: persons changed; partners and children of changed families; principals of changed events, resolving family participants; excludes nobody else.
  - `Session::commit(&mut self, cs: &ChangeSet, actor: Actor, batch: Option<Uuid>) -> Result<(), ApiError>`
  - `Session::undo_group(&mut self, group: Uuid) -> Result<UndoReport, ApiError>`, `Session::undo_batch(&mut self, batch: Uuid) -> Result<UndoReport, ApiError>`
  - DTOs (ts-rs): `UndoReport { undone: u32, skipped: Vec<SkippedFact> }`, `SkippedFact { kind: String, person: Option<PersonRef>, detail: String }`, `LogRowDto { group, ts, actor: ActorDto, batch: Option<BatchRef>, label, changes: Vec<LogChangeDto>, people: Vec<LogPersonDto>, reverted: bool, canUndo: bool }`, `ActorDto = { kind: 'user' } | { kind: 'agent', id, name } | { kind: 'beforeTracking' }`, `LogChangeDto { kind: String, op: HistoryOp, detail: String, sources: Vec<String> }`, `BatchDto { id, title, sourceFile, actorName, started, ended, report, groups: u32, people: u32, unreviewed: u32 }`
  - `Session::log(&self, f: LogFilterDto, offset, limit) -> Result<Page<LogRowDto>, ApiError>`, `Session::batches(&self)`, `Session::mark_reviewed(&mut self, scope: ReviewScopeDto)`, `Session::unreviewed_people(&self) -> Result<Vec<Uuid>, ApiError>`, `Session::person_agent_changes(&self, id: Uuid) -> Result<Vec<LogRowDto>, ApiError>`

Merge rules for `revert_change`, on `serde_json` values of `Record` (`after` -> `before` is the change to undo, applied to `current`):
1. `current` equal to `after` ignoring `common.changed` at every level: return exactly `before` (keeps existing undo byte-for-byte).
2. Create (`before` None): `current` absent -> `Unchanged`. `current` differs from `after` -> keep it, `Conflict(current)`, and add its id to `kept` (the caller iterates a group newest-change first and does a first pass to fill `kept`).
3. Update: walk the objects. Field where `before == after`: leave `current`. Field where `current == after`: set `before`. Field where `current` differs from both: keep `current`, report conflict. Never touch `changed` here; the caller stamps it.
4. Arrays: if items are objects with an `id`, match by `id` and recurse; otherwise compare by whole value. Item added by the change (in `after`, not in `before`): remove from `current`, unless its JSON contains the string of a `kept` id. Item removed by the change: append back at its old index, clamped. Other items stay in `current`'s order. `ChildLink` has no `id`: match on `person`.
5. A scalar equal to a `kept` id stays, such as `partner1` on a family.
6. Delete (`after` None), `current` absent: restore `before`. `current` present: `Unchanged`.

Undo rules:
- `Session::undo` and `redo` build their change set with `revert_change` over the stack entry and the current tree. Any `Conflict` -> return `ApiError` "An agent changed this since. Undo it from the change log first." with stacks untouched. Redo reverts the inverse.
- `undo_group` refuses user and before-tracking groups, and skips a group already reverted (`undone: 0`). Writes one new group: label `Undo: <label>`, `Actor::User`, `reverts` set, `changed` stamped now. `undo_batch` runs groups newest first, carrying `kept` across them. Neither touches the Ctrl+Z stacks.

- [ ] **Step 1: Write failing tests.** `tests/revert.rs` (pure): `untouched_returns_before_exactly`; `undo_one_of_two_notes_keeps_other`; `later_owner_scalar_edit_survives_and_conflicts`; `created_person_with_later_note_is_kept_with_family_link`; `created_note_with_edited_text_kept_and_reference_kept`; proptest `apply_then_revert_is_identity`. `tests/log.rs` (session): `user_edit_logged_as_user`; `agent_group_lists_people_of_family_link`; `undo_group_restores_tree_exactly` (compare `Session::tree()` ignoring `changed`); `undo_batch_survives_reopen`; `undo_twice_is_harmless`; `undo_is_logged_as_user`; `ctrl_z_keeps_agent_note_added_later`; `ctrl_z_stops_when_agent_changed_same_fact`; `agent_changes_not_on_ctrl_z_stack`; `log_change_detail_is_readable` (a note preview, event kind and date, source titles). Use a `#[cfg(test)]`-free helper `Session::commit_as_agent_for_tests` gated behind the existing test feature pattern, or Task 4's API once it exists; until then call `commit` through a `pub(crate)` path from an inline `#[cfg(test)]` module.
- [ ] **Step 2: Run** `cargo test -p gt-session --test revert --test log`. Expected: fails to compile.
- [ ] **Step 3: Implement `revert.rs`, then `log.rs`, then rework `undo.rs`.** `apply` stays `apply(cmd)` and means `Actor::User`; every private path goes through `commit`, whose `actor` argument is required.
- [ ] **Step 4: Run** `cargo test -q --workspace`. Expected: PASS, including `tests/undo.rs` and the `refresh.rs` fuzz unchanged. Regenerate bindings.
- [ ] **Step 5: Commit** `feat(session): undo per fact, and a log of who changed what`.

### Task 3: The unnamed-siblings marker

**Files:**
- Modify: `crates/gt-core/src/model.rs` (`Person`), `crates/gt-core/src/living.rs`, `crates/gt-store` load, save and search refresh, `crates/gt-session/src/{command.rs,ops/quick_add.rs or ops/family.rs,index.rs,query.rs,kinship.rs,chart.rs,family_view.rs,person_detail.rs,transfer.rs,session.rs}`, `crates/gt-session/src/dto.rs`, `app/src/lib/ui/PersonCard.svelte`, `app/src/lib/ui/PersonRow.svelte`, chart renderer in `app/src/lib/chart/`, family view, `app/src/lib/i18n/*`, MockApi and fixtures per Global Constraints.
- Test: `crates/gt-session/tests/sibling_marker.rs`, `app/src/lib/views/SiblingMarker.test.ts`

**Interfaces:**
- Consumes: column from Task 1.
- Produces: `Person.sibling_marker: bool` with `#[serde(default)]` (old log JSON has none); `EditCommand::AddSiblingMarker { family: Uuid }`, label "Add unnamed brothers and sisters"; `PersonRef.siblingMarker: boolean` and the same on rows and chart cards.

Grep every `tree.persons` use in `gt-session` and decide. Excluded: `person_count`, person list, search (write no search rows), kinship, living map, export (no INDI; drop its `CHIL`), privacy preview counts. Included: family view children, chart cards (dashed outline, muted, no dates). Opening one has no person page; the card offers only Delete. Delete it with the ordinary `DeletePerson`.

Label strings exact: English `<Some more Sisters/Brothers>`, Ukrainian `<Ще сестри/брати>`, drawn by the frontend from `siblingMarker`, so the record has no name.

- [ ] **Step 1: Write failing tests:** `marker_not_counted`; `marker_not_searched`; `marker_not_exported_and_gedcom_valid`; `marker_not_in_kinship_or_living`; `one_marker_per_family` (second is a clear error); `marker_undo_redo`; `marker_under_family_with_unknown_parents`. Frontend: `shows_translated_label`, `chart_card_has_no_dates`.
- [ ] **Step 2: Run** `cargo test -p gt-session --test sibling_marker`. Expected: fails to compile.
- [ ] **Step 3: Implement, including all fixture updates.**
- [ ] **Step 4: Run** all gates. Expected: PASS.
- [ ] **Step 5: Commit** `feat: a marker for unnamed brothers and sisters`.

### Task 4: The gate on agent writes

**Files:**
- Create: `crates/gt-session/src/agent.rs`
- Modify: `crates/gt-session/src/command.rs` (`AddNumber`), `crates/gt-session/src/ops/`, `crates/gt-session/src/transfer.rs`, `crates/gt-session/src/undo.rs`, `crates/gt-session/src/error.rs`, fixtures per Global Constraints
- Test: `crates/gt-session/tests/agent_gate.rs`

**Interfaces:**
- Consumes: Tasks 1-3.
- Produces:
  - `struct AgentCtx { pub id: Uuid, pub name: String }`
  - `Session::apply_as_agent(&mut self, cmd: EditCommand, ctx: &AgentCtx, batch: Uuid) -> Result<AgentWrite, ApiError>`; `enum AgentWrite { Done(EditResult), AlreadyThere { record: Uuid } }`
  - `Session::begin_batch(&mut self, ctx, title, source_file) -> Result<Uuid, ApiError>`, `Session::end_batch(&mut self, ctx, batch, report)`
  - `Session::find_by_number(&self, source_file: &str, number: &str) -> Option<Uuid>`
  - `EditCommand::AddNumber { person: Uuid, source_file: String, number: String }`, label "Record number". Stored in `Common.ext_ids` with `ExtIdKind::Exid`-style scheme `agent:<source_file>` (read the actual variant in `model.rs`). Export omits schemes starting `agent:`.
  - `ErrorCode::AgentRefused`; messages tell the agent what to do instead.

Order inside `apply_as_agent`:
1. Batch exists in this tree, belongs to this connection, and is not closed; otherwise refuse.
2. Only these commands: `CreatePerson`, `QuickAdd`, `LinkExisting`, `SaveFamily` (new only), `LinkChild`, `SaveName`, `SaveEvent`, `SaveNote`, `SaveSource`, `AddCitation`, `UpdateCitation`, `AddSiblingMarker`, `AddNumber`, and `UpdatePerson` with `sex` only. Anything else refuses; `UpdatePerson` carrying `living_override` or `private` refuses.
3. Repeat check on the command: identical note text on the same target, same event kind, `date_raw` and place on the same owner, or same source and page on the same target, each written by this connection -> `AlreadyThere`.
4. Domain checks. A person already has a `birth`, `death`, `burial` or `cremation` event -> refuse a new one of that kind. A father or mother link by birth where the child already has a parent in that role -> refuse. `LinkChild` into a second family where the child already has a birth family -> refuse. `AddNumber` where (`source_file`, `number`) already points at another person -> refuse.
5. Plan with `plan_with`. Compare every change in JSON, ignoring `changed`. Allowed on a record not created by this connection, or changed by anyone else since: scalar `null` -> value, empty string -> value, and arrays only gaining items. Everything else refuses, naming the fact and saying "add a note". A `preferred: true` on an added name changes the old one, so `SaveName` forces `preferred: false` on existing people. A record or embedded item is "own" when `rows_of`/`last_writers` show this connection introduced it and nobody else wrote it since.
6. Living check on the result: apply to the tree with `apply_to_tree_tracked`, run `living_maps`, undo with the inverse. If `effective` differs from `self.living` for any person that existed before, refuse. Message: "This would change whether <name> counts as alive or dead. Only the owner can do that. Leave a note and list it in your report."
7. `commit(cs, Actor::Agent{..}, Some(batch))`, not on the Ctrl+Z stack, redo untouched.

- [ ] **Step 1: Write failing tests:** `overwrite_refused_tree_unchanged`; `fill_empty_allowed`; `delete_commands_refused`; `second_birth_refused`; `second_father_refused`; `step_father_allowed`; `death_on_living_refused`; `old_birth_date_that_flips_living_refused`; `link_that_flips_living_refused`; `deceased_cannot_flip_to_living`; `living_override_refused`; `new_person_with_dates_allowed`; `own_note_editable`; `own_note_not_editable_after_owner_edit`; `other_agent_cannot_edit`; `filled_field_belongs_to_owner`; `added_name_keeps_preferred`; `identical_repeat_adds_nothing`; `two_files_share_a_number`; `number_points_at_one_person`; `closed_batch_refused`; `numbers_not_exported`.
- [ ] **Step 2: Run** `cargo test -p gt-session --test agent_gate`. Expected: fails to compile.
- [ ] **Step 3: Implement `agent.rs`.**
- [ ] **Step 4: Run** all Rust gates and regenerate bindings. Expected: PASS.
- [ ] **Step 5: Commit** `feat(session): enforce what agents may change`.

### Task 5: The `gt-mcp` crate

**Files:**
- Create: `crates/gt-mcp/{Cargo.toml,src/lib.rs,src/protocol.rs,src/http.rs,src/tools_read.rs,src/tools_write.rs,src/rules.md}`
- Test: `crates/gt-mcp/tests/{protocol.rs,tools.rs,http.rs}`, with synthetic trees built through `Session`

**Interfaces:**
- Consumes: Task 4, `Session` read APIs (`list_persons`, `search`, `get_person`, `family_view`, `get_family`, `suggest_sources`, `log`, `history`, media thumbnails).
- Produces:
```rust
pub trait Host: Send + Sync + 'static {
    fn with_session<T>(&self, f: &mut dyn FnMut(&mut Session) -> Result<T, ApiError>) -> Result<T, ApiError>;
    fn authenticate(&self, key_sha256: &[u8; 32]) -> Option<AgentCtx>;
    fn changed(&self, touched: &[Uuid]);   // the app refreshes its views
}
pub struct ServerHandle;                   // Drop or stop() shuts the listener down
pub fn serve<H: Host>(host: Arc<H>, port: u16) -> Result<ServerHandle, std::io::Error>;
pub fn handle_message<H: Host>(host: &H, ctx: &AgentCtx, body: &Value) -> Option<Value>;  // transport-free
pub const RULES: &str;                     // include_str!("rules.md")
```
(`with_session` with a generic `T` is not object-safe, so `Host` is used only as a generic bound, as written.)

**Protocol decision:** hand-written JSON-RPC over `tiny_http` in JSON-response mode, rather than an SDK. Tools-only, no streaming, and full control of the security checks. Read the official pages before coding: `https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/streamable-http`, `.../basic/versioning`, `.../basic/lifecycle` if present, `.../server/tools`, and `https://modelcontextprotocol.io/specification/2025-11-25/basic/transports`. Support both eras:
- Older clients: `initialize` (negotiate among `2025-03-26`, `2025-06-18`, `2025-11-25`, returning `instructions: RULES`), `notifications/initialized` (202), `ping`, `tools/list`, `tools/call`.
- `2026-07-28`: `server/discover` (with the rules as instructions), version from `_meta`, `resultType: "complete"` on results, `ttlMs` and `cacheScope: "private"` on `tools/list`, `UnsupportedProtocolVersion` -32022, mirrored-header checks as the spec requires.
- No session ids. Other HTTP methods on `/mcp`: 405. Anything but `/mcp`: 404.

**HTTP checks, in this order, before reading the body:** `Host` must be `127.0.0.1:<port>` or `localhost:<port>`, else 403. `Origin`, if present, must be `http://127.0.0.1:<port>` or `http://localhost:<port>`, else 403. `Authorization: Bearer <key>`, hashed and passed to `authenticate`, else 401. Body over 8 MiB: 413. A small fixed pool of worker threads. Never log bodies or keys. `gt-mcp` adds `tiny_http`, `sha2`, `serde_json`, `uuid` and `gt-session`.

**Tools.** Every tool's errors come back as `isError: true` text so the agent can react. Lists are paged (`offset`, `limit`, default 50, max 200).

| Read tool | Returns |
|---|---|
| `get_rules` | `RULES` |
| `tree_summary` | tree name, counts, home person |
| `search_people(query, offset, limit)` | hits with id, names, birth and death years, parents' and partners' names, to tell namesakes apart; uses existing search incl. phonetic |
| `list_people(offset, limit)` | same row shape |
| `get_person(id)` | everything: all names, events, notes in full, citations with source, media details, numbers, living status, `siblingMarker` |
| `get_relatives(id)` | parents, partners, children, siblings as rows, with family ids |
| `get_family(id)`, `get_source(id)`, `get_note(id)`, `get_citation(id)` | the record |
| `list_sources(query, offset, limit)`, `list_places(query, offset, limit)` | matching records |
| `get_media(id, max_size?)` | details plus the picture as MCP image content, downsized to `max_size` (default 1600 px), non-picture documents as resource blobs up to 8 MiB, else a clear message |
| `get_log(filters, offset, limit)`, `get_history(id)` | log rows; record history |
| `find_by_number(source_file, number)` | person id or null |

| Write tool (all take `batch` except the first) | Maps to |
|---|---|
| `begin_batch(title, source_file?)` / `end_batch(batch, report)` | Task 4 |
| `add_person(batch, given?, surnames, patronymic?, sex, birth_date?, birth_place?, relation?: {to, as, step?})` | `CreatePerson` or `QuickAdd` |
| `link_people(batch, person, relation: {to, as, step?, family?})` | `LinkExisting` or `LinkChild` |
| `add_family(batch, partner1?, partner2?, kind)` | `SaveFamily` |
| `add_name(batch, person, kind, given?, surnames, patronymic?, nickname?, ...)` | `SaveName` |
| `add_event(batch, person or family, kind, date?, place?, description?)` | `SaveEvent` |
| `add_note(batch, target, text, kind)` | `SaveNote` |
| `add_source(batch, title, author?, text?)` | `SaveSource` |
| `add_citation(batch, target, source, page?, text?, quality: 0..=3)` | `AddCitation` |
| `add_sibling_marker(batch, family)` | `AddSiblingMarker` |
| `set_number(batch, person, source_file, number)` | `AddNumber` |
| `set_sex(batch, person, sex)` | `UpdatePerson` sex |
| `update_own_note`, `update_own_event`, `update_own_name`, `update_own_citation` | the matching save; the gate decides |

After each successful write call `host.changed(touched)`. `rules.md` is the 13 rules from the spec's "Rules the agent receives", verbatim, plus: "You may never change whether a person counts as alive or dead. Leave a note and report it."

- [ ] **Step 1: Write failing tests.** `protocol.rs`: `initialize_old_client_gets_rules`; `discover_new_client_gets_rules`; `unsupported_version_32022`; `result_type_complete_on_new`; `notification_no_reply`; `tools_list_deterministic`. `http.rs`, on an ephemeral port: `no_key_401`; `revoked_key_401`; `foreign_origin_403`; `foreign_host_403` (DNS rebinding); `binds_loopback_only`; `get_405`; `oversized_body_413`; `client_cannot_claim_other_name` (a `name` in the arguments or `_meta` is ignored in the log). `tools.rs`: `search_finds_by_variant`; `namesakes_distinguishable`; `read_everything_including_living`; `get_person_full_notes`; `get_media_image_content`; `paged_lists`; `no_tree_open_clear_error`; `batch_of_other_tree_refused`; `write_needs_batch`; `cyrillic_name_roundtrip`; `bad_date_refused`; `changed_called_after_write`; `refusal_is_tool_error_not_protocol_error`.
- [ ] **Step 2: Run** `cargo test -p gt-mcp`. Expected: fails to compile.
- [ ] **Step 3: Implement `protocol.rs`, `http.rs`, `tools_read.rs`, `tools_write.rs`, `rules.md`.** Add `crates/gt-mcp` to the workspace (the `crates/*` glob covers it) and `tiny_http` to `[workspace.dependencies]`. Pictures go through `gt-session`'s `thumbs` code.
- [ ] **Step 4: Run** `cargo test -p gt-mcp` and clippy. Expected: PASS.
- [ ] **Step 5: Commit** `feat(mcp): a local MCP server with read and additive write tools`.

### Task 6: Run it from the app: connections, keys, commands

**Files:**
- Create: `app/src-tauri/src/agent_access.rs`
- Modify: `app/src-tauri/src/{lib.rs,state.rs,settings_file.rs,commands.rs,e2e_hooks.rs}`, `app/src-tauri/Cargo.toml`, `app/src/lib/api/{api.ts,api.contract.ts,types.ts}`, `app/src/lib/api/tauri/`, `app/src/lib/api/mock/`
- Test: `app/src-tauri/tests/agent_access.rs`, MockApi tests

**Interfaces:**
- Consumes: Task 5, Task 2's log API.
- Produces commands, each with Tauri, MockApi and contract entries:
  `agent_access_state() -> AgentAccessState { enabled, port, running, error: Option<String>, connections: Vec<ConnectionDto { id, name, created }> }`, `agent_access_set(enabled: bool, port: u16)`, `agent_connection_create(name) -> { connection: ConnectionDto, key: String, snippet: String }`, `agent_connection_revoke(id)`, `agent_rules() -> String`, `change_log(filter, offset, limit)`, `change_batches()`, `mark_reviewed(scope)`, `unreviewed_people()`, `person_agent_changes(id)`, `undo_change_group(group) -> UndoReport`, `undo_change_batch(batch) -> UndoReport`. Tauri event `tree-changed-by-agent` with `{ touched: string[], agent: string }`.

Details: default port `8765`. Enabled state, port and connection list (never keys) in `settings_file`. Key: 32 bytes from `getrandom`, prefixed `gt_`, hex. SHA-256 goes in the keychain under the existing service with user `mcp-<id>`, and is cached in memory when the server starts. With the e2e config-dir override, hashes go in a file in that directory instead (see `ai.rs` and `e2e_hooks.rs` for the keychain pattern). Revoking removes the keychain entry and cache entry at once. Host wraps `AppHandle` and uses `app.state::<AppState>().with_session`. `enabled` resumes at startup, a failure stored in `error` and never fatal. Snippet: `claude mcp add --transport http gene-tree http://127.0.0.1:<port>/mcp --header "Authorization: Bearer <key>"`. Stop the server on `RunEvent::Exit` before `close_project_impl`.

- [ ] **Step 1: Write failing tests:** `create_then_connect_then_revoke`; `key_not_in_settings_file`; `port_taken_reports_error`; `enabled_resumes_on_start`; `log_commands_roundtrip`.
- [ ] **Step 2: Run** `cargo test -p gt-app --test agent_access`. Expected: fails.
- [ ] **Step 3: Implement** and regenerate bindings.
- [ ] **Step 4: Run** all gates. Expected: PASS.
- [ ] **Step 5: Commit** `feat(app): agent access in the app, with connections and keys`.

### Task 7: Settings screen

**Files:**
- Create: `app/src/lib/views/AgentAccessSettings.svelte`, `app/src/lib/views/AgentAccessSettings.test.ts`
- Modify: the settings view, `app/src/lib/i18n/*`

**Interfaces:**
- Consumes: Task 6's commands.

Content per spec "Settings: Agent access". Exact warning: "A connected agent can read everything in the open tree, including living relatives, private notes and pictures. An agent may send what it reads to its AI provider." Ukrainian: «Підключений агент може читати все у відкритому родоводі, зокрема дані живих родичів, приватні нотатки й фото. Агент може надсилати прочитане своєму постачальнику ШІ.» The key and snippet show once in a dialog with a copy button; after closing they cannot be shown again. Revoke asks for confirmation. "Rules agents receive" opens a read-only view of `agent_rules()`. Keep focus inside the dialog when a focused control disappears.

- [ ] **Step 1: Write failing tests:** `off_by_default_warning_visible`; `create_shows_key_once`; `revoke_removes_row_after_confirm`; `port_error_shown`; `rules_readable`; `ukrainian_strings`.
- [ ] **Step 2: Run** `npx --no-install vitest run src/lib/views/AgentAccessSettings.test.ts`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** frontend gates. Expected: PASS.
- [ ] **Step 5: Commit** `feat(ui): settings for agent access`.

### Task 8: Log screen, badges, review, undo, live refresh

**Files:**
- Create: `app/src/lib/views/ChangeLog.svelte`, `app/src/lib/views/ChangeLog.test.ts`, `app/src/lib/state/agentChanges.svelte.ts`, `app/src/lib/views/AgentChangesPanel.svelte`
- Modify: `app/src/lib/views/Shell.svelte` (`NavName` gains `'log'`, label `nav.log`: "Change log" / «Журнал змін»), `app/src/lib/state/project.svelte.ts`, `app/src/lib/state/nav.svelte.ts`, `app/src/lib/ui/{PersonCard,PersonRow}.svelte`, the chart renderer, the person page in `app/src/lib/views/`, `app/src/lib/editors/` shared dialog, `app/src/lib/i18n/*`

**Interfaces:**
- Consumes: Task 6.
- Produces: `agentChanges` store: `unreviewed: Set<string>`, `reload()`, `lastTouched`. It listens for `tree-changed-by-agent`, reloads, and bumps the project revision that views already re-run on.

Behaviour per spec "Marker", "Log screen", "Undo". Rows show author "You", the agent's name, or "Before tracking" («До початку обліку»). Filters: author, batch, not yet reviewed. Batch heading: title, agent, counts, report, "unfinished" when not closed, "Mark reviewed", "Undo batch". A row: "Undo", disabled when reverted. Undo confirms, then shows the `UndoReport` with skipped facts named. Person page panel lists the agent's changes with sources and "Reviewed". An editor open on a touched person shows a banner "An agent changed this person. Reload to continue." and disables Save until reloaded.

- [ ] **Step 1: Write failing tests:** `rows_show_author_and_person_link`; `before_tracking_label`; `filters_apply`; `batch_review_clears_badges`; `person_review_clears_only_that_person`; `undo_batch_shows_skipped`; `refreshes_on_agent_event`; `badge_on_card_row_and_chart`; `stale_editor_blocks_save`; `ukrainian_strings_use_rodovid`.
- [ ] **Step 2: Run** `npx --no-install vitest run src/lib/views/ChangeLog.test.ts`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** frontend gates. Expected: PASS.
- [ ] **Step 5: Commit** `feat(ui): change log, agent badges and review`.

### Task 9: End to end, a real agent, and docs

**Files:**
- Create: `app/e2e-tauri/specs/agent-access.e2e.ts`, `crates/gt-mcp/tests/fixtures/transcript_summary.md` (synthetic), `crates/gt-mcp/tests/scripted_run.rs`, `docs/agent-access.md`
- Modify: `docs/requirements.md` if it lists features

**Interfaces:**
- Consumes: everything.

The synthetic summary has namesakes (two Ivan Kovals), an unnamed sibling group, a contested patronymic, an either-or parent, an inference, a death of someone living, and people not to connect. `docs/agent-access.md`: how to connect, what agents can and cannot do.

- [ ] **Step 1: `scripted_run.rs`:** a scripted client over real HTTP runs the file against a synthetic tree twice. Assert: person count unchanged on the second run; inference only in a note; no link for the either-or parent; contested value kept, plus a note; marker present; death refused, status unchanged, note present; batch undo restores the tree exactly.
- [ ] **Step 2: `agent-access.e2e.ts`:** in the real app enable access, create a connection, run the scripted client from Node with fetch while the app is open. Assert the view refreshes with no action, badge on card, log row names the agent, batch review clears badges, batch undo restores. Screenshots to `GT_E2E_SHOTS` in English and Ukrainian, with the dark theme for one. Run per the project's wdio command, using `./node_modules/.bin/wdio`, never npx.
- [ ] **Step 3: Run** all gates, then the whole e2e suite. Expected: PASS.
- [ ] **Step 4: Real agent, by hand.** Build with `npm --prefix app run tauri -- build --debug --no-bundle`. Run a Claude Code subagent connected to the running app over MCP, given only the fixture file and "update the tree from this". Check it follows the rules; if it breaks one, tighten `rules.md` or the gate and re-run.
- [ ] **Step 5: Commit** `test(e2e): agent access end to end`, merge to develop and main per the project's lane flow, rebuild and run the app.
