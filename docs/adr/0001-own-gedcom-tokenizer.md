# ADR 0001: Own GEDCOM tokenizer

Status: accepted (2026-11-03)

## Context

The engine must round-trip everything that came from a GEDCOM file, including unknown
tags, in their original order and with verbatim values. Imported files are untrusted and
frequently malformed: wrong or missing `CHAR`, ANSEL, UTF-16, lines over 255 bytes, level
jumps, and `CONC` lines that split a multi-byte character across two lines.

Existing 0.x Rust crates (`oxidged`, `ged_io`) parse into typed models, drop or normalise
unknown data, decode text before joining `CONC`/`CONT`, and have no stability guarantees.

## Decision

Write our own byte-level, lenient tokenizer in `gt-gedcom` (about 300 lines):

- Encoding is detected first (BOM, UTF-16 NUL pattern, `1 CHAR`, UTF-8 validity, fallback
  Windows-1252). `CHAR` is never trusted when the bytes contradict it. UTF-16 is converted
  to UTF-8 before tokenizing; every other encoding is decoded per payload.
- `CONC`/`CONT` payload bytes are joined into the parent value before decoding.
- Problems are reported as issues; nothing panics and nothing is silently dropped.
- Nodes are kept as a generic tree and map to `gt_core::RawNode` for verbatim storage.

`oxidged` and `ged_io` are used only as test oracles (dev-dependencies, Task 9), never at
runtime.

## Consequences

- We own about 300 lines of parser code and its tests (including a no-panic proptest).
- This narrows spec section 4.3: the spike gate criteria apply to our parser instead of
  to a third-party crate.
- ANSEL support is ours (Library of Congress MARC-8 table, combining marks reordered and
  NFC-normalised).
