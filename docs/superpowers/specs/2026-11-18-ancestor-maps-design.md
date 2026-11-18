# Ancestor maps: design

Goal: a Map view that shows where the ancestors lived. It works fully offline and uses no paid service, API key or online tiles.

## Basemap

Natural Earth (public domain) 1:10m vector data, converted once by `scripts/build-basemap.mjs` into one quantized TopoJSON file
(`app/src/lib/map/data/basemap.topo.json`, budget 15 MB, measured size in the final report). Layers: land, lakes, rivers,
country borders, admin-1 borders (oblasts), populated places (with `name` and `name_uk`). The app draws it on a Canvas 2D in
Web Mercator world coordinates with its own pan/zoom (zoom 0 to 18). No tile server, no detail pack. Runtime dependency:
`topojson-client` (ISC, pinned); build-time only: `topojson-server`, `topojson-simplify` (ISC).

## Coordinates for places

Place records already hold optional `lat` and `lon`. A new edit command `setPlaceCoordinates` (place id, lat, lon, both
absent to clear) goes through the normal session path, so it is undoable and in the change log.

Places without coordinates get them from an offline gazetteer: GeoNames (CC-BY 4.0). Crate `gt-gazetteer`:

- `parse`: one row of `<CC>.txt` to a record (name, ascii name, alternate names, feature class and code, admin1/admin2 code,
  population, lat, lon). Only classes P (populated places) and A (administrative divisions) are kept.
- `index`: a compact binary file per country in the app data folder (`gazetteer/<CC>.gtgz`); loads in a fraction of a second
  (a string table plus fixed-width rows; the search tables are rebuilt on load).
- `fold`: normalisation shared by index and query: lower case, no diacritics, Ukrainian Cyrillic to Latin (national 2010
  scheme), then a loose phonetic fold (y/j to i, h to g, w to v, doubled letters collapse), so "Іванівка", "Ivanivka" and
  "Iwaniwka" meet.
- `matcher`: candidates for a place text "Village, District, Oblast": the leaf is matched against name, ascii name and every
  alternate name (exact fold, prefix, edit distance 1 for names of 5+ letters); every other part of the hierarchy is matched
  against the candidate's admin1/admin2 names (a hint, not a requirement). Score = name quality + hints + a small population
  bonus; top 8 returned, each with the matched name, admin names and distance-free score.

Download: only in the Tauri shell, only on the user's click: `https://download.geonames.org/export/dump/<CC>.zip`, with the
size from a HEAD request shown before and a progress bar while it loads. The zip is converted to the index and deleted.
GeoNames attribution (CC-BY 4.0) is shown in the helper and the map footer.

The "Place coordinates" helper lists places without coordinates (with the number of events at each), shows suggestions,
and writes only after the user confirms: accept the best match, pick another candidate, or click the map to set the point.

## Map data

`mapData(scope)` (Rust, `gt-session::map`) returns one flat payload the frontend filters without further calls:

- `places`: id, label ("Leaf, Parent, ..."), lat/lon or none, event count.
- `people`: id, display name, surname, generation (for ancestors/descendants scope), living flag.
- `events`: id, kind, year (from the date, none for undated), place id, person id, one row per person on the event
  (family events give one row per partner).
- `links`: birth-place links parent to child (both person ids), for the movement mode.

Scope: whole tree, ancestors of X (N generations), descendants of X (N generations), one person. Records marked private are
never returned. Living people are returned (the owner sees the whole tree in the app, as in every other view) with a flag;
exports (PNG/SVG) leave them out.

## Modes (one view, shared filters)

1. Places: grid-clustered points sized by event count, label from the zoom where it fits; click opens a panel with the people
   and events at the place; a person opens their page.
2. Heat: event density as a blurred heat layer; event types, year range, optional animation by decade.
3. Movement: for a person, their events in time order as a path with arrows and years; for ancestors/descendants, each
   person's path plus parent-to-child birth-place links, coloured by generation or by surname; a legend.

Filters: scope, event types, year range. Events whose place has no coordinates are counted and link to the helper.

Export: PNG of the canvas (the same save path as the chart pictures) and SVG built from the same drawing list.

## Performance

10k events: filtering is a single pass over typed arrays; clustering is a grid per zoom; drawing only visits the visible
tiles of the basemap (per-ring bounding boxes). Gazetteer: load under 1 s and search under 50 ms for about 100k rows
(measured by a release-only test).

## Not done / later

MCP tool `set_place_coordinates` (only if time remains), other locales' strings (fall back to English).
