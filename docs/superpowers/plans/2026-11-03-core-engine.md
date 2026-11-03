# gene-tree Core Engine Implementation Plan (Plan 1 of 2)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a headless Rust engine and a `gt` command-line tool. `gt` imports any real-world GEDCOM 5.5.1/7.0 file without silent loss, stores it in a documented SQLite project folder, searches it, and exports it back to GEDCOM 5.5.1 or 7.0 with living-person privacy applied.

**Architecture:** A Cargo workspace of four crates.
- `gt-core` holds the format-neutral in-memory `Tree` model, the genealogical date engine, phonetic keys, the living-person rule and the privacy filter. It does no I/O.
- `gt-gedcom` converts bytes ↔ `Tree`. It has its own lenient tokenizer, which keeps every line it does not understand as `RawNode`s in their original position, and strict 5.5.1 and 7.0 writers.
- `gt-store` persists a `Tree` into a project folder (`tree.db` SQLite plus `media/` and `backups/`) and provides search and backups.
- `gt-cli` wires them together.
The crates talk only through `Tree` and the `Interfaces` blocks below. Plan 2 (Tauri desktop UI) builds on these crates.

**Tech Stack:** Rust 1.97 (edition 2024), rusqlite 0.41 (`bundled`, FTS5, collation), encoding_rs 0.8, uuid 1.29 (v7), indexmap 2, rphonetic 4, icu_collator 2, thiserror 2, serde/serde_json, sha2 0.11, clap 4, assert_cmd 2, tempfile 3, proptest 1.

**Spec:** `docs/requirements.md` (requirements v0.1, 2026-11-03). This plan implements the v0.1 engine parts of the spec:
- §3 data model
- §4 formats: GEDCOM 5.5.1/7.0 import and export
- §6.1 privacy filter
- §7.2 storage
- §7.5 schema principles
- §8 testing

The UI (§5.1 views, editors, undo) is Plan 2.

## Global Constraints

- Rust stable 1.97, `edition = "2024"`, `#![forbid(unsafe_code)]` in every crate. `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` must pass.
- Dependencies must be MIT and/or Apache-2.0 (or equally permissive). **No GPL/AGPL crates** (excludes `gedcomkit`, `heic`). Workspace licence `MIT OR Apache-2.0`.
- No network access at runtime. No telemetry.
- All record IDs are UUID v7 (`uuid::Uuid::now_v7()`). All timestamps are Unix milliseconds UTC (`i64`).
- Media regions are normalised `f32` rectangles in `0.0..=1.0`, stored on the *link*, never in the file.
- Living rule default: `max_age_years = 100`. Privacy default: `LivingMode::Placeholder`, `include_private = false`.
- GEDCOM 5.5.1 output: lines ≤ 255 bytes, CONC/CONT splitting, `CHAR UTF-8`, CRLF line endings. GEDCOM 7.0 output: UTF-8 without BOM, LF, no CONC, `GEDC.VERS 7.0`, every extension tag declared in `HEAD.SCHMA`.
- Our GEDCOM extension tags (the only ones we emit): `_GTID` (record UUID), `_EVID` (shared-event UUID), `_PREF Y` (preferred assertion), `_PROOF PROVEN|DISPROVEN`, `_REGION <x> <y> <w> <h>` (normalised), `_FREL`/`_MREL` (per-parent relationship), `_NO <TAG>` (5.5.1 only; 7.0 uses native `NO`). In 7.0 the SCHMA URIs are `https://gene-tree.app/gedcom/ext/<TAG without _>`.
- The guarantee of a lossless GEDCOM round trip covers **everything that came from a GEDCOM file**, plus the extension tags above. Data that exists only in the app (e.g. historical place names) is lossless only in the native project.
- Imported files are untrusted input: no `unwrap()`/`expect()`/indexing that can panic on input-derived data in `gt-gedcom` or `gt-core::date`.
- Commit messages end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. A GEDCOM with a UTF-8 BOM, CRLF endings, and a CONC split *inside* a multi-byte character (`é` split across lines) must import as correct text. Pinned in Task 6.
2. A person with no dates whose relatives have no dates either must be treated as **living** and hidden in every export. Pinned in Task 10.
3. Search must find names regardless of diacritics and script: `muller` finds `Müller`, `Иван` finds `Иван Петров`, `小明` finds `王小明`. Pinned in Task 13.
4. Vendor tags we don't understand (e.g. `_MTTAG` between `NAME` and `BIRT`) and `_UID` must survive import → export in their original position. Pinned in Task 9.
5. A backup taken while the database is open in WAL mode must be a valid database (`PRAGMA integrity_check` = `ok`). A project placed under a OneDrive/Dropbox/iCloud folder must produce a warning. Pinned in Task 14.

---

## File Structure

```
Cargo.toml                       workspace, shared deps, lints
rust-toolchain.toml              channel = "1.97"
.github/workflows/ci.yml         fmt + clippy + test on windows/ubuntu/macos
scripts/fetch-corpus.sh          clones GEDCOM test corpora into tests/corpus (gitignored)
docs/schema.md                   documented SQLite schema (anti-lock-in, spec §7.2)
docs/adr/0001-own-gedcom-tokenizer.md
crates/gt-core/src/
  lib.rs        re-exports
  ids.rs        new_id(), now_ms()
  model.rs      record + assertion types
  raw.rs        RawNode (verbatim unknown data)
  tree.rs       Tree container, normalized_for_compare()
  date/mod.rs   GenDate, DateValue, Date, Calendar, DayOrder
  date/calendar.rs  to_jdn/from_jdn per calendar
  date/parse.rs     lenient parser
  date/format.rs    GEDCOM 5.5.1 / 7.0 / display formatting
  phonetic.rs   soundex, daitch_mokotoff
  living.rs     LivingRule, compute_living
  privacy.rs    PrivacyPolicy, filter
crates/gt-gedcom/src/
  lib.rs        import(), export(), options, GedcomVersion
  encoding.rs   detect + decode (UTF-8/16, ANSEL, cp1252/code pages)
  ansel.rs      ANSEL → Unicode table
  tokenizer.rs  bytes → Vec<GedNode>
  node.rs       GedNode
  tags.rs       tag ↔ EventKind/Role tables
  import/{mod.rs, indi.rs, fam.rs, sour.rs, obje.rs, note.rs, header.rs, repair.rs, report.rs, media_paths.rs}
  export/{mod.rs, build.rs, write.rs}
crates/gt-store/src/
  lib.rs, project.rs, migrations/0001_init.sql, migrate.rs, save.rs, load.rs,
  search.rs, collation.rs, backup.rs, location.rs, media.rs
crates/gt-cli/src/main.rs
tests/fixtures/gedcom/*.ged      small hand-written fixtures (committed)
```

---

### Task 1: Workspace, CI, and the core model

**Files:**
- Create: `Cargo.toml`, `rust-toolchain.toml`, `.gitignore` (`target/`, `tests/corpus/`), `.github/workflows/ci.yml`, `crates/gt-core/{Cargo.toml, src/lib.rs, src/ids.rs, src/model.rs, src/raw.rs, src/tree.rs}`, empty lib crates `gt-gedcom`, `gt-store` and a bin crate `gt-cli` (binary name `gt`).
- Test: `crates/gt-core/tests/model.rs`

**Interfaces:**
- Produces (all `#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]`). Later tasks use these exact names:

```rust
// ids.rs
pub fn new_id() -> Uuid;            // Uuid::now_v7()
pub fn now_ms() -> i64;

// raw.rs — verbatim unknown data, format-neutral
pub struct RawNode { pub tag: String, pub xref: Option<String>, pub value: Option<String>,
                     pub pos: u32 /* index among source siblings */, pub children: Vec<RawNode> }

// model.rs
pub struct Common { pub id: Uuid, pub private: bool, pub changed: Option<i64>,
    pub citations: Vec<Uuid>, pub notes: Vec<Uuid>, pub media: Vec<MediaLink>,
    pub ext_ids: Vec<ExternalId>, pub unknown: Vec<RawNode> }
pub struct ExternalId { pub kind: ExtIdKind, pub value: String }
pub enum ExtIdKind { Xref, Uid, FamilySearch, AncestryApid, Exid { scheme: String }, Other(String) }
pub struct Assertion { pub preferred: bool, pub proof: Option<Proof>, pub confidence: Option<u8> }
pub enum Proof { Proven, Disproven }
pub enum Sex { Male, Female, Intersex, Unknown, Other(String) }
pub struct Person { pub common: Common, pub sex: Sex, pub names: Vec<Name>,
    pub living_override: Option<LivingOverride> }
pub enum LivingOverride { Living, Deceased }
pub struct Name { pub id: Uuid, pub kind: NameKind, pub assertion: Assertion, pub private: bool,
    pub prefix: Option<String>, pub given: Option<String>, pub call: Option<String>,
    pub surname_prefix: Option<String>, pub surnames: Vec<String>, pub patronymic: Option<String>,
    pub suffix: Option<String>, pub nickname: Option<String>, pub lang: Option<String>,
    pub date: Option<GenDate>, pub raw: Option<String>, pub citations: Vec<Uuid>,
    pub notes: Vec<Uuid>, pub unknown: Vec<RawNode> }
pub enum NameKind { Birth, Married, Aka, Religious, Nickname, Immigrant, Professional, Other(String) }
pub struct Family { pub common: Common, pub partner1: Option<Uuid>, pub partner2: Option<Uuid>,
    pub kind: UnionKind, pub children: Vec<ChildLink> }
pub enum UnionKind { Married, CivilUnion, Unmarried, Unknown, Other(String) }
pub struct ChildLink { pub person: Uuid, pub rel1: ParentRel, pub rel2: ParentRel,
    pub preferred: bool, pub assertion: Assertion, pub citations: Vec<Uuid>, pub unknown: Vec<RawNode> }
pub enum ParentRel { Birth, Adopted, Step, Foster, Guardian, Sealed, Unknown, Other(String) }
pub struct Event { pub common: Common, pub kind: EventKind, pub date: Option<GenDate>,
    pub place: Option<Uuid>, pub description: Option<String>, pub age: Option<String>,
    pub cause: Option<String>, pub negative: bool /* GEDCOM 7 NO */,
    pub assertion: Assertion, pub participants: Vec<Participant> }
pub struct Participant { pub target: ParticipantRef, pub role: Role }
pub enum ParticipantRef { Person(Uuid), Family(Uuid) }
pub enum Role { Principal, Witness, Godparent, Clergy, Officiator, Informant, Parent, Father,
    Mother, Child, Spouse, Husband, Wife, Friend, Neighbor, Multiple, Other(String) }
pub enum EventKind { Birth, Christening, Baptism, Death, Burial, Cremation, Adoption,
    Marriage, MarriageBanns, MarriageContract, MarriageLicense, MarriageSettlement, Engagement,
    Divorce, DivorceFiled, Annulment, Separation, Census, Residence, Occupation, Education,
    Religion, Nationality, Title, Emigration, Immigration, Naturalization, Probate, Will,
    Graduation, Retirement, Military, PhysicalDescription, Custom(String) }
pub struct Place { pub common: Common, pub names: Vec<PlaceName>, pub parent: Option<Uuid>,
    pub kind: Option<String>, pub lat: Option<f64>, pub lon: Option<f64>, pub raw: Option<String> }
pub struct PlaceName { pub name: String, pub lang: Option<String>, pub date: Option<GenDate> }
pub struct Source { pub common: Common, pub title: Option<String>, pub author: Option<String>,
    pub publication: Option<String>, pub abbreviation: Option<String>, pub text: Option<String>,
    pub repositories: Vec<RepoRef> }
pub struct RepoRef { pub repository: Uuid, pub call_number: Option<String> }
pub struct Repository { pub common: Common, pub name: String, pub address: Option<String>,
    pub urls: Vec<String> }
pub struct Citation { pub common: Common, pub source: Option<Uuid>, pub page: Option<String>,
    pub date: Option<GenDate>, pub text: Option<String>, pub quality: Option<u8>,
    pub evidence: Evidence }
pub struct Evidence { pub original: Option<bool>, pub primary_info: Option<bool>,
    pub direct: Option<bool> }
pub struct Media { pub common: Common, pub path: String /* relative to project, '/' separated */,
    pub original_name: Option<String>, pub mime: Option<String>, pub title: Option<String>,
    pub date: Option<GenDate>, pub description: Option<String>, pub sha256: Option<String> }
pub struct MediaLink { pub media: Uuid, pub region: Option<Region>, pub primary: bool }
pub struct Region { pub x: f32, pub y: f32, pub w: f32, pub h: f32 }
pub struct Note { pub common: Common, pub kind: NoteKind, pub text: String /* Markdown */,
    pub lang: Option<String>, pub shared: bool /* SNOTE/NOTE record vs inline */ }
pub enum NoteKind { General, Story, Research, Transcription, Conclusion, Other(String) }

// tree.rs — IndexMap keeps insertion order (stable export order)
pub struct TreeHeader { pub producer: Option<String>, pub source_version: Option<String>,
    pub unknown: Vec<RawNode> }
pub struct Tree { pub header: TreeHeader,
    pub persons: IndexMap<Uuid, Person>, pub families: IndexMap<Uuid, Family>,
    pub events: IndexMap<Uuid, Event>, pub places: IndexMap<Uuid, Place>,
    pub sources: IndexMap<Uuid, Source>, pub repositories: IndexMap<Uuid, Repository>,
    pub citations: IndexMap<Uuid, Citation>, pub media: IndexMap<Uuid, Media>,
    pub notes: IndexMap<Uuid, Note> }
impl Tree {
    pub fn new() -> Self;
    pub fn events_of(&self, person: Uuid) -> impl Iterator<Item = (&Event, &Role)>;
    pub fn families_as_partner(&self, person: Uuid) -> impl Iterator<Item = &Family>;
    pub fn families_as_child(&self, person: Uuid) -> impl Iterator<Item = (&Family, &ChildLink)>;
    /// copy with `changed` cleared and ExtIdKind::Xref ids removed — used by round-trip tests
    pub fn normalized_for_compare(&self) -> Tree;
}
```
`GenDate` is declared in `date/mod.rs` as a placeholder struct in this task and filled in by Task 2.

- [ ] **Step 1: Init the repo and workspace.** Run `git init`, then create the workspace (`members = ["crates/*"]`). Put shared dependency versions in `[workspace.dependencies]` and lints in `[workspace.lints]` (`unsafe_code = "forbid"`, clippy `unwrap_used = "deny"` for non-test code). CI matrix: `windows-latest`, `ubuntu-22.04`, `macos-latest`. Steps: checkout, `dtolnay/rust-toolchain@1.97` with clippy and rustfmt, `Swatinem/rust-cache`, `bash scripts/fetch-corpus.sh`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace`.
- [ ] **Step 2: Write failing tests** in `crates/gt-core/tests/model.rs`:
  - `ids_are_v7_and_unique`: 1000 calls to `new_id()` are all distinct and have `get_version_num() == 7`.
  - `tree_relationship_queries`: build P1 and P2 as the partners of family F, child C with `rel1: Birth, rel2: Adopted`, and a birth event for C with C as `Principal`. Assert `families_as_partner(P1)` yields F, `families_as_child(C)` yields `(F, link)` with `link.rel2 == Adopted`, and `events_of(C)` yields one event with `Role::Principal`.
  - `normalized_for_compare_strips_xrefs_and_changed`: a person with `changed: Some(5)` and ext_ids `[Xref "@I1@", Uid "ABC"]` normalizes to `changed: None` and ext_ids `[Uid "ABC"]`.
- [ ] **Step 3: Run** `cargo test -p gt-core`. Expected: compile failure (types missing).
- [ ] **Step 4: Implement** the types and methods above.
- [ ] **Step 5: Run** `cargo test -p gt-core && cargo clippy --all-targets -- -D warnings`. Expected: PASS, no warnings.
- [ ] **Step 6: Commit** with `feat(core): workspace, CI and format-neutral tree model`.

---

### Task 2: Calendars → Julian Day Numbers

**Files:**
- Create: `crates/gt-core/src/date/calendar.rs`
- Test: `crates/gt-core/tests/calendar.rs`

**Interfaces:**
- Produces:
```rust
pub enum Calendar { Gregorian, Julian, Hebrew, FrenchRepublican }
/// day may be None (→ first or last day of month) and month may be None (→ first or last month of year)
pub fn first_jdn(cal: Calendar, year: i32, month: Option<u8>, day: Option<u8>) -> Option<i32>;
pub fn last_jdn(cal: Calendar, year: i32, month: Option<u8>, day: Option<u8>) -> Option<i32>;
pub fn months_in_year(cal: Calendar, year: i32) -> u8;   // Hebrew leap year = 13
pub fn days_in_month(cal: Calendar, year: i32, month: u8) -> Option<u8>;
```
Years are astronomical (1 BCE = year 0) in this module. The BCE flag is converted in Task 3. Month numbering follows GEDCOM order:
- Hebrew: TSH=1, CSH=2, KSL=3, TVT=4, SHV=5, ADR=6, ADS=7, NSN=8, IYR=9, SVN=10, TMZ=11, AAV=12, ELL=13. ADR (Adar I) exists only in leap years.
- French Republican: VEND=1 … FRUC=12, COMP=13 (complementary days, 5 or 6 of them).

Hebrew uses the arithmetic algorithm from Reingold & Dershowitz, *Calendrical Calculations*. French Republican uses the historical leap years 3, 7 and 11 for years 1–14 and the Romme rule after that. Invalid dates (e.g. 30 FEB, or ADR in a non-leap year) return `None`.

- [ ] **Step 1: Write the failing tests:**
```rust
assert_eq!(first_jdn(Gregorian, 2000, Some(1), Some(1)), Some(2451545));
assert_eq!(first_jdn(Gregorian, 1582, Some(10), Some(15)), Some(2299161));
assert_eq!(first_jdn(Julian, 1582, Some(10), Some(4)), Some(2299160));
assert_eq!(first_jdn(Julian, 2000, Some(1), Some(1)), Some(2451558));
assert_eq!(first_jdn(Hebrew, 5760, Some(1), Some(1)), Some(2451433));   // 11 Sep 1999
assert_eq!(first_jdn(Hebrew, 5784, Some(8), Some(15)), Some(2460424));  // 23 Apr 2024
assert_eq!(first_jdn(FrenchRepublican, 1, Some(1), Some(1)), Some(2375840)); // 22 Sep 1792
assert_eq!(first_jdn(FrenchRepublican, 4, Some(1), Some(1)), Some(2376936)); // 23 Sep 1795
assert_eq!(last_jdn(Gregorian, 1900, Some(2), None), first_jdn(Gregorian, 1900, Some(2), Some(28)));
assert_eq!(last_jdn(Gregorian, 1900, None, None), first_jdn(Gregorian, 1900, Some(12), Some(31)));
assert_eq!(first_jdn(Gregorian, 1900, Some(2), Some(29)), None);
assert_eq!(months_in_year(Hebrew, 5784), 13);
assert_eq!(first_jdn(Hebrew, 5783, Some(6), Some(1)), None);           // no Adar I in a non-leap year
```
  Add a proptest `gregorian_roundtrip`: for any valid y in −4000..=4000, m and d, the JDN of (y, m, d) plus 1 equals the JDN of the next calendar day.
- [ ] **Step 2: Run** `cargo test -p gt-core --test calendar`. Expected: FAIL.
- [ ] **Step 3: Implement** the four functions.
- [ ] **Step 4: Run** the tests again. Expected: PASS.
- [ ] **Step 5: Commit** with `feat(core): calendar to JDN conversion (Gregorian, Julian, Hebrew, French Republican)`.

---

### Task 3: Genealogical dates (parse, sort range, format)

**Files:**
- Create: `crates/gt-core/src/date/{mod.rs, parse.rs, format.rs}`
- Test: `crates/gt-core/tests/date.rs`

**Interfaces:**
- Consumes: Task 2 `first_jdn`, `last_jdn`.
- Produces:
```rust
pub struct Date { pub calendar: Calendar, pub year: i32 /* as written, ≥1 */, pub bce: bool,
    pub month: Option<u8>, pub day: Option<u8>, pub dual_year: Option<i32> /* 1731/32 → Some(1732) */ }
pub enum DateValue { Exact(Date), About(Date), Calculated(Date), Estimated(Date),
    Before(Date), After(Date), Between(Date, Date), From(Date), To(Date), FromTo(Date, Date),
    Interpreted(Date), PhraseOnly, Unparsed }
pub struct GenDate { pub raw: String, pub value: DateValue, pub phrase: Option<String>,
    pub sort_min: Option<i32>, pub sort_max: Option<i32> }
pub enum DayOrder { Dmy, Mdy }
pub enum GedcomVersion { V551, V70 }          // defined here, re-exported by gt-gedcom
impl GenDate {
    pub fn parse(input: &str, order: DayOrder) -> GenDate;   // never fails
    pub fn to_gedcom(&self, v: GedcomVersion) -> (String, Option<String> /* PHRASE for 7.0 */);
    pub fn display(&self) -> String;                          // e.g. "about 1850", "12 Mar 1901"
    pub fn sort_key(&self) -> Option<i32>;                    // sort_min.or(sort_max)
}
```
Sort ranges (a **[call]**, recorded in the doc comment). `d.min`/`d.max` = `first_jdn`/`last_jdn` of the date at its precision; when `dual_year` is set, the computation uses `dual_year`.

| Value | sort_min | sort_max |
|---|---|---|
| Exact, Interpreted | d.min | d.max |
| About, Calculated, Estimated | d.min − 5 years | d.max + 5 years |
| Before(d) | `None` | d.max |
| After(d) | d.min | `None` |
| Between(a,b), FromTo(a,b) | a.min | b.max |
| From(d) | d.min | `None` |
| To(d) | `None` | d.max |
| PhraseOnly, Unparsed | `None` | `None` |

Parsing accepts (case-insensitive):
- GEDCOM keywords: ABT, CAL, EST, BEF, AFT, BET…AND, FROM…TO, INT … (phrase).
- 5.5.1 calendar escapes (`@#DJULIAN@`, `@#DHEBREW@`, `@#DFRENCH R@`, `@#DGREGORIAN@`) and the 7.0 keywords (`JULIAN`, `HEBREW`, `FRENCH_R`, `GREGORIAN`).
- `B.C.`, `BC`, `BCE`.
- English month abbreviations and full names, plus the Hebrew and French codes.
- Lenient forms: `abt`/`about`/`c.`/`ca.`/`circa`/`~`/a trailing `?` → About. `bef.`/`before`, `aft.`/`after`, `between X and Y`, `X-Y` (two years → Between).
- ISO `YYYY-MM-DD`, and `NN/NN/YYYY` using `order`.
- A text wholly in parentheses → PhraseOnly.

Formatting:
- 5.5.1 uses escapes and `B.C.`.
- 7.0 uses keywords and `BCE`, writes `dual_year` as the year with the original raw text in PHRASE, and writes `Interpreted` as the date plus PHRASE.

- [ ] **Step 1: Write failing table tests.** Each row is `(input, order, expected value, expected sort range)`:
  - `"12 MAR 1901"` → `Exact(G 1901-03-12)`, range `(jdn, jdn)`
  - `"abt 1850"`, `"c. 1850"`, `"circa 1850"`, `"~1850"`, `"1850?"` → `About(1850)`; sort_min is the JDN of 1 Jan 1845
  - `"Bef. 1900"` → `Before(1900)`, `(None, Some(jdn 31 Dec 1900))`
  - `"between 1900 and 1910"`, `"BET 1900 AND 1910"`, `"1900-1910"` → `Between`, range 1 Jan 1900 .. 31 Dec 1910
  - `"12/03/1901"` with Dmy → 12 Mar; with Mdy → 3 Dec
  - `"1901-03-12"` → Exact
  - `"MAR 1901"` → month precision, range 1..31 Mar
  - `"(during the war)"` → `PhraseOnly`, phrase set, range `(None, None)`
  - `"INT 1900 (about then)"` → `Interpreted(1900)`, phrase "about then"
  - `"@#DJULIAN@ 4 OCT 1582"` and `"JULIAN 4 OCT 1582"` → Julian, sort_min 2299160
  - `"1 MAR 1731/32"` → `dual_year Some(1732)`, sort_min = Gregorian 1 Mar 1732
  - `"44 B.C."`, `"44 BCE"` → `bce: true`
  - `"garbage text"` → `Unparsed`, raw preserved
  - `"30 FEB 1900"` → `Unparsed` (invalid date)

  Add these round-trip tests:
  - `to_gedcom(V551)` of every parsed GEDCOM-syntax row re-parses to the same `value`.
  - `"1 MAR 1731/32"` to V70 → `("1 MAR 1732", Some("1 MAR 1731/32"))`.
  - Proptest: `GenDate::parse` never panics on any `String`.
- [ ] **Step 2: Run** `cargo test -p gt-core --test date`. Expected: FAIL.
- [ ] **Step 3: Implement** the parser as a hand-written tokenizer: split on whitespace, `/`, `-` and parentheses, then classify keywords, numbers and months. No regex crate is needed.
- [ ] **Step 4: Run** the tests. Expected: PASS.
- [ ] **Step 5: Commit** with `feat(core): genealogical date parsing, sort ranges and GEDCOM formatting`.

---

### Task 4: Phonetic keys

**Files:**
- Create: `crates/gt-core/src/phonetic.rs`
- Test: `crates/gt-core/tests/phonetic.rs`

**Interfaces:**
- Produces: `pub fn soundex(name: &str) -> Option<String>` and `pub fn daitch_mokotoff(name: &str) -> Vec<String>`. Both return empty/None for names without Latin letters. Diacritics are folded first (`Müller` → `Muller`). Use `rphonetic` (`Soundex`, `DaitchMokotoffSoundex`). If `rphonetic` 4 does not expose Daitch–Mokotoff, implement it from the published rule table and record that in the commit message.

- [ ] **Step 1: Write the failing tests:**
  - soundex: `Robert`→`R163`, `Rupert`→`R163`, `Ashcraft`→`A261`, `Tymczak`→`T522`, `Müller`=`Muller`, `Иван`→None.
  - daitch_mokotoff: `Moskowitz` contains `"645740"`, `Auerbach` contains both `"097400"` and `"097500"`.
- [ ] **Step 2: Run** `cargo test -p gt-core --test phonetic`. Expected: FAIL.
- [ ] **Step 3: Implement** the two functions.
- [ ] **Step 4: Run** the tests. Expected: PASS.
- [ ] **Step 5: Commit** with `feat(core): soundex and Daitch-Mokotoff keys`.

---

### Task 5: Encoding detection and ANSEL

**Files:**
- Create: `crates/gt-gedcom/src/{encoding.rs, ansel.rs}`, and `docs/adr/0001-own-gedcom-tokenizer.md`
- Test: `crates/gt-gedcom/tests/encoding.rs`

**Interfaces:**
- Produces:
```rust
pub enum DetectedEncoding { Utf8 { bom: bool }, Utf16Le, Utf16Be, Ansel, Windows1252, Other(&'static encoding_rs::Encoding) }
/// Detection order: BOM; UTF-16 by NUL-byte pattern; `1 CHAR` header value (ANSEL / UTF-8 /
/// ANSI / IBMPC / ASCII); then a UTF-8 validity check; otherwise fall back to Windows-1252.
/// Never trust CHAR when the bytes contradict it (e.g. CHAR ANSEL but valid multi-byte UTF-8 → Utf8).
pub fn detect(bytes: &[u8]) -> DetectedEncoding;
/// UTF-16 is converted to UTF-8 bytes BEFORE tokenizing; other encodings are decoded per payload.
pub fn to_line_bytes(bytes: &[u8], enc: &DetectedEncoding) -> std::borrow::Cow<'_, [u8]>;
pub fn decode_payload(bytes: &[u8], enc: &DetectedEncoding) -> String;
pub fn decode_ansel(bytes: &[u8]) -> String; // combining marks precede the base letter → emit base + mark, then NFC
```
The ADR records this decision. A tokenizer of our own (≈300 lines) is required for verbatim, position-preserving storage of unknown data. The 0.x crates `oxidged` and `ged_io` are used only as **test oracles** in Task 9 (dev-dependencies), not at runtime. This narrows spec §4.3: the spike gate criteria apply to our parser instead.

Add `unicode-normalization` (MIT/Apache) for NFC.

- [ ] **Step 1: Write the failing tests:**
  - UTF-8 with a BOM → `Utf8{bom:true}`.
  - The UTF-16LE encoding of `"0 HEAD\r\n"` → `Utf16Le`.
  - `1 CHAR ANSEL` with bytes `0xE2 0x65` → `decode_ansel` gives `"é"` (U+00E9 after NFC).
  - `1 CHAR ANSEL` but the body contains valid UTF-8 `"Müller"` bytes → `Utf8`.
  - Byte `0xFC` with no CHAR → `Windows1252`, and decodes to `"ü"`.
- [ ] **Step 2: Run** `cargo test -p gt-gedcom --test encoding`. Expected: FAIL.
- [ ] **Step 3: Implement.** The ANSEL table covers the 0xA1–0xC8 specials and the 0xE0–0xFE combining marks (Library of Congress MARC-8 ANSEL table).
- [ ] **Step 4: Run** the tests. Expected: PASS.
- [ ] **Step 5: Commit** with `feat(gedcom): encoding detection, ANSEL decoding, ADR 0001`.

---

### Task 6: Lenient tokenizer

**Files:**
- Create: `crates/gt-gedcom/src/{node.rs, tokenizer.rs}`
- Test: `crates/gt-gedcom/tests/tokenizer.rs`

**Interfaces:**
- Consumes: Task 5.
- Produces:
```rust
pub struct GedNode { pub line: u32, pub level: u8, pub xref: Option<String>, pub tag: String,
    pub value: Option<String>, pub children: Vec<GedNode> }
pub enum TokenIssue { BadLine { line: u32, text: String }, LevelJump { line: u32 },
    OrphanConc { line: u32 } }
pub struct Tokenized { pub encoding: DetectedEncoding, pub records: Vec<GedNode>,
    pub issues: Vec<TokenIssue> }
pub fn tokenize(bytes: &[u8]) -> Tokenized;
impl GedNode { pub fn to_raw(&self, pos: u32) -> gt_core::RawNode; }
```
Rules:
- Split on CR, LF or CRLF.
- Allow leading whitespace and indentation.
- Lines longer than 255 bytes are accepted.
- `CONC`/`CONT` payload **bytes** are joined into the parent's value before decoding (CONT inserts `\n`). The space a CONC line starts with is kept.
- A level jump greater than one is attached to the nearest valid parent and reported.
- An unparsable line becomes `BadLine` and is skipped.
- `@@` in a payload is unescaped to `@`.

- [ ] **Step 1: Write the failing tests:**
  - `nested_structure`: `0 @I1@ INDI / 1 NAME John /Smith/ / 2 GIVN John` → 1 record, xref `@I1@`, child NAME with child GIVN.
  - `conc_splits_multibyte_char` (Review Focus #1): a UTF-8 BOM file with CRLF, `1 NOTE caf` + bytes `0xC3` / `2 CONC ` + bytes `0xA9` + ` ok` → value `"café ok"`. The CONC line here contains raw byte 0xA9 directly after the tag's single delimiter space.
  - `cont_inserts_newline`: `1 NOTE a / 2 CONT b` → `"a\nb"`.
  - `ansel_file_decodes`: a fixture `tests/fixtures/gedcom/ansel.ged` with `1 CHAR ANSEL` and a name `M` + `0xE8` + `uller` → `"Müller"`.
  - `garbage_does_not_panic`: proptest on arbitrary `Vec<u8>` (len ≤ 4096); `tokenize` returns.
  - `level_jump_reported`: `0 HEAD / 2 X y` → one `LevelJump` issue, and the node is still kept.
- [ ] **Step 2: Run** `cargo test -p gt-gedcom --test tokenizer`. Expected: FAIL.
- [ ] **Step 3: Implement** with a stack-based builder over byte lines.
- [ ] **Step 4: Run** the tests. Expected: PASS.
- [ ] **Step 5: Commit** with `feat(gedcom): lenient byte-level tokenizer`.

---

### Task 7: Import of individuals, events, places and names

**Files:**
- Create: `crates/gt-gedcom/src/{lib.rs (import entry), tags.rs, import/mod.rs, import/header.rs, import/indi.rs, import/report.rs}`
- Test: `crates/gt-gedcom/tests/import_indi.rs`, fixture `tests/fixtures/gedcom/basic551.ged`

**Interfaces:**
- Consumes: Tasks 1, 3, 6.
- Produces:
```rust
pub struct ImportOptions { pub day_order: DayOrder, pub media_roots: Vec<PathBuf> }
pub struct ImportOutcome { pub tree: Tree, pub report: ImportReport,
    pub resolved_media: Vec<(Uuid /*Media id*/, PathBuf /*absolute source file*/)> } // filled by Task 8
pub struct ImportReport { pub encoding: String, pub version: Option<String>, pub producer: Option<String>,
    pub counts: BTreeMap<String, usize>, pub repairs: Vec<Repair>,
    pub unmapped_tags: BTreeMap<String, usize>, pub unparsed_dates: Vec<(u32, String)>,
    pub missing_media: Vec<String> }
pub enum Repair { DanglingPointer { line: u32, xref: String }, DuplicateXref { line: u32, xref: String },
    OneWayLink { xref_from: String, xref_to: String }, MissingHeader, MissingTrailer,
    Token(TokenIssue) }
pub fn import(bytes: &[u8], opts: &ImportOptions) -> ImportOutcome;   // never fails
impl ImportReport { pub fn to_text(&self) -> String; }
// import/mod.rs internal context used by Tasks 8–9:
pub(crate) struct Ctx { pub tree: Tree, pub report: ImportReport, pub xref_map: HashMap<String, Uuid>,
    pub opts: ImportOptions, pub version: GedcomVersion }
pub(crate) fn place_from_text(ctx: &mut Ctx, text: &str, node: &GedNode) -> Uuid; // dedups by full text
```
Mapping rules:
- **Two passes.** Pass 1 assigns a UUID to every xref, using `_GTID` when present. Pass 2 maps the records.
- **Events.** `tags.rs` maps BIRT, CHR, BAPM, DEAT, BURI, CREM, ADOP, MARR, MARB, MARC, MARL, MARS, ENGA, DIV, DIVF, ANUL, CENS, RESI, OCCU, EDUC, RELI, NATI, TITL, EMIG, IMMI, NATU, PROB, WILL, GRAD, RETI, `_MILT`/`_MILI`, DSCR, and `EVEN`/`FACT` + `TYPE` → `Custom(type)`.
  - An INDI event becomes an `Event` with that person as `Principal`.
  - An `_EVID` value merges events with the same id into one Event and adds the participants.
  - `ASSO @X@` + `RELA`/`ROLE` under an event → a participant with the mapped Role.
- **Names.** `NAME Given /Surname/ Suffix` is parsed. NPFX, GIVN, NICK, SPFX, SURN (comma-separated → several surnames), NSFX and TYPE override it. `_MARNM` becomes an extra `Married` name. The first NAME is `preferred`.
- **Place.** `PLAC "A, B, C"` creates or reuses a chain C ⊃ B ⊃ A (dedup by chain text) and keeps `raw`. `MAP/LATI N52.5`, `LONG E13.4` become signed f64.
- **Living status.** `RESN PRIVACY` or `RESN CONFIDENTIAL` → `private = true`. `_PREF Y`, `_PROOF` and `QUAY` → `Assertion`. A 7.0 `NO XXX` becomes an `Event{negative:true}`, and so does a 5.5.1 `_NO XXX`.
- **Unknown data.** Any child tag not mapped is pushed as `RawNode` (via `to_raw(pos)`) to the nearest owning `unknown` vec and counted in `unmapped_tags`. `_UID` → ExtIdKind::Uid, `_FSFTID` → FamilySearch, `EXID` + `TYPE` → Exid, and the record xref → Xref.
- **Dates.** A date that is `Unparsed` goes into `report.unparsed_dates`.

- [ ] **Step 1: Write the fixture and the failing tests:**
  - `basic551.ged` contains HEAD (SOUR ANCESTRY), two INDI, a BIRT with `ABT 1850` at `Berlin, Brandenburg, Germany`, a `_MARNM`, `_UID`, an unknown `_MTTAG` and TRLR.
  - Assert: 2 persons, the birth event's `sort_min` is about 1845, place chain depth 3 with `raw == "Berlin, Brandenburg, Germany"`, the second Name is `Married`, ext_ids contain `Uid`, and `report.producer == Some("ANCESTRY")`.
  - Assert that `unmapped_tags["_MTTAG"] == 1` and the person's `unknown[0].tag == "_MTTAG"` with its source `pos`.
  - `shared_event_merges_by_evid`: two INDI each holding `BAPM` + `_EVID abc` → 1 Event with 2 participants.
  - `negative_assertion`: a 7.0 `1 NO MARR` → Event{kind: Marriage, negative: true}.
- [ ] **Step 2: Run** `cargo test -p gt-gedcom --test import_indi`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** the tests. Expected: PASS.
- [ ] **Step 5: Commit** with `feat(gedcom): import individuals, names, events, places`.

---

### Task 8: Import of families, sources, notes, media, and repairs

**Files:**
- Create: `crates/gt-gedcom/src/import/{fam.rs, sour.rs, note.rs, obje.rs, repair.rs, media_paths.rs}`
- Test: `crates/gt-gedcom/tests/import_rest.rs`, fixtures `broken.ged` and `family7.ged`

**Interfaces:**
- Consumes: Task 7 `Ctx`, `place_from_text`.
- Produces: no new public API. It fills in `Tree` and `ImportReport`.

Mapping rules:
- **FAM.**
  - HUSB → `partner1` and WIFE → `partner2`. There is no sex assumption: a same-sex couple keeps both.
  - Each `CHIL` becomes a `ChildLink`. The relationship type comes from `INDI.FAMC.PEDI` (applied to both rel1 and rel2), and `_FREL`/`_MREL` (on FAM.CHIL or INDI.FAMC) override rel1 and rel2. PEDI values map birth→Birth, adopted→Adopted, foster→Foster, sealed→Sealed, and 7.0 `OTHER`/`PHRASE` → Other.
  - `FAMC._PREF Y` or the first FAMC → `preferred`.
  - FAM events become Events with `ParticipantRef::Family(id)` as Principal.
- **SOUR record** → Source. **REPO** → Repository. An inline or pointer `SOUR` under any structure → a new `Citation` (PAGE, DATA.DATE, DATA.TEXT, QUAY, `_APID` → ext id), attached to that structure's citations. An inline SOUR without a pointer creates an untitled Source from its text.
- **NOTE/SNOTE.** A record or a 7.0 `SNOTE` → Note `shared: true`. An inline NOTE → Note `shared: false`. A pointer NOTE `@N1@` links the note.
- **OBJE.** A record or an inline OBJE → Media. `FILE` path normalised to `/`, FORM/MIME, and TITL. A link-level `_PRIM Y` or `_PHOTO` → `primary`, and `_REGION` → Region.
- **Media paths.** `media_paths.rs` resolves each FILE: an absolute path that exists; else the relative path against each `opts.media_roots`; else the basename searched recursively in each root. Unresolved files are added to `report.missing_media`, and `Media.path` keeps the original text. A resolved file is pushed to `ImportOutcome.resolved_media`, and its basename goes into `Media.original_name`. Task 15 copies the file and rewrites `Media.path`.
- **Repairs.**
  - A dangling pointer drops the link and adds `DanglingPointer`.
  - A duplicate xref makes the second record get a fresh id, plus `DuplicateXref`.
  - When FAMC/FAMS and FAM.CHIL/HUSB/WIFE disagree, the union of both sides is used, plus `OneWayLink`.
  - A missing HEAD or TRLR is reported. Swapped HUSB/WIFE needs no repair, because the partners are unordered.

- [ ] **Step 1: Write fixtures and failing tests:**
  - `same_sex_couple`: FAM with HUSB @I1@ and HUSB @I2@ (a dialect quirk), both male → partner1 and partner2 both set.
  - `per_parent_relationship`: CHIL with `_FREL Natural`, `_MREL Adopted` → rel1 Birth, rel2 Adopted.
  - `citation_on_name`: a citation under NAME is attached to `Name.citations`, PAGE preserved, QUAY 3 → `quality Some(3)`.
  - `snote_shared`: 7.0 SNOTE record → `shared: true`, and the text includes CONT newlines.
  - `media_region_and_primary`: OBJE link with `_PRIM Y` and `_REGION 0.1 0.2 0.3 0.4`.
  - `broken.ged`: a dangling `FAMC @F9@`, a duplicate `@I1@`, CHIL listed only in FAM, and no TRLR → each produces its Repair and nothing panics.
  - `missing_media_reported`: `FILE C:\Users\x\photo.jpg` with no roots → `missing_media == ["C:/Users/x/photo.jpg"]`.
  - `media_found_by_basename`: a temp dir holding `sub/photo.jpg` passed as a root → resolved.
- [ ] **Step 2: Run** `cargo test -p gt-gedcom --test import_rest`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** the tests. Expected: PASS.
- [ ] **Step 5: Commit** with `feat(gedcom): import families, sources, notes, media; repairs and media path resolution`.

---

### Task 9: GEDCOM export (5.5.1 and 7.0) and round-trip corpus tests

**Files:**
- Create: `crates/gt-gedcom/src/export/{mod.rs, build.rs, write.rs}`, `scripts/fetch-corpus.sh`
- Test: `crates/gt-gedcom/tests/{export.rs, roundtrip.rs, perf.rs}`

**Interfaces:**
- Consumes: Tasks 1, 3, 7, 8.
- Produces:
```rust
pub struct ExportOptions { pub version: GedcomVersion }
pub fn export(tree: &Tree, opts: &ExportOptions) -> Vec<u8>;
```
`build.rs` turns the Tree into `Vec<GedNode>`, and `write.rs` serialises it.

Build rules:
- **HEAD.**
  - `SOUR GENE-TREE` + VERS (the crate version), `GEDC.VERS` and `CHAR UTF-8` (5.5.1 only).
  - 7.0 adds `SCHMA` with one `TAG` line per extension tag used in the file.
  - Header unknown nodes are written back.
- **Records.** Xrefs are regenerated as `@I1@`, `@F1@`, `@S1@`, `@R1@`, `@O1@`, `@N1@` in tree order. Every record gets `_GTID <uuid>`.
- **Families.** partner1 → HUSB and partner2 → WIFE, except that a female partner1 and male partner2 are swapped so standard readers see the expected sex.
- **Shared events.** An event with several Person principals is written under each principal with `_EVID`. A non-principal participant is written as `ASSO @X@` + `RELA <role>` (5.5.1) or `ROLE` (7.0) under the event copy.
- **Places.** `PLAC` = the hierarchy chain joined with `, `. `MAP/LATI/LONG` use `N`/`S`/`E`/`W` prefixes.
- **Unknown nodes.** Each RawNode is inserted among the known children at index `min(pos, len)`, so the original order is restored.
- **Version differences.** 5.5.1 splits values to ≤ 255 bytes per line with CONC (never inside a UTF-8 character) and uses CONT for newlines. 7.0 uses CONT only. Dates are written via `GenDate::to_gedcom(version)`.

- [ ] **Step 1: Write the failing tests** in `export.rs`:
  - `line_limit_551`: a 1000-character note → every output line ≤ 255 bytes, and re-import gives the identical text.
  - `schma_declares_extensions_70`: 7.0 output contains `2 TAG _GTID https://gene-tree.app/gedcom/ext/GTID`.
  - `unknown_tag_position_preserved` (Review Focus #4): import `basic551.ged` and export 5.5.1. In the person record `_MTTAG` must appear after `NAME` and before `BIRT`, and `_UID` is present.
  - `female_partner1_swapped`.
- [ ] **Step 2: Write `roundtrip.rs`.**
  - For every `.ged` in `tests/fixtures/gedcom` and `tests/corpus/**` (skip silently when the corpus is absent) and for both versions: `a = import(file)`, then `b = import(export(a.tree, v))`, then assert `a.tree.normalized_for_compare() == b.tree.normalized_for_compare()`. Print the file name on failure.
  - Oracle check (dev-dependency `ged_io`): for each corpus file our importer's person count equals `ged_io`'s individual count. If `ged_io` fails to parse a file, skip it.
  - `fetch-corpus.sh` shallow-clones `https://github.com/gedcom7code/test-files` and `https://github.com/FamilySearch/GEDCOM.io` (only `testfiles/`) into `tests/corpus/`.
- [ ] **Step 3: Write `perf.rs`** (`#[ignore]`, run in CI with `--release -- --ignored`). Generate a synthetic tree of 50,000 persons and 20,000 families with births, deaths and places, export it to 5.5.1, then time `import`. Expected under 10 s, the spec's spike gate (§4.3).
- [ ] **Step 4: Run** `bash scripts/fetch-corpus.sh && cargo test -p gt-gedcom`. Expected: FAIL on export.
- [ ] **Step 5: Implement**, then run `cargo test -p gt-gedcom && cargo test -p gt-gedcom --release --test perf -- --ignored`. Expected: PASS. A corpus file that fails round-trip is a bug in import or export, so fix it rather than skip it.
- [ ] **Step 6: Commit** with `feat(gedcom): 5.5.1 and 7.0 export, round-trip corpus and perf tests`.

---

### Task 10: Living-person rule

**Files:**
- Create: `crates/gt-core/src/living.rs`
- Test: `crates/gt-core/tests/living.rs`

**Interfaces:**
- Consumes: Tree queries (Task 1), `GenDate` sort range (Task 3).
- Produces:
```rust
pub struct LivingRule { pub max_age_years: u32 }       // Default: 100
pub fn compute_living(tree: &Tree, rule: &LivingRule, today_jdn: i32) -> HashMap<Uuid, bool>;
```
Rules, in order (fail safe, from spec §3.3):
1. `living_override` wins.
2. Any non-negative Death, Burial, Cremation or Probate event with this person as Principal → dead.
3. The latest bound of their birth/christening/baptism (`sort_max`, falling back to `sort_min`) is older than `today − max_age` → dead.
4. Any child's birth bound is older than `today − (max_age − 12)` years → dead.
5. Any parent's birth bound is older than `today − (max_age + 70)` years → dead.
6. Otherwise living. No spouse inference and no transitive inference **[call]**: spouse age gaps are unbounded.

`today − N years` is computed in the Gregorian calendar.

- [ ] **Step 1: Write the failing tests** (today = 2026-11-03):
  - `no_dates_anywhere_is_living` (Review Focus #2): a person and their parents with no events → living.
  - Death with no date → dead.
  - Birth `ABT 1920` → dead (sort_max 1925 < 1926).
  - Birth `ABT 1930` → living.
  - Birth `BEF 1920` → dead.
  - Birth `AFT 1900` (no max, sort_min 1900) → living (an open upper bound never proves age).
  - A child born 1910 → parent dead.
  - A parent born 1850 → child dead.
  - Override `Living` on a person who has a death event → living.
- [ ] **Step 2: Run** `cargo test -p gt-core --test living`. Expected: FAIL.
- [ ] **Step 3: Implement.** Note that rule 3's fallback to `sort_min` is used only when `sort_max` exists; `AFT` has no max, so it is not proof.
- [ ] **Step 4: Run** the tests. Expected: PASS.
- [ ] **Step 5: Commit** with `feat(core): fail-safe living-person rule`.

---

### Task 11: Privacy filter

**Files:**
- Create: `crates/gt-core/src/privacy.rs`
- Test: `crates/gt-core/tests/privacy.rs`

**Interfaces:**
- Consumes: Task 10.
- Produces:
```rust
pub enum LivingMode { Exclude, Placeholder, IncludeAll }
pub struct PrivacyPolicy { pub living: LivingMode, pub include_private: bool, pub rule: LivingRule }
pub struct PrivacySummary { pub hidden_persons: usize, pub placeholder_persons: usize,
    pub removed_records: usize, pub mention_warnings: Vec<(Uuid /*note*/, Uuid /*person*/)>,
    pub group_media: Vec<Uuid> /* kept media also linked to a hidden person */ }
pub fn filter(tree: &Tree, policy: &PrivacyPolicy, today_jdn: i32) -> (Tree, PrivacySummary);
```
Semantics (spec §6.1):
1. **Private beats mode.** Any record, Name, Event or ChildLink with `private == true` is removed unless `include_private`.
2. **Placeholder mode, hidden person.** Names are replaced by one Name `{given: "Living"}` (no surname). Sex becomes Unknown. Their events, attributes, notes, media links, citations, ext_ids and unknown nodes are removed. Their participation in shared events is removed. An event left with no principal is dropped. Family and ChildLink structure is kept.
3. **Exclude mode, hidden person.** The person is removed, along with every reference to them (partner slots → None, ChildLinks removed, participants removed). A family left with no partners and no children is removed.
4. **Unreferenced records.** Notes, citations, media, places, sources and repositories that end up unreferenced are removed (garbage-collected).
5. **Mention warnings (best effort).** For each kept Note whose text contains the `given + " " + surname` of a hidden person (case-insensitive), add a mention warning.
6. **Group media.** Media kept through another link but also linked from a hidden person is listed in `group_media`.

- [ ] **Step 1: Write the failing tests:**
  - `placeholder_hides_everything_but_structure`: a living child in a family with dead parents → the child's name is "Living", no events, the ChildLink exists, and the parents are untouched.
  - `exclude_removes_and_cleans`: the same tree in Exclude mode → the child is gone, the family's children are empty, and the child's only note is gone.
  - `private_beats_include_all`: a private dead person with `IncludeAll`, `include_private: false` → removed.
  - `private_fact_removed`: a private Event on a dead person → removed, and the person kept.
  - `shared_census_keeps_others`: a census with a dead principal and a living witness → the event is kept and the witness participant removed.
  - `mention_warning`: a kept note mentions "Anna Schmidt", who is living → 1 warning.
  - `undated_living_hidden_in_export` (Review Focus #2, end to end): a person with no dates in Placeholder mode → `placeholder_persons == 1`.
- [ ] **Step 2: Run** `cargo test -p gt-core --test privacy`. Expected: FAIL.
- [ ] **Step 3: Implement** as a clone-then-prune pass followed by a reachability GC from persons, families and events.
- [ ] **Step 4: Run** the tests. Expected: PASS.
- [ ] **Step 5: Commit** with `feat(core): privacy filter shared by all exports`.

---

### Task 12: Project folder, schema v1 and migrations

**Files:**
- Create: `crates/gt-store/src/{lib.rs, project.rs, migrate.rs, migrations/0001_init.sql}`, and `docs/schema.md`
- Test: `crates/gt-store/tests/project.rs`

**Interfaces:**
- Produces:
```rust
pub struct Project { pub dir: PathBuf, conn: rusqlite::Connection }
pub enum StoreError { Io(std::io::Error), Sql(rusqlite::Error), NotAProject(PathBuf),
    NewerSchema { found: i32, supported: i32 }, AlreadyExists(PathBuf) }   // thiserror
pub const SCHEMA_VERSION: i32 = 1;
impl Project {
    pub fn create(dir: &Path) -> Result<Project, StoreError>;  // makes dir, media/, thumbs/, backups/
    pub fn open(dir: &Path) -> Result<Project, StoreError>;    // runs pending migrations
    pub fn schema_version(&self) -> i32;
    pub(crate) fn conn(&self) -> &rusqlite::Connection;
}
```
Connection setup: `journal_mode=WAL`, `foreign_keys=ON`, `synchronous=NORMAL`. `PRAGMA user_version` holds the schema version, and table `meta(key, value)` holds `app_version` and `created_ms`.

Schema v1 (spec §7.5). Every row table has `id TEXT PRIMARY KEY` (uuid), `private INTEGER`, `changed INTEGER` and `unknown_json TEXT`. Tables:
- **Core:** `person` (sex, living_override, living_cached), `name` (person_id, pos, kind, preferred, proof, confidence, every name part, surnames_json, lang, date columns*, raw), `family` (partner1_id, partner2_id, kind), `child_link` (family_id, person_id, pos, rel1, rel2, preferred, proof, confidence).
- **Events:** `event` (kind, date columns*, place_id, description, age, cause, negative, preferred, proof, confidence), `participant` (event_id, pos, target_type, target_id, role).
- **Places:** `place` (parent_id, kind, lat, lon, raw), `place_name` (place_id, pos, name, lang, date columns*).
- **Sources:** `source`, `repo_ref` (source_id, pos, repository_id, call_number), `repository` (urls_json), `citation` (source_id, page, date columns*, text, quality, ev_original, ev_primary, ev_direct).
- **Media and notes:** `media` (path, original_name, mime, title, date columns*, description, sha256), `note` (kind, text, lang, shared).
- **Polymorphic links:** `citation_link`, `note_link` and `media_link` (target_type, target_id, pos, …; media_link adds primary and region x/y/w/h), `ext_id` (target_type, target_id, pos, kind, value).
- **Derived and history:** `name_phonetic` (name_id, algo, code), `change_log` (id, ts, target_type, target_id, op, before_json) for Plan 2's undo and history, `tree_header` (single row), and the FTS tables (Task 13).

*"date columns" = `<prefix>_raw, <prefix>_json, <prefix>_sort_min, <prefix>_sort_max`. `target_type` is one of `person|name|family|child_link|event|place|source|repository|citation|media|note`.

Indexes on every foreign key and on `event(date_sort_min)`. `docs/schema.md` describes every table and column in plain words.

- [ ] **Step 1: Write the failing tests:**
  - `create_then_open`: create in a tempdir → the subfolders exist, `schema_version() == 1`, and `journal_mode` is `wal`.
  - `create_refuses_existing_project` → `AlreadyExists`.
  - `open_non_project` → `NotAProject`.
  - `open_newer_schema_refused`: set `user_version = 99` → `NewerSchema{found: 99, supported: 1}`.
  - `schema_doc_lists_every_table`: every table name in `sqlite_master` appears in `docs/schema.md` (read via `include_str!`).
- [ ] **Step 2: Run** `cargo test -p gt-store --test project`. Expected: FAIL.
- [ ] **Step 3: Implement** (migrations are an ordered `&[(&str /*name*/, &str /*sql*/)]` applied in one transaction each).
- [ ] **Step 4: Run** the tests. Expected: PASS.
- [ ] **Step 5: Commit** with `feat(store): project folder, schema v1, migrations, schema docs`.

---

### Task 13: Save, load, search and collation

**Files:**
- Create: `crates/gt-store/src/{save.rs, load.rs, search.rs, collation.rs}`, and migration `0001_init.sql` is extended with the FTS tables (still schema v1, since nothing has shipped)
- Test: `crates/gt-store/tests/{persist.rs, search.rs}`

**Interfaces:**
- Consumes: Tasks 1, 4, 10, 12.
- Produces:
```rust
impl Project {
    /// Replaces the whole tree in ONE transaction; recomputes name_phonetic, person.living_cached
    /// (LivingRule from meta key "living_max_age", default 100) and the FTS rows.
    pub fn save_tree(&mut self, tree: &Tree, today_jdn: i32) -> Result<(), StoreError>;
    pub fn load_tree(&self) -> Result<Tree, StoreError>;
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>, StoreError>;
}
pub struct SearchHit { pub kind: HitKind, pub id: Uuid, pub label: String, pub snippet: Option<String> }
pub enum HitKind { Person, Place, Source, Note }
```
FTS:
- `search_words` (`fts5(kind UNINDEXED, id UNINDEXED, label, body, tokenize='unicode61 remove_diacritics 2')`)
- `search_trigram` (`fts5(kind UNINDEXED, id UNINDEXED, text, tokenize='trigram')`)

A person's body holds all name variants and the places of their events. `search()` unions three sources: the words match (each query term as a prefix), a trigram match (only when the query has ≥ 3 characters, or contains CJK), and phonetic matches (the soundex of the query equals `name_phonetic.code`). It dedups by id, ranks words > trigram > phonetic, and truncates to `limit`.

Collation: register `icu_root` via `icu_collator` (root locale, `compiled_data`) as a rusqlite collation, and use it for `ORDER BY` in label queries.

- [ ] **Step 1: Write the failing tests:**
  - `save_load_roundtrip`: import `basic551.ged` and `family7.ged` with `gt_gedcom::import` (dev-dependency), then `save_tree` → `load_tree` → equal Tree (exact, including order and unknown nodes).
  - `save_replaces`: save tree A then tree B → load gives B.
  - `living_cached_written`: a person with no dates → `living_cached = 1`.
  - `search_diacritics` (Review Focus #3): `muller` finds `Müller`.
  - `search_cyrillic`: `Иван` finds `Иван Петров`.
  - `search_cjk`: `小明` finds `王小明`.
  - `search_phonetic`: `Smyth` finds `Smith`.
  - `collation_orders_accents`: labels `Zoe`, `Émile`, `Eva` sort as `Émile`, `Eva`, `Zoe` under `icu_root`.
  - `bulk_save_perf` (`#[ignore]`): the 50k synthetic tree from Task 9 saves in under 20 s in release.
- [ ] **Step 2: Run** `cargo test -p gt-store`. Expected: FAIL.
- [ ] **Step 3: Implement** with prepared statements reused across rows inside the single transaction. Dates are stored with `serde_json` in `_json` columns; sort columns come from `GenDate`.
- [ ] **Step 4: Run** `cargo test -p gt-store && cargo test -p gt-store --release -- --ignored`. Expected: PASS.
- [ ] **Step 5: Commit** with `feat(store): lossless save/load, FTS5 + trigram + phonetic search, ICU collation`.

---

### Task 14: Backups, location warnings and media copy-in

**Files:**
- Create: `crates/gt-store/src/{backup.rs, location.rs, media.rs}`
- Test: `crates/gt-store/tests/{backup.rs, media.rs}`

**Interfaces:**
- Produces:
```rust
pub enum BackupReason { DailyOpen, BeforeImport, BeforeMerge, Manual }
impl Project {
    /// `VACUUM INTO backups/tree-<yyyyMMdd-HHmmss>-<reason>.db`, then prune to `keep` newest.
    pub fn backup(&self, reason: BackupReason, keep: usize) -> Result<PathBuf, StoreError>;
    /// Copies `src` into media/<sha256[0..2]>/<sha256>.<ext>; returns (relative path, sha256).
    /// Identical content is stored once.
    pub fn add_media_file(&self, src: &Path) -> Result<(String, String), StoreError>;
}
pub enum LocationWarning { SyncedFolder(&'static str /*provider*/), NetworkShare }
pub fn check_location(dir: &Path) -> Option<LocationWarning>;
```
`check_location`:
- **Synced folders.** A path component matching `OneDrive*`, `Dropbox`, `iCloudDrive`, `iCloud Drive`, `Mobile Documents` (macOS), or `Google Drive`/`GoogleDrive`/`My Drive` → `SyncedFolder`. Also `%OneDrive%`, when set and a prefix of the path.
- **Network shares.** A Windows UNC path (`\\server\…`) → `NetworkShare`. On Linux, a mount under `/mnt` or `/media` is **not** flagged (too noisy) **[call]**.

- [ ] **Step 1: Write the failing tests:**
  - `backup_while_open_is_valid` (Review Focus #5): save a tree, keep the connection open, run `backup(Manual, 5)`, open the backup with plain rusqlite → `PRAGMA integrity_check` returns `"ok"` and the person count matches.
  - `backup_prunes`: 7 backups with keep=5 → 5 files remain, the newest ones.
  - `onedrive_warns`: `C:\Users\a\OneDrive - Contoso\Family` → `SyncedFolder("OneDrive")`.
  - `dropbox_warns`: `/home/a/Dropbox/tree` → `SyncedFolder("Dropbox")`.
  - `unc_warns`: `\\nas\share\tree` → `NetworkShare`.
  - `plain_dir_ok`: a tempdir → `None`.
  - `media_dedup`: adding the same file twice → the same relative path, and one file on disk.
- [ ] **Step 2: Run** `cargo test -p gt-store --test backup --test media`. Expected: FAIL.
- [ ] **Step 3: Implement.** Timestamps in names use UTC. Two backups taken in the same second get a `-2` suffix.
- [ ] **Step 4: Run** the tests. Expected: PASS.
- [ ] **Step 5: Commit** with `feat(store): WAL-safe backups, synced-folder warnings, content-addressed media`.

---

### Task 15: `gt` command-line tool

**Files:**
- Create: `crates/gt-cli/src/main.rs`
- Test: `crates/gt-cli/tests/cli.rs`

**Interfaces:**
- Consumes: `gt_gedcom::{import, export, ImportOptions, ExportOptions}`, `gt_core::privacy::{filter, PrivacyPolicy, LivingMode}`, `gt_store::{Project, check_location, BackupReason}`.
- Produces the commands below (clap derive). Exit code 0 on success, 1 on a user error, and 2 on an internal error. Errors print one line to stderr.
```
gt check  <file.ged> [--mdy]                         print ImportReport::to_text(); no project
gt import <file.ged> --into <dir> [--mdy] [--media-root <dir>]... [--report <out.txt>]
          creates the project, copies resolved media (add_media_file, rewriting Media.path and
          sha256), save_tree, prints counts + report summary; prints the LocationWarning if any
gt export <dir> -o <out.ged> [--gedcom 5.5.1|7.0] [--living exclude|placeholder|all] [--include-private]
          default --gedcom = the version recorded in tree_header.source_version (fallback 5.5.1);
          default --living = placeholder; prints the PrivacySummary (hidden counts, warnings)
gt search <dir> <query> [--limit 20]                 one hit per line: kind  label  id
gt backup <dir> [--keep 10]                          prints the backup path
```
`today_jdn` is taken from the system clock (UTC) in `main.rs` only. The library crates never read the clock.

- [ ] **Step 1: Write the failing tests** (assert_cmd + tempfile):
  - `check_prints_report`: `gt check basic551.ged` → stdout contains `ANCESTRY` and `_MTTAG`.
  - `import_then_search`: import into a temp dir, then `gt search <dir> muller` prints the person's line.
  - `export_hides_living_by_default`: a fixture with a living person (no dates) → the exported file has no `NAME` line containing their surname, but does contain `Living`.
  - `export_include_all`: with `--living all` the surname is present.
  - `export_version_follows_source`: import a 7.0 fixture, export without `--gedcom` → `2 VERS 7.0`.
  - `import_into_existing_fails`: exit code 1 and a one-line error.
  - `media_copied`: a fixture referencing `photo.jpg`, with `--media-root` pointing to the fixture dir → the file exists under `<dir>/media/`.
- [ ] **Step 2: Run** `cargo test -p gt-cli`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test --workspace && cargo clippy --all-targets -- -D warnings`. Expected: PASS.
- [ ] **Step 5: Commit** with `feat(cli): gt check/import/export/search/backup`.

---

### Task 16: Fuzzing the untrusted-input parsers

**Files:**
- Create: `fuzz/Cargo.toml`, `fuzz/fuzz_targets/{tokenize.rs, import.rs, date.rs}`, and a CI job `fuzz` (nightly, ubuntu only, 60 s per target, on `schedule` weekly plus manual dispatch)
- Test: the fuzz targets themselves, with a seed corpus copied from `tests/fixtures/gedcom`

**Interfaces:**
- Consumes: `gt_gedcom::tokenize`, `gt_gedcom::import`, `gt_core::GenDate::parse`.

- [ ] **Step 1: Write the three targets.** Each calls its function on arbitrary bytes; `date.rs` uses `String::from_utf8_lossy`. In `import.rs`, `export` then runs on the result, and that export must not panic either.
- [ ] **Step 2: Run** `cargo +nightly fuzz run import -- -max_total_time=300`. Expected: no crash. For any crash, add the input as a regression fixture and test, fix the bug, and re-run.
- [ ] **Step 3: Commit** with `test: cargo-fuzz targets for tokenizer, importer and dates`.

---

## Spec coverage notes (deviations and calls)

- **§4.3** The library spike is replaced by our own tokenizer (ADR 0001), with `ged_io` as a test oracle. The spike gate's criteria (no panic on the corpus, no silent line loss, equal round-trip, 50k in < 10 s) are Tasks 6, 9 and 16. **[call]**
- **§7.5** Union events use `ParticipantRef::Family` with role Principal instead of two `partner`-role rows. This keeps unions with an unknown partner. **[call]**
- **§3.3** There is no spouse-based inference. Child (−12 years) and parent (+70 years) margins are fixed. **[call]**
- **§4.1** Merge into an existing tree, GEDZIP, the Gramps/GEDCOM X formats, and EXIF reading are later phases or Plan 2.
- **Plan 2 (UI).** It covers the Tauri shell, editors, views, undo via `change_log`, the media viewer and settings, and it starts with the 10k-node canvas spike on Linux, Windows and macOS (spec §7.3). The owner's real GEDCOM file should be added to `tests/fixtures/private/` (gitignored) before Plan 2 starts.
