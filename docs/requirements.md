# gene-tree — Requirements (draft v0.1, 2026-11-03)

A desktop genealogy app, written in Rust, for Windows, Linux and macOS. It collects and organises a family tree:
people, relationships, dates, places, sources, photos and stories. It reads and writes open genealogy formats,
starting with an existing GEDCOM export.

Status: brainstormed requirements. Calls that were not made by the user are marked **[call]** with the reasoning,
so they can be overturned. Research facts that could not be verified are marked **[unverified]**.

---

## 1. Intent

- **Who:** the owner (the single editor) plus their family. Family members mostly *view* what the owner has built,
  and send back photos, stories and corrections. **[call]** Real multi-user editing is out of v1 (see §9).
- **Why:** to own the family history in an open, durable form. It should not be locked in a subscription site or a proprietary file,
  and it should still open in 30 years.
- **First moment of use:** import a GEDCOM export (most likely 5.5.1 or a vendor dialect of it, from Ancestry, MyHeritage,
  FamilySearch, RootsMagic, Gramps, etc.), see the whole tree with nothing lost, then start adding photos and stories.

### Success looks like
1. Importing the owner's GEDCOM loses nothing. Every line is either understood or kept verbatim, and an import report
   says what was repaired.
2. Exporting back to GEDCOM and re-importing gives the same data (round-trip).
3. Adding a person, a photo with tagged faces, a dated event with a source, and a story takes seconds, not minutes.
4. Relatives can browse a shared copy with no install, and living people are hidden by default.

### Anti-goals (failure even if it "works")
- Silent data loss on import or export: the #1 complaint about every genealogy app.
- A proprietary or undocumented storage format.
- Leaking living people's data through any export path.
- Silent automatic merging of people.
- A UI that bogs down at 10k–50k people.
- Requiring an account, a server, or a network connection for core use.

---

## 2. Scope by release

| Phase | Theme | Contents |
|---|---|---|
| **v0.1 (MVP)** | Import, browse, edit | GEDCOM 5.5.1 and 7.0 import (as a new tree only), core data model, person and family editors, search, pedigree and descendant views, media attachments, GEDCOM 5.5.1 and 7.0 export with privacy filter, backups |

**Rule:** the v0.1 **schema** already holds everything later phases need (see §7.5): media regions, conflicting values with a preferred flag, `NO` assertions, evidence fields, historical place names, stable IDs and change timestamps. Phases only add the UI on top. The privacy filter is built in v0.1 as a model-level view that every later export path reuses.
| **v0.2** | Media and stories | Face/region tagging, rich-text stories, media inbox, GEDZIP import and export, data-quality checks, relationship calculator |
| **v0.3** | Sharing | Static HTML site export, PDF charts and reports, change-set exchange with relatives (diff and accept), duplicate detection and merge |
| **v1.0** | Polish | Gramps XML import, GEDCOM X import and export, i18n, installers signed for all OSes, auto-update (opt-in), accessibility pass |
| **Later** | | Fan/hourglass charts, maps and timelines, DNA records, historical place gazetteer, books, hosted collaboration |

---

## 3. Data model

The model is a superset of GEDCOM 7 and Gramps, so either can be imported without loss.

### 3.1 Records (all have: stable UUID, original xref/ids from import, created/changed timestamps, private flag, notes, citations, media links, custom attributes)

| Record | Key requirements | Priority |
|---|---|---|
| **Person** | Sex (M/F/X/unknown, free-form allowed). Ordered list of names. Events, attributes. Living status: computed, with a manual override. | Must |
| **Name** | Several per person, each typed (birth, married, alias, religious, nickname, immigrant…) and optionally dated. Parts: prefix, given, call name, surname prefix (van, de), surname(s) (multi-part), patronymic, suffix, title. Script and language tag for non-Latin names and transliterations. Sort and display rules are configurable. | Must (script tag: Should) |
| **Family / union** | Two **unordered** partner slots, either of which can be empty. No father=male/mother=female rule, so same-sex couples work from day one. Type: married, civil union, unmarried, unknown. A person can be in any number of unions. Divorce, annulment and separation are *events*, not flags. | Must |
| **Parent–child link** | Relationship **per parent**: birth, adopted, step, foster, guardian, sealed, unknown. "Preferred parents" flag for people with several parent sets. | Must |
| **Event / fact** | First-class and shareable: type (built-in + custom), date, place, description, age, cause. Participants have **roles** (principal, witness, godparent, informant…), so one census or baptism event links to many people. Facts (occupation, religion, residence, title, nationality) use the same structure. | Must |
| **Date** | See §3.2. | Must |
| **Place** | Hierarchy (parish < town < county < state < country), optional lat/lon, place type. Historical names with date ranges (Breslau/Wrocław) are a Should. Flat GEDCOM place strings are split on commas into a hierarchy when possible, and the original text is kept. | Must (basic), Should (historical names) |
| **Source / Citation / Repository** | Source = the work. Citation = a specific reference (page, entry, date accessed, URL, transcription) plus quality. Repository = archive with a call number. Citations attach to *any* assertion (event, name, relationship, attribute), not only to a person. Quality: GEDCOM QUAY 0–3, plus optional evidence-analysis fields (original/derivative source; primary/secondary info; direct/indirect evidence). | Must (source and citation), Should (repository, evidence fields) |
| **Media** | File (relative path in the project), MIME type, title, date, description, EXIF metadata. Attach to any record. **Regions**: a normalised rectangle in the *link*, used for face tagging and for marking an area of a document. "Primary photo" per person. | Must (attach), Should (regions) |
| **Note / Story** | Standalone and shareable (GEDCOM 7 SNOTE). Typed (general, story, research, transcription, conclusion). Rich text (Markdown or a limited HTML subset **[call]**), with links to people. | Must |
| **Conflicting claims** | Several values for one fact (two birth dates from two sources), one marked *preferred*. Proven/disproven flags. GEDCOM 7 `NO` negative assertions ("never married"). | Should |
| **Task / research log** | To-do list per person, with status. | Could |
| **DNA match** | Import DNA-related tags as attributes or notes; a dedicated model comes later. | Could |
| **Unknown/extension data** | Any tag the app does not understand is kept verbatim against its record and written back on export (lossless round-trip). | Must |

### 3.2 Dates
Genealogical dates are **not** timestamps. For each date, store: the raw text, a parsed structure and a sort key. Never fill an unknown day or month with "Jan 1".
- Qualifiers: exact, ABT, CAL, EST, BEF, AFT, BET…AND, FROM…TO, and a phrase in parentheses: "(during the war)".
- Partial dates (year only, or month and year), dual dating (`1 MAR 1731/32`), BC years.
- Calendars: Gregorian (Must). Julian, Hebrew and French Republican on import and display (Should). Editing those calendars is Could.
- Lenient input: accept "abt 1850", "1850?", "c. 1850", "12/03/1901" with a configurable DMY/MDY preference. Always show how the input was understood.
- Age arithmetic and "age at event" display, with the uncertainty carried through.
- **[call]** Write our own date parser. No mature Rust crate covers GEDCOM free-text dates.

### 3.3 Living-person rule
A person counts as living unless one of these applies: they have a death or burial event; they were born more than N years ago (N defaults to 100 and can be changed); or a parent, spouse or child's dates imply that they are dead. A manual override per person works in both directions, "treat as living" and "treat as deceased".
**Fail safe:** the inference from relatives uses only the most conservative end of any date range. A person with no dates at all whose relatives are also undated counts as **living**. The computed result is stored as a derived, filterable column. It is rebuilt whenever dates, events or relationships change, and on startup if the rule's settings changed.

---

## 4. Formats (open-format support)

| Format | Import | Export | Priority | Notes |
|---|---|---|---|---|
| **GEDCOM 5.5.1** (and vendor dialects) | ✅ | ✅ | **Must, v0.1** | The real-world default. Most large vendors still do not export 7 (the FamilySearch implementation list shows RootsMagic, MyHeritage and FamilySearch as "TBD"). |
| **GEDCOM 7.0.x** (latest 7.0.18, Feb 2026) | ✅ | ✅ | **Must, v0.1** | UTF-8, SCHMA extension declarations, SNOTE, EXID, `NO`, strict dates and ages, MIME FORM. **[call]** The default export version follows the version the tree was imported from (usually 5.5.1), so the owner's other tools can read it. |
| **GEDZIP (.gdz)** | ✅ | ✅ | Should, v0.2 | A GEDCOM 7 file with its media in one zip: the best "send the whole tree with photos" format. |
| **GEDCOM 5.5.5** | ✅ | ❌ | Should | Treat as a strict variant of 5.5.1. |
| **Gramps XML (.gramps) / package (.gpkg)** | ✅ | Could | Should, v1.0 | The richest open format apart from GEDCOM. Schema 1.7.2 for Gramps 6.0. `.gpkg` structure **[unverified]**. |
| **GEDCOM X** (JSON/XML, file zip) | ✅ | ✅ | Should, v1.0 | Still maintained by FamilySearch and the basis of its API. The `gedcomx` crate exists. |
| **CSV** | ✅ wizard | ✅ | Could | Spreadsheet lists of people, and simple bulk entry. |
| **GeneWeb (.gw)** | Could | ❌ | Later | `geneweb` crate. |
| **Static HTML site** | – | ✅ | Should, v0.3 | See §6. |
| **PDF / SVG / PNG charts and reports** | – | ✅ | Should, v0.3 | |
| **Native project** | ✅ | ✅ | Must | See §7.2. A documented SQLite schema plus a media folder, and itself an open format. |

### 4.1 GEDCOM import requirements (Must)
- **Encoding detection** by sniffing the bytes, without trusting the `CHAR` header: UTF-8 (with or without BOM), UTF-16 LE/BE, ANSEL (combining marks *before* the base letter), ASCII, Windows-1252 and other code pages. Accept CR, LF and CRLF line endings, indented levels, and lines over 255 characters.
- **Correct CONC/CONT handling**, including CONC splits in the middle of a word or a multi-byte character, and the leading space a CONC line should keep.
- **Vendor dialects**: map the known `_TAGS` (`_UID`, `_MARNM`, `_PHOTO`/`_PRIM`, `_FREL`/`_MREL`, `_APID`, `_FSFTID`, `_TREE`, `_STAT`, …) to model fields. Keep anything that cannot be mapped verbatim. Detect the producer from `HEAD.SOUR` and apply a per-producer profile.
- **Broken structures are repaired and reported:** dangling pointers, duplicate xrefs, one-way FAMC/FAMS links, swapped HUSB/WIFE, missing HEAD/TRLR.
- **Media paths**: resolve absolute Windows paths, relative paths and URLs. Offer a "locate missing media folder" step, then copy the found files into the project.
- **Import report** (Must): counts by record type, every repair (with its line number), every unmapped tag, every unparsed date. The report can be saved.
- **Import as a new tree** (Must), or **merge into the existing tree** (Should, through the duplicate review in §5).
- Performance target: a 50k-person / 30 MB GEDCOM imports in under 30 s on a mid-range laptop **[call]**.

### 4.2 Export requirements (Must)
- Every export (GEDCOM, GEDZIP, HTML, PDF, CSV, change set) goes through **one privacy filter** (§6.1), with a preview showing how many people and records are affected.
- Exports conform to their spec strictly: the lenient reader is paired with a strict writer.
- Round-trip test: import → export → import → compare must give identical data for the official test corpus (§8).
- **Which paths are lossless:** the native project, GEDZIP and GEDCOM 7 (our extensions declared in `SCHMA`) carry everything. GEDCOM 5.5.1 carries everything only through `_` extension tags (roles, `NO`, shared notes, EXID). Other tools may drop those tags, and the export dialog says so.
- 5.5.1 output follows the 255-character line limit and splits lines with CONC/CONT. The importer is lenient about long lines. On import, lines are joined at the **byte** level and decoded afterwards, so a CONC split inside a multi-byte character is safe.

### 4.3 Library choice for GEDCOM **[call, needs a spike]**
The Rust GEDCOM ecosystem is young and moving fast. Candidates (crates.io, 2026-11-03):
- `oxidged` 0.3.2 (MIT): reads and writes 5.5.1, 7.0 and the 7.1 draft. Lenient, lossless, ANSEL and code pages, GEDZIP, 34 producer profiles. Very new, about 900 downloads.
- `ged_io` 0.17.0: 5.5.1 and 7.0, streaming, has a writer, GEDZIP as a feature. More established.
- `gedcomkit` 0.1.19: byte-preserving round-trip. **GPL-3.0**, which forces the app's licence, so avoid it unless the app is GPL.

**Decision (2026-11-03, ADR 0001 in the core-engine plan):** our own lenient tokenizer is used, because unknown data must be kept verbatim *in its original position*. `ged_io` serves as a test oracle only. The gate criteria below now apply to our parser.
Original approach: put the library behind our own `GedcomReader`/`GedcomWriter` trait. Run a one-week spike that tests `oxidged` and `ged_io` against the official corpus and the owner's real export. Write our own parser only if both fall short.
**Spike gate:** a candidate passes when (1) every file in the official 7.0 test files and the gedcom7code corpus parses with no panic, (2) every line of the owner's export either maps to the model or is kept as unknown data (no line silently dropped), (3) parse → write → parse gives an equal tree for all of them, and (4) a 50k-person file parses in under 10 s. Pin the chosen version exactly.

---

## 5. Features

### 5.1 Must (v0.1)
- **Tree projects**: create, open, recent list, "Save a copy", automatic timestamped backups (configurable count), and a crash-safe store.
- **Person editor**: names, sex, events, attributes, parents, partners, children, notes, media, citations. Quick actions: "add father / mother / partner / child / sibling" with relationships prefilled.
- **Family editor**: partners, union type, union events, children with per-parent relationship types and child order.
- **Event editor**: date with a live "understood as…" preview, place picker with typeahead and create-new, participants with roles, citations.
- **Source and citation editor**, with "reuse the last citation" and "clone citation" shortcuts (citation entry is a known pain point).
- **Navigation views**:
  - Person list: virtualised, sortable, filterable (surname, living, has no source…).
  - Family view: a person with their parents, partners and children, the classic genealogy "home" screen.
  - Pedigree chart (ancestors) and descendant chart: zoom, pan, click to re-centre, "home person".
  - **[call]** Charts are drawn on a canvas with the layout computed in Rust. An accessible list or outline view mirrors the chart.
- **Search**: one search box over names (all name variants, diacritic-insensitive, with phonetic matching such as Soundex or Daitch–Mokotoff), places, notes and sources. Full-text search uses SQLite FTS5.
- **Media**: attach photos and documents (JPEG, PNG, WebP, TIFF, PDF, GIF) by dragging them in, generate thumbnails, read EXIF date and GPS into suggested fields, view in a zoomable viewer. Files are copied into the project's media folder.
- **Undo/redo** for every edit command (one command can touch many rows, e.g. "add child"), and an **edit history** per record. An import or a merge is not an undo step: the app takes a backup before it, and "undo import" restores that backup.
- **Privacy**: a private flag on every record and fact, the living-person rule (§3.3), and the export filter (§6.1).
- **Settings**: date format (DMY/MDY), name display order, living threshold, home person, theme (light/dark).

### 5.2 Should (v0.2–v0.3)
- **Face/region tagging** on photos: draw a box, then link it to a person. The person's gallery shows the cropped face, and a crop can be chosen as their portrait.
- **Stories**: a rich-text editor with headings, lists, links to people and places, embedded images and a word count. Stories can be exported to the HTML site.
- **Media inbox**: a watched "inbox" folder where relatives' photos and files land, to be triaged one by one into people and events.
- **Data-quality checks**: child born before a parent was 12 or after they were 60, birth after death, age over 110, a woman giving birth after 55, marriage before 14, facts without a source, missing or disconnected people, duplicate events. The thresholds can be configured.
- **Duplicate detection and merge**: candidates scored on name, dates, place and relatives. A side-by-side compare where the user picks each field, then the merge with undo. **No automatic merge, ever.**
- **Relationship calculator**: "X is Y's second cousin once removed", with the path highlighted. Handles multiple paths and pedigree collapse.
- **Static HTML site export** (§6.2).
- **Reports**: family group sheet, ancestor (Ahnentafel) report, descendant report, individual summary. Output as PDF and HTML.
- **Printable charts**: pedigree and descendant charts as SVG and PDF, including large-format posters tiled across multiple pages.
- **Change-set exchange with relatives** (§6.3).
- **Places**: a map showing a place's location, coordinates entered by hand or picked on the map. **[call]** The map uses OpenStreetMap tiles online and degrades gracefully offline.

### 5.3 Could (later)
- Fan, hourglass and "all relatives" charts. Timeline view. A map of the events in a person's life or migration path.
- Statistics (surnames, lifespans, places) and consanguinity / pedigree-collapse analysis.
- Research tasks and log. Evidence Explained citation templates.
- DNA match records (shared cM, segments).
- Historical place-name gazetteer lookup.
- A family-history book with typeset PDF output (e.g. via `typst` as a library).
- HEIC photo support (licensing and packaging cost; see §7.3).
- A single-file read-only HTML viewer of the whole tree.
- Optional local LLM/OCR help for transcribing documents (anti-goal: sending the family's data to the cloud without explicit consent).

---

## 6. Privacy and sharing

### 6.1 Privacy filter (Must, shared by every export path)
- Modes for living people: **exclude**, **show as "Living"** (name hidden, structure kept), or **include all**. The default is "show as Living".
- Records flagged private are always excluded unless "include private" is chosen explicitly.
- Hiding a person also hides their notes, media, media regions and citations, and their part in shared events.
- Strip EXIF and GPS metadata from exported photos (on by default).
- Warn when free text in notes about other people mentions a hidden person's name. Full redaction of free text is not attempted.
- Rationale: GDPR/CCPA apply to data about living people, and family trusts it with you.
- **Precedence:** a private flag always beats the living mode. "Include all" exports private records only when "include private" is also ticked.
- **"Show as Living" exposes no dates, places, events, notes or media for that person.** It keeps only the placeholder and the links. A family whose partners are all hidden appears as a placeholder. A child whose parents are hidden still links to the placeholders, so that the tree structure stays intact.
- The name-mention warning is **best effort** (nicknames and initials are missed), and the UI says so.
- Photo *content* cannot be filtered. Media attached only to hidden people is excluded; a group photo that includes a living person is a manual decision, and the export preview lists such photos.
- **Unfiltered paths** that are for the owner only, and say so in the UI: backups, "Save a copy", the project zip. The undo log and edit history are never included in any export, including the project zip **[call]**.

### 6.2 Static HTML site (Should, v0.3)
- One folder or zip that works from a USB stick, by opening `index.html`, or on any static host (GitHub Pages, Netlify, NAS). No server or install needed.
- Contents: person pages, family pages, surname index, interactive pedigree view, photo galleries with face tags, stories, sources, and offline search (an embedded JSON index).
- Goes through the privacy filter. "Generated on" date and owner contact on every page.
- **[call]** Each page carries a link to "suggest a correction", which opens a pre-filled e-mail. The e-mail holds only the page's person ID and title, no data. Showing the owner's contact address is an opt-in setting.
- No map tiles or any other third-party resources in the site (place coordinates link out to OpenStreetMap instead).

### 6.3 Contributions from relatives (Should, v0.3)
- **Change-set file**: the owner exports a branch as GEDZIP (or a small JSON change set). A relative edits it in any GEDCOM-capable tool, or in a future lightweight viewer, and sends it back.
- On import, the app matches records by stable UUID/`_UID`/EXID and shows a **per-change diff** (added, changed, removed facts and media) to accept or reject one by one. Unmatched people go through the duplicate review.
- Stable IDs, change timestamps and the export base version are kept in the model from v0.1, so this can be built later without a migration.

---

## 7. Technical requirements

### 7.1 Platforms and distribution
- Windows 10/11 x64 (arm64 Could), macOS 12+ (arm64 and x86_64, universal build), Linux x64 (AppImage, .deb, .rpm; Flatpak Could).
- Built natively on GitHub Actions runners, one per OS (no cross-compiling GUI apps). Release artifacts: `.msi` and NSIS `.exe`, `.dmg`, `.AppImage`/`.deb`/`.rpm`.
- Signing: macOS Developer ID plus notarisation (needed to get past Gatekeeper; $99/yr). Windows code signing via Azure Trusted Signing or an OV certificate in a cloud HSM **[unverified current CA rules]**. Linux is unsigned or uses repository signing.
- Auto-update is **opt-in**. A local, offline-first app should not phone home without being asked.
- Works fully offline. Network is used only for map tiles, the update check and optional lookups.

### 7.2 Storage
- **[call]** One project = a folder:
  ```
  MyFamily.genetree/
    tree.db          SQLite (WAL), schema version in PRAGMA user_version + a meta table
    media/           original files, content-hash names (dedup), never modified
    thumbs/          regenerable cache
    backups/         timestamped copies of tree.db
  ```
- SQLite through `rusqlite` with the `bundled` feature. Schema migrations are versioned and tested. The schema is **documented in `docs/schema.md`**, so the data stays readable without the app (anti-lock-in).
- Full-text search: SQLite FTS5 (`unicode61 remove_diacritics`), with phonetic keys stored as computed columns. `tantivy` only if FTS5 proves insufficient.
- "Export project as zip" and "open project from zip" for moving it between machines.
- Every edit is a transaction, and the undo log is persisted.
- Backups use `VACUUM INTO` or the SQLite backup API, never a raw file copy while WAL is active. Triggers: on open (once per day), before every import or merge, and on a configurable interval. The last N backups are kept.
- **Unsupported locations:** a project on a network share or a live-synced folder (OneDrive, Dropbox, iCloud) risks corruption with WAL. The app detects common sync folders and warns. For syncing, the supported path is backups or the project zip.
- Media files keep their original filename (and source path) in the database next to the content-hash name.
- Sorting: names are sorted with ICU collation via the `icu_collator` crate, registered as a SQLite collation (bundled SQLite has no ICU). For CJK names, an FTS5 `trigram` index is added next to `unicode61`, because `unicode61` cannot split CJK text into words.

### 7.3 Application stack **[call, confirm with a spike]**
| Concern | Choice | Why |
|---|---|---|
| Shell / UI | **Tauri 2** (2.12.x) + TypeScript frontend (Svelte or SolidJS) | Rich-text stories, face-region drawing, image zoom, accessible forms and large charts are all mature on the web side. Tauri also has the best installer, updater and signing story. Native Rust toolkits (egui, Slint, Iced) have no mature rich-text editor. |
| Fallback | egui/eframe | If a web stack is unacceptable, at the cost of a home-made rich-text editor. |
| Core logic | Pure Rust crates in a workspace: `core` (model, dates, privacy), `store` (SQLite), `gedcom` (adapter), `layout` (charts), `export` (HTML/PDF), `app` (Tauri commands) | The core can be tested without a UI, and could later have a CLI or a server front. |
| Chart layout | Our own code: tidy-tree layout (Walker / Reingold–Tilford with the Buchheim fix) for descendants, a generation grid for pedigrees, and marriage nodes for couples. Pedigree collapse is drawn as a link to the duplicate. | There is no genealogy-aware Rust layout crate. |
| Images | `image` 0.25, `fast_image_resize`, `kamadak-exif` | Pure Rust. HEIC deferred (`libheif-rs` brings a C library and HEVC patent baggage; the pure-Rust `heic` crate is AGPL). |
| PDF/SVG | SVG output → `svg2pdf`/`krilla` for PDF, `resvg` for PNG. `typst` later for books. | What is on screen is what gets printed. |
| i18n | Fluent (fluent-js in the UI). English first, with the date and name rules locale-aware. **[call]** The second language depends on the owner's family; to be decided. |
| Licence | **[call] MIT/Apache-2.0** for the app. Avoid GPL dependencies (`gedcomkit`) unless the owner chooses GPL. |

- Risk to retire first: **WebKitGTK performance on Linux** with a 10k-node chart **[unverified]**. Spike: render a 10k-person tree on all three OSes before committing. Use the Canvas 2D API with viewport culling; an SVG DOM will not keep up. Measure WebView2 and WKWebView as well.
- **IPC:** Rust→UI data goes through paged, windowed commands (a page of the person list, the people inside the chart viewport), never through whole-tree JSON payloads.
- **Fonts:** PDF and SVG output embeds fonts and shapes text (rustybuzz) so that CJK, Hebrew and Cyrillic names print correctly. A bundled Noto subset is the fallback **[call]**.

### 7.5 Schema principles (decide before v0.1 code; changing later forces migrations)
- **Polymorphic links** in one table each for citations, notes, media and custom attributes: `(target_type, target_id, …)`. Any record or assertion can carry them: a name, an event role, a parent–child link, an attribute.
- **Per-assertion rows:** names, events, attributes and links are rows with their own UUID, private flag, confidence, preferred flag, proven/disproven status and changed timestamp. They are not columns on `person`.
- **Union events are family events:** an event row whose principal participant is the *family* itself, so a union with an unknown partner still works. A GEDCOM FAM event maps to it and back.
- **Parent sets:** a child links to a *family* (the GEDCOM FAMC), and the link stores one relationship type per partner of that family (GEDCOM 7 puts PEDI on FAMC, and 5.5.1 dialects use `_FREL`/`_MREL`). A child may link to several families, and one is marked preferred.
- **Dates:** raw text, calendar, qualifier, precision, and **sort_min / sort_max** as day numbers (ranges and dual dates need both), plus a parse status.
- **Places:** `place` (hierarchy, type, coordinates) plus `place_name` (name, language, date range) from day one.
- **External IDs:** a separate table holding many per record: import xrefs per import, `_UID`, EXID, FamilySearch IDs.
- **Unknown data:** a verbatim GEDCOM subtree (tag, payload, children, position) attached to the record or assertion it came from, written back in its original order.
- **Phonetic keys** (Soundex and Daitch–Mokotoff) are computed columns on name parts. A derived `living` column on person (§3.3).
- **Notes:** stored as Markdown **[call]**. Links to people and places are `genetree://person/<uuid>` URIs, so text survives renames.
- **Calendars:** every calendar is converted to a day number for sorting. Julian and Hebrew conversion is needed already for v0.1 sorting, if the import contains them.

### 7.4 Non-functional
- **Performance**: open a 50k-person tree in under 2 s. Search results in under 100 ms. Chart pan and zoom at 60 fps with viewport culling. Memory under 500 MB at 50k people **[call]**.
- **Reliability**: no data loss on crash or power cut (SQLite WAL plus atomic writes). Backup before every import or merge.
- **Accessibility**: full keyboard use, screen-reader labels, and an accessible list or outline mirror for every canvas chart. Respect system font size, dark mode and reduced motion.
- **Unicode everywhere**: names in Cyrillic, Hebrew (RTL), CJK and diacritics display, sort (ICU collation) and search correctly.
- **Security**: no telemetry by default. Exported HTML is free of scripts that load third-party resources. Imported files are untrusted input, so the parsers must be fuzz-tested.

---

## 8. Testing and quality
- Unit tests in `core` for the date parser, living-person rule, privacy filter and relationship calculator. Property tests (`proptest`) for date round-trips.
- **GEDCOM corpus**: the FamilySearch `GEDCOM.io/testfiles` (gedcom70 and gedcom71), the `gedcom7code/test-files` corpus, plus anonymised real vendor exports (Ancestry, MyHeritage, RootsMagic, FTM, Gramps).
- **Round-trip tests** in CI: import → export → re-import gives equal data, and exported 7.0 files pass an external validator (`js-gedcom` or `gedcom7` checker).
- Fuzzing (`cargo-fuzz`) of the GEDCOM and date parsers.
- Migration tests: every older schema version opens and upgrades.
- Front-end end-to-end tests for the key flows (import, add a person, tag a face, export with privacy).
- Pull-request CI on all three OSes runs clippy, fmt and the tests. A tagged release builds signed installers.

---

## 9. Non-goals for v1
- Multi-user real-time editing, accounts, or a hosted server (the data model keeps the door open).
- Mobile apps.
- Integrations with Ancestry, MyHeritage or FamilySearch APIs (OAuth and partner agreements; FamilySearch's GEDCOM X API is a later Could).
- Record hints or automatic matching against online databases.
- DNA analysis beyond storing match records.

---

## 10. Open questions for the owner
1. **Family use**: do relatives only view (HTML site and shared files), or should some of them edit in their own copy of the app? This decides how early §6.3 is needed.
2. **Source of the GEDCOM**: which program or site produced the export, and how many people and photos does it hold? This decides the dialect profile and performance targets, and is the first test file.
3. **Languages and scripts** in the family (e.g. Cyrillic or Hebrew names, non-English UI)?
4. **Licence and distribution**: personal use only, or published as open source (and under which licence)?
5. **Code signing budget**: are the Apple $99/yr and a Windows signing service acceptable, or unsigned builds for family use?
6. **Story format**: Markdown (portable, plain text) or WYSIWYG rich text (friendlier, exported as HTML)? The current call is Markdown storage behind a WYSIWYG-style editor.
7. **"Rust app" meaning**: with Tauri, the core (data, GEDCOM, layout, exports) is Rust, but the screens are TypeScript in a webview. Is that acceptable, or should the UI be pure Rust (egui), giving up a polished rich-text editor?
8. **Fuzzing scope**: relatives' files from the media inbox are also untrusted input, so image and PDF decoders join GEDCOM in the fuzzing plan.

---

## 11. Research items not verified
WebKitGTK chart performance; the depth of `oxidged`'s date handling; the internal structure of `.gpkg`; whether Ancestry actually ships GEDCOM 7 export (listed "by Mar 2026") and RootsMagic's export version; the `nom-exif` licence; current Windows code-signing CA rules and GitHub runner labels.
