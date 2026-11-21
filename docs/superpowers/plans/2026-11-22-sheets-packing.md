# Packed sheets for the "Built around" print — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** "Sheets by branch" prints a big "Built around" chart as few A3 pages as readability allows: one page when the whole chart reads at 8 pt, otherwise pieces cut only where needed, each in its own best direction, compacted on both axes and shelf-packed several to a page, with captions and «→ арк. N» references.

**Architecture:** Three pure modules do the work and are tested on committed synthetic family-shaped layouts: `sheets.ts` cuts one layout into pieces (hang tree, largest block out first, two-way references); `pieceView.ts` cuts each piece out of the four direction layouts, compacts it on both axes and picks its direction; `pack.ts` shelf-packs the pieces onto A3 pages with one scale per page; `sheetPlan.ts` ties them together. `ChartCanvas.svelte` only lays out the four print charts, calls `planSheets`, and paints each page (several `draw` calls on one canvas).

**Tech Stack:** Svelte 5 + TypeScript (`app/src/lib/chart`), Vitest; Rust only for the fixture writer (`crates/gt-layout/tests`).

**Spec:** `docs/superpowers/specs/2026-11-21-around-print-layouts-design.md`, section "Amendment 21.11.2026 — packed sheets" (it overrides the earlier Decisions 2 and 4 and section B where they differ).

## Global Constraints

- «як зараз» and «компактно» print and save byte for byte as today: `a3Golden.test.ts` (`testing/a3-today-print.golden.html`) and every existing a3/picture test stay green without edits to their expectations. `compactX`, `layoutA3`, `printDirection`, `printStretch`, `buildA3` keep their behaviour.
- Target name size `SHEET_TARGET_PT = 8`; floor `A3_MIN_PT = 6`; below 6 pt only for a piece marked unsplittable, and then the existing warning (`picture.a3SmallSheets` + `picture.a3Small`) lists its page.
- A3 only; scale cap `A3_MAX_SCALE = 1.5`; slot caption band `CAPTION_MM = 6` with text `CAPTION_PT = 9`; gap `PIECE_GAP_MM = 6`; caption ellipsis at `CAPTION_MAX_MM = 140`; smallest movable block `PIECE_MIN_PEOPLE = 3`.
- Page header: chart title left; right `picture.sheetOf` («аркуш N з M») + « · » + tree name and date. Piece caption: `picture.branchTitle` with name `<surname> (<given>)`, or `<surname>` without a given name; the cover has none. Reference tag: `picture.sheetRef` («→ арк. N»). Screen frame label: `picture.sheetFrame`.
- No new i18n keys are expected; if one is needed, en + uk only (other locales fall back to en), never Russian; in Ukrainian a family tree is «родовід».
- Saving a set: one save dialog, a NEW folder (`api.makePictureFolder`), files « — арк. N», the folder opened in Explorer — `savePictures` unchanged.
- Synthetic data only: fixtures, tests, screenshots never hold real names. Fixture names come from the loader's fixed lists.
- Work in the worktree `C:/Users/User/gene-tree/.claude/worktrees/sheets-pack` (branch `lane/sheets-pack`); `app/node_modules` is a junction to the main checkout's: never run `npm install`. `CARGO_TARGET_DIR=E:/gene-tree-target/sheets-pack`.
- Run gates in the foreground; commit before the final gate. Status lines in English or Ukrainian only.
- Commits end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. **A reference tag beside a box after compaction**: the y squeeze could pull a neighbour onto the «→ арк. N» text. Expect: tags lie inside the piece bounds and touch no other box. Test `tags_inside_bounds_and_clear` in Task 3.
2. **Mixed directions on one page**: pieces drawn with their own direction and camera on one canvas; the second `draw` must not wipe the first, and slots must not overlap. Tests `no_overlap_*` in Task 4 and `page_draws_every_piece` in Task 6.
3. **Pedigree collapse and several centres**: a repeated ancestor (`dupOf`) and centres in different rows must still give everyone in full once and references to the right page. Tests `pedigree_collapse`, `several_centres_in_different_rows` in Task 2 and the fam gates in Task 5.
4. **A fixed page orientation chosen** (portrait or landscape instead of auto) with pieces whose best shape is the other way: every page has the chosen orientation, no piece is lost, oversize pieces go alone. Test `fixed_orientation_everywhere` in Task 4.
5. **An unsplittable giant** (one couple with 300 leaf children): the piece prints alone below the target, the warning names its page, and no other page drops below 8 pt because of it. Tests `unsplittable_giant` in Task 2 and `giant_page_warned_others_at_target` in Task 5.

---

### Task 1: Family-shaped fixtures, node keys, compaction on both axes

**Files:**
- Create: `crates/gt-layout/tests/fam_fixtures.rs` (from the investigation harness `zz_sheets_inv.rs` in the investigation folder: same generator, same five cases minus `fam-s`)
- Modify: `crates/gt-layout/Cargo.toml` (`[dev-dependencies] flate2.workspace = true`)
- Create: `app/src/lib/chart/testing/fam/fam-{a,b,c,d}-{top,bottom,left,right}.json.gz` (16 files, written by the Rust test)
- Create: `app/src/lib/chart/testing/famScenes.ts`
- Modify: `app/src/lib/chart/picture.ts` (add `compactBoth`), `app/src/lib/chart/sheets.ts` (add `nodeKeys`)
- Test: `app/src/lib/chart/picture.test.ts`, `app/src/lib/chart/famScenes.test.ts`

**Interfaces:**
- Produces:
  - `loadFam(name: 'fam-a' | 'fam-b' | 'fam-c' | 'fam-d', dir: ChartDirection): { scene: Scene; root: number }` — nodes carry `person.id = 'k<key>'` and synthetic names from the harness lists (`GIVEN`, `SUR`); read with `zlib.gunzipSync`.
  - `export const FAMS = ['fam-a', 'fam-b', 'fam-c', 'fam-d'] as const` and `TODAY_SHEETS = { 'fam-a': 9, 'fam-b': 9, 'fam-c': 5, 'fam-d': 15 }` (today's counts, best mode, for the log in Task 5).
  - `compactBoth(scene: Scene, gap = 40, occupied?: (n: ChartNode) => Rect): Scene` in `picture.ts`: `compactX`'s monotone squeeze along x, then the same along y; `occupied(n)` (default: the box) is the span a node holds on each axis.
  - `export type NodeKey = string; nodeKeys(scene: Scene): Map<number, NodeKey>` in `sheets.ts`: `p:<personId>` for a box with a person and no live `dupOf`; `d:<personId>><key of the box at the other end of its first parent edge, else first partner edge>` for a repeat; `l:<label>:<gen>` for a box without a person, `#2`, `#3`… appended in scene order when one repeats.

- [ ] **Step 1: Write the fixture writer** `fam_fixtures.rs`: test `write_fam_fixtures`, `#[ignore = "writes app fixtures: FAM_FIXTURES_DIR=<dir>"]`, for fam-a (seed 11, 6 gens, sib 8, big [5], marry 55), fam-b (23, 6, 10, [4], 30), fam-c (37, 5, 12, [2, 3], 40), fam-d (51, 7, 12, [6], 20) and the four directions: `around_open(&s, 1, 255, AroundBase::All, &[], true, &m, 100_000)` with `Metrics::compact().for_direction(Direction::Top, d)` then `orient`, JSON as in the harness with coordinates rounded to 0.1, gzip-compressed. Run it: `CARGO_TARGET_DIR=E:/gene-tree-target/sheets-pack FAM_FIXTURES_DIR=C:/Users/User/gene-tree/.claude/worktrees/sheets-pack/app/src/lib/chart/testing/fam cargo test -p gt-layout --test fam_fixtures -- --ignored`. Expected: 16 files, fam-d ≈ 1,568 nodes per direction, total < 600 KB.
- [ ] **Step 2: Write the failing tests.**
  - `famScenes.test.ts` `loads_every_fixture`: each of 16 loads; every parent edge's ends exist; fam-d has ≥ 1,500 nodes.
  - `famScenes.test.ts` `keys_match_across_directions`: for each fam, `nodeKeys` values are unique within a direction and the four key sets are equal.
  - `picture.test.ts` `compactBoth_keeps_order_and_gaps`: on `loadFam('fam-a','left').scene`, the x order and the y order of every pair of nodes are unchanged, no two boxes overlap, every edge still starts and ends on the same box sides (endpoints map with their boxes), and `sceneBounds` h is at most half the input's.
  - `picture.test.ts` `compactBoth_respects_occupied`: two boxes 300 apart on x with `occupied` widening the first by 120 to the right → the gap from the occupied edge is exactly `gap`.
  - `picture.test.ts` `compactBoth_without_y_gaps_equals_compactX`: a scene whose y gaps are all ≤ 40 gives exactly `compactX(scene)`.
- [ ] **Step 3: Run** `cd app && npx vitest run src/lib/chart/picture.test.ts src/lib/chart/famScenes.test.ts` → FAIL (`compactBoth`, `loadFam`, `nodeKeys` missing).
- [ ] **Step 4: Implement** `loadFam`, `compactBoth` (x pass is `compactX`'s mapping generalised to take spans from `occupied`; y pass is the same on transposed coordinates), `nodeKeys`.
- [ ] **Step 5: Run** the same command → PASS; `cd app && npx vitest run src/lib/chart` → PASS (nothing else changed).
- [ ] **Step 6: Commit** `test(print): family-shaped fixtures, node keys, compaction on both axes`.

### Task 2: Cutting a layout into pieces (`cutPieces`)

**Files:**
- Modify: `app/src/lib/chart/sheets.ts` — add `cutPieces` beside `splitSheets`, reusing its lineage / generation-distance / owner-search code (move it into shared helpers). The old `splitSheets`, `Sheet`, `SplitOptions`, `SHEET_MIN_PEOPLE`, `fitsA3`, `branchNames` and the old `screenFrames` stay until Task 6 removes them with their tests, so the app keeps building between tasks.
- Test: `app/src/lib/chart/sheets.test.ts` (new `cutPieces` cases; reuse the `Tree`, `chain`, `base` helpers)

**Interfaces:**
- Consumes: `Scene`; `nodeKeys` (Task 1) is not needed here — pieces are node ids of the cut scene.
- Produces:
  ```ts
  export const PIECE_MIN_PEOPLE = 3;
  export interface Piece {
    index: number;                 // 0 = cover; others in the order they were moved out
    top: number | null;            // node the block was cut at (caption person); null for the cover
    parent: number | null;         // index of the piece it was moved out of
    full: Set<number>;             // shown in full
    refs: Map<number, number>;     // reference box node id -> index of the piece holding that person in full
    plain: Set<number>;            // repeats (dupOf) whose first occurrence is on this piece
    unsplittable: boolean;         // did not fit and had no block left to move
  }
  export interface CutOptions { fits: (p: Piece) => boolean; minPeople?: number }
  export function cutPieces(scene: Scene, centres: number[], o: CutOptions): Piece[];
  ```
- Rules (from the amendment, "Pieces" and "References"):
  - Hang tree: lineage nodes hang from their lineage child with the smaller generation distance (father's side first on a tie); every other node from the owner-search step that reached it (the existing multi-source search, recording the predecessor).
  - Movable block at node x: the hang subtree of x restricted to the piece, when x is not on the lineage, or x is on the lineage with generation distance ≥ 3; it must hold ≥ `minPeople` people in full and fewer than the piece itself.
  - Loop: a queue starting with the whole scene as piece 0; while `!fits(piece)`, move out the largest movable block (full count; ties: lower lineage rank of the block's lineage owner, then lower node id) as a new piece, recomputing the references of both; queue the new piece. No block left → `unsplittable = true`.
  - References: for every edge of kind `parent`, `partner` or `sibling` whose ends are in full on different pieces, each end's piece gets the other end as a ref pointing at the other piece. Repeats keep today's rule (plain when the original is on the same piece, else a ref to the original's piece).

- [ ] **Step 1: Write the failing tests** (`always = () => true`; `upTo(n) = (p) => p.full.size + p.refs.size <= n`):
  - `whole_fits_is_one_piece`: `base` + `chain(…, 3, 2)` with `always` → 1 piece, every node in `full`, no refs.
  - `everyone_once`: same tree with `upTo(15)` → `full` sets disjoint, union = all non-repeat nodes.
  - `spine_stays_on_cover`: with `upTo(15)`, c, f, m and the four grandparents are in piece 0's `full`.
  - `largest_block_moves_first`: centre + parents + an aunt (child of ff, fm) with 3 children each with partner and 2 children (13 people) + an uncle with 4 → with `upTo(20)` exactly the aunt's block moves (piece 1, `top` = aunt), the uncle stays on the cover.
  - `collateral_head_leaves_reference`: in that case piece 0's `refs` has aunt → 1; piece 1's `refs` has ff → 0 and fm → 0; the aunt is in piece 1's `full` only.
  - `line_cut_is_two_way`: grandparent ff with `chain(ff, 3, 4)` and `upTo(25)` → a moved line piece with `top` at generation distance 3; it refs ff → 0 (foot), and piece 0 refs its top person → that piece.
  - `small_blocks_never_move`: a block of 2 people is never a piece's `top`; no piece except 0 has fewer than 3 in `full`.
  - `unsplittable_giant`: a centre with partner and 300 leaf children, `upTo(50)` → 1 piece, `unsplittable` true.
  - `pedigree_collapse`: the existing case (second occurrence `dupOf`) → the ancestor in full on one piece; the repeat is a ref to that piece or plain on the same piece.
  - `several_centres_in_different_rows`: the existing `centres_in_different_rows` tree, `upTo(12)` → everyone once, both centres on piece 0.
  - `refs_point_to_holders`: for every piece and every `[id, k]` of `refs`, `pieces[k].full.has(id)` (or, for a repeat, holds its `dupOf`).
  - `fam_fixtures_everyone_once`: for each fam (left), `cutPieces` with `upTo(120)` → everyone once and `refs_point_to_holders` holds.
- [ ] **Step 2: Run** `cd app && npx vitest run src/lib/chart/sheets.test.ts` → FAIL (`cutPieces` missing).
- [ ] **Step 3: Implement** `cutPieces` per the rules above. Keep the piece loop O(n) per move (subtree sizes from one post-order pass over the piece).
- [ ] **Step 4: Run** the same command → PASS.
- [ ] **Step 5: Commit** `feat(print): cut the around chart into pieces, largest block first, two-way references`.

### Task 3: A piece in its best direction (`pieceView.ts`)

**Files:**
- Create: `app/src/lib/chart/pieceView.ts`
- Modify: `app/src/lib/chart/a3.ts` (add `SHEET_TARGET_PT = 8` and `targetScale(pt = SHEET_TARGET_PT): number` = `pt / (NAME_PX * 0.75)`)
- Test: `app/src/lib/chart/pieceView.test.ts`

**Interfaces:**
- Consumes: `Piece`, `NodeKey`, `nodeKeys` (Tasks 1–2), `compactBoth` (Task 1), `refTagAt`, `layoutA3`, `sceneBounds`.
- Produces:
  ```ts
  export interface DirScene { direction: ChartDirection; scene: Scene; keys: Map<number, NodeKey> }
  export interface PieceView {
    direction: ChartDirection;
    scene: Scene;                  // full + refs + plain boxes of the piece in this direction, compactBoth-ed
    refs: Map<number, number>;     // node id in `scene` -> piece index holding it in full
    bounds: Rect;                  // sceneBounds of `scene` with every ref tag rect
    standalone: A3Layout;          // layoutA3(bounds, choice, header)
  }
  export interface ViewOptions { choice: A3Choice; header: boolean; tagW: number /* tag text width, world units at the target scale */; tagH: number }
  export function viewsOf(piece: Piece, cut: DirScene, dirs: DirScene[], o: ViewOptions): PieceView[]; // one per direction in `dirs`
  export function fitsTarget(views: PieceView[]): boolean;              // some view's standalone.scale >= targetScale()
  export function pickView(views: PieceView[], screenDir: ChartDirection): PieceView;
  ```
- `viewsOf` maps the piece's node ids in `cut` to keys, and keys to node ids in each direction scene; `occupied` for `compactBoth` widens a ref box by its tag rect (`refTagAt(box, dir, 2)` with `tagW × tagH`).
- `pickView`: among views with `standalone.scale >= targetScale()`, the smallest `bounds.w * bounds.h`; if none, the largest `standalone.scale`; ties within 0.1% keep `screenDir`, then the order top, bottom, left, right.
- `fitsTarget` rejects first, without building scenes, when the piece's card area (`Σ w·h` of its boxes in `cut`) × targetScale² exceeds the printable A3 area — `cutPieces`' `fits` is `p => fitsTarget(viewsOf(p, …))` and is called many times.

- [ ] **Step 1: Write the failing tests:**
  - `same_people_every_direction`: a fam-a piece from Task 2's cut → each of the 4 views holds exactly that piece's keys (full + refs + plain).
  - `compacted_both_axes`: for a fam-a line piece in `left`, the view's bounds are ≤ the bounds of the same boxes compacted with `compactX` only, and strictly smaller in h.
  - `tags_inside_bounds_and_clear`: for every view of every fam-a piece (`upTo(120)` cut), each ref tag rect is inside `bounds` and intersects no box of the view other than its own.
  - `picks_smallest_area_that_fits`, `picks_largest_when_none_fits`, `tie_keeps_screen_direction`: hand-made `PieceView` lists.
  - `target_scale_is_8pt`: `printedPt(NAME_PX, targetScale())` = 8 (±1e-9).
- [ ] **Step 2: Run** `cd app && npx vitest run src/lib/chart/pieceView.test.ts` → FAIL.
- [ ] **Step 3: Implement** `pieceView.ts` and the two `a3.ts` exports.
- [ ] **Step 4: Run** the same command and `npx vitest run src/lib/chart/a3.test.ts src/lib/chart/a3Golden.test.ts` → PASS.
- [ ] **Step 5: Commit** `feat(print): each piece in its own best direction, compacted on both axes`.

### Task 4: Shelf packing with one scale per page (`pack.ts`)

**Files:**
- Create: `app/src/lib/chart/pack.ts`
- Modify: `app/src/lib/chart/a3.ts` — export `a3Area(o: A3Orientation, title = true): Rect` (the printable area `fitTo` computes today; `fitTo` calls it; numbers unchanged)
- Test: `app/src/lib/chart/pack.test.ts`, `app/src/lib/chart/a3.test.ts`

**Interfaces:**
- Produces:
  ```ts
  export const CAPTION_MM = 6, CAPTION_PT = 9, CAPTION_MAX_MM = 140, PIECE_GAP_MM = 6;
  export interface PackItem { id: number; w: number; h: number; capW: number } // w, h world units; capW layout px (0 = no caption)
  export interface PackedSlot { id: number; x: number; y: number; capY: number | null } // layout px on the page: top-left of the drawing; caption baseline band top
  export interface PackedPage { orientation: A3Orientation; scale: number; slots: PackedSlot[]; alone: boolean }
  export function packPages(items: PackItem[], o: { target: number; choice: A3Choice; header: boolean }): PackedPage[];
  ```
  `items[0]` is the cover: first slot of page 1.
- Algorithm (fixed by the rulings): slot at scale s = `max(w·s, min(capW, mm(CAPTION_MAX_MM)))` × `h·s + (capW > 0 ? mm(CAPTION_MM) : 0)`, `mm(PIECE_GAP_MM)` between slots and shelves. An item whose slot at `target` fits no allowed orientation is `alone`: its own page, `layoutA3({0,0,w,h}, choice, header)` scale and orientation. The rest: cover first, then by slot height descending (ties: id); each goes on the first shelf of the first page with room, else a new shelf on the first page with height left, else a new page (landscape unless only portrait fits; the chosen orientation when fixed). Then per page: binary search (24 steps) the largest s in [target, A3_MAX_SCALE] at which the same items, in the same order, shelf-pack into one page, for each allowed orientation; take the larger s (landscape on a tie); centre the block in `a3Area`.

- [ ] **Step 1: Write the failing tests:**
  - `a3.test.ts` `a3Area_matches_layoutA3`: for both orientations and `title` true/false, `a3Area` equals `layoutA3(b, o, title).area`.
  - `every_item_once` and `no_overlap_random`: 200 seeded random items (sizes 50–3,000 world units, capW 0–400) → each id in exactly one slot; slots (with caption band, plus half the gap round each) pairwise disjoint and inside `a3Area`.
  - `page_scale_at_least_target`: every non-`alone` page has `scale >= target`.
  - `scale_grows_to_fill`: one item 400 × 300 → one page at `A3_MAX_SCALE`; two items each half a landscape page wide at target → one page, scale > target.
  - `cover_first`: `items[0]` is page 1's first slot even when smallest.
  - `ffd_count`: 6 items each slightly over half a landscape area high at target → 6 pages; 6 items a third high and a full width → 2 pages.
  - `fixed_orientation_everywhere`: `choice: 'portrait'` with wide items → every page portrait, wide-too-big items `alone`.
  - `oversize_alone`: an item 3× the page at target → `alone`, scale < target.
  - `caption_wider_than_piece`: a 50-unit-wide item with capW 300 → slot width 300 px, no overlap with its neighbour.
  - `deterministic`: two runs give equal output.
- [ ] **Step 2: Run** `cd app && npx vitest run src/lib/chart/pack.test.ts src/lib/chart/a3.test.ts` → FAIL.
- [ ] **Step 3: Implement** `a3Area` (pure refactor of `fitTo`) and `pack.ts`.
- [ ] **Step 4: Run** the same command plus `npx vitest run src/lib/chart/a3Golden.test.ts` → PASS.
- [ ] **Step 5: Commit** `feat(print): first-fit-decreasing shelf packing, one scale per A3 page`.

### Task 5: The sheet plan, quality gates and the speed bound (`sheetPlan.ts`)

**Files:**
- Create: `app/src/lib/chart/sheetPlan.ts`
- Test: `app/src/lib/chart/sheetPlan.test.ts` (small cases), `app/src/lib/chart/sheetsQuality.test.ts` (fam gates, perf)

**Interfaces:**
- Consumes: `cutPieces`, `Piece` (Task 2); `DirScene`, `viewsOf`, `fitsTarget`, `pickView`, `PieceView` (Task 3); `packPages` (Task 4); `targetScale`, `A3_MIN_PT`, `printedPt`, `NAME_PX` (`a3.ts`).
- Produces:
  ```ts
  export interface PlanOptions {
    choice: A3Choice; header: boolean; screenDir: ChartDirection;
    centres: NodeKey[];                                  // root and chosen people, as keys
    capW: (top: ChartNode) => number;                    // caption width, layout px at CAPTION_PT
    tagW: number; tagH: number;                          // as ViewOptions
  }
  export interface PlannedPiece { index: number; top: ChartNode | null; view: PieceView; page: number; x: number; y: number; capY: number | null; refPages: Map<number, number> /* view node id -> page */; unsplittable: boolean }
  export interface PlannedPage { no: number; orientation: A3Orientation; scale: number; namePt: number; pieces: PlannedPiece[] }
  export interface SheetPlan { single: boolean; pages: PlannedPage[] }
  export function planSheets(dirs: DirScene[], o: PlanOptions): SheetPlan;
  ```
- Steps inside `planSheets`: (1) the whole chart as one piece: if `fitsTarget(viewsOf(whole…))` → `single: true`, one page with `pickView` of the largest standalone scale, no caption. (2) `cutPieces` on the `screenDir` scene with `fits: p => fitsTarget(viewsOf(p, …))`. (3) `pickView` per piece. (4) `packPages` with `capW` 0 for the cover. (5) page numbers, `refPages` from `refs` (piece index → that piece's page), `namePt = printedPt(NAME_PX, scale)`.

- [ ] **Step 1: Write the failing tests.** `sheetPlan.test.ts`:
  - `small_chart_one_page`: a `base` tree with 20 people in four synthetic directions (the same scene in each) → `single`, 1 page, no captions, page namePt ≥ 8.
  - `cover_on_page_one`, `ref_pages_resolved`: on fam-b → page 1 holds piece 0; every `refPages` entry is the page of the piece holding that key in full.
  `sheetsQuality.test.ts`, for each of fam-a…fam-d (`choice 'auto'`, `header true`, `screenDir 'top'`, `tagW 60`, `tagH 14`, `capW = () => 200`):
  - `everyone_once`: the union of all pieces' full keys = all non-repeat keys of the top scene, each once.
  - `refs_point_to_the_right_page`: every ref box's page tag = the page holding that key in full.
  - `at_least_95_percent_at_6pt`: people in full on pages with namePt ≥ 6 / all people ≥ 0.95.
  - `shared_pages_at_target`: every page with ≥ 2 pieces has namePt ≥ 8; a page below 6 pt holds one piece and it is `unsplittable`.
  - `fill`: mean over all pages but the last of Σ (slot area at the page scale) / `a3Area` ≥ 0.5 (today's bbox fill: 0.03–0.13).
  - `page_count_near_bound`: pages ≤ ceil(1.5 × LB) + 1, LB = ceil(Σ piece slot area at target / landscape `a3Area`).
  - `giant_page_warned_others_at_target`: fam-b plus one extra couple with 300 leaf children grafted on a grandparent's sibling → that block's page is alone and `unsplittable`, every other page ≥ 8 pt.
  - `deterministic`: two runs equal.
  - `perf_1500_people_four_directions`: fam-d (≥ 1,500 nodes per direction, all four directions loaded) → `planSheets` after one warm-up run takes < 2,000 ms (`performance.now()`).
  - A `console.table` (not an assertion) per fam: today's sheets (`TODAY_SHEETS`), new pages, LB, share ≥ 6 pt, share ≥ 8 pt, mean fill — the numbers the owner's open question needs.
- [ ] **Step 2: Run** `cd app && npx vitest run src/lib/chart/sheetPlan.test.ts src/lib/chart/sheetsQuality.test.ts` → FAIL.
- [ ] **Step 3: Implement** `planSheets`.
- [ ] **Step 4: Run** the same command → PASS; copy the table into the task report. If a gate fails, report the measured numbers before touching a threshold; thresholds change only on the controller's word.
- [ ] **Step 5: Commit** `feat(print): plan packed A3 sheets; quality gates on family-shaped trees`.

### Task 6: Printing, saving and screen frames from the plan

**Files:**
- Modify: `app/src/lib/chart/renderer.ts` (`DrawOpts.keep?: boolean` — skip the `clearRect` at the start of `draw`)
- Modify: `app/src/lib/chart/ChartCanvas.svelte` (`printScenes`, `planA3Sheets` replacing `cutA3Sheets`, `buildA3Sheets`, `frameSheets`; remove `branchNames` use)
- Modify: `app/src/lib/chart/sheets.ts` (`screenFrames` rewritten)
- Test: `app/src/lib/chart/renderer.test.ts`, `app/src/lib/chart/ChartPicture.test.ts`, `app/src/lib/chart/SheetFrames.test.ts`, `app/src/lib/chart/sheets.test.ts`

**Interfaces:**
- Consumes: `planSheets`, `SheetPlan` (Task 5); `DirScene`, `nodeKeys`; `CAPTION_PT`, `CAPTION_MAX_MM` (Task 4); `targetScale` (Task 3).
- Produces:
  - `screenFrames(plan: SheetPlan, keysOf: (pieceNodeId: number, dir: ChartDirection) => NodeKey | undefined, screen: Scene): { no: number; x: number; y: number; w: number; h: number }[]` — one frame per piece (cover included) around the screen boxes whose key is in the piece's full set, `no` = its page; `[]` when `plan.pages.length <= 1`.
  - `printScenes(choice, header)`: lays out the four directions with `chartRequest(d, true, 'sheets')` (whatever `a3Direction` says), stretches each with `stretchScene(laid, need)` where `need` comes from `printCardsNeed(…, nameMinPx: nameFloorPx(targetScale()))`, keeps each direction's `meta` (focus ids differ per direction), closes every print chart in `finally` as `printChart` does.
  - `buildA3Sheets(choice, format)`: `single` → the one-page build: `buildA3For({ ...c, pick: { direction: view.direction, layout } }, view.scene, layout, format, { title, sub: a3Sub(), warning })` with `layout = layoutA3(view.bounds, choice, header)` and `c` built from that direction's meta (`buildA3For`'s signature does not change). Otherwise one `BuiltPicture` per page: page size from the page orientation; `paint` draws each piece with `draw(ctx, view.scene, cam, …, { dir: view.direction, focus/focusMore of that direction's meta, nameMinPx: nameFloorPx(page.scale), keep: i > 0 })`, `cam = { zoom: page.scale, x: view.bounds.x - slot.x / page.scale, y: view.bounds.y - slot.y / page.scale }`; ref tags as today (`refTagAt` with the piece's direction, `picture.sheetRef` with the page number); the caption at `CAPTION_PT` in `th.ink2`, ellipsized to `CAPTION_MAX_MM`; title block: chart title, sub `picture.sheetOf` + « · » + `a3Sub()`. Warning on page 1 only: pages with namePt < 6 (`picture.a3SmallSheets` + `picture.a3Small`, without `a3SmallBestHint`).
  - `pictureA3`, `printPictures`, `savePictures`: unchanged.

- [ ] **Step 1: Write the failing tests:**
  - `renderer.test.ts` `keep_does_not_clear`: two `draw` calls on one `SvgContext`, the second with `keep: true` → both scenes' cards are in the output; without `keep`, today's output (existing snapshot) is unchanged.
  - `ChartPicture.test.ts` `sheets_print_pages_from_plan` (serve fam-b, all four directions, via `servePrintCharts`): the printed HTML has as many `<svg` as the plan has pages; each page's text has «аркуш N з M»; at least one «Гілка: … (…)» caption; every «→ арк. N» names a page that exists.
  - `ChartPicture.test.ts` `page_draws_every_piece`: on a page with ≥ 2 pieces, every full person's name of both pieces appears in that page's SVG.
  - `ChartPicture.test.ts` `sheets_ignore_direction_setting`: with `a3Direction = 'screen'`, the sheets print still opens four print charts (one per direction).
  - `ChartPicture.test.ts` `today_and_compact_unchanged`: the existing golden/compact tests run unchanged (no edits to their expectations).
  - `ChartPicture.test.ts` `sheets_save_into_new_folder`: saving a multi-page set calls `makePictureFolder` once, writes « — арк. 1…M» and reveals the folder (adapt the existing case to page counts).
  - `SheetFrames.test.ts` `one_frame_per_piece_with_its_page`: frames count = pieces count, labels «арк. N» match the pieces' pages, the cover is framed; a single-page plan → no frames.
- [ ] **Step 2: Run** `cd app && npx vitest run src/lib/chart` → FAIL on the new cases.
- [ ] **Step 3: Implement** the renderer option, `printScenes`, `planA3Sheets` (`planSheets` with `capW` measured by the `SvgContext` meter at `CAPTION_PT`, `tagW`/`tagH` measured for «→ арк. 99» at the tag size of the target scale, divided by `targetScale()`), `buildA3Sheets`, `screenFrames`, `frameSheets`. Delete `splitSheets`, `Sheet`, `SplitOptions`, `SHEET_MIN_PEOPLE`, `fitsA3`, `branchNames`, the old `screenFrames`, `cutA3Sheets`, and their tests.
- [ ] **Step 4: Run** `cd app && npx vitest run` (whole suite) → PASS; `cd app && npx svelte-check` → no new errors; `CARGO_TARGET_DIR=E:/gene-tree-target/sheets-pack cargo test -p gt-layout` → PASS.
- [ ] **Step 5: Screenshots** (synthetic only, into `Saved/sheets-pack/` of the worktree): the around chart on screen with the frames (fam-b), and pages 1 and 2 of the fam-a set rendered to PNG through `renderPng` in a test script. Check by eye: no overlaps, captions above their pieces, tags readable.
- [ ] **Step 6: Commit** `feat(print): packed A3 sheets — pages from the plan, captions, references, frames per piece`.
