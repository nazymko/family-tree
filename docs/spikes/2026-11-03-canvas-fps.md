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
