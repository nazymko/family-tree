# gene-tree Desktop UI Implementation Plan (Plan 2 of 2)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the v0.1 desktop app: a Tauri 2 + Svelte 5 UI for creating or importing a family tree, browsing it (person list, family view, pedigree and descendant charts), editing it (people, events, families, sources, media) with undo, and exporting it with a privacy preview.

**Architecture:** The frontend (`app/`) talks to Rust only through one typed TypeScript `Api` interface, with two implementations: `MockApi` (in-memory fixtures, used for the whole first half of this plan) and `TauriApi` (`invoke`). On the Rust side a new library crate `gt-session` holds the open project, the in-memory `Tree`, derived indexes, edit commands with undo, DTOs and chart windows; `gt-app` (`app/src-tauri`) is a thin Tauri shell over it; `gt-layout` computes chart layouts. All data crosses IPC paged or windowed, never as a whole tree. Edits persist granularly through a new `gt-store::edit` module (per-record upsert/delete plus `change_log`).

**Tech Stack:** Tauri 2.12.x, Svelte 5 (runes) + TypeScript + Vite, Vitest + @testing-library/svelte + jsdom, Playwright (smoke, Apache-2.0), Rust 1.97 / edition 2024, `ts-rs` (DTO type generation), `image` 0.25, `fast_image_resize`, `kamadak-exif`, `tauri-plugin-dialog`, `tauri-plugin-opener`, plus Plan 1's crates. Pin the latest stable of every npm package at implementation time and record it in `package-lock.json`.

**Spec:** `docs/requirements.md` (esp. §1, §5.1, §6.1, §7.3, §7.4) and Plan 1 `docs/superpowers/plans/2026-11-03-core-engine.md` (its Interfaces blocks are the exact Rust API used here). Face tagging, rich stories, HTML site, reports and duplicate merge (spec §5.2) are out of scope.

## Start here: what can begin before Plan 1 is finished

| Group | Tasks | Needs |
|---|---|---|
| **A. Frontend on MockApi only** | 1-16 | Nothing from Plan 1. Start immediately. |
| **B1. Rust, no data model needed** | 17 (Tauri shell + canvas spike on real webviews), 18 (`gt-layout`) | Plan 1 **Task 1** only (the Cargo workspace root). |
| **B2. Rust over the engine** | 19 | Plan 1 **Tasks 12, 13, 14** (store schema, save/load/search, backup/media). |
| | 20 | Plan 1 **Tasks 1, 3, 10, 12, 13**; Task 19 (meta API). |
| | 21 | Task 19, Task 20; Plan 1 Tasks 1, 3. |
| | 22 | Task 20; Plan 1 **Tasks 3, 4, 13**. |
| | 23 | Tasks 20, 21; Plan 1 **Tasks 7, 8, 9, 11, 14** (Task 15 is the reference flow). |
| | 24 | Tasks 18, 20. |
| | 25 | Task 21; Plan 1 **Task 14**. |
| **C. Wiring** | 26 (`gt-app` commands) needs 17, 20-25. 27 (`TauriApi`) needs 3, 26. 28 (e2e + perf budgets) needs everything. | |

Parallel lanes once Plan 1 lands: one agent on 19-23 in order, one on 18 then 24, one on 25; the frontend lane finishes 1-16 meanwhile. Tasks 4 and 17 are the risk-retirement spike (spec §7.3): do them first; their gate decides whether Tasks 14 and 18 proceed as written.

## Global Constraints

- Tauri **2** (2.12.x), Svelte **5** with runes, TypeScript `strict: true`, Vite. Frontend in `app/`, Rust shell in `app/src-tauri` (crate `gt-app`, added to the root workspace `members`). Rust 1.97, edition 2024, `#![forbid(unsafe_code)]`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`.
- Dependencies MIT and/or Apache-2.0 (or equally permissive: ISC, BSD-2). No GPL/AGPL, **no MPL (so no axe-core)**. Bundled font files are OFL-1.1 assets (not code); list them in `THIRD_PARTY.md`.
- **No network at runtime, no telemetry.** CSP: `default-src 'self'; img-src 'self' data: gtmedia: http://gtmedia.localhost; style-src 'self' 'unsafe-inline'; connect-src ipc: http://ipc.localhost`. No remote fonts or scripts. No updater plugin, no `tauri-plugin-http`, no `tauri-plugin-fs` (all file access goes through Rust commands).
- Windows 10/11, macOS 12+, Linux (WebKitGTK 4.1) must all pass the same tests. Minimum window 960x640.
- **IPC is paged or windowed** (spec §7.3): person list pages of at most 200 rows, chart viewport queries, search limited to 50. No command may return a whole `Tree` or a list longer than 200 items (except `recent_projects`, `get_person` sub-lists, and import-report lists, which are capped at 5000 with a `truncated` flag).
- Budgets (spec §7.4), measured on a 50k-person tree in Task 28: open project < 2 s; `search` < 100 ms; `list_persons` page < 30 ms; `chart_window` < 8 ms; `apply` (one edit) < 50 ms; chart pan/zoom 60 fps with 10k nodes; resident memory < 500 MB.
- **Accessibility** (spec §7.4): every feature usable by keyboard alone; every control has an accessible name; every canvas chart has an outline mirror; respect system font size (all sizes in `rem`), dark mode (`prefers-color-scheme`) and `prefers-reduced-motion`. Text contrast >= 4.5:1 (3:1 for large text and UI boundaries).
- **Unicode everywhere:** names in Cyrillic, Hebrew (RTL), CJK and diacritics render (`<bdi dir="auto">` around every person name, `lang` attribute where known), sort (Rust ICU collation) and search correctly.
- Living-person privacy: the UI never computes privacy itself. Export preview and export both call the same Rust `gt_core::privacy::filter`; defaults `LivingMode::Placeholder`, `include_private = false`, living `max_age_years = 100`.
- Every edit is one `EditCommand` -> one transaction -> one undo step. The UI acknowledges an edit only after Rust confirms it was persisted.
- Copy: sentence case; buttons name their action ("Save event", "Add father"); the toast repeats the verb ("Event saved"); errors say what happened and what to do, with no apology; no ALL-CAPS labels, no middle-dot meta strings, no arrow suffixes.
- Commit messages end with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.

## Visual direction: "Reading room"

The product is a quiet archive: parish registers, index cards, acid-free board, iron-gall ink, a ribbon bookmark. The interface should feel like a well-kept finding aid, not an admin template. The one memorable element is the **ruled index card** (the focus person's card in the family view, note areas, and editor headers: ruled 1px lines every 28 px behind the text) together with the **ribbon** (a small red bookmark glyph marking the home person and the current row). Everything else is flat, hairline-bordered paper layers with no card shadows and no gradients.

**Palette tokens** (CSS custom properties on `:root`; dark values under `@media (prefers-color-scheme: dark)` guarded by `:root:not([data-theme="light"])` and again under `:root[data-theme="dark"]`; the Settings theme choice sets `data-theme`):

| Token | Light | Dark | Use |
|---|---|---|---|
| `--paper-0` | `#F2F3EE` | `#12161D` | app background (cool sage-grey paper, not cream) |
| `--paper-1` | `#FBFBF8` | `#1A1F28` | cards, editors, inputs |
| `--paper-2` | `#E4E7DF` | `#0E1117` | sidebar, sunken wells |
| `--rule` | `#C6CABF` | `#2C3340` | 1px borders and index-card ruling |
| `--ink-0` | `#1B2230` | `#E3E5DD` | primary text (iron-gall blue-black) |
| `--ink-1` | `#454D5C` | `#A9AFBC` | secondary text |
| `--ink-2` | `#626A79` | `#8A91A0` | tertiary text, placeholders (>= 4.5:1 on `--paper-1`) |
| `--ribbon` | `#9B2C3F` | `#E58A98` | accent: home person, current row, primary button fill (light) / text (dark) |
| `--ribbon-wash` | `#F1DDE0` | `#3A1F26` | selected-row background |
| `--on-ribbon` | `#FFFFFF` | `#1B0F13` | text on a ribbon fill |
| `--verdigris` | `#2B6B62` | `#7CCBBD` | positive: saved, living-is-hidden confirmations |
| `--foxing` | `#8A5A12` | `#E3B561` | warnings (text); `--foxing-wash` `#F3E6C8` / `#38290F` |
| `--danger` | `#A3262A` | `#F08A8C` | destructive actions and errors |
| `--focus` | `#2D5BA8` | `#86AEF2` | 2px focus ring, offset 2px (never the same hue as `--ribbon`) |

Status is never colour alone: "Living" is a verdigris dot plus the word; sex is a text glyph plus an `aria-label`; warnings carry an icon and a sentence.

**Type** (bundled variable fonts via `@fontsource-variable/source-serif-4` and `@fontsource/atkinson-hyperlegible-next`; fallbacks `Georgia, 'Noto Serif', serif` and `system-ui, 'Segoe UI', 'Noto Sans', sans-serif`; CJK and Hebrew fall through to system fonts). Names, headings and notes use **Source Serif 4**; all controls, lists and chrome use **Atkinson Hyperlegible Next** (chosen for legibility for older relatives). Numbers in dates and tables use `font-variant-numeric: tabular-nums`. Base size 15 px = `0.9375rem`.

| Token | Size / line | Face and weight | Use |
|---|---|---|---|
| `--t-xs` | 0.75rem / 1rem | sans 500 | badges, chart secondary text |
| `--t-sm` | 0.8125rem / 1.125rem | sans 400 | table cells, hints |
| `--t-base` | 0.9375rem / 1.4rem | sans 400 | controls, lists |
| `--t-md` | 1.0625rem / 1.5rem | serif 450 | note body (line-height 1.65, max 68ch) |
| `--t-lg` | 1.3125rem / 1.75rem | serif 600 | section headings |
| `--t-xl` | 1.625rem / 2rem | serif 600 | page titles |
| `--t-2xl` | 2.125rem / 2.5rem | serif 600 | the focus person's name on the index card |

**Spacing** (4 px grid): `--s-1` 2, `--s-2` 4, `--s-3` 8, `--s-4` 12, `--s-5` 16, `--s-6` 24, `--s-7` 32, `--s-8` 48, `--s-9` 64 px. Sidebar 232 px (collapses to 56 px icons below 1100 px window width); content column max 1120 px; form measure 68ch; list row height 40 px; chart node 176x56 px with 40 px horizontal and 12 px vertical gaps. Radii: `--r-1` 2 px (inputs, cards, chart nodes), `--r-2` 6 px (dialogs, menus), badges fully round. Borders are 1 px `--rule`. Only dialogs and menus have a shadow (`0 8px 24px rgb(0 0 0 / .18)`, `.5` in dark).

**Layout concept (family view, the home screen)**

```
+-----------+--------------------------------------------------------+
| sidebar   |  Parents:  [Father card]   [Mother card]                |
| paper-2   |                 |______________|                        |
|           |  +--ruled index card-------------+                      |
| Family    |  | Anna Maria Schmidt     ribbon |  [Partner card] ...   |
| People    |  | 1850-1921   Hamburg           |                       |
| Pedigree  |  +-------------------------------+                      |
| ...       |  Children:  [card] [card] [card]   (birth order)        |
+-----------+--------------------------------------------------------+
```
Content is left-aligned; the focus card is the only large element.

**Component inventory** (all in `app/src/lib/ui/`, each with a Vitest file): `Button` (primary / quiet / danger), `IconButton`, `Icon` (Lucide-derived ISC paths copied into `icons.ts`, about 24 glyphs), `TextField`, `TextArea`, `Select`, `Checkbox`, `RadioGroup`, `Combobox` (typeahead, used by place and source pickers), `Field` (label + hint + error wiring), `Dialog` (modal, focus trap, Esc closes, returns focus), `Menu`, `Tabs`, `Banner` (info / warning / error), `Toast` (with Undo action), `EmptyState`, `Badge`, `Kbd`, `ProgressBar`, `VirtualList`, `PersonCard`, `Ribbon`, `IndexCard`.

**Motion rules.** Tokens: `--dur-fast` 90 ms, `--dur-base` 160 ms, `--dur-slow` 260 ms; easing `cubic-bezier(.2, 0, 0, 1)`. Allowed: hover and focus colour changes (fast); dialog and menu open (opacity plus 4 px translate, base); toast in (base); the family view re-centre (cards FLIP to their new places, slow) and the chart re-centre (camera tween 280 ms) because they answer a click. Forbidden: mount-time entrance animations, hover lifts, parallax, skeleton shimmer (loading rows are static at 40% opacity). `@media (prefers-reduced-motion: reduce)` sets every duration token to `0ms` and disables the camera tween.

## Review Focus

1. **Non-Latin and RTL names** (`יעקב כהן`, `Иван Петров`, `王小明`, `Müller`): must render in cards, list rows, chart nodes and the outline without bidi-scrambled punctuation (`<bdi dir="auto">`), stay in correct sort order (Rust sorts, the UI never re-sorts), and the A-Z rail must still reach them (an "other scripts" bucket). Pinned in Tasks 7 and 8.
2. **A stale or wrong privacy preview:** a person with no dates must be counted as hidden/placeholder in the export preview; changing any option or any edit after the preview must disable "Export" until the preview is recomputed. Pinned in Tasks 15 and 23.
3. **Degenerate tree shapes:** an empty tree, a person who is their own ancestor (bad GEDCOM cycle), a person with 12 partners or 40 children, a pedigree root with no parents, and a 50k-person list must never hang, crash, or render a blank page (empty states say what to do next). Pinned in Tasks 6, 8, 14, 18, 24.
4. **Unparseable dates must never be lost or blocked:** `"sometime in the 1850s?"` is saved verbatim as a phrase/unparsed date with an amber "Not understood as a date, saved as written" note; `03/04/1901` previews differently under DMY and MDY. Pinned in Task 10.
5. **Hostile file input at the drop zone:** dropping a folder, a 0-byte file, an unsupported type (`.exe`), an unreadable path, or the same photo twice must give a per-file message and leave the others attached; non-ASCII Windows paths must work. A project placed in OneDrive/Dropbox must show the sync warning before first save. Pinned in Tasks 13, 23, 25.

---

## File Structure

```
Cargo.toml                          (modify) add "app/src-tauri" to workspace members
crates/gt-layout/                   chart layout, no dependency on gt-core
  src/{lib.rs, types.rs, pedigree.rs, descendants.rs, grid.rs}
crates/gt-store/src/edit.rs         (new) granular upsert/delete + change_log + meta
crates/gt-store/src/migrations/0002_edit.sql
crates/gt-session/                  (new lib) everything the UI needs, testable without a webview
  src/{lib.rs, error.rs, dto.rs, session.rs, index.rs, names.rs, query.rs, family_view.rs,
       person_detail.rs, command.rs, ops/{mod.rs, person.rs, quick_add.rs, family.rs,
       event.rs, cite.rs, media.rs, delete.rs}, undo.rs, lookup.rs, dates.rs, places.rs,
       transfer.rs (import/export/relink), charts.rs, thumbs.rs, exif.rs, settings.rs}
app/
  package.json, vite.config.ts, tsconfig.json, svelte.config.js, eslint.config.js, index.html, spike.html
  playwright.config.ts, e2e/smoke.spec.ts
  src/main.ts, src/App.svelte
  src/styles/{tokens.css, base.css, fonts.css}
  src/lib/ui/*                      component inventory above
  src/lib/api/{types.ts, generated/ (ts-rs output, Task 20), api.ts, mock/{MockApi.ts, fixtures.ts, mockLayout.ts, mockParse.ts},
               tauri/{TauriApi.ts, TauriPlatform.ts}, context.ts}
  src/lib/state/{nav.svelte.ts, settings.svelte.ts, undo.svelte.ts, shortcuts.ts, toast.svelte.ts}
  src/lib/chart/{camera.ts, renderer.ts, window.ts, hit.ts, ChartCanvas.svelte, OutlineMirror.svelte, synth.ts}
  src/lib/views/{Welcome, ImportReport, Shell, People, FamilyView, PersonEditor, Sources, MediaView,
                 Pedigree, Descendants, ExportDialog, Settings, ShortcutSheet}.svelte
  src/lib/editors/{EventEditor, NameEditor, FamilyEditor, CitationEditor, NoteEditor, QuickAdd, DateField, PlacePicker, SourcePicker}.svelte
  src/lib/media/{Dropzone.svelte, MediaGrid.svelte, MediaViewer.svelte}
  src/spike/Spike.svelte
  src-tauri/{Cargo.toml, tauri.conf.json, build.rs, capabilities/default.json, src/{main.rs, lib.rs, state.rs, commands.rs, protocol.rs, settings_file.rs}}
docs/spikes/2026-11-03-canvas-fps.md   spike results (Task 17)
THIRD_PARTY.md                         fonts and icon licences
```

---

# Group A: frontend on MockApi only (no Plan 1 dependency)

### Task 1: Scaffold `app/`, test runners and CI

**Files:**
- Create: `app/package.json`, `app/vite.config.ts`, `app/tsconfig.json`, `app/svelte.config.js`, `app/eslint.config.js`, `app/index.html`, `app/src/main.ts`, `app/src/App.svelte`, `app/src/test-setup.ts`, `THIRD_PARTY.md`
- Modify: `.github/workflows/ci.yml` (add job `frontend`; created by Plan 1 Task 1, so if it does not exist yet create it with only this job and let Plan 1 merge)
- Test: `app/src/App.test.ts`

**Interfaces:**
- Produces npm scripts: `dev`, `build`, `check` (`svelte-check --fail-on-warnings`), `lint` (eslint with `eslint-plugin-svelte`, all `svelte/a11y-*` rules at `error`), `test` (`vitest run`), `test:watch`, `e2e` (`playwright test`), `spike` (`VITE_GT_MODE=spike vite`), `tauri` (`tauri`).
- Produces `main.ts` mode switch: `import.meta.env.VITE_GT_MODE === 'spike'` mounts `src/spike/Spike.svelte` (Task 4), otherwise `App.svelte`.
- Vitest config: environment `jsdom`, `setupFiles: ['src/test-setup.ts']` (imports `@testing-library/jest-dom/vitest`, stubs `ResizeObserver`, `matchMedia`, `HTMLCanvasElement.getContext` returning a recording fake), `resolve.conditions: ['browser']` so Svelte 5 resolves its client build.

- [ ] **Step 1: Write the failing test** `App.test.ts`: `renders_root_landmark` renders `App` and asserts `screen.getByRole('main')` exists and `document.documentElement.lang === 'en'`.
- [ ] **Step 2: Run** `npm --prefix app test` -> FAIL (no project).
- [ ] **Step 3: Scaffold** with `npm create vite` (Svelte + TS template), upgrade to Svelte 5 runes mode (`compilerOptions.runes: true`), add devDependencies: vitest, jsdom, @testing-library/svelte, @testing-library/user-event, @testing-library/jest-dom, eslint, eslint-plugin-svelte, typescript-eslint, svelte-check, @playwright/test. CI job runs on `ubuntu-22.04` only: `npm ci`, `check`, `lint`, `test`, `build`.
- [ ] **Step 4: Run** `npm --prefix app run check && npm --prefix app run lint && npm --prefix app test` -> all pass.
- [ ] **Step 5: Commit** `feat(app): scaffold Svelte 5 + Vite + Vitest + CI`.

### Task 2: Design tokens, theming and base components

**Files:**
- Create: `app/src/styles/{tokens.css, base.css, fonts.css}`, `app/src/lib/ui/*.svelte` (inventory above), `app/src/lib/ui/icons.ts`, `app/src/lib/state/theme.ts`
- Test: `app/src/lib/ui/tokens.test.ts`, `app/src/lib/ui/ui.test.ts`

**Interfaces:**
- Produces `theme.ts`: `export type ThemePref = 'light'|'dark'|'system'; export function applyTheme(pref: ThemePref): void` (sets or removes `data-theme` on `<html>`).
- Produces `contrast(fg: string, bg: string): number` in `app/src/lib/ui/contrast.ts` (WCAG relative-luminance ratio).
- Component props (Svelte 5 `$props`): `Button {variant?: 'primary'|'quiet'|'danger'; type?; disabled?; onclick; children}`, `Field {label: string; hint?: string; error?: string; children}` (renders `<label for>` and links hint/error via `aria-describedby`), `Dialog {open: boolean; title: string; onclose: () => void; children; footer?}`, `Combobox<T> {label; items: T[]; value: T|null; text: string; onText: (s: string)=>void; onPick: (t:T)=>void; render: (t:T)=>string; createLabel?: (text:string)=>string; onCreate?: (text:string)=>void}`, `Toast` store API in Task 5.

- [ ] **Step 1: Write failing tests.**
  - `tokens.test.ts` parses `tokens.css` and, for light and dark, asserts `contrast(--ink-0, --paper-0) >= 7`, `contrast(--ink-1, --paper-1) >= 4.5`, `contrast(--ink-2, --paper-1) >= 4.5`, `contrast(--ribbon, --paper-1) >= 4.5`, `contrast(--on-ribbon, --ribbon) >= 4.5` (light), `contrast(--foxing, --foxing-wash) >= 4.5`, `contrast(--focus, --paper-1) >= 3`, `contrast(--rule, --paper-1) >= 1.3` (decorative only), and that every colour token in the Visual direction table exists in both themes.
  - `ui.test.ts`: `field_links_label_hint_error` (input has accessible name = label, `aria-describedby` includes hint id, `aria-invalid="true"` when error); `dialog_traps_focus_and_restores` (Tab cycles inside, Esc calls `onclose`, focus returns to the opener); `combobox_keyboard` (ArrowDown opens, Enter picks, type with no match shows the create row `createLabel(text)`, role `combobox` with `aria-expanded` and `aria-activedescendant`); `button_disabled_not_focusable_action`; `theme_applies_data_attribute` (`applyTheme('dark')` sets `data-theme="dark"`, `'system'` removes it); `reduced_motion_tokens` (CSS contains the `prefers-reduced-motion` block setting all `--dur-*` to `0ms`).
- [ ] **Step 2: Run** `npm --prefix app test -- ui tokens` -> FAIL.
- [ ] **Step 3: Implement** tokens exactly as in the Visual direction tables (light, `@media dark` guarded, `[data-theme=dark]`), base reset (`html{font-size:100%}`, body background `--paper-0`, `:focus-visible` ring 2px `--focus` offset 2px), and the components. `IndexCard` draws ruling with `repeating-linear-gradient(to bottom, transparent 0 27px, var(--rule) 27px 28px)`. `Ribbon` is an inline SVG with `role="img"` and a `title` prop.
- [ ] **Step 4: Run** the tests -> PASS. Also `npm --prefix app run lint` (a11y rules).
- [ ] **Step 5: Commit** `feat(app): reading-room design tokens, theme switching, base components`.

### Task 3: Api contract, Platform, MockApi, app context and state

**Files:**
- Create: `app/src/lib/api/{types.ts, api.ts, context.ts}`, `app/src/lib/api/mock/{MockApi.ts, fixtures.ts, mockParse.ts}`, `app/src/lib/state/{nav.svelte.ts, settings.svelte.ts, undo.svelte.ts}`
- Test: `app/src/lib/api/api.contract.ts` (exported `runApiContract(make: () => Promise<Api>)`), `app/src/lib/api/mock/MockApi.test.ts`

**Interfaces (this block is the contract every later task and Rust DTO follows; field names are camelCase on the wire; Rust uses `#[serde(rename_all = "camelCase")]`):**

```ts
// types.ts
export type Id = string;                       // UUID v7
export type DayOrder = 'dmy' | 'mdy';
export type NameOrder = 'given-first' | 'surname-first';
export type SexKind = 'male' | 'female' | 'intersex' | 'unknown' | 'other';
export interface PersonRef { id: Id; name: string; lifespan: string /* "1850-1921", "b. 1850", "" */;
  sex: SexKind; living: boolean; private: boolean }
export interface PersonRow extends PersonRef { surname: string; given: string; birth: string; death: string;
  sourceCount: number; changed: number /* unix ms */ }
export interface PersonQuery { sort: 'surname'|'given'|'birth'|'changed'; desc: boolean; text: string;
  living: 'any'|'living'|'deceased'; noSource: boolean; noBirthDate: boolean; privateOnly: boolean }
export interface Page<T> { total: number; offset: number; items: T[]; letters?: { letter: string; offset: number }[] }
export interface ApiError { code: 'not_found'|'invalid'|'conflict'|'io'|'no_project'|'stale'|'internal'; message: string }

export interface DateDto { raw: string; display: string; status: 'ok'|'phrase'|'unparsed'; sortMin: number|null; sortMax: number|null }
export interface DatePreview { raw: string; understoodAs: string; range: string|null; status: 'ok'|'phrase'|'unparsed'; warnings: string[] }
export interface PlaceOption { id: Id; name: string; full: string; kind: string|null }
export interface NameDto { id: Id; kind: 'birth'|'married'|'aka'|'religious'|'nickname'|'immigrant'|'professional'|'other';
  kindText: string|null; preferred: boolean; private: boolean; display: string; prefix: string|null; given: string|null;
  call: string|null; surnamePrefix: string|null; surnames: string[]; patronymic: string|null; suffix: string|null;
  nickname: string|null; lang: string|null; date: DateDto|null; citations: CitationDto[] }
export type EventKindId = string;   // 'birth' | 'death' | ... | 'custom:<text>' (Rust EventKind, lower_snake)
export interface ParticipantDto { person: PersonRef; role: string }
export interface EventDto { id: Id; kind: EventKindId; label: string; isFact: boolean; date: DateDto|null;
  place: PlaceOption|null; description: string|null; age: string|null; cause: string|null; negative: boolean;
  private: boolean; preferred: boolean; participants: ParticipantDto[]; citations: CitationDto[]; notes: NoteDto[]; media: MediaLinkDto[] }
export interface SourceDto { id: Id; title: string|null; author: string|null; publication: string|null;
  abbreviation: string|null; text: string|null; citationCount: number }
export interface CitationDto { id: Id; source: SourceDto|null; page: string|null; date: DateDto|null; text: string|null;
  quality: 0|1|2|3|null; evidence: { original: boolean|null; primaryInfo: boolean|null; direct: boolean|null } }
export interface NoteDto { id: Id; kind: 'general'|'story'|'research'|'transcription'|'conclusion'|'other'; text: string; shared: boolean; lang: string|null }
export interface MediaDto { id: Id; title: string|null; mime: string|null; originalName: string|null; date: DateDto|null;
  description: string|null; resolved: boolean /* file is in the project */; width: number|null; height: number|null; sha256: string|null }
export interface MediaLinkDto { media: MediaDto; primary: boolean }
export type ParentRelId = 'birth'|'adopted'|'step'|'foster'|'guardian'|'sealed'|'unknown'|'other';
export type UnionKindId = 'married'|'civil_union'|'unmarried'|'unknown'|'other';
export interface ParentSetDto { familyId: Id; parents: [PersonRef|null, PersonRef|null]; rel: [ParentRelId, ParentRelId]; preferred: boolean; summary: string|null }
export interface ChildDto { person: PersonRef; rel: [ParentRelId, ParentRelId]; birth: string }
export interface PartnershipDto { familyId: Id; partner: PersonRef|null; kind: UnionKindId; summary: string|null /* "m. 1 Jun 1878, Hamburg" */;
  events: EventDto[]; children: ChildDto[] }
export interface FamilyViewDto { focus: PersonRef; focusSummary: string; parentSets: ParentSetDto[]; partnerships: PartnershipDto[]; siblings: PersonRef[] }
export interface PersonDetail { ref: PersonRef; sex: SexKind; sexText: string|null; livingOverride: 'living'|'deceased'|null;
  livingComputed: boolean; private: boolean; names: NameDto[]; events: EventDto[]; parentSets: ParentSetDto[];
  partnerships: PartnershipDto[]; notes: NoteDto[]; media: MediaLinkDto[]; citations: CitationDto[]; changed: number }
export interface SearchHitDto { kind: 'person'|'place'|'source'|'note'; id: Id; label: string; snippet: string|null; person: PersonRef|null }
export interface RecentProject { dir: string; name: string; openedAt: number; missing: boolean }
export interface ProjectInfo { dir: string; name: string; personCount: number; homePerson: Id|null; sourceVersion: string|null; locationWarning: LocationWarningDto|null }
export interface LocationWarningDto { kind: 'synced_folder'|'network_share'; provider: string|null }
export interface Settings { dayOrder: DayOrder; nameOrder: NameOrder; theme: 'light'|'dark'|'system'; backupsToKeep: number;
  livingMaxAge: number /* project */; homePerson: Id|null /* project */ }
export interface ImportRequest { file: string; projectDir: string; projectName: string; mediaRoots: string[]; dayOrder: DayOrder }
export interface ImportReportDto { project: ProjectInfo; encoding: string; version: string|null; producer: string|null;
  counts: Record<string, number>; repairs: { kind: string; line: number|null; detail: string }[];
  unmappedTags: Record<string, number>; unparsedDates: { line: number; text: string }[]; missingMedia: string[]; truncated: boolean; text: string }
export interface ImportProgress { phase: 'reading'|'parsing'|'copying_media'|'saving'|'indexing'; done: number; total: number }
export interface UndoState { canUndo: boolean; canRedo: boolean; undoLabel: string|null; redoLabel: string|null }
export interface EditResult { label: string; focus: Id|null; touched: Id[]; undo: UndoState }
export interface HistoryEntry { ts: number; label: string; op: 'create'|'update'|'delete' }
export interface ExportOptions { version: '5.5.1'|'7.0'; living: 'exclude'|'placeholder'|'all'; includePrivate: boolean }
export interface PrivacyPreview { options: ExportOptions; defaultVersion: '5.5.1'|'7.0'; exportedPersons: number; placeholderPersons: number;
  hiddenPersons: number; removedRecords: number; mentionWarnings: { noteExcerpt: string; hiddenPerson: string }[];
  groupMedia: { title: string; originalName: string|null }[]; losslessNotes: string[]; token: string /* identifies this preview */ }
export interface ChartRequest { kind: 'pedigree'|'descendants'; root: Id; generations: number }
export interface ChartMeta { chartId: number; kind: 'pedigree'|'descendants'; bounds: { w: number; h: number }; nodeCount: number;
  rootNode: number; truncated: boolean; version: number }
export interface Rect { x: number; y: number; w: number; h: number }
export interface ChartNode { id: number; person: PersonRef|null; x: number; y: number; w: number; h: number; gen: number;
  ahnen: number|null; dupOf: number|null; moreUp: boolean; moreDown: boolean;
  nav: { parent: number|null; child: number|null; prev: number|null; next: number|null }; label: string }
export interface ChartEdge { from: number; to: number; kind: 'parent'|'partner'|'dup'; pts: number[] /* flat x,y pairs */ }
export interface ChartWindow { nodes: ChartNode[]; edges: ChartEdge[] }
export interface OutlineItem { nodeId: number; person: PersonRef|null; label: string /* "2 Father", "Child 3" */; hasChildren: boolean }
export interface ExifSuggestion { date: string|null; lat: number|null; lon: number|null }
export interface AttachResult { attached: MediaDto[]; failed: { path: string; reason: string }[] }
```

```ts
// EditCommand: discriminated union on `type`. Rust mirrors it as #[serde(tag = "type", rename_all = "camelCase")].
export type OwnerRef = { kind: 'person'|'family'|'name'|'event'|'media'|'citation'|'childLink'; id: Id };
export interface NameInput { id?: Id; kind: NameDto['kind']; kindText?: string; preferred: boolean; private: boolean; prefix?: string; given?: string;
  call?: string; surnamePrefix?: string; surnames: string[]; patronymic?: string; suffix?: string; nickname?: string; lang?: string; dateRaw?: string }
export interface EventInput { id?: Id; kind: EventKindId; dateRaw?: string; placeText?: string; placeId?: Id; description?: string; age?: string;
  cause?: string; negative: boolean; private: boolean; preferred: boolean; participants?: { person: Id; role: string }[] }
export interface PersonDraft { given?: string; surnames: string[]; sex: SexKind; birthDateRaw?: string; birthPlaceText?: string }
export interface CitationInput { source: Id|{ title: string }; page?: string; dateRaw?: string; text?: string; quality?: 0|1|2|3;
  evidence?: { original?: boolean; primaryInfo?: boolean; direct?: boolean } }
export type EditCommand =
 | { type: 'createPerson'; draft: PersonDraft }
 | { type: 'updatePerson'; id: Id; sex?: SexKind; sexText?: string; livingOverride?: 'living'|'deceased'|null; private?: boolean }
 | { type: 'deletePerson'; id: Id }
 | { type: 'saveName'; person: Id; name: NameInput } | { type: 'deleteName'; person: Id; name: Id }
 | { type: 'saveEvent'; owner: { kind: 'person'|'family'; id: Id }; event: EventInput } | { type: 'deleteEvent'; owner: { kind: 'person'|'family'; id: Id }; event: Id }
 | { type: 'quickAdd'; anchor: Id; relation: 'father'|'mother'|'partner'|'child'|'sibling'; draft: PersonDraft; family?: Id; newParentSet?: boolean }
 | { type: 'saveFamily'; id?: Id; partner1: Id|null; partner2: Id|null; kind: UnionKindId }
 | { type: 'deleteFamily'; id: Id }
 | { type: 'linkChild'; family: Id; child: Id; rel: [ParentRelId, ParentRelId]; preferred: boolean }
 | { type: 'updateChildLink'; family: Id; child: Id; rel: [ParentRelId, ParentRelId]; preferred: boolean }
 | { type: 'unlinkChild'; family: Id; child: Id }
 | { type: 'reorderChildren'; family: Id; order: Id[] }
 | { type: 'saveNote'; target: OwnerRef; note: { id?: Id; kind: NoteDto['kind']; text: string } } | { type: 'deleteNote'; target: OwnerRef; note: Id }
 | { type: 'saveSource'; source: { id?: Id; title?: string; author?: string; publication?: string; abbreviation?: string; text?: string } }
 | { type: 'addCitation'; target: OwnerRef; citation: CitationInput | { reuseLast: true } | { cloneOf: Id } }
 | { type: 'updateCitation'; id: Id; citation: Omit<CitationInput,'source'> & { source?: Id } }
 | { type: 'removeCitation'; target: OwnerRef; citation: Id }
 | { type: 'attachMedia'; target: OwnerRef; media: Id[] } | { type: 'detachMedia'; target: OwnerRef; media: Id }
 | { type: 'setPrimaryMedia'; person: Id; media: Id } | { type: 'updateMedia'; id: Id; title?: string; description?: string; dateRaw?: string };

export interface Api {
  recentProjects(): Promise<RecentProject[]>;
  createProject(dir: string, name: string): Promise<ProjectInfo>;
  openProject(dir: string): Promise<ProjectInfo>;
  closeProject(): Promise<void>;
  saveCopy(dest: string): Promise<void>;
  checkLocation(dir: string): Promise<LocationWarningDto | null>;
  importGedcom(req: ImportRequest, onProgress?: (p: ImportProgress) => void): Promise<ImportReportDto>;
  saveImportReport(dest: string): Promise<void>;
  discardImport(): Promise<void>;                 // deletes the project created by the last import (only before the first edit)
  relinkMedia(folder: string): Promise<{ relinked: number; stillMissing: string[] }>;
  listPersons(q: PersonQuery, offset: number, limit: number): Promise<Page<PersonRow>>;
  getPerson(id: Id): Promise<PersonDetail>;
  getFamilyView(id: Id): Promise<FamilyViewDto>;
  search(query: string, limit: number): Promise<SearchHitDto[]>;
  apply(cmd: EditCommand): Promise<EditResult>;
  undo(): Promise<EditResult | null>; redo(): Promise<EditResult | null>; undoState(): Promise<UndoState>;
  history(record: Id, limit: number): Promise<HistoryEntry[]>;
  parseDate(input: string, order: DayOrder): Promise<DatePreview>;
  suggestPlaces(prefix: string, limit: number): Promise<PlaceOption[]>;
  suggestSources(prefix: string, limit: number): Promise<SourceDto[]>;
  lastCitation(): Promise<CitationDto | null>;
  chartOpen(req: ChartRequest): Promise<ChartMeta>;
  chartWindow(chartId: number, rect: Rect, zoom: number): Promise<ChartWindow>;
  chartOutline(chartId: number, parentNode: number | null, limit: number): Promise<OutlineItem[]>;
  chartClose(chartId: number): Promise<void>;
  mediaUrl(id: Id, size: 'thumb' | 'preview' | 'original'): string;           // synchronous: a gtmedia: URL
  exifSuggestion(media: Id): Promise<ExifSuggestion | null>;
  attachFiles(target: OwnerRef, paths: string[]): Promise<AttachResult>;      // add_media_file + Media record + link, one undo step
  previewExport(opts: ExportOptions): Promise<PrivacyPreview>;
  exportGedcom(opts: ExportOptions, dest: string, previewToken: string): Promise<{ bytes: number }>;
  getSettings(): Promise<Settings>; setSettings(patch: Partial<Settings>): Promise<Settings>;
}
export interface Platform {                        // OS dialogs and drag-drop; separate from Api so Mock can fake it
  pickFile(opts: { title: string; extensions?: string[] }): Promise<string | null>;
  pickFiles(opts: { title: string; extensions?: string[] }): Promise<string[]>;
  pickFolder(title: string): Promise<string | null>;
  pickSavePath(opts: { title: string; defaultName: string; extensions: string[] }): Promise<string | null>;
  onFilesDropped(cb: (paths: string[]) => void, onHover?: (over: boolean) => void): () => void;
  openExternally(path: string): Promise<void>;
  reveal(path: string): Promise<void>;
}
// context.ts: setApi(api, platform) / getApi() / getPlatform() via Svelte context; `defaultApi()` returns TauriApi when
// `'__TAURI_INTERNALS__' in window`, else MockApi.
```

State modules (`.svelte.ts`, runes): `nav` = `{ route: Route; focusPerson: Id|null; history: Route[]; go(r: Route): void; back(): void; forward(): void }` with `type Route = {name:'welcome'}|{name:'importReport'}|{name:'family'}|{name:'people'}|{name:'pedigree'}|{name:'descendants'}|{name:'person', id:Id}|{name:'sources'}|{name:'media'}|{name:'settings'}`; `settings` store loads via `api.getSettings` and calls `applyTheme`; `undoState` store refreshed from every `EditResult`.

- [ ] **Step 1: Write failing tests.**
  - `MockApi.test.ts` calls `runApiContract(async () => new MockApi(fixtureTree(500)))`. The contract asserts: `list_paging` (total 500, `listPersons(q, 0, 200)` returns 200 items, `offset 400` returns 100, limit above 200 is clamped to 200); `list_sort_surname` (items sorted by `surname` with `localeCompare`); `list_filter_living` (every item `living === true`); `letters_cover_total` (letters offsets strictly increasing); `family_view_roundtrip` (`getFamilyView(id).focus.id === id`); `apply_then_undo` (`createPerson` then `listPersons` total +1, `undo()` returns total to 500, `redo()` +1, and `undoState().canRedo` toggles); `edit_clears_redo`; `parse_date_dmy_vs_mdy` (`parseDate('03/04/1901','dmy').understoodAs` contains `3 Apr 1901`; `'mdy'` contains `4 Mar 1901`); `parse_date_unparsed_keeps_raw` (`status === 'unparsed'`, `raw` unchanged); `search_diacritics` (`search('muller')` finds `Müller`); `search_limit`; `chart_window_returns_only_visible` (nodes all intersect the rect); `unknown_id_rejects` (`getPerson('x')` rejects with `code: 'not_found'`).
  - `nav.test.ts`: `back_forward_restores_routes`, `focus_person_follows_person_routes`.
- [ ] **Step 2: Run** `npm --prefix app test -- api nav` -> FAIL.
- [ ] **Step 3: Implement** types exactly as above; `MockApi` over a seeded generator `fixtureTree(n, seed=1)` in `fixtures.ts` that builds a plausible multi-generation tree (couples, 0-6 children, mixed living/deceased, 8% with Cyrillic/Hebrew/CJK names, a few private, a few without sources). The generator also includes the hand-written cases: a person with 12 partners, a pedigree cycle (A parent of B parent of A) and an undated person. `mockParse.ts` implements a small subset of `GenDate::parse` (exact dates, `abt`, `bef`, `aft`, `between..and`, `d/m/y` with order, phrase in parentheses, else unparsed). `MockApi.apply` implements every `EditCommand` variant with the semantics in Task 21 over plain objects; undo is a stack of inverse snapshots. `MockApi.chartOpen/chartWindow/chartOutline` use `mockLayout.ts` (a simple generation grid / indented layout; not Walker) so Tasks 14 and 16 can run.
- [ ] **Step 4: Run** the tests -> PASS; `npm run check` clean.
- [ ] **Step 5: Commit** `feat(app): Api contract, Platform, MockApi, nav/settings/undo state`.

### Task 4: Canvas chart engine and 10k-node spike harness (browser)

**Files:**
- Create: `app/src/lib/chart/{camera.ts, renderer.ts, window.ts, hit.ts, synth.ts}`, `app/src/spike/Spike.svelte`, `app/spike.html`
- Test: `app/src/lib/chart/{camera,renderer,window,hit}.test.ts`

**Interfaces:**
- Produces:
```ts
// camera.ts
export interface Camera { x: number; y: number; zoom: number }            // world point at the viewport's top-left, zoom = px per world unit
export const ZOOM_MIN = 0.1, ZOOM_MAX = 2.5;
export function zoomAt(c: Camera, factor: number, anchorPx: {x:number;y:number}): Camera;   // keeps the world point under anchorPx fixed, clamps zoom
export function panBy(c: Camera, dxPx: number, dyPx: number, bounds: {w:number;h:number}, view: {w:number;h:number}): Camera; // clamps so >= 25% of bounds stays visible
export function viewRect(c: Camera, view: {w:number;h:number}): Rect;
export function centerOn(c: Camera, node: Rect, view: {w:number;h:number}): Camera;
export function tween(from: Camera, to: Camera, ms: number, step: (c: Camera) => void, reduced: boolean): { cancel(): void };
// renderer.ts
export type Lod = 'full' | 'boxes' | 'dots';            // zoom >= 0.55 full, >= 0.25 boxes (no text), else dots
export function lodFor(zoom: number): Lod;
export interface Theme { paper: string; rule: string; ink: string; ink2: string; ribbon: string; wash: string; font: string; serif: string }
export function draw(ctx: CanvasRenderingContext2D, win: ChartWindow, cam: Camera, view: {w:number;h:number;dpr:number},
                     opts: { selected: number|null; focus: number|null; theme: Theme }): { drawn: number };
// window.ts — client-side window cache so panning never calls IPC per frame
export class WindowCache { constructor(fetch: (r: Rect, zoom: number) => Promise<ChartWindow>);
  need(view: Rect, zoom: number): void;           // fetches view inflated by 100% each side when view leaves the cached rect, or lodFor changes
  current(): { win: ChartWindow; rect: Rect } | null; onChange(cb: () => void): void; dispose(): void }
// hit.ts
export function hitTest(win: ChartWindow, worldX: number, worldY: number): ChartNode | null;
export function neighbor(win: ChartWindow, from: number, dir: 'up'|'down'|'left'|'right'|'parent'|'child'): number | null;
// synth.ts — spike data, no Api involved
export function synthChart(n: number, seed?: number): { meta: ChartMeta; all: ChartNode[]; edges: ChartEdge[]; query(r: Rect): ChartWindow };
```
- Spike harness `Spike.svelte` (mode `VITE_GT_MODE=spike`): builds `synthChart(10000)`, scripted camera path for 10 s (zoom oscillating 0.15..1.5 with period 6 s plus a Lissajous pan), records one `performance.now()` delta per `requestAnimationFrame`, then shows and logs `{ n, frames, medianMs, p95Ms, p99Ms, over25ms, dpr, ua }`. In a Tauri window it also calls `invoke('spike_report', { json })` if available (Task 17).

- [ ] **Step 1: Write failing tests.**
  - `camera.test.ts`: `zoom_keeps_anchor_fixed` (world point under the anchor is identical before and after, within 1e-6); `zoom_clamped` (zoom never < 0.1 or > 2.5); `pan_keeps_content_in_reach`; `center_on_puts_node_in_middle`; `tween_reduced_motion_jumps` (with `reduced=true` the step callback is called once with the target).
  - `renderer.test.ts` with a `RecordingContext` (counts `fillText`, `fillRect`, `stroke`, `arc`): `lod_thresholds` (`lodFor(0.6)==='full'`, `0.3==='boxes'`, `0.2==='dots'`); `no_text_below_0_55` (zoom 0.3 -> zero `fillText` calls); `draws_only_given_window` (`drawn === win.nodes.length`); `selected_node_has_ribbon_stroke`; `edges_batched` (50 edges of the same kind produce one `beginPath`/`stroke` pair).
  - `window.test.ts`: `no_refetch_while_inside` (100 `need()` calls with views inside the cached rect -> `fetch` called once); `refetch_when_leaving`; `refetch_on_lod_change`; `stale_response_dropped` (two overlapping fetches resolve in reverse order -> `current()` is the newer one).
  - `hit.test.ts`: `hit_node_and_miss`, `neighbor_follows_nav_links`.
- [ ] **Step 2: Run** `npm --prefix app test -- chart` -> FAIL.
- [ ] **Step 3: Implement.** `draw` must: reset transform to `setTransform(dpr*zoom, 0, 0, dpr*zoom, -x*dpr*zoom, -y*dpr*zoom)`; draw edges first, batched per kind into one `Path2D`; node boxes as one batched path for `boxes`/`dots`; text only in `full` (name in serif 600 at 13 px, lifespan in sans 11 px, Ahnentafel number small at top-right when present, ribbon stroke on `selected`); never allocate per node per frame beyond the draw call. `WindowCache` keeps at most 2 in-flight fetches. `synthChart` is a balanced binary tree for pedigree-like density with deterministic names from a seeded PRNG.
- [ ] **Step 4: Run** the tests -> PASS. Then `npm --prefix app run spike` in Edge/Chrome, note the numbers in the commit message (informational only; the gate is Task 17 on real webviews).
- [ ] **Step 5: Commit** `feat(app): canvas chart engine (camera, LOD renderer, window cache) and fps harness`.

### Task 5: App shell, routing, shortcuts, global search, toasts, undo UI

**Files:**
- Create: `app/src/lib/views/Shell.svelte`, `app/src/lib/views/ShortcutSheet.svelte`, `app/src/lib/ui/SearchPalette.svelte`, `app/src/lib/state/{shortcuts.ts, toast.svelte.ts}`
- Modify: `app/src/App.svelte` (provides api/platform context, renders Welcome or Shell by `nav.route`)
- Test: `app/src/lib/views/Shell.test.ts`, `app/src/lib/state/shortcuts.test.ts`

**Interfaces:**
- Produces:
```ts
// shortcuts.ts
export interface Shortcut { id: string; keys: string /* 'Mod+K', 'Mod+Shift+Z', 'Alt+ArrowLeft', 'g p' (sequence) */; label: string; run: () => void; when?: () => boolean }
export function registerShortcuts(list: Shortcut[]): () => void;   // returns unregister; 'Mod' = Ctrl on Windows/Linux, Cmd on macOS
export function formatKeys(keys: string, platform: 'mac'|'other'): string;
// toast.svelte.ts
export const toasts: { items: {id:number; text:string; action?: {label:string; run:()=>void}; kind:'ok'|'warn'|'error'}[]; push(t: Omit<Item,'id'>): void; dismiss(id:number): void };
```
- Shortcut table (registered by Shell): `Mod+K` open search; `/` focus current list filter (when a list is shown); `Mod+Z` undo; `Mod+Shift+Z` and `Mod+Y` redo; `Mod+N` new person dialog; `Mod+E` open the focus person's editor; `Mod+Home` go to home person; `Alt+ArrowLeft` / `Alt+ArrowRight` nav back / forward; `Mod+1`..`Mod+5` Family, People, Pedigree, Descendants, Sources; `Mod+,` Settings; `Mod+S` toasts "All changes are saved" (edits persist immediately); `?` opens the shortcut sheet. Shortcuts never fire while focus is in a text input, except `Mod+K`, `Mod+Z` (when the field is not being edited by the browser's own undo, i.e. the app handles it only if `activeElement` is not text-editable), and `Escape`.
- Sidebar items (with icons, `nav` landmark labelled "Main"): Family, People, Pedigree, Descendants, Sources, Media, Export (opens dialog), Settings; footer shows project name, person count and a "Saved" indicator (`role="status"`).

- [ ] **Step 1: Write failing tests.**
  - `shortcuts.test.ts`: `mod_maps_to_ctrl_on_windows_cmd_on_mac`; `sequence_g_p_navigates`; `ignored_inside_text_input` (typing `?` in an input does not open the sheet); `unregister_removes`; `formatKeys('Mod+Shift+Z','mac') === '⌘⇧Z'` and `'Ctrl+Shift+Z'` otherwise.
  - `Shell.test.ts` (MockApi): `sidebar_navigates_and_marks_current` (clicking "People" sets `aria-current="page"` and renders the people view); `ctrl_k_opens_palette_and_enter_opens_person` (type `Smith`, `await` results, ArrowDown, Enter -> route is `family` with `focusPerson` set); `palette_debounced_and_limited` (typing 5 chars quickly calls `api.search` once with limit 20); `palette_empty_state_text` ("No matches. Try a different spelling; search also finds similar-sounding names."); `undo_via_keyboard_and_toast` (after an edit a toast "… saved" with an Undo button; `Mod+Z` calls `api.undo`; sidebar footer shows "Saved"); `shortcut_sheet_lists_all` (every row of the shortcut table appears).
- [ ] **Step 2: Run** `npm --prefix app test -- Shell shortcuts` -> FAIL.
- [ ] **Step 3: Implement.** The palette is a `Dialog` with a `Combobox`-style listbox; results grouped People / Places / Sources / Notes using `SearchHitDto.kind`; Enter on a person sets `nav.focusPerson` and routes to `family`. Toasts live in a `role="status"` region (`aria-live="polite"`), auto-dismiss after 6 s, paused on hover or focus.
- [ ] **Step 4: Run** the tests -> PASS.
- [ ] **Step 5: Commit** `feat(app): shell, routing, keyboard shortcuts, global search palette, toasts`.

### Task 6: Welcome screen and import report screen

**Files:**
- Create: `app/src/lib/views/{Welcome,ImportReport}.svelte`, `app/src/lib/views/ImportWizard.svelte`
- Test: `app/src/lib/views/{Welcome,ImportReport}.test.ts`

**Interfaces:**
- Consumes: Task 3 `Api.recentProjects/createProject/openProject/importGedcom/saveImportReport/discardImport/relinkMedia/checkLocation`, `Platform.pickFile/pickFolder/pickSavePath`.
- Produces: `Welcome.svelte` (three primary actions: "Import a GEDCOM file", "Create a new tree", "Open a tree"; below them the recent list with name, folder and last opened; a missing folder shows a `Banner` "This folder is not there any more" and a Remove button), `ImportWizard.svelte` (choose file, choose project name and folder, choose date order DMY/MDY with the sentence "Used only for dates like 03/04/1901", optional media folders list, progress bar driven by `ImportProgress`), `ImportReport.svelte`.
- Import report layout: heading "Imported {n} people from {producer}"; summary table of `counts`; sections (each a native `<details>` that is open when non-empty): "Repairs" (grouped by `kind`, each with line number), "Tags we kept but did not understand" (`unmappedTags`), "Dates we could not read" (`unparsedDates`, text shown verbatim in `<bdi>`), "Photos and files we could not find" (`missingMedia`, with "Locate media folder" button -> `Platform.pickFolder` -> `api.relinkMedia` -> toast "Linked {n} files; {m} still missing"); a `Banner kind="warning"` when `project.locationWarning` is set: "This folder is inside {provider}. Syncing can corrupt the database. Keep the tree outside synced folders and use backups to move it."; buttons "Save report…" (`saveImportReport`), "Undo import" (confirm Dialog "Delete the imported tree? Your GEDCOM file is not touched." -> `discardImport` -> Welcome) and primary "Open the tree" (-> `family` for `project.homePerson` or the first person).

- [ ] **Step 1: Write failing tests.**
  - `welcome_lists_recent_and_actions` (3 actions are buttons; recent items are links with accessible name containing project name and folder).
  - `welcome_empty_state` (no recents: text "No trees yet. Import a GEDCOM file to start, or create an empty tree.").
  - `missing_recent_shows_remove`.
  - `import_flow_reports_progress` (wizard: pick file -> name -> start; a `MockApi.importGedcom` that emits 3 phases advances the `progressbar` `aria-valuenow`; ends on the report).
  - `report_sections_hidden_when_empty` (a report with no repairs has no "Repairs" `details`).
  - `report_shows_location_warning` (warning text includes provider "OneDrive").
  - `report_unparsed_dates_verbatim_rtl` (a date text `ט״ו באב` renders inside `bdi`).
  - `locate_media_updates_list` (relink reports 2 linked, 1 missing -> list shows 1, toast text "Linked 2 files; 1 still missing").
  - `undo_import_confirms` (dialog first; cancel leaves state; confirm calls `discardImport` once).
  - `empty_tree_opens_to_add_first_person` (a project with `personCount === 0` routes to People showing "This tree is empty. Add the first person." with a primary "Add person" button).
- [ ] **Step 2: Run** `npm --prefix app test -- Welcome ImportReport` -> FAIL.
- [ ] **Step 3: Implement** with `MockApi.importGedcom` returning a canned report from a fixture (including a location warning, 3 repairs, 2 unmapped tags, 1 unparsed date, 2 missing media).
- [ ] **Step 4: Run** -> PASS.
- [ ] **Step 5: Commit** `feat(app): welcome screen, import wizard and import report`.

### Task 7: Virtualised person list with filters

**Files:**
- Create: `app/src/lib/ui/VirtualList.svelte`, `app/src/lib/views/People.svelte`, `app/src/lib/views/PeopleFilters.svelte`, `app/src/lib/ui/PersonRow.svelte`
- Test: `app/src/lib/ui/VirtualList.test.ts`, `app/src/lib/views/People.test.ts`

**Interfaces:**
- Produces:
```ts
// VirtualList.svelte props
{ total: number; rowHeight: number /* 40 */; pageSize: number /* 200 */;
  loadPage: (offset: number, limit: number) => Promise<unknown[]>;   // called per missing page
  row: Snippet<[item: unknown | null, index: number, selected: boolean]>; // null while loading (render a static placeholder)
  selectedIndex: number; onSelect: (i: number) => void; onActivate: (i: number) => void; ariaLabel: string; keyPrefix: string }
```
Behaviour: renders only `ceil(viewport/rowHeight) + 10` rows; spacer height `total*rowHeight`; keeps an LRU of 20 pages; `role="listbox"` with `aria-rowcount={total}` and `aria-rowindex` on rows; ArrowUp/Down/PageUp/PageDown/Home/End move selection and scroll it into view; Enter calls `onActivate`.
- `People.svelte`: toolbar (filter input with label "Filter by name", Living segmented control Any/Living/Deceased, checkboxes "No source", "No birth date", "Private only", sort Select Surname/Given name/Birth/Recently changed with a direction toggle), the A-Z rail built from `Page.letters` (buttons "A".."Z" plus "Other scripts"; clicking scrolls to `offset`), the list, and a status line "{total} people". Row: `<bdi dir="auto">` name, lifespan, a verdigris dot plus visually hidden "Living", a `Ribbon` on the home person, source-count badge. Double-click or Enter -> `nav.go({name:'family'})` with `focusPerson`; `Mod+E` -> editor; `Delete` -> confirm Dialog then `deletePerson` (toast with Undo).
- Filter changes are debounced 150 ms and reset the scroll to 0.

- [ ] **Step 1: Write failing tests.**
  - `VirtualList.test.ts`: `renders_window_not_all` (total 50000 -> DOM rows < 60); `loads_pages_on_scroll` (set `scrollTop = 40*1000` -> `loadPage` called with offset 800 or 1000 and limit 200, not for offset 0 again); `lru_evicts` (21 distinct pages -> first page requested again on return); `placeholder_while_loading` (row snippet receives `null`, row has `data-loading` and opacity class); `keyboard_navigation` (End -> `selectedIndex === total-1`); `aria_counts` (`aria-rowcount="50000"`).
  - `People.test.ts` (MockApi, 5000 people): `initial_page_shows_first_surnames_sorted`; `filter_text_debounced_single_call`; `living_filter_only_living`; `letter_rail_jumps` (click "M" -> first visible row surname starts with M); `rtl_and_cjk_names_render_in_bdi_in_order` (the Hebrew, Cyrillic, CJK fixtures appear inside `bdi[dir="auto"]`, and the order of `listPersons` is preserved, not re-sorted in the UI); `other_scripts_bucket_exists`; `empty_filter_result_text` ("Nobody matches these filters. Clear filters"); `delete_asks_then_toasts_undo`; `home_person_has_ribbon_with_label` (`img` named "Home person").
- [ ] **Step 2: Run** -> FAIL. **Step 3: Implement.** **Step 4: Run** -> PASS.
- [ ] **Step 5: Commit** `feat(app): virtualised paged person list with filters and A-Z rail`.

### Task 8: Family view (home screen)

**Files:**
- Create: `app/src/lib/views/FamilyView.svelte`, `app/src/lib/ui/{PersonCard,IndexCard}.svelte` (extend), `app/src/lib/views/flip.ts`
- Test: `app/src/lib/views/FamilyView.test.ts`, `app/src/lib/views/flip.test.ts`

**Interfaces:**
- Consumes: `Api.getFamilyView`, `nav.focusPerson`.
- Produces: `flip.ts`: `export function flipMove(container: HTMLElement, before: Map<string, DOMRect>, ms: number, reduced: boolean): void` (animates elements with `data-flip-key` from their previous rects to their new ones via Web Animations API; no-op when `reduced`).
- Layout: parents row (one `PersonCard` per parent slot; parent sets beyond the preferred are collapsed under "Other parents ({n})"; relationship chips "Adopted", "Step", "Foster", "Guardian", "Sealed" appear on the card, "Birth" shows nothing), the **focus index card** (name in `--t-2xl`, lifespan, key facts from `focusSummary`, buttons "Edit person" and the "Add" menu), partnerships (each partner card with union summary text; selecting a partner shows that union's children), children row in birth order with `birth` text, siblings list under a disclosure. Clicking any card calls `nav.focusPerson = id` and re-centres with `flipMove` (260 ms, or immediate under reduced motion). Cards are real links/buttons with accessible name `"{name}, {lifespan}, {relationship}"`.
- The "Add" menu (`QuickAdd` entry points) lists: Add father, Add mother, Add partner, Add child, Add sibling (Task 9 supplies the dialog; here it emits `onQuickAdd(relation)`); an item is disabled with a reason tooltip where it cannot apply (e.g. father/mother when both slots of the preferred parent set are filled -> label becomes "Add another set of parents").

- [ ] **Step 1: Write failing tests.**
  - `shows_parents_partners_children` (fixture person with 2 parents, 1 partner, 3 children: counts of cards by role region `aria-label` "Parents", "Partners", "Children").
  - `unknown_parent_slot_shows_add_prompt` ("Father not recorded" button "Add father").
  - `click_card_refocuses_and_loads` (clicking a child calls `getFamilyView(child.id)` and the focus card heading changes; `flipMove` called once with `reduced=false`).
  - `reduced_motion_skips_flip` (matchMedia reduce -> `flipMove` receives `reduced=true`, no `element.animate` calls).
  - `many_partners_and_children_do_not_break` (12 partnerships, one with 40 children: renders, partner selector is a listbox, children region has `role="list"` with 40 items, no horizontal page scroll style).
  - `cycle_fixture_renders` (person who is own ancestor: view loads).
  - `rtl_name_in_bdi` (`יעקב כהן` inside `bdi`).
  - `living_flag_text_not_colour` (living card contains visible text "Living").
  - `adopted_chip_present` and `private_person_shows_lock_label` (`img` named "Private").
  - `empty_tree_state` (no focus person: "No one to show. Add the first person." with action).
- [ ] **Step 2: Run** -> FAIL. **Step 3: Implement.** **Step 4: Run** -> PASS.
- [ ] **Step 5: Commit** `feat(app): family view home screen with animated re-centring`.

### Task 9: Person editor and quick add

**Files:**
- Create: `app/src/lib/views/PersonEditor.svelte`, `app/src/lib/editors/{NameEditor,NoteEditor,QuickAdd}.svelte`
- Test: `app/src/lib/views/PersonEditor.test.ts`, `app/src/lib/editors/QuickAdd.test.ts`

**Interfaces:**
- Consumes: `Api.getPerson/apply`, EditCommands `updatePerson`, `saveName`, `deleteName`, `saveNote`, `deleteNote`, `quickAdd`, `createPerson`, `deletePerson`.
- Produces `PersonEditor.svelte` (route `{name:'person', id}`): a header index card (name, lifespan, Ribbon toggle "Make home person" -> `setSettings({homePerson})`), then sections as landmarks with headings: Names (list, "Add name", dialog `NameEditor`), Basics (Sex Select with "Other" free-text, Living override Select "Computed ({living|deceased}) / Treat as living / Treat as deceased", Private checkbox), Events and facts (list from `PersonDetail.events` with "Add event" -> Task 10 dialog), Relationships (parents, partners, children with "Edit family" -> Task 11), Notes (`NoteEditor`: Markdown textarea, kind Select, 68ch measure, ruled background), Media (Task 13), Sources (Task 12), and History (disclosure listing `api.history`). Simple fields (sex, override, private) commit on change and toast "{Field} saved" with Undo; complex records open in a `Dialog` that saves on Enter/"Save {thing}" and discards on Esc (asks "Discard changes?" only if dirty).
- `QuickAdd.svelte` props: `{ anchor: PersonRef; relation: 'father'|'mother'|'partner'|'child'|'sibling'; family?: Id; onDone: (r: EditResult) => void; onCancel: () => void }`. Dialog title "Add {relation} of {anchor.name}" with fields Given name, Surname(s) (prefilled: child/sibling/father get the anchor's surname; mother/partner blank), Sex (prefilled: Male for father, Female for mother, Unknown otherwise), Birth date (`DateField`, Task 10) and Birth place. On save: one `quickAdd` command; on success, navigate focus to the new person only when "Open new person afterwards" is checked (default off).
- When the anchor has several partnerships and relation is `child`, a required Select "With which partner?" appears (mapped to `family`).

- [ ] **Step 1: Write failing tests.**
  - `PersonEditor.test.ts`: `sex_change_commits_one_command` (`api.apply` receives `{type:'updatePerson', id, sex:'female'}` exactly once, toast "Sex saved" with Undo); `living_override_options_show_computed` ("Computed (living)"); `name_dialog_roundtrip` (add a Married name with prefix and two surnames -> `saveName` payload has `surnames:['Müller','Schmidt']`, `kind:'married'`); `discard_prompt_only_when_dirty`; `delete_person_requires_confirmation_and_lists_consequences` (dialog text: "This removes {name} from {n} families and deletes {m} events only they took part in."); `make_home_person_sets_setting` (ribbon toggle, `setSettings({homePerson:id})`); `note_markdown_saved_verbatim` (text with `genetree://person/<id>` link unchanged); `history_lists_entries`; `save_error_is_shown_not_swallowed` (rejecting `apply` with `{code:'io'}` shows a Banner with the message and keeps dialog content).
  - `QuickAdd.test.ts`: `father_prefills_surname_and_sex`; `mother_prefills_female`; `child_with_two_partnerships_requires_choice` (Save disabled until chosen; `family` set in command); `sibling_without_parents_creates_command_with_no_family` (command has `relation:'sibling'`, no `family`); `submit_with_enter`; `empty_name_allowed_but_warns` ("No name entered: saved as Unknown" - an unnamed person is valid in genealogy); `cancel_does_not_call_api`.
- [ ] **Step 2: Run** -> FAIL. **Step 3: Implement.** **Step 4: Run** -> PASS.
- [ ] **Step 5: Commit** `feat(app): person editor, name/note editors and quick add`.

### Task 10: Event editor, live date preview and place picker

**Files:**
- Create: `app/src/lib/editors/{EventEditor,DateField,PlacePicker}.svelte`
- Test: `app/src/lib/editors/{DateField,PlacePicker,EventEditor}.test.ts`

**Interfaces:**
- Consumes: `Api.parseDate/suggestPlaces/apply` (`saveEvent`, `deleteEvent`), `settings.dayOrder`.
- Produces:
```ts
// DateField.svelte props
{ label: string; value: string; onChange: (raw: string) => void; onPreview?: (p: DatePreview) => void; order: DayOrder }
```
It shows under the input a `role="status"` line: `Understood as: {understoodAs}` (+ ` ({range})` when present); for `phrase`: `Saved as a phrase: "{raw}"`; for `unparsed`: amber `Not understood as a date. It will be saved exactly as you typed it.` (with icon, never blocks saving). Debounce 80 ms; ignores out-of-order responses.
- `PlacePicker.svelte` props `{ label; value: PlaceOption|null; text: string; onChange: (v: {placeId?: Id; placeText?: string}|null) => void }`; uses `Combobox`; typeahead calls `suggestPlaces(prefix, 8)`; when no exact match, last row is `Create "{text}"` (place hierarchy typed with commas, "Hamburg, Germany", is created as a chain on save); shows `full` chain under each option.
- `EventEditor.svelte` props `{ owner: {kind:'person'|'family'; id:Id}; event?: EventDto; onSaved: (r: EditResult) => void; onCancel: () => void }`: Kind Select (grouped: Life — birth, christening, baptism, death, burial, cremation; Family — marriage, divorce, annulment, engagement, separation; Facts — occupation, residence, education, religion, nationality, title, census, emigration, immigration, naturalization, probate, will, military, plus "Other…" free text -> `custom:<text>`), `DateField`, `PlacePicker`, Description, Age, Cause (cause only for death/burial kinds), Participants table (person typeahead via `search`, Role Select: principal, witness, godparent, clergy, officiator, informant, parent, father, mother, child, spouse, friend, neighbor, other), checkboxes "This event did not happen" (`negative`, helper text "Records that the person never married, had no children, and so on"), "Private", "Preferred when there are several", and the Citations section (Task 12).

- [ ] **Step 1: Write failing tests.**
  - `DateField.test.ts`: `shows_understood_as_live` (typing `abt 1850` shows "Understood as: about 1850"); `dmy_vs_mdy_changes_preview` (`03/04/1901` -> "3 Apr 1901" under dmy, "4 Mar 1901" under mdy); `unparsed_is_saved_verbatim_with_note` (input `sometime in the 1850s?` -> status line contains "saved exactly as you typed it", `onChange` receives the raw string unchanged); `phrase_in_parentheses`; `out_of_order_responses_ignored`; `status_is_polite_live_region` (`aria-live="polite"`).
  - `PlacePicker.test.ts`: `typeahead_lists_matches_with_chain`; `create_row_when_no_match`; `create_returns_place_text` (`onChange({placeText:'Hamburg, Germany'})`); `keyboard_only_flow`.
  - `EventEditor.test.ts`: `birth_event_saves_expected_command` (`saveEvent` payload: `kind:'birth'`, `dateRaw:'abt 1850'`, `placeText:'Berlin, Brandenburg, Germany'`, `negative:false`); `cause_hidden_for_birth`; `custom_kind_prefix` (`'custom:Baptism of adult'`); `negative_checkbox_helper_text`; `participant_role_defaults_to_witness`; `enter_saves_esc_cancels`; `editing_existing_event_keeps_id`.
- [ ] **Step 2: Run** -> FAIL. **Step 3: Implement.** **Step 4: Run** -> PASS.
- [ ] **Step 5: Commit** `feat(app): event editor with live date preview and place picker`.

### Task 11: Family editor and relationships

**Files:**
- Create: `app/src/lib/editors/FamilyEditor.svelte`, `app/src/lib/editors/PersonPicker.svelte`
- Test: `app/src/lib/editors/FamilyEditor.test.ts`

**Interfaces:**
- Consumes: `Api.getPerson`, `search`, EditCommands `saveFamily`, `deleteFamily`, `linkChild`, `updateChildLink`, `unlinkChild`, `reorderChildren`, `saveEvent` (owner `family`).
- Produces `FamilyEditor.svelte` props `{ family?: Id /* edit */; seedPartner?: Id; onDone: (r: EditResult) => void; onCancel: () => void }`: two unordered partner slots (each a `PersonPicker`, either may be empty, helper "Leave a slot empty when the partner is unknown"), union type Select (married, civil union, unmarried, unknown, other), union events list (divorce, annulment, separation are events, not flags; "Add event" opens `EventEditor` with `owner.kind='family'`), children list in order with per-parent relationship Selects (birth, adopted, step, foster, guardian, sealed, unknown, other; two selects labelled "Relationship to {parent 1 name}" and "Relationship to {parent 2 name}"), "Preferred parents for this child" checkbox, drag handle plus keyboard reordering (`Alt+ArrowUp/Down` on a focused row), "Add existing person as child" (`PersonPicker`) and "Add new child" (`QuickAdd` with `family`), and "Delete family" (confirm; children are kept).
- `PersonPicker.svelte` props `{ label; value: PersonRef|null; onPick: (p: PersonRef|null) => void; exclude?: Id[] }` (combobox over `api.search`, kind person only).

- [ ] **Step 1: Write failing tests.**
  - `same_sex_couple_allowed` (both partners male: `saveFamily` sent with no sex validation).
  - `empty_slot_allowed_one_partner_only`.
  - `cannot_pick_same_person_twice` (second pick excludes the first; message "Choose a different person").
  - `per_parent_relationship_payload` (`updateChildLink` rel `['birth','adopted']`).
  - `reorder_with_keyboard` (Alt+ArrowDown on child 1 -> `reorderChildren` order `[c2,c1,c3]` and focus stays on the moved row).
  - `preferred_parents_checkbox_payload`.
  - `divorce_is_an_event_not_a_flag` (no "divorced" checkbox exists; "Add event" lists Divorce).
  - `delete_family_keeps_children_message`.
  - `cannot_make_person_their_own_parent` (picking the anchor child as partner shows an error and disables Save).
- [ ] **Step 2: Run** -> FAIL. **Step 3: Implement.** **Step 4: Run** -> PASS.
- [ ] **Step 5: Commit** `feat(app): family editor with per-parent relationships and child ordering`.

### Task 12: Source and citation editor, Sources view

**Files:**
- Create: `app/src/lib/editors/{CitationEditor,SourcePicker}.svelte`, `app/src/lib/views/Sources.svelte`
- Test: `app/src/lib/editors/CitationEditor.test.ts`, `app/src/lib/views/Sources.test.ts`

**Interfaces:**
- Consumes: `Api.suggestSources/lastCitation/apply` (`saveSource`, `addCitation`, `updateCitation`, `removeCitation`).
- Produces: `CitationEditor.svelte` props `{ target: OwnerRef; citation?: CitationDto; onDone: (r: EditResult) => void; onCancel: () => void }`: `SourcePicker` (typeahead with "Create source "{text}""), Page / entry, Date (`DateField`), Transcription (textarea), Quality Select (3 Direct and primary evidence, 2 Secondary evidence, 1 Questionable, 0 Unreliable; stored `0..3`), and a collapsed "Evidence analysis" group (Original source / Derivative; Primary / Secondary information; Direct / Indirect evidence as three tri-state Selects).
- Citation shortcuts, placed above the form as quiet buttons: **"Use the last citation"** (enabled when `api.lastCitation()` is non-null; sends `{type:'addCitation', target, citation:{reuseLast:true}}` immediately, one step, toast "Citation added"), and in the citation list each row has **"Clone"** (sends `{cloneOf: id}` then opens the clone for editing the page number). `Mod+Shift+C` in any editor dialog triggers "Use the last citation".
- `Sources.svelte`: list of sources via `suggestSources('', 200)` with citation counts, a filter box, "New source" dialog, selecting a source shows its fields (edit via `saveSource`).

- [ ] **Step 1: Write failing tests.**
  - `reuse_last_adds_in_one_command` (exactly one `apply` call with `{reuseLast:true}`; no form shown).
  - `reuse_last_disabled_when_none` (button `aria-disabled`, hint "No citation yet").
  - `clone_sends_cloneOf_and_focuses_page_field`.
  - `new_source_inline_creates_with_title` (`citation.source` is `{title:'1881 Census'}`).
  - `quality_labels_map_to_numbers` (choosing "Secondary evidence" -> `quality: 2`).
  - `evidence_group_collapsed_by_default_and_tristate_payload` (unset stays `undefined`, not `false`).
  - `mod_shift_c_shortcut_inside_dialog`.
  - `Sources.test.ts`: `lists_with_counts`, `filter_narrows`, `edit_source_saves`.
- [ ] **Step 2: Run** -> FAIL. **Step 3: Implement.** **Step 4: Run** -> PASS.
- [ ] **Step 5: Commit** `feat(app): citation editor with reuse-last and clone, sources view`.

### Task 13: Media attach, thumbnails and viewer

**Files:**
- Create: `app/src/lib/media/{Dropzone,MediaGrid,MediaViewer}.svelte`, `app/src/lib/views/MediaView.svelte`
- Test: `app/src/lib/media/{Dropzone,MediaGrid,MediaViewer}.test.ts`

**Interfaces:**
- Consumes: `Platform.onFilesDropped/pickFiles/openExternally`, `Api.attachFiles/mediaUrl/exifSuggestion/apply` (`detachMedia`, `setPrimaryMedia`, `updateMedia`).
- Produces: `Dropzone.svelte` props `{ target: OwnerRef; onAttached: (r: AttachResult) => void }`: whole-window drop overlay while hovering ("Drop photos and documents to attach them to {name}"), plus an "Add files…" button (`pickFiles` with extensions `jpg jpeg png webp tif tiff gif pdf`). It calls `api.attachFiles(target, paths)` once for the whole drop, then shows per-file failures in a `Banner` list ("scan.exe: unsupported type; we attach JPEG, PNG, WebP, TIFF, GIF and PDF files"), and a toast "Attached {n} files" with Undo. After attaching one image with an EXIF suggestion, a `Banner` offers "Use the photo's date ({date}) for a new event?" (opens `EventEditor` pre-filled; never auto-applies).
- `MediaGrid.svelte` props `{ items: MediaLinkDto[]; target?: OwnerRef; onOpen: (i: number) => void }`: thumbnails `<img loading="lazy" alt="{title or originalName}" src={api.mediaUrl(id,'thumb')}>` in a CSS grid of 140 px tiles; primary photo marked with `Ribbon`; PDFs and unresolved files show an icon tile; unresolved shows "File missing" text.
- `MediaViewer.svelte` props `{ items: MediaLinkDto[]; index: number; onClose: () => void }`: modal dialog, preview image (`'preview'` size), zoom with wheel / `+` / `-` / `0` (fit), drag to pan, Left/Right to move, `P` set as primary photo, `Esc` closes, "Open original" (`openExternally`), title and description editable inline (`updateMedia`).
- `MediaView.svelte` (sidebar "Media"): all-media browsing via `listPersons`-style paging is not in v0.1; the view shows the current focus person's media with the Dropzone (empty state: "No photos or documents for {name}. Drop files here to attach them.").

- [ ] **Step 1: Write failing tests.**
  - `drop_attaches_all_paths_in_one_call` (`attachFiles` called once with 3 paths, target preserved).
  - `partial_failure_reported_per_file` (result `failed:[{path:'C:\\Users\\Zoë\\scan.exe', reason}]` -> list shows the file name; the other two appear in the grid).
  - `non_ascii_windows_path_passthrough`.
  - `duplicate_file_attached_once` (MockApi dedups by name+size; grid shows one tile; message "Already attached").
  - `drag_overlay_toggles_with_hover`.
  - `grid_alt_text_and_primary_ribbon`; `pdf_tile_has_document_icon_and_open_button`; `missing_file_tile_text`.
  - `viewer_zoom_pan_keys` (`+` doubles to max 8x, `0` resets to fit, arrow keys move index, wraps not); `viewer_p_sets_primary` (`setPrimaryMedia`); `viewer_focus_trapped_and_esc_closes`; `exif_banner_offers_not_applies` (no `apply` call until clicked).
- [ ] **Step 2: Run** -> FAIL. **Step 3: Implement** (MockApi `mediaUrl` returns `data:image/svg+xml` placeholders; `MockApi.attachFiles` rejects `.exe`, empty names and directories by extension rules). **Step 4: Run** -> PASS.
- [ ] **Step 5: Commit** `feat(app): drag-and-drop media attach, thumbnail grid and zoomable viewer`.

### Task 14: Pedigree and descendant chart views with outline mirror

**Files:**
- Create: `app/src/lib/chart/{ChartCanvas,OutlineMirror}.svelte`, `app/src/lib/views/{Pedigree,Descendants}.svelte`
- Test: `app/src/lib/chart/{ChartCanvas,OutlineMirror}.test.ts`, `app/src/lib/views/Charts.test.ts`

**Interfaces:**
- Consumes: Task 4 engine, `Api.chartOpen/chartWindow/chartOutline/chartClose`, `nav.focusPerson`.
- Produces `ChartCanvas.svelte` props `{ kind: 'pedigree'|'descendants'; root: Id; generations: number; onRecentre: (id: Id) => void; onOpenPerson: (id: Id) => void }`. Behaviour: `ResizeObserver` + `devicePixelRatio` canvas sizing; `requestAnimationFrame` draw only when the camera or window changed; wheel zoom (Ctrl+wheel or plain wheel, anchored at the pointer), drag pan, pinch via pointer events, toolbar (Zoom in, Zoom out, Fit, "Go to home person", generations Select 3-8, orientation fixed); click selects, double-click or Enter re-centres (`onRecentre`, camera tween 280 ms), `E` opens the editor; arrow keys move selection through `ChartNode.nav` (`neighbor`), Home goes to the root node; `moreUp`/`moreDown` nodes draw a "+" handle that re-roots on that person; a dup node (`dupOf`) draws a dashed link and clicking it jumps to the original.
- Canvas accessibility: `<canvas role="application" aria-label="{Pedigree|Descendant} chart of {name}. Use arrow keys to move between people, Enter to centre, E to edit." tabindex="0">` with `aria-activedescendant` pointing at a visually hidden `<div id role="option">` holding the selected node's `label`, updated on selection; a `role="status"` live region announces "{label}, {n} of {m} in this generation" on selection change.
- `OutlineMirror.svelte` props `{ chartId: number; onPick: (personId: Id) => void }`: toggle "Chart / Outline" (Tabs) in both views. The outline is a `role="tree"` loaded lazily with `chartOutline(chartId, parentNode, 50)` (expand on ArrowRight, collapse ArrowLeft, Enter picks), item text is `OutlineItem.label` plus the person's name and lifespan (pedigree labels start with the Ahnentafel number: "2 Father", "3 Mother", "6 Mother's father" is rendered as "6 Mother's father"). It is the default view when `matchMedia('(prefers-reduced-motion: reduce)')` or when the user last chose it (stored in `localStorage` in try/catch).
- Views reopen the chart when `chartMeta.version` differs from the data version (after any edit; `ApiError.code === 'stale'` from `chartWindow` triggers a reopen).

- [ ] **Step 1: Write failing tests.**
  - `ChartCanvas.test.ts` (fake canvas context; MockApi chart): `opens_chart_and_draws_once_per_change` (no draw calls without camera change over 10 `rAF` ticks); `wheel_zoom_calls_window_only_after_leaving_cache`; `arrow_keys_move_selection_via_nav` (ArrowRight on root -> selected becomes `nav.child`/parent as defined: pedigree Right = parent, Left = child; descendants reversed; Up/Down = prev/next); `enter_recentres_and_tweens` (tween called with 280 ms; with reduced motion called with `reduced=true`); `live_region_announces_selection`; `activedescendant_label_matches`; `dup_node_click_jumps_to_original`; `resize_updates_backing_store_size` (canvas width = css width * dpr); `stale_error_triggers_reopen`.
  - `OutlineMirror.test.ts`: `tree_roles_and_aria_expanded`; `lazy_loads_children_on_expand` (one `chartOutline` call per expand); `enter_picks_person`; `ahnentafel_labels_in_pedigree` (items "1 …", "2 Father", "3 Mother").
  - `Charts.test.ts`: `view_toggle_chart_outline_persists_choice`; `empty_root_without_parents_shows_helper` ("No parents recorded for {name}. Add a father or mother." with a button); `cycle_fixture_opens_without_hanging` (MockApi cycle person: chart opens, `nodeCount` finite, test timeout 2 s); `generations_select_reopens_chart`.
- [ ] **Step 2: Run** -> FAIL. **Step 3: Implement.** **Step 4: Run** -> PASS.
- [ ] **Step 5: Commit** `feat(app): pedigree and descendant canvas charts with accessible outline mirror`.

### Task 15: Export dialog with privacy preview, and Settings

**Files:**
- Create: `app/src/lib/views/{ExportDialog,Settings}.svelte`
- Test: `app/src/lib/views/{ExportDialog,Settings}.test.ts`

**Interfaces:**
- Consumes: `Api.previewExport/exportGedcom/getSettings/setSettings/saveCopy`, `Platform.pickSavePath`.
- Produces `ExportDialog.svelte` (opened from sidebar "Export" and `Mod+Shift+E`): Format RadioGroup "GEDCOM 5.5.1" / "GEDCOM 7.0" (pre-selected from `PrivacyPreview.defaultVersion`, with the note "Defaults to the version your file came in, so your other programs can read it."); Living people RadioGroup "Show as Living (default)", "Leave out", "Include everyone"; Checkbox "Include records marked private"; a **live preview panel** (`role="region"` "Privacy preview") showing exported people, "Shown as Living", "Left out", "Records removed", `mentionWarnings` (heading "Notes that may name hidden people", each "{excerpt} mentions {hiddenPerson}", with the sentence "This check is best effort: nicknames and initials are not caught."), `groupMedia` ("Photos that also show hidden people: you decide whether to keep them in the shared copy"), and `losslessNotes` (for 5.5.1: "Roles, 'did not happen' records, shared notes and external IDs are written as extension tags; other programs may drop them."). Also the fixed line: "Photos are not included in a GEDCOM file." Primary button "Export…" is **disabled** while a preview request is in flight or when options changed since the last completed preview; it passes `previewToken`. "Include everyone" and "Include records marked private" each show a `Banner kind="warning"`: "Living people's details will be written to the file." Success toast "Exported {n} KB to {file}" with Reveal action.
- A separate section in the same dialog: "Save a copy of the whole tree" (`saveCopy`) with the owner-only note "This copy is not filtered. It includes living people, private records and history; keep it to yourself."
- `Settings.svelte`: Date order (DMY/MDY, sentence "How 03/04/1901 is read when you type it"), Name order (given-first/surname-first), Living threshold (number 1-150, default 100, "People born more than {n} years ago with no other information are treated as deceased"), Home person (`PersonPicker`), Theme (Light/Dark/System), Backups to keep (1-50, default 10). Changes apply immediately and persist via `setSettings`.

- [ ] **Step 1: Write failing tests.**
  - `ExportDialog.test.ts`: `default_options_are_safest` (living radio "Show as Living" checked; private unchecked); `export_disabled_until_preview_matches_options` (change radio -> button `disabled`; resolves -> enabled; change again -> disabled again; the token sent to `exportGedcom` is the latest preview's); `stale_preview_response_ignored`; `undated_person_counted_as_placeholder` (MockApi tree with an undated person: `placeholderPersons >= 1` displayed); `include_all_shows_warning`; `version_defaults_from_source` (`defaultVersion:'7.0'` -> radio 7.0 checked); `mention_warnings_listed_with_best_effort_text`; `lossless_note_only_for_551`; `save_copy_has_unfiltered_warning`; `export_failure_shows_error_and_keeps_dialog`.
  - `Settings.test.ts`: `theme_change_applies_immediately` (`data-theme` attribute); `living_threshold_clamped` (0 -> 1, 500 -> 150); `home_person_saved`; `date_order_persists`.
- [ ] **Step 2: Run** -> FAIL. **Step 3: Implement.** **Step 4: Run** -> PASS.
- [ ] **Step 5: Commit** `feat(app): export dialog with privacy preview and settings`.

### Task 16: Accessibility and keyboard pass, Playwright smoke on MockApi

**Files:**
- Create: `app/src/lib/a11y.test.ts`, `app/e2e/smoke.spec.ts`, `app/playwright.config.ts`
- Modify: views as the audit finds (record findings in commit message)

**Interfaces:** none new.

- [ ] **Step 1: Write failing tests** (`a11y.test.ts`, mounting each view on MockApi): `every_interactive_has_accessible_name` (walk `button, a, input, select, textarea, [role=button], [role=option], canvas`; each has non-empty `computedName` using `dom-accessibility-api`, MIT, already a Testing Library dependency); `one_h1_per_view_and_landmarks` (`main`, `nav` named "Main"); `tab_order_has_no_positive_tabindex`; `dialogs_are_modal_named_and_focus_returning`; `no_colour_only_status` (living/warning elements contain text); `rem_units_only` (no `px` font-size declarations in `src/**/*.css` and `<style>` blocks outside `tokens.css`); `reduced_motion_zeroes_durations`; `skip_link_focuses_main`; `shortcut_sheet_matches_registered_shortcuts`.
- [ ] **Step 2: Playwright smoke** `smoke.spec.ts` (Chromium against `vite preview`, MockApi): `welcome_to_family_view` (click "Import a GEDCOM file" -> mock wizard -> report -> "Open the tree" -> heading is the home person); `keyboard_only_add_child_and_undo` (Tab/Enter/typing only: open Add menu, "Add child", type given name, Enter, assert child card appears; `Control+Z`, assert it is gone); `search_palette` (`Control+K`, type, Enter); `dark_theme_follows_system` (`page.emulateMedia({colorScheme:'dark'})`, body background computed equals `--paper-0` dark value).
- [ ] **Step 3: Run** `npm --prefix app test -- a11y` -> FAIL, fix views, `npx --prefix app playwright install chromium` then `npm --prefix app run e2e` -> PASS.
- [ ] **Step 4: Run** `npm --prefix app run check && npm --prefix app run lint && npm --prefix app test` -> all green.
- [ ] **Step 5: Commit** `test(app): accessibility audit and Playwright smoke on MockApi`.

---

# Group B: Rust and wiring (needs Plan 1; see the table at the top for exact tasks)

### Task 17: Tauri shell skeleton and the 10k-node canvas spike on real webviews

**Needs:** Plan 1 Task 1 (workspace root). Run this before Task 18.

**Files:**
- Create: `app/src-tauri/{Cargo.toml, tauri.conf.json, build.rs, capabilities/default.json, src/main.rs, src/lib.rs}`, `docs/spikes/2026-11-03-canvas-fps.md`
- Modify: root `Cargo.toml` (`members += ["app/src-tauri"]`), `app/package.json` (script `tauri`), `.github/workflows/ci.yml` (job `shell-build` on the three OSes: `cargo build -p gt-app`, installs `libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev patchelf` on Ubuntu)
- Test: `app/src-tauri/tests/config.rs`

**Interfaces:**
- Produces crate `gt-app` (lib `gt_app_lib`, bin `gene-tree`), identifier `app.gene-tree.desktop`, window `main` 1280x800 (min 960x640), `dragDropEnabled: true`, CSP exactly as in Global Constraints, `devUrl http://localhost:1420`, `frontendDist ../dist`, bundle targets `msi nsis dmg appimage deb rpm`. Plugins registered: `tauri-plugin-dialog`, `tauri-plugin-opener`. Capability set: `core:default`, `dialog:allow-open`, `dialog:allow-save`, `opener:allow-reveal-item-in-dir`, `opener:allow-open-path`; nothing else.
- Spike-only command (compiled only under `cfg(any(debug_assertions, feature = "spike"))`): `#[tauri::command] fn spike_report(app: AppHandle, json: String) -> Result<(), String>` writes `spike-<os>-<timestamp>.json` into `app_data_dir()`.

- [ ] **Step 1: Write failing test** `config.rs`: parse `tauri.conf.json` and assert `app.security.csp` contains `default-src 'self'` and does not contain `http://` or `https://` other than `http://ipc.localhost` and `http://gtmedia.localhost`; assert `capabilities/default.json` permissions equal the exact list above; assert no `updater` plugin key and no `fs`/`http` permissions.
- [ ] **Step 2: Run** `cargo test -p gt-app --test config` -> FAIL.
- [ ] **Step 3: Implement** the shell (`tauri::Builder::default().plugin(...).invoke_handler(generate_handler![spike_report])`). Build the spike as a release binary with the mode baked in: `VITE_GT_MODE=spike npm --prefix app run tauri -- build --no-bundle`.
- [ ] **Step 4: Run the spike on each OS and fill in `docs/spikes/2026-11-03-canvas-fps.md`** (table: OS, webview and version, GPU, DPR, median ms, p95 ms, p99 ms, frames over 25 ms, 10k and 30k nodes).
  - **Windows 11 (WebView2)** and **macOS (WKWebView):** run the built binary, wait 10 s, copy the JSON from the window or the app data folder.
  - **Linux WebKitGTK:** on Ubuntu 22.04 or 24.04 install `libwebkit2gtk-4.1-dev build-essential libssl-dev libxdo-dev libayatana-appindicator3-dev librsvg2-dev patchelf`, then on a real desktop session with a GPU run `./target/release/gene-tree`; run it three times: default, `WEBKIT_DISABLE_COMPOSITING_MODE=1`, and `WEBKIT_DISABLE_DMABUF_RENDERER=1`. For a headless CI-style lower bound run `xvfb-run -a -s "-screen 0 1920x1080x24" ./target/release/gene-tree` (software rendering: record as "lower bound", it is not the gate).
  - **Gate (each OS, real GPU, 10k nodes, 1280x800 at DPR 1 and 2):** median frame <= 16.9 ms and p95 <= 22 ms. If any OS fails, apply in order and re-measure: (1) lower the `boxes` LOD threshold from 0.25 to 0.35; (2) cache the static layer in an `OffscreenCanvas` bitmap tile per window fetch and only blit while panning; (3) draw edges as straight segments below zoom 0.4; (4) reduce `WindowCache` inflation to 50%. Record which mitigation shipped. If nothing passes on WebKitGTK, stop and escalate: the spec fallback is a different renderer for Linux, not a silent drop of the 60 fps target.
- [ ] **Step 5: Commit** `feat(app): Tauri shell and canvas fps spike results`.

### Task 18: `gt-layout` crate (pedigree grid and descendant tidy tree)

**Needs:** Plan 1 Task 1 only (no `gt-core` dependency).

**Files:**
- Create: `crates/gt-layout/{Cargo.toml, src/lib.rs, src/types.rs, src/pedigree.rs, src/descendants.rs, src/grid.rs}`
- Test: `crates/gt-layout/tests/{pedigree.rs, descendants.rs, perf.rs}`

**Interfaces:**
- Produces:
```rust
pub type Key = u32;                                        // dense person index chosen by the caller
pub trait ChartSource {
    /// Preferred parents, upper slot first (callers put the male parent first when only one is male).
    fn parents(&self, p: Key) -> [Option<Key>; 2];
    /// Partnerships of `p` in display order: (partner or None, children in birth order).
    fn unions(&self, p: Key) -> Vec<(Option<Key>, Vec<Key>)>;
}
pub struct Metrics { pub node_w: f32, pub node_h: f32, pub gap_x: f32, pub gap_y: f32, pub couple_gap: f32 }  // Default: 176, 56, 40, 12, 8
pub struct Rect { pub x: f32, pub y: f32, pub w: f32, pub h: f32 }
pub struct Node { pub id: u32, pub key: Key, pub rect: Rect, pub gen: i16, pub ahnen: Option<u64>, pub dup_of: Option<u32>,
                  pub more_up: bool, pub more_down: bool, pub nav: Nav }
pub struct Nav { pub parent: Option<u32>, pub child: Option<u32>, pub prev: Option<u32>, pub next: Option<u32> }
pub enum EdgeKind { Parent, Partner, Dup }
pub struct Edge { pub from: u32, pub to: u32, pub kind: EdgeKind, pub pts: Vec<f32> }
pub struct Layout { pub nodes: Vec<Node>, pub edges: Vec<Edge>, pub bounds: Rect, pub root: u32, pub truncated: bool, /* grid index private */ }
impl Layout { pub fn query(&self, r: &Rect) -> (Vec<&Node>, Vec<&Edge>); }   // uniform grid, cell 512 world units
pub fn pedigree(src: &dyn ChartSource, root: Key, generations: u8, m: &Metrics, max_nodes: usize) -> Layout;
pub fn descendants(src: &dyn ChartSource, root: Key, generations: u8, m: &Metrics, max_nodes: usize) -> Layout;
```
- Pedigree: root at the left; generation g at `x = g * (node_w + gap_x)`; Ahnentafel numbering (root 1, father 2n, mother 2n+1); vertical placement by post-order leaf counting so only existing ancestors take space, each person centred between its parents; a person already placed (pedigree collapse) is emitted as a dup node (`dup_of` = id of the first occurrence) with an `EdgeKind::Dup` edge; nodes at the generation limit that have parents get `more_up = true`.
- Descendants: Walker / Buchheim linear-time tidy tree; the unit of layout is a couple box (person, then partner(s) to the right at `couple_gap`), generations go downward at `y = depth * (node_h + gap_y * 4)`; a person with several unions is drawn once with each union's partner to the right and children grouped per union; nodes at the limit with children get `more_down`.
- Both: a `visited` set guarantees termination on cyclic input (a person met again as their own ancestor/descendant becomes a dup node); `max_nodes` stops expansion and sets `truncated`.

- [ ] **Step 1: Write failing tests.**
  - `pedigree.rs`: `ahnentafel_numbers` (full 4-generation tree -> 15 nodes numbered 1..=15, node 6 is mother of... father's? assert `ahnen(6)` is the father of person 3); `generation_x_positions` (`x == gen * 216`); `sparse_tree_uses_no_empty_slots` (only paternal line known -> bounds.h == `node_h`); `parent_centred_between_parents` (y of node n == mean of y of 2n and 2n+1 within 1e-3); `collapse_emits_dup` (same grandparent twice -> exactly one node with `dup_of` and one `Dup` edge); `generation_limit_flags_more_up`; `cycle_terminates` (A's parent is B, B's parent is A, 10 generations -> finite nodes, one dup); `root_without_parents` (1 node, bounds >= node); `nav_links_consistent` (for every node with `nav.parent = Some(p)`, node p's `nav.child` chain reaches it).
  - `descendants.rs`: `parent_centred_over_children`; `no_overlap_property` (proptest: random trees up to 200 nodes, no two rects on the same depth overlap, min separation `gap_x`); `couple_adjacent` (partner x = person x + node_w + couple_gap); `many_unions_one_node` (12 unions: person appears once, 12 partner nodes); `forty_children_width` (bounds.w >= 40 * node_w + 39 * gap_x); `max_nodes_truncates`; `cycle_terminates`; `edges_are_orthogonal_polylines` (every `pts` has even length >= 4 and consecutive points share x or y).
  - `perf.rs`: `layout_10k_under_100ms_release` (`#[ignore]` run with `--release -- --ignored`): descendants of a synthetic 10k-node tree < 100 ms; `query_viewport_under_1ms` (1000 queries of a 1500x900 rect over 10k nodes average < 1 ms).
- [ ] **Step 2: Run** `cargo test -p gt-layout` -> FAIL.
- [ ] **Step 3: Implement.** `grid.rs` is the uniform-grid spatial index (nodes and edge bounding boxes bucketed by cell). Use an explicit stack, not recursion, in both layouts (50k-deep chains must not overflow).
- [ ] **Step 4: Run** `cargo test -p gt-layout && cargo test -p gt-layout --release -- --ignored && cargo clippy -p gt-layout --all-targets -- -D warnings` -> PASS.
- [ ] **Step 5: Commit** `feat(layout): pedigree grid and Walker tidy-tree descendant layout with viewport index`.

### Task 19: `gt-store::edit`, granular persistence with change log and meta

**Needs:** Plan 1 Tasks 12, 13, 14 (schema, save/load/search, backup/media). Scheduled after Plan 1's store tasks.

**Files:**
- Create: `crates/gt-store/src/edit.rs`, `crates/gt-store/src/migrations/0002_edit.sql`
- Modify: `crates/gt-store/src/save.rs` (extract the per-record row writers into `pub(crate) fn write_person(tx, &Person)`, `write_family`, `write_event`, `write_place`, `write_source`, `write_repository`, `write_citation`, `write_media`, `write_note`, and matching `delete_*`; `save_tree` calls them), `crates/gt-store/src/migrate.rs` (register `0002`), `crates/gt-store/src/lib.rs` (`SCHEMA_VERSION = 2`, re-exports), `docs/schema.md` (document the new `change_log` columns and `meta` keys), `crates/gt-store/tests/project.rs` (the `open_newer_schema_refused` expectation uses `SCHEMA_VERSION`, not the literal 1)
- Test: `crates/gt-store/tests/edit.rs`

**Interfaces:**
- Consumes: `gt_core::{Tree, Person, Family, Event, Place, Source, Repository, Citation, Media, Note}` (Plan 1 Task 1), `Project`/`StoreError` (Task 12), the FTS and phonetic writers (Task 13).
- Produces (migration `0002_edit.sql`: `ALTER TABLE change_log ADD COLUMN group_id TEXT; ADD COLUMN label TEXT; ADD COLUMN after_json TEXT;` plus `CREATE INDEX change_log_target ON change_log(target_id, ts)` and `CREATE INDEX change_log_group ON change_log(group_id)`):
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] pub enum RecordKind { Person, Family, Event, Place, Source, Repository, Citation, Media, Note }
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Record { Person(Person), Family(Family), Event(Event), Place(Place), Source(Source),
                  Repository(Repository), Citation(Citation), Media(Media), Note(Note) }
impl Record { pub fn kind(&self) -> RecordKind; pub fn id(&self) -> Uuid; }
#[derive(Clone, Debug, PartialEq)] pub struct Change { pub kind: RecordKind, pub id: Uuid, pub before: Option<Record>, pub after: Option<Record> }
#[derive(Clone, Debug, PartialEq)] pub struct ChangeSet { pub group: Uuid, pub label: String, pub ts: i64, pub changes: Vec<Change> }
impl ChangeSet { pub fn inverse(&self) -> ChangeSet; }       // reversed order, before/after swapped, same label prefixed "Undo: " by the caller
#[derive(Clone, Copy, Debug, PartialEq)] pub enum ChangeOp { Create, Update, Delete }
pub struct HistoryEntry { pub ts: i64, pub group: Uuid, pub label: String, pub op: ChangeOp, pub kind: RecordKind }
impl Project {
    /// ONE transaction (`defer_foreign_keys=ON`): for each change, after=Some -> delete the record's owned rows then write it
    /// (Person owns name, name_phonetic, citation_link/note_link/media_link/ext_id rows of itself and its names;
    /// Family owns child_link; Event owns participant), after=None -> delete it and its owned rows. It does NOT cascade to
    /// referencing records: the caller supplies those changes. Then refreshes name_phonetic and FTS rows for every person
    /// touched directly or as an event participant (before or after), applies `living` ((person, cached value) pairs) to
    /// person.living_cached, and appends one change_log row per change with group_id, label, before_json, after_json.
    pub fn apply_changes(&mut self, cs: &ChangeSet, living: &[(Uuid, bool)]) -> Result<(), StoreError>;
    pub fn history(&self, record: Uuid, limit: usize) -> Result<Vec<HistoryEntry>, StoreError>;   // newest first
    pub fn meta_get(&self, key: &str) -> Result<Option<String>, StoreError>;
    pub fn meta_set(&mut self, key: &str, value: &str) -> Result<(), StoreError>;                 // keys used: "living_max_age", "home_person", "last_import_id"
    pub fn set_living_cached_all(&mut self, living: &HashMap<Uuid, bool>) -> Result<(), StoreError>;
    pub fn change_log_is_empty(&self) -> Result<bool, StoreError>;
}
```

- [ ] **Step 1: Write failing tests** (`edit.rs`, base tree = import of `basic551.ged` then `save_tree`):
  - `v1_project_upgrades_to_2` (create a DB by applying only migration 0001, open with `Project::open` -> `schema_version() == 2`, `change_log` has `group_id`).
  - `incremental_equals_full_save` (apply a random sequence of 50 `ChangeSet`s built from a seeded generator, mirror each on an in-memory `Tree`, then `load_tree()` equals the mirrored tree via `normalized_for_compare`; proptest, 32 cases).
  - `upsert_person_replaces_names` (change a person from 2 names to 1 -> `name` table has 1 row for that person).
  - `delete_event_removes_participants_and_links`.
  - `search_reflects_edit` (rename `Müller` to `Meier` -> `search("muller")` empty, `search("meier")` finds; add an event with place `Hamburg` -> `search("hamburg")` finds the principal).
  - `living_cached_applied` (pass `[(p, false)]` -> `person.living_cached == 0`).
  - `change_log_rows_and_history` (one 3-change group writes 3 rows sharing `group_id`; `history(person, 10)[0].label == "Edit name"`, newest first).
  - `inverse_roundtrip` (apply cs then `cs.inverse()` -> `load_tree()` equals the original).
  - `failed_apply_rolls_back` (second change references a missing place with FK on -> `Err`, nothing from the first change persisted, `change_log` unchanged).
  - `meta_roundtrip_and_survives_reopen`.
  - `apply_is_fast_on_50k` (`#[ignore]`, release: one 3-change group on a 50k-person project < 50 ms).
- [ ] **Step 2: Run** `cargo test -p gt-store --test edit` -> FAIL.
- [ ] **Step 3: Implement.** Reuse Task 13's row writers (do not duplicate SQL). Serialise `before_json`/`after_json` with `serde_json` of `Record`.
- [ ] **Step 4: Run** `cargo test -p gt-store && cargo test -p gt-store --release -- --ignored` -> PASS (Plan 1's store tests still pass).
- [ ] **Step 5: Commit** `feat(store): granular edit module, change log, meta API (schema v2)`.

### Task 20: `gt-session` read side: DTOs, open/close, indexes, paging, family view, person detail

**Needs:** Plan 1 Tasks 1, 3, 10, 12, 13; Task 19 (`meta_get/meta_set`).

**Files:**
- Create: `crates/gt-session/{Cargo.toml, src/lib.rs, src/error.rs, src/dto.rs, src/session.rs, src/index.rs, src/names.rs, src/query.rs, src/family_view.rs, src/person_detail.rs, src/settings.rs, src/dates.rs}`, `crates/gt-session/tests/{read.rs, perf.rs}`, `crates/gt-session/tests/export_bindings.rs`, `app/src/lib/api/generated/*.ts`
- Modify: `crates/gt-store/src/collation.rs` (add `pub fn collate(a: &str, b: &str) -> std::cmp::Ordering` using the same ICU root collator), `app/src/lib/api/types.ts` (re-export from `generated/`)

**Interfaces:**
- Consumes: Plan 1 `Tree` queries, `compute_living`, `GenDate::display`, `Project::{create, open, load_tree, search}`; Task 19 meta.
- Produces:
```rust
// dto.rs — every Task 3 TypeScript type has a Rust twin: #[derive(Serialize, Deserialize, TS)] #[serde(rename_all = "camelCase")] #[ts(export, export_to = "../../../app/src/lib/api/generated/")]
// error.rs
#[derive(thiserror::Error, Debug, Serialize, TS)] pub struct ApiError { pub code: ErrorCode, pub message: String }
pub enum ErrorCode { NotFound, Invalid, Conflict, Io, NoProject, Stale, Internal }
impl From<gt_store::StoreError> for ApiError;
// session.rs
pub struct Session { /* project, tree, index, undo/redo stacks, settings, clock */ }
pub struct SessionClock(pub Box<dyn Fn() -> (i32 /*today_jdn*/, i64 /*now_ms*/) + Send + Sync>);
impl Session {
    pub fn open(dir: &Path, clock: SessionClock) -> Result<Session, ApiError>;
    pub fn info(&self) -> ProjectInfo;
    pub fn list_persons(&self, q: &PersonQuery, offset: usize, limit: usize) -> Page<PersonRow>;   // limit clamped to 200
    pub fn get_person(&self, id: Uuid) -> Result<PersonDetail, ApiError>;
    pub fn family_view(&self, id: Uuid) -> Result<FamilyViewDto, ApiError>;
    pub fn search(&self, q: &str, limit: usize) -> Result<Vec<SearchHitDto>, ApiError>;           // limit clamped to 50
}
// names.rs
pub fn display_name(p: &Person, order: NameOrder) -> String;        // preferred Name, falls back to first, then "Unknown"
pub fn lifespan(tree: &Tree, p: &Person) -> String;                 // "1850-1921" with an en dash, "b. 1850", "d. 1921", or ""
```
- `index.rs`: `PersonIndex` holds one `PersonSummary` per person (id, display name, surname/given sort keys, birth/death display, living, source count, changed), the id-to-slot map, and lazily built sorted order vectors keyed by `(sort, desc)` using `gt_store::collation::collate`; filters evaluate over summaries; `letters` offsets are computed for the surname sort (first letter after diacritic folding; non-Latin first letters go to `"Other scripts"` after Z).
- DTO generation: `cargo test -p gt-session --test export_bindings` regenerates `app/src/lib/api/generated/*.ts` (ts-rs `export` tests); CI step `git diff --exit-code app/src/lib/api/generated` guards drift. `app/src/lib/api/types.ts` becomes `export * from './generated'` plus the hand-written `EditCommand`/`Api`/`Platform` interfaces; any field the generated files disagree with from Task 3 is fixed in the Rust DTO, not in the frontend.

- [ ] **Step 1: Write failing tests** (`read.rs`, project = `tests/fixtures/gedcom/basic551.ged` plus a 5-person hand-built family imported through `gt_gedcom::import` and `save_tree`; clock fixed at today = 2026-11-03):
  - `open_info_counts` (`info().personCount`, `homePerson == None`, `sourceVersion` from header).
  - `list_sorted_by_surname_icu` (`Émile`, `Eva`, `Zoe` order; `Müller` between `Mueller` and `Munro` per ICU root).
  - `list_limit_clamped_to_200` and `offset_beyond_total_returns_empty_items_with_total`.
  - `filters_combine` (living + noSource -> only matching rows; `text` matches any name variant diacritic-insensitively).
  - `letters_offsets_point_at_first_row_of_letter` and `non_latin_goes_to_other_scripts`.
  - `living_flag_matches_core_rule_undated_is_living` (Review Focus #2 at the data layer).
  - `family_view_has_parents_partners_children_in_birth_order`, `family_view_with_cycle_terminates`, `person_with_12_partners_and_40_children`.
  - `person_detail_names_events_roles_citations` (participant roles present; `isFact` true for occupation).
  - `display_name_orders` (`given-first` "Anna Schmidt", `surname-first` "Schmidt, Anna"); `lifespan_formats`.
  - `unknown_id_is_not_found`.
  - `search_limit_clamped_and_person_ref_attached`.
  - `perf.rs` (`#[ignore]`, release, synthetic 50k tree): `open_under_2s`, `list_page_under_30ms` (second call, after the order cache is built), `first_sort_build_under_300ms`, `search_under_100ms`.
  - `export_bindings.rs`: ts-rs generation test for every exported DTO.
- [ ] **Step 2: Run** `cargo test -p gt-session --test read` -> FAIL.
- [ ] **Step 3: Implement.** Session holds `Tree` in memory (one `load_tree` on open); all UI reads are served from the tree and the index, never from SQL except `search`. Use `spawn_blocking`-friendly pure functions (no async in this crate).
- [ ] **Step 4: Run** `cargo test -p gt-session && cargo test -p gt-session --release -- --ignored && npm --prefix app run check` (generated types compile against all frontend code) -> PASS.
- [ ] **Step 5: Commit** `feat(session): DTOs, paged person list, family view, person detail, ts-rs bindings`.

### Task 21: `gt-session` edit commands and undo/redo

**Needs:** Task 19, Task 20; Plan 1 Tasks 1, 3.

**Files:**
- Create: `crates/gt-session/src/{command.rs, undo.rs, ops/{mod.rs, person.rs, quick_add.rs, family.rs, event.rs, cite.rs, media.rs, delete.rs}}`
- Test: `crates/gt-session/tests/{ops.rs, undo.rs}`

**Interfaces:**
- Consumes: Task 3's `EditCommand` (Rust twin `command::EditCommand`, same serde tags), Task 19 `ChangeSet`/`apply_changes`.
- Produces:
```rust
/// Pure: builds the change set for `cmd` against `tree`; never mutates, never touches the database.
pub fn plan(tree: &Tree, cmd: &EditCommand, now_ms: i64, today: DayOrder) -> Result<(ChangeSet, Option<Uuid> /*focus*/), ApiError>;
pub fn apply_to_tree(tree: &mut Tree, cs: &ChangeSet);          // after = Some -> insert/replace in the matching IndexMap; None -> remove
impl Session {
    pub fn apply(&mut self, cmd: EditCommand) -> Result<EditResult, ApiError>;   // plan -> project.apply_changes (with recomputed living diff) -> apply_to_tree -> update index -> push undo, clear redo
    pub fn undo(&mut self) -> Result<Option<EditResult>, ApiError>;              // apply cs.inverse() as a new group labelled "Undo: <label>"
    pub fn redo(&mut self) -> Result<Option<EditResult>, ApiError>;
    pub fn undo_state(&self) -> UndoState;                                       // stacks hold at most 200 groups; per session
    pub fn history(&self, record: Uuid, limit: usize) -> Result<Vec<HistoryEntry>, ApiError>;
}
```
Semantics fixed here (the MockApi implements the same):
- **Labels** (used in toasts and `undoLabel`): `createPerson` "Add person", `updatePerson` "Edit person", `deletePerson` "Delete person", `saveName` "Edit name", `deleteName` "Delete name", `saveEvent` "Save event", `deleteEvent` "Delete event", `quickAdd` "Add {relation}", `saveFamily` "Save family", `deleteFamily` "Delete family", `linkChild` "Add child to family", `updateChildLink` "Edit child link", `unlinkChild` "Remove child from family", `reorderChildren` "Reorder children", note/citation/media variants "Save note", "Add citation", "Attach media", etc.
- **`quickAdd` father/mother:** use the anchor's preferred parent family (`families_as_child`, `preferred`, else the first); if it has an empty partner slot, fill it (new person gets the requested sex); if both slots are full and `newParentSet` is false -> `ApiError{Conflict}`; with `newParentSet` or no parent family, create a new Family with the new person in `partner1`, add the anchor as `ChildLink{rel1: Birth, rel2: Birth, preferred: true}` (preferred only when the anchor had no parent family). **partner:** a new Family (anchor in `partner1`, new person in `partner2`, `UnionKind::Unknown`) unless `family` names an existing family with an empty slot. **child:** `family` if given; else the anchor's only family; else (none) a new Family with the anchor and an empty slot; several families and no `family` -> `Invalid`; `ChildLink{Birth, Birth}` appended last. **sibling:** child of the anchor's preferred parent family; if the anchor has no parent family, create an empty Family and link both the anchor and the new person as children. Every `quickAdd` creates exactly one `ChangeSet` and, if `draft.birthDateRaw`/`birthPlaceText` are set, a Birth `Event` with the new person as `Principal` (place created or reused by comma-chain text, de-duplicated by full text).
- **`deletePerson`:** removes the person; sets their partner slots to `None` in families; removes their `ChildLink`s; deletes events where they were the only `Principal` and removes them as participant elsewhere; deletes a Family left with no partners and no children; deletes notes/citations referenced only by them (shared notes are kept); all in one group. The Conflict-free dry run counts are returned by `Session::delete_impact(id) -> {families: usize, events: usize}` for the confirmation text.
- **`saveEvent`:** `placeText` goes through `places::ensure_chain(tree, text) -> (Uuid, Vec<Place> new)`; `dateRaw` through `GenDate::parse(raw, day_order)` keeping `raw` exactly; an `Unparsed` date is saved as is (never an error). Kind strings map to `EventKind` (`custom:<text>` -> `Custom`). Family-owned events use `ParticipantRef::Family` with `Role::Principal`.
- **Validation errors** (`Invalid`): a family partner equal to a child of that family; the same person in both partner slots; parent set cycle is **not** rejected (imports may contain it), but `saveFamily`/`linkChild` reject a direct self-link (person as own child).
- **`addCitation` `{reuseLast:true}`** copies the Session's `last_citation` (set whenever a citation is created or cloned, kept across undo); with none -> `Invalid` "No citation to reuse". `{cloneOf}` copies every field with a new id.
- **Living recompute:** after each applied group the Session recomputes `compute_living` for the whole tree (50k < 30 ms budget is checked in Task 28), diffs against the index, and passes the changed pairs to `apply_changes`.

- [ ] **Step 1: Write failing tests.**
  - `ops.rs`: `create_person_has_name_and_id`; `quick_add_father_fills_empty_slot`; `quick_add_father_without_parents_creates_family_and_preferred_link`; `quick_add_father_when_full_conflicts_unless_new_parent_set`; `quick_add_child_single_family_default`; `quick_add_child_multiple_families_requires_choice` (`Invalid`); `quick_add_sibling_without_parents_creates_empty_family_with_both_children`; `quick_add_creates_birth_event_with_place_chain` (`Berlin, Brandenburg, Germany` -> 3 places, reused on the second add); `quick_add_is_one_changeset`; `save_event_unparsed_date_kept_verbatim` (`raw == "sometime in the 1850s?"`, `value == Unparsed`); `save_event_mdy_vs_dmy`; `negative_event_flag`; `delete_person_cleans_everything` (families, children links, sole-principal events, orphan notes; shared note kept); `delete_impact_counts`; `same_sex_family_saved`; `self_child_rejected`; `reorder_children_changes_only_order`; `reuse_last_citation_copies`; `clone_citation_new_id`; `apply_to_tree_matches_plan_inverse` (apply cs then inverse -> tree equal).
  - `undo.rs` (against a real temp project): `undo_restores_multi_row_command` (quick-add child, undo -> `load_tree()` from disk equals the original); `redo_after_undo`; `new_edit_clears_redo`; `undo_label_and_state`; `stack_capped_at_200`; `edit_persisted_before_ack` (kill-simulation: drop the Session right after `apply` returns, reopen -> edit present); `living_cache_updates_after_edit` (adding a death event makes the person's row `living == false` in the next `list_persons` and in `person.living_cached`); `history_contains_group_labels`; `sorted_list_reflects_rename_without_restart`.
- [ ] **Step 2: Run** `cargo test -p gt-session --test ops --test undo` -> FAIL.
- [ ] **Step 3: Implement.** Keep `plan` pure so it is testable without SQLite; `EditCommand` is `#[serde(tag = "type", rename_all = "camelCase")]` and generated to TS by ts-rs as a check against Task 3's hand-written union (a test `edit_command_json_matches_ts_fixtures` deserialises `app/src/lib/api/__fixtures__/commands.json`, one object per variant, written in Task 3's TypeScript `satisfies EditCommand`).
- [ ] **Step 4: Run** `cargo test -p gt-session && cargo clippy --all-targets -- -D warnings` -> PASS.
- [ ] **Step 5: Commit** `feat(session): edit commands, quick add, delete cleanup, undo/redo over change sets`.

### Task 22: `gt-session` lookups: date preview, place and source suggestions

**Needs:** Task 20; Plan 1 Tasks 3, 4, 13.

**Files:**
- Create: `crates/gt-session/src/{lookup.rs, places.rs}` (extend `dates.rs`)
- Test: `crates/gt-session/tests/lookup.rs`

**Interfaces:**
- Produces:
```rust
pub fn parse_date_preview(input: &str, order: DayOrder) -> DatePreview;      // GenDate::parse + display + range text; never fails
impl Session {
    pub fn suggest_places(&self, prefix: &str, limit: usize) -> Vec<PlaceOption>;   // limit clamped to 20
    pub fn suggest_sources(&self, prefix: &str, limit: usize) -> Vec<SourceDto>;    // limit clamped to 200
    pub fn last_citation(&self) -> Option<CitationDto>;
}
```
- `dates.rs`: `fn year_of_jdn(jdn: i32) -> i32` (Gregorian civil-from-days; Plan 1 exposes only first/last-JDN, not the inverse) used to render ranges as `"1845 to 1855"` or `"1 Jan 1900 to 31 Dec 1910"` (full dates when the span is under 2 years); `understoodAs` is `GenDate::display()`; `warnings` are: `"Day and month are ambiguous; read as {order}"` for numeric `NN/NN/YYYY` with both parts <= 12, `"Not understood as a date. It will be saved exactly as you typed it."` for `Unparsed`.
- Place index: `PlaceIndex` built at open: maps lower-cased chain-segment prefixes to place ids with a use count (number of events referencing the place or its descendants); `suggest_places` ranks exact-leaf-prefix > chain-prefix, then by use count, then ICU order; `full` is the chain from leaf to root joined with `, `.

- [ ] **Step 1: Write failing tests:** `preview_about_has_range` (`abt 1850` -> understoodAs `about 1850`, range `1845 to 1855`); `preview_dmy_mdy_ambiguity_warns`; `preview_unparsed_status_and_raw`; `preview_phrase`; `preview_bce`; `preview_never_panics` (proptest over any `String`); `places_prefix_matches_leaf_and_chain` (`ham` finds `Hamburg, Germany`; `ger` finds places under Germany ranked after leaf matches); `places_use_count_ranking`; `places_after_edit_include_new_chain` (via Task 21 `saveEvent` with `placeText`); `places_limit_clamped_20`; `sources_prefix_and_counts`; `last_citation_none_then_some`; `parse_date_under_5ms` (release).
- [ ] **Step 2: Run** `cargo test -p gt-session --test lookup` -> FAIL. **Step 3: Implement.** **Step 4: Run** -> PASS.
- [ ] **Step 5: Commit** `feat(session): date preview, place and source suggestions`.

### Task 23: `gt-session` import, export with privacy preview, relink media, save copy

**Needs:** Tasks 20, 21; Plan 1 Tasks 7, 8, 9, 11, 14 (Task 15 is the reference flow for import).

**Files:**
- Create: `crates/gt-session/src/transfer.rs`
- Test: `crates/gt-session/tests/transfer.rs`

**Interfaces:**
- Consumes: `gt_gedcom::{import, export, ImportOptions, ExportOptions, ImportOutcome}`, `gt_core::privacy::{filter, PrivacyPolicy, LivingMode, PrivacySummary}`, `Project::{create, backup, add_media_file, save_tree, check_location}`.
- Produces:
```rust
pub fn import_gedcom(req: &ImportRequest, clock: &SessionClock, progress: &dyn Fn(ImportProgress)) -> Result<(Session, ImportReportDto), ApiError>;
//   1. Project::create(req.project_dir) (refuses an existing project -> Conflict); check_location -> info.locationWarning
//   2. read file bytes, gt_gedcom::import (phases reading/parsing)
//   3. for each resolved_media (id, src): add_media_file, set Media.path = relative path and sha256 (phase copying_media, done/total)
//   4. save_tree (phase saving), meta "last_import_id" = new uuid, open a Session (phase indexing)
//   5. ImportReportDto from ImportReport (lists capped at 5000, `truncated`), `text` = ImportReport::to_text()
impl Session {
    pub fn discard_import(dir: &Path) -> Result<(), ApiError>;      // only if meta last_import_id is set AND change_log is empty; deletes the folder; else Conflict
    pub fn relink_media(&mut self, folder: &Path) -> Result<RelinkResult, ApiError>;   // for each Media with sha256 == None: try path as relative to folder, then basename search (depth <= 8); add_media_file; one undoable ChangeSet "Link media files"
    pub fn preview_export(&self, opts: &ExportOptions) -> Result<PrivacyPreview, ApiError>;   // filter + summary; token = hash of (options, tree revision)
    pub fn export_gedcom(&self, opts: &ExportOptions, dest: &Path, token: &str) -> Result<u64, ApiError>;  // token must equal the current preview token else Stale; writes dest.tmp then renames
    pub fn save_copy(&self, dest: &Path) -> Result<(), ApiError>;   // backup(Manual) semantics into a new project folder incl. media; unfiltered
}
```
- `PrivacyPreview` mapping: `exportedPersons` = persons in the filtered tree that are not placeholders; `placeholderPersons`, `hiddenPersons`, `removedRecords` from `PrivacySummary`; `mentionWarnings` = (first 80 chars of the note, hidden person's display name); `groupMedia` = (media title or original name); `losslessNotes` = the 5.5.1 sentence from Task 15 when `version == 5.5.1`; `defaultVersion` from `tree.header.source_version` (starts with `7` -> `7.0`, else `5.5.1`). Tree revision increments on every applied group and on relink.

- [ ] **Step 1: Write failing tests.**
  - `import_creates_project_and_report` (counts, producer `ANCESTRY`, `unmappedTags["_MTTAG"] == 1`).
  - `import_copies_media_and_rewrites_paths` (fixture with `photo.jpg` and `media_roots` -> file under `media/`, `Media.sha256` set, `Media.path` relative).
  - `import_missing_media_listed`.
  - `import_progress_phases_in_order`.
  - `import_into_existing_conflicts`.
  - `import_in_onedrive_path_reports_warning`.
  - `import_report_caps_lists_and_flags_truncated` (6000 unparsed dates -> 5000 + `truncated`).
  - `discard_import_only_before_edit` (after one `apply`, `discard_import` -> `Conflict`).
  - `relink_media_basename_search_and_undo` (relinks 1 of 2; `stillMissing` has 1; undo restores unresolved state).
  - `preview_default_is_placeholder_and_counts_undated_person_as_hidden` (Review Focus #2).
  - `preview_include_all_counts_zero_hidden`.
  - `preview_mentions_and_group_media`.
  - `export_requires_current_token` (edit after preview -> `Stale`).
  - `export_file_has_no_living_surname_in_placeholder_mode` (same assertions as Plan 1 CLI test, via the Session).
  - `export_version_follows_source` and `export_atomic_write_leaves_no_tmp_on_success`.
  - `export_matches_cli_output` (bytes equal `gt export` for the same options: both call the same library path).
  - `save_copy_includes_living_and_media` and `save_copy_opens_as_project`.
- [ ] **Step 2: Run** `cargo test -p gt-session --test transfer` -> FAIL. **Step 3: Implement.** **Step 4: Run** -> PASS.
- [ ] **Step 5: Commit** `feat(session): import with report, privacy-previewed export, relink media, save copy`.

### Task 24: `gt-session` chart windows and outline

**Needs:** Task 18, Task 20.

**Files:**
- Create: `crates/gt-session/src/charts.rs`
- Test: `crates/gt-session/tests/charts.rs`

**Interfaces:**
- Consumes: `gt_layout::{pedigree, descendants, Layout, ChartSource, Metrics}`.
- Produces:
```rust
struct TreeSource<'a> { tree: &'a Tree, index: &'a PersonIndex }      // impl ChartSource: preferred parent family, male parent first when exactly one is male; unions from families_as_partner
impl Session {
    pub fn chart_open(&mut self, req: &ChartRequest) -> Result<ChartMeta, ApiError>;   // generations clamped (pedigree 1..=12, descendants 1..=8); max_nodes 20_000; keeps at most 2 open charts (LRU); records tree revision as `version`
    pub fn chart_window(&self, chart: u32, rect: &Rect, zoom: f32) -> Result<ChartWindow, ApiError>;   // Stale if revision changed; zoom < 0.25 returns person: None and empty `label` (dots LOD needs no text)
    pub fn chart_outline(&self, chart: u32, parent: Option<u32>, limit: usize) -> Result<Vec<OutlineItem>, ApiError>;   // limit clamped to 100
    pub fn chart_close(&mut self, chart: u32);
}
```
Outline labels: pedigree `"{ahnen} {Father|Mother|Mother's father|Father's mother|Great-grandfather ...}"` derived from the Ahnentafel number path (1 "Self", 2 "Father", 3 "Mother", 4 "Father's father", 5 "Father's mother", 6 "Mother's father", 7 "Mother's mother", deeper: "{n}x great-" prefixes built from the bit path, e.g. 8 "Father's father's father"); descendants `"Child {k}"`, `"Partner"`, `"Grandchild {k}"`. A dup node's label ends with "(appears earlier)".

- [ ] **Step 1: Write failing tests:** `open_pedigree_meta` (node count for a known 3-generation tree, `rootNode` set, `truncated == false`); `window_returns_only_intersecting_nodes_and_their_edges`; `low_zoom_strips_text` (zoom 0.2 -> every `person` is `None`); `stale_after_edit` (apply a command, `chart_window` -> `ErrorCode::Stale`); `lru_keeps_two_charts` (third open closes the oldest, old id -> `NotFound`); `cycle_chart_terminates_with_dup`; `truncation_at_20000` (synthetic 30k descendants -> `truncated`, nodes == 20000); `outline_labels_ahnentafel` (`2 Father`, `6 Mother's father`); `outline_limit_clamped`; `outline_children_lazy` (parent None -> root's direct items; parent Some -> that node's children); `chart_window_under_8ms_10k` (`#[ignore]`, release); `generations_clamped`.
- [ ] **Step 2: Run** `cargo test -p gt-session --test charts` -> FAIL. **Step 3: Implement.** **Step 4: Run** -> PASS.
- [ ] **Step 5: Commit** `feat(session): chart windows and outline over gt-layout`.

### Task 25: Media pipeline: attach, thumbnails, EXIF, `gtmedia:` protocol payloads

**Needs:** Task 21; Plan 1 Task 14 (`add_media_file`).

**Files:**
- Create: `crates/gt-session/src/{thumbs.rs, exif.rs}`; extend `ops/media.rs`
- Test: `crates/gt-session/tests/media.rs`, fixtures `tests/fixtures/media/{photo-exif.jpg, tiny.png, scan.tif, doc.pdf, zero.bin, fake.jpg}` (all generated by a test helper, no real photos committed)

**Interfaces:**
- Produces:
```rust
pub enum ThumbSize { Thumb /*256 px longest side*/, Preview /*1600*/ }
impl Session {
    pub fn attach_files(&mut self, target: OwnerRef, paths: &[PathBuf]) -> Result<AttachResult, ApiError>;   // one ChangeSet "Attach media" for all successes; failures listed per file
    pub fn media_bytes(&self, id: Uuid, size: MediaSize /* Thumb|Preview|Original */) -> Result<(Vec<u8>, String /*mime*/), ApiError>;   // thumbs cached at thumbs/<sha>-<size>.jpg (JPEG q82), regenerable
    pub fn exif_suggestion(&self, id: Uuid) -> Result<Option<ExifSuggestion>, ApiError>;   // DateTimeOriginal -> "12 MAR 1901" style raw, GPS -> signed lat/lon
}
```
- Accepted types by content sniffing, not extension: JPEG, PNG, WebP, GIF, TIFF (decoded through `image`), PDF (stored; thumbnail is `None`, UI shows a document icon). Anything else -> failure reason `"unsupported type"`; 0-byte -> `"file is empty"`; directory -> `"folders are not attached; drop the files inside"`; unreadable -> `"cannot read file"`. Identical content already attached to the same target -> failure `"already attached"` (content dedup is automatic in `add_media_file`). Decoding is bounded: images over 100 megapixels fail with `"image too large to preview"` but are still stored and attached. Resizing uses `fast_image_resize`; EXIF orientation is applied when generating thumbs; GPS and EXIF are never written into exports (GEDCOM export carries no pixels).
- Protocol payload rule for Task 26: `gtmedia://localhost/<media-uuid>/<thumb|preview|original>` (Windows/Android form `http://gtmedia.localhost/...`), response headers `Content-Type`, `Cache-Control: private, max-age=31536000, immutable` (content-addressed).

- [ ] **Step 1: Write failing tests:** `attach_mixed_batch_per_file_results` (jpg ok, `.exe` renamed `fake.jpg` with MZ header -> "unsupported type", zero.bin -> "file is empty", directory -> message, PDF ok, one ChangeSet); `attach_same_photo_twice_reports_already_attached`; `attach_non_ascii_path` (`Zoë/foto Ünï.jpg`); `thumb_dimensions_and_orientation` (EXIF orientation 6 landscape source -> portrait thumb, longest side == 256); `thumb_cached_and_regenerated_after_delete`; `preview_not_upscaled` (small image stays original size); `pdf_has_no_thumb_but_original_served`; `exif_date_gps_suggestion`; `no_exif_returns_none`; `huge_image_stored_but_no_preview`; `undo_attach_removes_links_but_keeps_file` (file stays in `media/`, content-hash dedup); `media_bytes_unknown_id_not_found`; `thumb_generation_under_80ms_for_12mp` (`#[ignore]`, release).
- [ ] **Step 2: Run** `cargo test -p gt-session --test media` -> FAIL. **Step 3: Implement.** **Step 4: Run** -> PASS.
- [ ] **Step 5: Commit** `feat(session): media attach pipeline, thumbnails and EXIF suggestions`.

### Task 26: `gt-app` commands, settings file, recents, `gtmedia:` protocol

**Needs:** Tasks 17, 20, 21, 22, 23, 24, 25.

**Files:**
- Create: `app/src-tauri/src/{state.rs, commands.rs, protocol.rs, settings_file.rs}`
- Modify: `app/src-tauri/src/lib.rs` (register commands, protocol, events), `capabilities/default.json` (unchanged list; commands are app-defined)
- Test: `app/src-tauri/tests/{commands.rs, settings.rs}`

**Interfaces:**
- Produces: `AppState { session: Mutex<Option<Session>>, last_import: Mutex<Option<ImportReportDto>>, settings: Mutex<AppSettings>, config_dir: PathBuf }`; one `#[tauri::command]` per `Api` method with the snake_case name `recent_projects, create_project, open_project, close_project, save_copy, check_location, import_gedcom, save_import_report, discard_import, relink_media, list_persons, get_person, get_family_view, search, apply, undo, redo, undo_state, history, parse_date, suggest_places, suggest_sources, last_citation, chart_open, chart_window, chart_outline, chart_close, exif_suggestion, attach_files, preview_export, export_gedcom, get_settings, set_settings`; all return `Result<T, ApiError>`; commands that do heavy work (`open_project`, `import_gedcom`, `preview_export`, `export_gedcom`, `chart_open`, `attach_files`) run inside `tauri::async_runtime::spawn_blocking`. `import_gedcom` emits the Tauri event `import-progress` with `ImportProgress`. Commands called with no open project return `ApiError{NoProject}`.
- `settings_file.rs`: app settings (`dayOrder` default `dmy`, `nameOrder` default `given-first`, `theme` default `system`, `backupsToKeep` default 10) and the recent list (max 12, most recent first, de-duplicated by canonical path) stored as JSON in the Tauri app config dir, written atomically (temp + rename). Project settings (`livingMaxAge` default 100, `homePerson`) go to `meta`; changing `livingMaxAge` recomputes the living flags for the whole tree (`set_living_cached_all`) and refreshes the index.
- `protocol.rs`: `register_uri_scheme_protocol("gtmedia", ...)` parses `/<uuid>/<size>`, calls `Session::media_bytes`, returns 404 for unknown ids, 400 for malformed paths; no path component is ever used as a filesystem path.
- Daily backup on open: `open_project` calls `backup(DailyOpen, backupsToKeep)` when the newest backup is older than 24 h.

- [ ] **Step 1: Write failing tests** (Rust, using `tauri::test::mock_builder` and a temp config dir): `every_api_method_has_a_command` (read `app/src/lib/api/types.ts`, extract `Api` method names, convert to snake_case, assert each is in the `generate_handler!` list via a `COMMAND_NAMES` const exported by `commands.rs`); `no_project_error_code`; `open_then_list_then_close`; `list_persons_rejects_nothing_over_200` (limit 10_000 returns 200); `import_emits_progress_events_in_order`; `recent_list_dedups_caps_and_marks_missing`; `settings_atomic_write_and_defaults`; `living_threshold_change_recomputes` (a person born 1930 flips from living to deceased when `livingMaxAge` goes 100 -> 80); `home_person_persists_in_project`; `gtmedia_protocol_serves_thumb_and_404` and `gtmedia_rejects_traversal` (`/../../etc/passwd`); `daily_backup_created_once_per_day`; `errors_serialise_as_code_message`.
- [ ] **Step 2: Run** `cargo test -p gt-app` -> FAIL. **Step 3: Implement** (commands are thin: lock, call `Session`, map errors). **Step 4: Run** `cargo test -p gt-app && cargo clippy -p gt-app --all-targets -- -D warnings` -> PASS.
- [ ] **Step 5: Commit** `feat(app): Tauri commands, settings and recents, gtmedia protocol`.

### Task 27: `TauriApi`, `TauriPlatform`, and switching the app over

**Needs:** Tasks 3, 26.

**Files:**
- Create: `app/src/lib/api/tauri/{TauriApi.ts, TauriPlatform.ts, errors.ts}`
- Modify: `app/src/lib/api/context.ts` (`defaultApi()`), `app/package.json` (`@tauri-apps/api`, `@tauri-apps/plugin-dialog`, `@tauri-apps/plugin-opener`)
- Test: `app/src/lib/api/tauri/TauriApi.test.ts`

**Interfaces:**
- Produces: `class TauriApi implements Api` where each method is `invoke<T>(snake_case_name, args)`; errors are normalised by `toApiError(e: unknown): ApiError` (a string or malformed payload becomes `{code:'internal', message}`); `mediaUrl(id, size)` returns `convertFileSrc(`${id}/${size}`, 'gtmedia')`; `importGedcom` subscribes to `listen('import-progress')` for the call duration and unsubscribes in `finally`; `TauriPlatform` wraps `plugin-dialog` `open`/`save` and `getCurrentWebview().onDragDropEvent` (`type 'over'` -> `onHover(true)`, `'drop'` -> `cb(paths)`, `'leave'` -> `onHover(false)`).

- [ ] **Step 1: Write failing tests** using `@tauri-apps/api/mocks` (`mockIPC`, `mockWindows`): `list_persons_args_shape` (`invoke` called with `{ q, offset, limit }` for command `list_persons`); `apply_passes_command_object_unchanged`; `string_error_becomes_internal`; `structured_error_preserved`; `import_progress_listener_cleaned_up_on_failure`; `media_url_uses_gtmedia_scheme`; `drag_drop_events_map_to_callbacks`; `every_api_method_implemented` (`Object.getOwnPropertyNames(TauriApi.prototype)` contains each key of `MockApi`'s `Api` surface; fails when one side adds a method); `default_api_picks_tauri_when_internals_present`.
- [ ] **Step 2: Run** `npm --prefix app test -- TauriApi` -> FAIL. **Step 3: Implement.** **Step 4: Run** `npm --prefix app test && npm --prefix app run check` -> PASS.
- [ ] **Step 5: Commit** `feat(app): TauriApi and TauriPlatform; app uses real commands inside Tauri`.

### Task 28: End-to-end on the real stack and performance budgets

**Needs:** everything above.

**Files:**
- Create: `app/e2e-tauri/{wdio.conf.ts, specs/flows.e2e.ts}`, `crates/gt-session/tests/budget.rs`, `scripts/make-synthetic-ged.py` (writes a 50k-person GEDCOM with Cyrillic/Hebrew/CJK names, 6% undated, shared events, media references)
- Modify: `.github/workflows/ci.yml` (nightly job `e2e-tauri` on Windows and Ubuntu with `tauri-driver`; PR jobs unchanged), `docs/spikes/2026-11-03-canvas-fps.md` (final numbers with the production renderer and real layout)

**Interfaces:** none new. WebdriverIO + `tauri-driver` (both MIT/Apache) drive the built binary; this suite is optional on PR (nightly only) because `tauri-driver` has no macOS support.

- [ ] **Step 1: Write the tests.**
  - `budget.rs` (release, `#[ignore]`, 50k synthetic project): `open_under_2s`, `list_page_under_30ms`, `search_under_100ms`, `apply_under_50ms`, `chart_window_under_8ms`, `chart_open_pedigree_8_gens_under_50ms`, `resident_memory_under_500mb` (read process RSS via `/proc/self/status` on Linux, `GetProcessMemoryInfo` skipped elsewhere), `living_recompute_under_30ms`.
  - `flows.e2e.ts`: `import_report_then_family_view` (pick the synthetic 5k-person file by setting an env-provided path through a test-only `--open-dialog-result` argument; report shows counts and the OneDrive warning when the project dir is under a `OneDrive` folder; open tree; heading shows the home person); `add_child_edit_undo_redo_persists_across_restart` (add child with birth date `abt 1850`, quit, relaunch, open project, child exists with preview text "about 1850"; Ctrl+Z; restart; child gone); `export_preview_matches_file` (preview says N placeholders; the exported file contains exactly N `Living` names and no living surname); `drop_files_attach` (use `Platform` test hook to emit a drop of 3 files with one `.exe`; two thumbnails appear, one error line); `pedigree_outline_and_chart_agree` (outline item count for generation 3 equals the node count at gen 3 reported by the chart live region); `rtl_and_cjk_names_visible` (names present in list text content and pass bidi check via `dir="auto"`).
- [ ] **Step 2: Run** `cargo test -p gt-session --release --test budget -- --ignored` on a mid-range laptop; `npm --prefix app run tauri -- build --debug --no-bundle` then `npx wdio app/e2e-tauri/wdio.conf.ts` -> expected FAIL first where budgets or flows are missed.
- [ ] **Step 3: Fix** whatever misses a budget at the source (index structure, caching), never by raising a budget; record the final numbers in the spike document.
- [ ] **Step 4: Run** the full verification: `cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && npm --prefix app run check && npm --prefix app run lint && npm --prefix app test && npm --prefix app run e2e` -> all green; budgets and e2e green on at least Windows and Ubuntu.
- [ ] **Step 5: Commit** `test: tauri e2e flows and 50k performance budgets`.

---

## Spec coverage notes (deviations and calls)

- **Task count:** 28 (16 frontend-only, 12 Rust or wiring). Spec §5.1 Must items map as: tree projects (6, 23, 26), person editor and quick add (9, 21), family editor (11), event editor and date preview (10, 22), source and citation editor with reuse/clone (12), person list (7, 20), family view (8), pedigree and descendant charts with outline (4, 14, 17, 18, 24), search (5, 20), media (13, 25), undo/redo and history (19, 21), privacy and export preview (15, 23), settings (15, 26), import report (6, 23). Face tagging, stories editor, HTML site, reports and duplicate merge are out of scope.
- **[call] `gt-session` crate** is added so that everything except the Tauri glue is testable without a webview; `gt-app` stays thin.
- **[call] The app keeps the whole `Tree` in memory** (Plan 1 only offers whole-tree `load_tree`), serving paged DTOs from indexes; this fits the 500 MB budget at 50k people and keeps IPC paged. SQL is used for writes, search and history only.
- **[call] Undo stack is per session** (200 groups); history is persisted in `change_log`. Import is never an undo step: "Undo import" deletes the just-created project, allowed only before the first edit.
- **[call] Schema v2** (`0002_edit.sql`) instead of editing Plan 1's `0001_init.sql`, so a Plan 1 project upgrades cleanly and the migration path is tested.
- **[call] Media relinking** lives in `gt-session` (basename search under a chosen folder) rather than re-running the GEDCOM import, because `gt-gedcom`'s resolver is crate-private.
- **[call] No axe-core** (MPL-2.0): accessibility is checked with Testing Library roles, `dom-accessibility-api` names, Svelte a11y lint rules at error level, and manual keyboard flows in Playwright.
- **[call] Fonts** (Source Serif 4, Atkinson Hyperlegible Next) are OFL-1.1 assets, bundled locally; confirm this is acceptable under the "MIT/Apache" rule, otherwise swap to system font stacks (tokens already list the fallbacks).
- **[call] PDF thumbnails and HEIC** are not supported in v0.1 (PDF shows an icon and opens externally); GEDCOM export carries no photos, so EXIF stripping (spec §6.1) becomes relevant with GEDZIP and the HTML site in v0.2/v0.3.
- **[call] Chart orientation** is fixed (pedigree left to right, descendants top to bottom); fan and hourglass charts are later.
- **Owner's real GEDCOM** (Plan 1's note) should be placed in `tests/fixtures/private/` (gitignored) before Task 28; the synthetic generator covers CI.
