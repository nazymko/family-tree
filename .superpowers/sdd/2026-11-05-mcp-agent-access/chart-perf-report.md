# Whole-tree chart: empty cards, unjoined siblings, and agent-change cost

## A. Chart

Root causes (found by reading, then pinned by failing tests):

1. `drawWhole` (wholeRenderer.ts) had no unnamed-siblings marker path at all. It filled the box in the
   unknown-sex colour with a bar, and wrote the name, which is empty for a marker: an empty card with a bar.
   `renderer.ts` (ancestor/descendant/relatives) handled markers only at the full zoom level; far out
   they were solid blocks. Saved pictures call the same `drawWhole`, so they had the same fault.
   Fix: shared `drawMarkerBoxes` / `drawMarkerLabel` in renderer.ts, used by both renderers at every
   zoom level (dashed outline, label, no colour bar, no solid fill).
2. `layout_component` (gt-layout whole.rs) drew child lines only from a family's first partner, so a
   family with no partner got none. Fix: each neighbouring pair of children of a partner-less family is
   joined by a bracket over their tops (a `Parent` edge from one sibling to the next; no parent card is
   invented). The layout already placed them together on one row (test confirms), so no placement change.
   A family with one child plus a marker has two children, so it gets one join.
3. Ancestor / descendant / relatives charts: not changed. A family with no partners is not reachable
   from any person there (`unions` and `parents` read partners only), so its children never appear
   next to each other on those charts. Showing them would be a new feature (extend `ChartSource`).
4. Not done: a family with one partner with an empty name still draws an `Unknown` card (not in the required list).

Tests: `children_of_a_family_without_partners_sit_together_and_are_joined`,
`one_child_and_a_marker_without_partners_are_joined` (gt-layout/tests/whole.rs);
`drawWhole unnamed-siblings marker` x2 (wholeRenderer.test.ts); `renderer unnamed-siblings marker far out`.

Verified: vitest with a recording fake canvas (no real pixels), Rust layout tests. Not verified:
a real browser or Tauri window (the running app was not touched).

## B. Performance (debug build, median of 7, synthetic data)

Setup: agent writes through `commit_command_as`, one batch, nothing reviewed, none undone.
Small: 85 people, 220 groups. Large: 5,000 people, 3,000 groups. Harness:
`crates/gt-session/tests/agent_perf.rs` (`--ignored --nocapture`).

| call | 85 p before | 85 p after | 5000 p before | 5000 p after |
|---|---|---|---|---|
| unreviewed_people | 5.9 ms | 0.34 ms | 22.6 s | 20.8 ms |
| change_batches | 14.2 ms | 0.66 ms | 59.0 s | 40.6 ms |
| person_agent_changes (one person) | 1.0 ms | 1.2 ms | 20.7 ms | 10.0 ms |
| change_log first page (25) | 4.0 ms | 6.4 ms | 250 ms | 131 ms |
| chart_open_whole after a write | 0.8 ms | 0.8 ms | 156 ms | 158 ms |
| chart_open_whole cached | 3 us | 3 us | 152 us | 153 us |
| sum per agent event | 25.9 ms | 9.4 ms | 82 s | 0.36 s |

(Small and large "before/after" differences of a few ms in the unchanged rows are noise.)

Root cause: `NOT EXISTS (SELECT 1 FROM change_log v WHERE v.reverts_group = x.group_id)` is
correlated and `reverts_group` has no index on this base, so every group scanned the whole log
(quadratic). Used by `unreviewed_people`, `batch_counts`, `batch_standing` and the unreviewed filter of
`log_page`. Fix (gt-store/src/authors.rs, no schema change, so it does not clash with the schema-4
index): `group_id NOT IN (SELECT reverts_group ... IS NOT NULL)`, an uncorrelated set SQLite builds once.
Test: `reading_who_is_unreviewed_and_the_batches_does_not_scan_the_log_per_group`
(1,500 groups, budget 400 ms each; the old code took 4.8 s on `unreviewed_people`).

Findings that did not need a fix:
- `agentChanges` already coalesces: leading plus trailing refresh every 250 ms (`REFRESH_MS`), and
  `touchedAt` records every event's ids immediately, so stale guards see all of them (tests exist). The
  suspected per-event `project.rev++` is throttled in this base.
- Each refresh still re-opens the open chart (`open(true)`) = `chart_open_whole`: 0.8 ms at 85 people,
  about 156 ms at 5,000, at most 4 per second during a burst. Left as is (no measurement shows a drop at
  the owner's size).
- Redraw loop: none; `invalidate()` schedules one rAF only when dirty. `drawAgentBadges` is a Set lookup
  per visible node. No idle timer or poll found in WholeCanvas, ChartCanvas, agentChanges.
- `person_agent_changes` uses `usize::MAX` and `log_row` per group: 1.2 ms at 85 people, 10 to 20 ms for a
  person with 1 group in 5,000. Not worth changing.
- `log_page` runs its grouping subquery twice (count and page): 131 ms at 3,000 groups in debug; not changed.
- Not measured: the Svelte side timings (frame time, person page render), the real app, a release build.
  Debug SQLite is compiled unoptimised, so absolute times are pessimistic; the ratio is the point.
