# Around chart: open side branches Implementation Plan

> **For agentic workers:** executed inline by the builder (no subagents).

**Goal:** On the "Built around" chart of ONE person, let the owner open chosen people's brothers and sisters, or children, to a chosen depth.

**Architecture:** `ChartRequest.open` carries per-person openings. `gt_layout::around_open` reuses the "all relatives" machinery (`fill_group` + `solve_all`) but filters which families are hung and how deep; with `open` absent/empty the old code paths run unchanged. A post-pass computes per-node flags `canOpenSiblings` / `canOpenChildren`. The UI reads the flags and shows two segmented controls in the selection bar.

**Tech Stack:** Rust (gt-layout, gt-session, ts-rs), Svelte 5, Vitest.

**Spec:** `docs/superpowers/specs/2026-11-21-around-open-branches-design.md`

## Global Constraints
- Open absent or `[]`: identical chart, identical code path.
- Only blood relatives and people on the line get controls; partners who married in get none.
- Several chosen people: `open` ignored. Invented names only. UI: en + uk only (родовід, never дерево).
- Generated TS types are regenerated, never hand-edited.

## Review Focus
- Opening a person who is not on the chart: no effect, no error.
- Unknown id / sibling marker in `open`: ignored.
- Same person twice: last entry wins.
- `kin = all` with openings: siblings openings are no-ops, children openings work.
- Overlaps: quality checks stay clean after openings.

### Task 1: Layout (gt-layout)
**Files:** `crates/gt-layout/src/relatives.rs`, `lib.rs`, `types.rs`; test `crates/gt-layout/tests/around_open.rs`
**Interfaces:** Produces `BranchDepth{Children,All}`, `OpenSpec{key,siblings,children}`, `AroundBase{Line,Siblings,All}`, `around_open(src, root, generations, base, &[OpenSpec], m, max_nodes) -> Layout`, `open_flags(src, &Layout, base) -> Vec<(bool,bool)>` per node.
- [ ] Failing tests: empty open equals old layout for each base; siblings of an ancestor (Children) add exactly sibling+partner+children; All adds grandchildren; an opening under a closed one is a no-op; quality clean.
- [ ] Implement via `lay_out` with an optional open plan; `Tree::Kin(up, limit)`; per-unit child override in `expand`.
- [ ] Commit.

### Task 2: Session (gt-session)
**Files:** `dto.rs` (OpenBranch, BranchDepth, ChartRequest.open, ChartNode.can_open_siblings/children/opened), `chart.rs`; test `crates/gt-session/tests/around_open.rs`
- [ ] Failing tests through `chart_open`; regenerate ts-rs bindings; commit.

### Task 3: UI
**Files:** `ChartView.svelte`, `ChartCanvas.svelte`, MockApi, i18n en/uk, Vitest.
- [ ] Failing Vitest: controls appear for a selected card, set the request, clear on rebuild; print request carries `open`; print dialog shows person count.
- [ ] Implement; commit.

### Task 4: Gates and screenshots
- [ ] clippy, nextest, svelte-check, `test.ps1`, `-Full`; debug build; screenshots in `Saved/around-open/`.
