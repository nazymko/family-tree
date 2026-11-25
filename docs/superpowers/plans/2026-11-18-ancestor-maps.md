# Ancestor maps: plan

1. `gt-gazetteer` crate: fold, parse, index file, matcher (Rust tests with a synthetic country file).
2. `gt-session`: `setPlaceCoordinates` command, `map_data`, place list with event counts (Rust tests).
3. Bindings, `Api` interface, MockApi, TauriApi, Tauri commands (gazetteer status/download/remove/suggest).
4. Basemap build script and data file.
5. Frontend `lib/map`: projection, filters, clustering, heat, movement path (Vitest), canvas, view, helper, export.
6. Navigation entry, strings (en, uk), screenshots with synthetic data, gates.
7. Optional: MCP `set_place_coordinates`.
