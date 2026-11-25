# Around chart print layouts (compact, sheets by branch) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** On the "Built around" screen the owner chooses a layout — as today / compact / sheets by branch — so a 300+ person chart prints readably on A3.

**Architecture:** `gt-layout` gets an opt-in `fold` pass that lays out long runs of leaf boxes in several short columns; the request carries `fold`. The front end cuts the folded scene into sheets (a pure module `sheets.ts`), lays each sheet out with the existing A3 machinery, and prints all sheets as one multi-page HTML document or saves one file per sheet.

**Tech Stack:** Rust (`crates/gt-layout`, `crates/gt-session`, ts-rs generated types), Svelte 5 + TypeScript (`app/src/lib/chart`), Vitest, cargo test.

**Spec:** `docs/superpowers/specs/2026-11-21-around-print-layouts-design.md`

## Global Constraints

- Layout "as today" (no `fold`, no sheets) must give byte-identical charts and prints: every existing gt-layout, gt-session and chart test stays green unchanged.
- A3 only; names on a sheet must print at ≥ `A3_MIN_PT` (6 pt) or carry the existing "names are small" warning (`picture.a3Small`).
- A fold run qualifies only at `n ≥ 6` consecutive leaf boxes (no partner box, nothing hanging from them) in one sibling or children group.
- A line sheet with fewer than 8 people is not made; its people stay on the sheet below.
- UI strings in en + uk only (other locales fall back to en). uk wording: «Розміщення», «як зараз», «компактно», «аркуші по гілках», «→ арк. N», «аркуш N з M», «Гілка: …». In Ukrainian a family tree is «родовід», never «дерево».
- Real family data never goes into tests, fixtures, screenshots or commits: synthetic names only.
- Commits end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. A chart where no run reaches 6 (small families): compact must equal "as today" exactly — test in Task 1.
2. Folded runs next to non-leaf siblings (a big family where only some children have their own families): non-leaves keep their places, no overlap — test in Task 1.
3. A chart built around several people (`roots`) or with nothing above the grandparents: sheets mode must give one cover sheet only, never an empty sheet — test in Task 3.
4. Pedigree collapse (the same ancestor reached through two lines; the layout marks the second as `dupOf`): the person is in full on one sheet only — test in Task 3.
5. Switching layout while a print is running, and the screen choice surviving a re-layout (opening a branch): the choice stays, the print uses the choice at the moment it was started — test in Task 4.

---

### Task 1: `fold` in the layout engine

**Files:**
- Modify: `crates/gt-layout/src/relatives.rs` (unit/contour packing used by `lay_out`, `around`, `around_with_siblings`, `around_all_kin`, `around_open`, `around_many_kin_with`)
- Modify: `crates/gt-layout/src/lib.rs` (re-export) and `crates/gt-layout/src/types.rs` if a field is needed
- Test: `crates/gt-layout/tests/around_fold.rs` (new)

**Interfaces:**
- Produces: a `fold: bool` parameter on `around_open(src, root, generations, base, open, fold: bool, m, max_nodes)` and on `around_many_kin(…, fold: bool, …)`; with an empty `open` and `fold = true`, `around_open` goes through `lay_out` too. Existing `around`, `around_with_siblings`, `around_all_kin` keep their signatures and mean `fold = false`.
- A folded run: in canonical (top-down) coordinates the run's boxes form a grid `k = ceil(n / c)` boxes across and `c` sub-rows deep, sub-row pitch = box height + `m.gap_y_small` (pick an existing small gap from `Metrics` or add `fold_gap` = 8.0); the group's contour reserves every generation row the grid reaches, so neighbours clear it through the normal `pack`. One parent bracket feeds the grid (edges of kind `Parent` from the family junction to each box, routed down the grid's left side; or `Sibling` bars per sub-row — implementer's choice, no line may cross a box).
- `c` = the smallest `c ≥ 2` such that `k ≤ 3·c` … rule is the implementer's (spec leaves it), but it must be deterministic and must reduce the whole chart's `max(w,h)/min(w,h)` toward √2 on the synthetic big chart.

- [ ] **Step 1: Write the failing tests** in `crates/gt-layout/tests/around_fold.rs`, using the synthetic source helpers the other `around_*` tests use:
  - `fold_off_is_today`: for every existing fixture used in `around.rs` / `around_open.rs`, `around_open(..., fold=false, ...)` equals the pre-change result (compare node rects and edges).
  - `small_runs_do_not_fold`: a family with 5 leaf children → identical layout with `fold=true` and `fold=false`.
  - `big_leaf_run_folds`: a person with 18 leaf children → with `fold=true` the 18 boxes occupy ≥ 2 distinct `y` (canonical) sub-rows, no two boxes overlap, every child still has a `Parent` edge from the family.
  - `non_leaves_keep_place`: 12 children where children 3 and 9 have partners and children; with fold the two non-leaves have the same `x` order relative to each other as without fold and nothing overlaps (assert no rect intersections across the whole layout, and no edge segment crossing a box).
  - `big_chart_closer_to_a3`: synthetic chart of ~300 people (4 generations up, every ancestor with 10–20 leaf siblings opened `All`) → aspect `max/min` of bounds with fold is smaller than without, and closer to √2.
- [ ] **Step 2: Run** `cargo test -p gt-layout --test around_fold` → FAIL (no `fold` parameter).
- [ ] **Step 3: Implement** the fold pass in `relatives.rs`: detect qualifying runs when building a group's units (leaf = no partners, no children units, not on the line, not `dup_of`), give the run a grid contour, place boxes in the grid in `place_units`, and emit edges. Thread `fold` from `around_open` / `around_many_kin`.
- [ ] **Step 4: Run** `cargo test -p gt-layout` → all PASS (old tests unchanged).
- [ ] **Step 5: Commit** `feat(layout): fold long runs of leaf boxes into columns (around charts, opt-in)`.

### Task 2: `fold` in the request

**Files:**
- Modify: `crates/gt-session/src/dto.rs` (`ChartRequest`), `crates/gt-session/src/chart.rs` (calls near line 810), regenerate TS (`app/src/lib/api/generated/ChartRequest.ts` via the project's ts-rs export test)
- Modify: `app/src/lib/api/mock*.ts` (MockApi: accept and ignore or honour `fold`)
- Test: `crates/gt-session/tests/around_chart.rs` (add cases)

**Interfaces:**
- Consumes: Task 1 `around_open(..., fold, ...)`, `around_many_kin(..., fold, ...)`.
- Produces: `ChartRequest.fold?: boolean` — "Chart built around people only: lay long runs of people with nothing below them in several short columns. Absent or false: as before."

- [ ] **Step 1: Failing tests** in `around_chart.rs`: `fold_absent_equals_before` (same boxes as a request without the field) and `fold_true_reaches_layout` (a synthetic person with 18 children → boxes on ≥ 2 sub-rows).
- [ ] **Step 2: Run** `cargo test -p gt-session --test around_chart` → FAIL.
- [ ] **Step 3: Implement**: add the field (`#[serde(default, skip_serializing_if = "Option::is_none")] #[ts(optional)] pub fold: Option<bool>`), pass `fold.unwrap_or(false)` through; regenerate the TS types the way the repo does (`cargo test -p gt-session export_bindings` or the repo's script — check `package.json`/`CLAUDE.md`).
- [ ] **Step 4: Run** `cargo test -p gt-session` and `cd app && npx vitest run src/lib/api` → PASS.
- [ ] **Step 5: Commit** `feat(session): ChartRequest.fold for around charts`.

### Task 3: cutting a scene into sheets (pure module)

**Files:**
- Create: `app/src/lib/chart/sheets.ts`
- Test: `app/src/lib/chart/sheets.test.ts`

**Interfaces:**
- Consumes: `Scene` from `picture.ts` (nodes with `id`, `gen`, `rect`, `dupOf`; edges `parent` / `partner` / `sibling` / `dup`), `layoutA3`, `sceneBounds`, `compactX`, `A3_MIN_PT`.
- Produces:
  ```ts
  export interface Sheet {
    no: number;                 // 1-based; 1 is the cover
    top: number | null;         // lineage node id the sheet is cut at (null: cover)
    ids: Set<number>;           // nodes shown in full
    refs: Map<number, number>;  // node id -> sheet number, drawn as a reference mark on this sheet
    scene: Scene;               // the sub-scene (ids + refs nodes, edges between them), compacted with compactX
  }
  export function splitSheets(scene: Scene, centres: number[], opts?: { minPeople?: number; fits?: (s: Scene) => boolean }): Sheet[];
  ```
  `minPeople` defaults to 8; `fits` defaults to `s => !layoutA3(sceneBounds(s), 'auto', true).tooSmall`.

Algorithm (fixed by the spec, decisions 2–4):
- Lineage = `centres` plus every node reached from them by walking `parent` edges upward (nodes with `dupOf` count as their original).
- Every other node hangs from the lineage node it reaches first in a multi-source BFS from all lineage nodes over `parent`/`partner`/`sibling` edges (undirected) that never steps onto another lineage node; ties go to the lower-generation-distance source, then father's side first (the parent reached through the first `parent` edge in scene order).
- Cover = lineage nodes with `gen ≥ -2` and everything hanging from them, plus all descendants of the centres.
- For each gen −2 lineage node G with ancestors: a candidate sheet = G's ancestors + everything hanging from them; G itself goes in `refs` → 1 (cover). If the candidate has fewer than `minPeople` full nodes, its people join the cover instead. If it does not `fits`, split it again at its gen −3 lineage nodes the same way (recursively upward; the split-off sheet's foot ref points to the sheet that holds its child).
- A node with `dupOf` is shown in full only where its original is; elsewhere it is a ref to that sheet.
- Sheet numbers: cover 1, then depth-first, father's side before mother's.

- [ ] **Step 1: Failing tests** in `sheets.test.ts` with synthetic scenes built by a small helper `chain(...)` in the test file:
  - `everyone_once`: union of all `ids` = all non-dup nodes; `ids` sets pairwise disjoint.
  - `cover_holds_three_generations`: centre, parents, grandparents, an aunt with her opened children, and the centre's children are on sheet 1.
  - `line_sheet_per_grandparent`: 4 grandparents each with 3 generations of ancestors and 10 opened siblings → 5 sheets; each line sheet's `refs` has its grandparent → 1.
  - `tiny_line_stays_on_cover`: a grandparent with 1 parent and nothing opened → no sheet for that line; the parent is on the cover.
  - `splits_when_too_big`: with `fits: s => s.nodes.length <= 40`, a 200-person line is split into sheets at gen −3, each `fits`, refs chain correctly.
  - `several_centres_or_no_ancestors`: two centres, nothing above parents → exactly one sheet.
  - `pedigree_collapse`: an ancestor reached through two lines (second occurrence `dupOf`) → full on one sheet, ref on the other.
- [ ] **Step 2: Run** `cd app && npx vitest run src/lib/chart/sheets.test.ts` → FAIL.
- [ ] **Step 3: Implement** `splitSheets` as above.
- [ ] **Step 4: Run** the same command → PASS.
- [ ] **Step 5: Commit** `feat(chart): cut an around chart into A3 sheets by branch`.

### Task 4: the layout choice on the "Built around" screen

**Files:**
- Modify: `app/src/lib/views/ChartView.svelte` (state next to `chosenKin`), `app/src/lib/chart/ChartCanvas.svelte` (prop, control next to the kin control, request at line ~332)
- Modify: `app/src/lib/i18n/en.ts`, `app/src/lib/i18n/uk.ts` (keys below)
- Test: `app/src/lib/chart/ChartCanvas.test.ts` (or a new `AroundLayout.test.ts` using `testing/Host.svelte`)

**Interfaces:**
- Produces: `export type PrintLayout = 'today' | 'compact' | 'sheets'` in `app/src/lib/chart/sheets.ts`; ChartCanvas props `printLayout?: PrintLayout` and `onPrintLayout?: (v: PrintLayout) => void` (shown only for `kind === 'around'`).
- Request: `fold: true` added only when `printLayout !== 'today'` (both the screen request and `chartRequest(d, true)` for prints).
- i18n keys: `chart.layout` "Layout" / «Розміщення»; `chart.layoutToday` "As today" / «як зараз»; `chart.layoutCompact` "Compact" / «компактно»; `chart.layoutSheets` "Sheets by branch" / «аркуші по гілках».

- [ ] **Step 1: Failing tests**: the control shows the three choices only on the around chart; choosing "Compact" sends a request with `fold: true` (MockApi records requests); "As today" sends none; the choice survives opening a branch (`onOpenings`); a print started under "Compact" keeps `fold: true` even if the choice changes while it runs.
- [ ] **Step 2: Run** `cd app && npx vitest run src/lib/chart/ChartCanvas.test.ts` → FAIL.
- [ ] **Step 3: Implement** the state, control (same style as the kin control), and request field; remember the choice in `a3Pref.svelte.ts` style storage (optional, spec allows).
- [ ] **Step 4: Run** the test file and `npx vitest run src/lib/i18n` → PASS.
- [ ] **Step 5: Commit** `feat(ui): layout choice on the around chart (as today / compact / sheets)`.

### Task 5: printing and saving the set of sheets

**Files:**
- Modify: `app/src/lib/chart/ChartCanvas.svelte` (`buildA3` → `buildA3For(scene, meta, title, refs)`; `pictureA3`), `app/src/lib/chart/a3.ts` (`printDocumentPages`), `app/src/lib/chart/pictureExport.ts` (`printPictures`, `savePictures`)
- Modify: `app/src/lib/i18n/en.ts`, `uk.ts`: `picture.sheetOf` "Sheet {n} of {m}" / «аркуш {n} з {m}», `picture.branch` "Branch: {name}" / «Гілка: {name}», `picture.sheetRef` "→ sheet {n}" / «→ арк. {n}», `picture.sheetSuffix` " — sheet {n}" / « — арк. {n}».
- Test: `app/src/lib/chart/a3.test.ts`, `app/src/lib/chart/pictureExport.test.ts`, `app/src/lib/chart/ChartPicture.test.ts`

**Interfaces:**
- Consumes: `splitSheets`, `Sheet` (Task 3); `PrintLayout` (Task 4).
- Produces:
  - `printDocumentPages(pages: { svg: string; orientation: A3Orientation }[], fontCss?: string): string` — one HTML document, one `@page` per orientation (named pages `@page land{size:A3 landscape}` / `@page port{size:A3 portrait}`), each SVG in its own block with `break-after: page` except the last.
  - `printPictures(args: { build: (format) => Promise<BuiltPicture[] | null>; setBusy }): Promise<void>` and `savePictures(ctx, { fileName, build, setBusy })`: one save dialog for sheet 1, the others written next to it with `picture.sheetSuffix`.
- Under "sheets": the print chart is requested with `fold: true`, laid out as today (direction choice, card stretch), then `splitSheets(scene, [rootNode, ...chosenNodes])`; each sheet gets its own `layoutA3` and title: the cover keeps the chart title, other sheets «Гілка: <surname of the sheet's top person>» + «аркуш N з M» in the right-hand part of the title block. Reference nodes are drawn as normal cards plus a small tag `picture.sheetRef` under the card (the paint callback draws it after `draw`). Warning `picture.a3Small` lists the sheet numbers that are still too small.
- Under "compact": exactly today's single-sheet path with `fold: true`.

- [ ] **Step 1: Failing tests**:
  - `a3.test.ts`: `printDocumentPages` with 3 pages (landscape, portrait, landscape) has 3 `<svg`, 2 page breaks, both named `@page` rules.
  - `pictureExport.test.ts`: `savePictures` with 3 built pictures asks once, writes `X.png`, `X — арк. 2.png`, `X — арк. 3.png` (uk) through the platform mock.
  - `ChartPicture.test.ts`: with layout "sheets" on a synthetic big chart, printing produces one document with M pages and each non-cover page's text contains «аркуш N з M» and «→ арк. 1»; with "as today" the printed HTML is identical to the current output (snapshot of the existing path).
- [ ] **Step 2: Run** `cd app && npx vitest run src/lib/chart` → FAIL on the new cases.
- [ ] **Step 3: Implement** as above.
- [ ] **Step 4: Run** `cd app && npx vitest run` (whole suite) → PASS; `cd app && npx svelte-check` → no new errors.
- [ ] **Step 5: Commit** `feat(print): A3 sheets by branch — one print document, one file per sheet`.

### Task 6: sheet frames on screen, screenshots, ship

**Files:**
- Modify: `app/src/lib/chart/ChartCanvas.svelte` / `renderer.ts` (overlay)
- Test: `app/src/lib/chart/renderer.test.ts`

**Interfaces:**
- Consumes: `splitSheets` on the screen scene when `printLayout === 'sheets'`.
- Produces: a thin dashed frame around each sheet's nodes' bounds with its number in a corner («арк. N»), drawn under the cards; nothing when the layout is not "sheets".

- [ ] **Step 1: Failing test** in `renderer.test.ts`: with an overlay of 3 sheets the SvgContext output has 3 dashed rects and the labels «арк. 1..3»; with none, output equals today's.
- [ ] **Step 2: Run** → FAIL. **Step 3: Implement.** **Step 4: Run** `cd app && npx vitest run` → PASS; `cargo test --workspace` → PASS.
- [ ] **Step 5: Screenshots** with a synthetic 300-person tree (never the owner's file): around chart as today / compact / sheets on screen, and two printed sheets (print to PDF or SVG render) into the lane's `Saved/print-layouts/`.
- [ ] **Step 6: Commit** `feat(ui): sheet frames on the around chart; screenshots`.
