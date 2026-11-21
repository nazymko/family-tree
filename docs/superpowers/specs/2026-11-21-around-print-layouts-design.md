# "Built around" chart: two print layouts for big charts (compact sheet, set of branch sheets)

Date: 2026-11-21. Status: approaches chosen by the owner, design for review.

## Why

The owner prints the "Built around" chart on A3 paper, **to read and to give as a gift**. With side
branches opened (see `2026-11-21-around-open-branches-design.md`) a real chart holds 300+ people. Today
the A3 print fits the whole chart onto one sheet: the chart comes out as a tall, narrow strip
(generations as columns, every child of a 10-20-child family on its own row) shrunk to about a third of
a readable size. Names cannot be read.

The owner asked for two layouts to choose from on the "Built around" screen:

- **A. One sheet, compact**: the same chart folded so it fills the A3 sheet's proportions.
- **B. A set of A3 sheets by branch**: a cover sheet with the main line, then one sheet per family
  branch, with "continued on sheet N" marks; folding as in A inside each sheet.

## Goals

- On the "Built around" screen the owner picks the layout: **as today** (the default), **compact**, or
  **sheets by branch**.
- Compact: a large group of brothers and sisters (or of children) that have nothing hanging under them
  is laid out in several short columns instead of one long column, so the chart's shape comes close to
  A3 (√2 : 1) and prints larger.
- Sheets by branch: every person on the chart appears on exactly one sheet as a full box (plus, where a
  branch continues on another sheet, as a small reference mark). Every sheet is printed at a readable size
  (names at least 6 pt, the existing `A3_MIN_PT`).
- The print, the PDF (via the system print dialog) and the saved pictures show what the screen shows.
- **Nothing chosen = today's chart and today's print, unchanged.**

## Non-goals

- No change to the pedigree, descendants, relatives and whole-tree charts.
- No other paper sizes (A3 only, both orientations as today).
- No saving the choice in the project file (it may be remembered like the other A3 preferences in
  `a3Pref.svelte.ts`).
- No hand-editing of page contents.

## Anti-goals

- Overlapping boxes or lines through boxes.
- A person missing from the set of sheets, or a person shown in full on two sheets.
- A sheet whose names come out below 6 pt without a warning.
- Any change to the chart when the layout is "as today" (existing layout tests and snapshots stay green).

## Approaches considered

- **A alone**: cannot hold 300+ people readably on one A3 (about 250-300 boxes fit); good for medium charts.
- **B with A's folding inside each sheet (recommended, chosen together with A as a separate choice)**:
  readable at any size, suits a gift (folder or bound set).
- **C: one big chart cut into tiles to glue into a poster**: seams spoil the look; rejected for a gift.

The owner chose to have **both A and B selectable**.

## Design

### Where the choice lives

On the "Built around" screen, next to the existing chart controls, a choice **Layout: As today /
Compact / Sheets by branch** (en + uk strings; uk: «Розміщення: як зараз / компактно / аркуші по
гілках»). "As today" is the default. "Compact" changes the chart on screen too (what you see is what
prints). "Sheets by branch" shows the compact chart on screen plus thin frames showing which part goes to
which sheet, with the sheet numbers; print and save produce the set.

### A. Compact (layout engine)

`ChartRequest` (Rust `crates/gt-session/src/dto.rs` → generated TS) gets an optional field for the
"around" kind: `fold: Option<bool>`. Absent or false: the old path, untouched.

With `fold`, the layout in `crates/gt-layout/src/relatives.rs` (around / around_open) lays out a **leaf
run** in columns: a run of consecutive boxes in one sibling group (or one children group) where none has a
partner box or anything hanging below/beside it. A run of `n ≥ 6` boxes becomes `c` columns of at most
`ceil(n / c)` boxes each, side by side along the generation axis, under one bracket line from the parent
junction. `c` is chosen so the whole chart's bounds come closest to the A3 ratio in the orientation the
print picks (start with `c = ceil(sqrt(n / 3))`, then adjust; the exact rule is the builder's, tested on the
real chart shape). Boxes with partners or descendants are never folded: they keep today's placement.

### B. Sheets by branch (front end, from the compact layout)

The sheets are cut from the folded chart's **lineage**: the centred person's ancestor tree.

- **Sheet 1 (cover)**: the centred person (or people), their partners and descendants, their parents and
  grandparents, and the brothers and sisters opened at those levels.
- **One sheet per grandparent line** (up to 4): that grandparent's ancestors and everything opened around
  them. A line with no ancestors and nothing opened stays on the cover.
- **Too big for one readable sheet**: the sheet is split again one generation further up (per
  great-grandparent line), recursively, until each sheet prints with names ≥ 6 pt or cannot be split.
  A sheet that still does not fit prints with the existing "names are small" warning.
- **Reference marks**: where a line continues on another sheet, the box of the person where it leaves
  stays on both sheets: in full on the lower sheet, and as a small marked copy on the upper sheet
  ("→ sheet 4" / «→ арк. 4»), so the reader can follow the line. That copy is the only repeat allowed.
- Each sheet has the existing A3 title block; the title says the branch («Гілка: <surname of the line's
  top person>») and "sheet N of M".
- Every sheet is laid out with the existing A3 machinery (`layoutA3`, `printDirection`, title block) at its
  own best scale and orientation.

Output: **Print** builds one HTML document with all sheets (one SVG per page, page breaks), sent through
the existing `printHtml` (the system dialog; "Microsoft Print to PDF" gives a PDF of the set). **Save**
writes one file per sheet (`… — арк. 1.png`, `… — арк. 2.png`, or SVG), named after the chart.

### Decisions made after the builder check (my calls, owner may correct)

1. **One layout, cut by the front end.** The folded chart is laid out once by `gt-layout` (with `fold`); the
   sheets are cut from that one scene, the way the existing "branch" picture scope cuts a sub-scene
   (`sceneFor` / `branchIds` / `compactX` in `picture.ts`). No per-sheet Rust layout. The column counts of
   folded runs are chosen for the whole chart's A3 ratio and simply carry over into each sheet.
2. **Who goes on which sheet.** Every box hangs from one ancestor of the centred person(s): the ancestor
   it is, or whose partner, opened brother/sister, or descendant of an opened brother/sister it is.
   A person goes on the sheet that holds that ancestor. The cover holds generations 0, −1, −2 (centred
   people, parents, grandparents) and everything hanging from them (so an aunt and her whole opened
   subtree are on the cover; a grandparent's brother is on the cover too). The centred people's partners
   and descendants are on the cover. A grandparent line's sheet holds that grandparent's parents and
   everything above and hanging from them; the grandparent's own box is repeated there as the reference
   mark at the foot of the sheet. With several centred people, the cover holds all of them and the
   lines are taken per grandparent of each. An ancestor reached twice (pedigree collapse) is placed on
   the first sheet that reaches it (order: father's side before mother's); the other place shows a
   reference mark.
3. **Reference marks** are drawn by the front end, not by the layout: a copy of the person's box at its
   place in the cut sub-scene (the space is already there), with a small tag «→ арк. N» / "→ sheet N", and
   the lines to it as in the scene.
4. **Recursion and tiny sheets.** No cap on the number of sheets. A line sheet with fewer than 8 people is
   not made: those people stay on the sheet below (the cover or the line sheet they hang from). Splitting
   stops when a sheet prints at ≥ 6 pt or has nothing left to split.
5. **Saving** asks once (the save dialog for the first file); the other sheets go next to it with the
   suffix « — арк. N» / " — sheet N", in the chosen format (PNG or SVG, as today).
6. **Folding** applies to any qualifying run (sibling group or children group at any level, including in
   opened branches); columns run across the generation axis in whatever direction the chart is drawn.

### Testing

- Rust (`gt-layout`): fold off = identical layout to today for the existing fixtures; fold on = no overlaps,
  every person once, leaf runs ≥ 6 folded, non-leaves untouched, aspect closer to √2 than without fold on a
  synthetic 300-person chart.
- Front end (Vitest): sheet split puts every person on exactly one sheet; reference marks point to the right
  sheet; recursion stops; the print document has M pages; "as today" produces today's print.
- Screenshots (synthetic data only, never real names) of: compact on screen, the sheet frames, two printed
  sheets.

## Left to the builder

- The exact column-count rule and the bracket drawing for folded runs.
- How the sheet frames look on screen.
- Whether the choice is remembered between sessions (`a3Pref` style) — fine either way.

## Amendment 21.11.2026 — packed sheets

Status: the owner approved the full redesign (21.11.2026). Plan: `docs/superpowers/plans/2026-11-22-sheets-packing.md`.

Why: an investigation on synthetic family-shaped trees (fam-a…fam-d, 369–1,568 people, all kin opened) found that
today's sets have 5–15 sheets and that 84–98% of the people on them print below 6 pt. Causes: the cover is never
checked for size and takes every aunt's and uncle's whole family; collateral families are never split; a sheet is
compacted along x only, so in a left/right chart the whole chart's gaps stay in it; one direction serves the whole
chart; every grandparent line becomes a sheet however small (6–8-person sheets on an empty page) and nothing is
packed. This amendment specifies that report's fixes F1–F6.

### What it replaces

- Decision 2's **cover contents** ("the cover holds generations 0, −1, −2 and everything hanging from them") and
  section B's **one sheet per grandparent line**: the cover is what remains after big blocks move out (see
  "Pieces"), and a line is cut wherever it needs to be, not at each grandparent.
- Decision 4's **8-person minimum** (`SHEET_MIN_PEOPLE`) and the merge-back rule: small blocks are not merged back;
  they are packed beside other blocks on a shared page.
- Section B's **one branch per page** and **"the title says the branch"**: a page may hold several pieces; every
  page's title is the chart title, and each piece carries its own branch caption.
- Section B's "every sheet is laid out with `printDirection`": every piece has its own direction, whatever the A3
  direction setting says (owner decision 1).

Unchanged: «як зараз» and «компактно» print and save exactly as before, byte for byte; a set is saved into a new
folder that is then opened in Explorer; A3 only; en + uk strings only; synthetic data only in tests and screenshots.

### Owner decisions (binding)

1. Each piece gets its own best direction and orientation and is compacted on both axes (F1, F2, F5).
2. The cover and big collateral families are split. A moved block leaves a reference box «→ арк. N» (F3).
3. If the whole chart reads on one A3 page, one page is printed.
4. Otherwise small branches are packed together onto pages. Each piece carries a caption with its branch name, and
   names do not go below the target size. Expected outcome: 2–4x fewer pages (F4, F6); see "Expected outcome".
5. «як зараз» and «компактно» unchanged; saving into a new folder opened in Explorer; en + uk strings only.

### Controller's rulings (21.11.2026)

- **Target name size 8 pt** (`SHEET_TARGET_PT`). Never below 6 pt (`A3_MIN_PT`) unless a piece cannot be split
  further; then the existing "names are small" warning names its page.
- **A3 only.**
- **Page header**: the chart title on the left; «аркуш N з M» on the right, before the tree name and date (as today).
  **Each packed piece** gets a small caption «Гілка: <surname> (<given name of the top person>)».
- **Screen frames**: one dashed frame per piece, labelled «арк. N», N being the page the piece lands on; a page may
  therefore have several frames on screen.
- **Packing**: first-fit-decreasing shelf packing. Each page uses ONE common scale, the largest that still keeps
  every piece on it at or above the target. Pieces never turn relative to each other; mixed directions on one page
  are accepted.

### Design

**One page when it reads.** The whole chart, in each of the four directions, compacted on both axes (below), on one
A3 page in the chosen orientation (the better one under 'auto'): if the best of these prints names at ≥ 8 pt, the
set is that one page: no caption, no «аркуш 1 з 1», the title block as «компактно» has it.

**Four layouts, one cut.** "Sheets by branch" always lays the print chart out in all four directions (compact
cards, folded), whatever the A3 direction setting. The cut is made once, on the screen direction's layout. A box is
matched between the four layouts by a **node key** (the person id; for a repeated box, the person id and the box it
hangs on), never by node id.

**Compacted on both axes (F1).** A piece's sub-scene is squeezed along x and then along y by the existing
`compactX` rule (each empty stretch wider than the gap shrinks to the gap; order, rows and connections stay). The
room a reference tag needs beside its box counts as occupied, so the squeeze never pushes a neighbour onto a tag.

**Card height at the target (F5).** Each direction's layout is stretched once for its cards' text, with the name
floor of the 8 pt scale, fully (no longer only into the page's empty room: the packer now decides the room). Pages
print at or above the target unless they hold one unsplittable piece, so names keep their floor.

**Pieces (F3, F6).** Every box hangs from the lineage (centred people and their ancestors) as decision 2 already
says; that **hang tree** gives every box one box it hangs from. A **block** is the subtree of the hang tree under
one box: a person not on the lineage with their partners and everything hanging below them (a collateral family),
or a lineage person three or more generations above a centre with everything above and beside them (a line).
Starting from the whole chart: while a piece does not print at ≥ 8 pt in any direction, its **largest** block (most
people in full; ties: father's side first, then scene order) of at least 3 people (`PIECE_MIN_PEOPLE`) moves out
into a piece of its own, which is treated the same way. A piece with nothing left to move is unsplittable. The
cover is the piece holding the centred people: them, their spine up to the grandparents, and whatever near family
was not moved out.

**References (F3).** Wherever a cut separates two boxes joined by a line (parent, partner or sibling), each side
shows the other side's box as a reference box tagged «→ арк. N», N being the page holding that person in full. So a
moved collateral family leaves its head person as a reference box among their brothers and sisters, and the moved
piece shows the head's parents as reference boxes; a moved line shows the generation below it as reference boxes at
its foot, and the piece below shows the line's top people as reference boxes. Repeated boxes (`dupOf`) keep the
existing rule: a plain repeat when the first occurrence is on the same piece, else a reference box. Everyone appears
in full exactly once in the set.

**Direction per piece (F2).** Each piece is cut out of each of the four layouts (by node key) and compacted. A piece
takes the direction with the smallest area among those that fit one A3 page at 8 pt; when none does, the one that
prints it largest (it then gets a page of its own). Ties keep the screen direction.

**Packing (F4).** Pieces are packed first-fit-decreasing by slot height onto shelves at the target scale. A slot is
the piece's drawing plus a caption band above it (6 mm, text 9 pt) and a 6 mm gap; a slot is at least as wide as
its caption (captions longer than 140 mm end in an ellipsis). The cover goes first, on page 1. A page opens
landscape unless its first piece fits only portrait; with a fixed orientation chosen every page has it. Then each
page's scale grows to the largest (at most 1.5, `A3_MAX_SCALE`) at which its pieces still pack on it, both
orientations tried under 'auto'. The packed block is centred on the page. A piece too big for a page at 8 pt goes
alone on a page at its own best scale and orientation. Pages are numbered in the order they were opened; the
references are then resolved to page numbers.

**Captions.** «Гілка: <surname> (<given>)» (the existing `picture.branchTitle` with that name) from the piece's top
person; without a given name «Гілка: <surname>». The cover has no caption: the page title names the chart. Two equal
captions are allowed (references point to pages). The old `branchNames` de-duplication goes.

**Screen frames.** For a set of more than one page, one dashed frame per piece (cover included) around the screen
boxes of its people in full, labelled «арк. N» with its page. Frames may overlap.

**Output.** Same form as today: one print document with one page per A3 page; save writes one file per page into a
new folder opened in Explorer. The warning lists the pages that print below 6 pt.

### Expected outcome, against the target

From the investigation's numbers (compact card 124 × 56; a compacted piece is about 20–35% cards; about 85% shelf
efficiency) an A3 page holds roughly 60–95 people at 8 pt and 110–170 at 6 pt. At 8 pt that gives about fam-a
8–13 pages (today 9), fam-b 4–7 (9), fam-c 6–9 (5), fam-d 17–27 (15): every name readable, but not fewer pages for
the biggest charts. "2–4x fewer pages" holds for medium charts and holds for big ones only at today's unreadable
sizes. The plan's Task 5 measures the real numbers and prints them; its gates are readability, fill and correctness,
not beating today's page count.

### Open question for the owner

For charts above about 500 people the 8 pt floor and "2–4x fewer pages" cannot both hold. Which wins for big charts:
names at ≥ 8 pt (more pages), or a lower floor (for example 6.5–7 pt) for fewer pages? Until answered, 8 pt rules;
the floor is one constant (`SHEET_TARGET_PT`).
