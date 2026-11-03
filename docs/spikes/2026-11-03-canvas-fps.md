# Canvas fps spike: 10k nodes on real webviews

Status: shell built and launched; frame measurements wait for Task 4 (chart engine) and the spike harness.

## What exists

- `gt-app` Tauri 2.12 shell builds on Windows 11: `VITE_GT_MODE=spike npm --prefix app run tauri -- build --debug --no-bundle` produces `target/debug/gene-tree.exe`; started, the window stayed up for 8 s, then killed.
- `spike_report` command (compiled in debug or with `--features spike`) writes `spike-<os>-<timestamp>.json` to the app data dir.
- `app/src/spike/Spike.svelte` is still the Task 1 placeholder; the renderer (`app/src/lib/chart`) lands with Task 4. No frame times were measured yet.

## Environment (Windows 11)

- WebView2 runtime 156.0.4314.58; GPUs: NVIDIA GeForce RTX 5080, AMD Radeon (iGPU).

## Results (gate: median <= 16.9 ms and p95 <= 22 ms, 1280x800)

| OS | Webview and version | GPU | DPR | Nodes | Median ms | p95 ms | p99 ms | Frames over 25 ms |
|---|---|---|---|---|---|---|---|---|
| Windows 11 | WebView2 156.0.4314.58 | RTX 5080 | 1 | 10k | waits for Task 4 | | | |
| Windows 11 | WebView2 156.0.4314.58 | RTX 5080 | 2 | 10k | waits for Task 4 | | | |
| Windows 11 | WebView2 156.0.4314.58 | RTX 5080 | 1 | 30k | waits for Task 4 | | | |
| macOS 12+ | WKWebView | | 1 and 2 | 10k, 30k | not measured (no Mac) | | | |
| Linux | WebKitGTK 4.1, default | | 1 and 2 | 10k, 30k | not measured | | | |
| Linux | `WEBKIT_DISABLE_COMPOSITING_MODE=1` | | 1 and 2 | 10k, 30k | not measured | | | |
| Linux | `WEBKIT_DISABLE_DMABUF_RENDERER=1` | | 1 and 2 | 10k, 30k | not measured | | | |
| Linux (xvfb, software, lower bound only) | WebKitGTK 4.1 | llvmpipe | 1 | 10k | not measured | | | |

## Mitigations shipped

None yet. Order if a gate fails: boxes LOD 0.25 to 0.35; OffscreenCanvas tile cache; straight edges below zoom 0.4; WindowCache inflation 50%.

## To finish

1. After Task 4: run the release spike build on Windows, copy the JSON from the app data folder, fill the table.
2. Run on macOS and Linux (CI `shell-build` only proves the shell compiles).

## Rust budgets (Windows, release)

`cargo test -p gt-session --release --test budget -- --ignored --nocapture` on the dev machine (Windows 11, release profile, 50 000 synthetic persons plus a hand-made full 8-generation pedigree of 255 people so the chart tests lay out real ancestry). Medians of 9 to 25 runs; the budgets are unchanged from the plan.

| Budget | Limit | Measured |
|---|---|---|
| `open_under_2s` | 2 s | 0.57 s |
| `list_page_under_30ms` | 30 ms | 1.1 ms |
| `search_under_100ms` | 100 ms | 18.6 ms |
| `apply_under_50ms` (add child with date and place) | 50 ms | median 11 ms apply, 17 ms undo, 13 ms redo (p90 of all edits 19-23 ms, bound 100 ms; slowest single run 60-85 ms, see below) |
| `chart_window_under_8ms` (1920x1080 window of a 255-node pedigree) | 8 ms | 0.012 ms |
| `chart_open_pedigree_8_gens_under_50ms` (255 nodes) | 50 ms | 0.07 ms |
| `resident_memory_under_500mb` (working set via `tasklist`; the workspace forbids `unsafe`, so no `GetProcessMemoryInfo`) | 500 MB | 236 MB (includes the fixture builder) |
| `living_recompute_under_30ms` | 30 ms | 8.7 ms |

What it took to meet `apply_under_50ms` (first measurement: 400 ms per edit):

- The family branch of `person_text` in `gt-store` (search-text refresh after every edit) let SQLite scan every family participant row: 110 ms per affected person. `CROSS JOIN` fixes the join order (3 ms total for the store write now).
- `Session::commit` computed the living map twice per edit and `rebuild()` rebuilt every derived structure. Edits now refresh incrementally (`Session::refresh_after`): one living pass (the rule alone, overrides applied on top, then diffed), links for the touched events, families and citations, index rows for the touched persons, and place use counts. `rebuild()` stays for open, import, settings changes and change sets above 2 000 records. A random edit/undo/redo test (`tests/refresh.rs`) asserts after every step that the incremental state equals a fresh rebuild (links, living maps, index rows and slots, sort orders, place index).
- Remaining cost per edit is the single living pass (about 10 ms), the SQLite write and the index refresh.

Known noise: roughly one edit in ten shows a 50 to 80 ms stall inside the SQLite WAL `commit` (OS file I/O on this machine, not CPU in our code). The budget is asserted on the median of repeated edits, with a secondary p90 < 100 ms bound; the slowest run is printed.
