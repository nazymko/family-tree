# Third-party notices

## Bundled fonts (SIL Open Font License 1.1)

Font files are assets, not code. They ship inside the desktop app and are never fetched at runtime.

| Font | npm package | Licence |
|---|---|---|
| Source Serif 4 (variable) | `@fontsource-variable/source-serif-4` | OFL-1.1 |
| Atkinson Hyperlegible Next | `@fontsource/atkinson-hyperlegible-next` | OFL-1.1 |
| Noto Sans (only its Cyrillic Extended files: the old letters ѣ, ѳ, ѵ in the sans text) | `@fontsource/noto-sans` | OFL-1.1 |

## Map data

| Data | Source | Licence |
|---|---|---|
| World basemap (land, borders, regions, rivers, lakes, populated places), simplified by `scripts/build-basemap.mjs` into `app/src/lib/map/data/basemap.topo.json` | Natural Earth, naturalearthdata.com | Public domain |
| Place coordinates for the "Place coordinates" helper | GeoNames, geonames.org, downloaded once per country only when the person asks | CC BY 4.0 (attribution shown in the app) |

The map code uses `topojson-client` (ISC) at runtime; `topojson-server` and `topojson-simplify` (ISC) only in the build script.

## Frontend dependencies

Runtime and development dependencies of `app/` are MIT, Apache-2.0, BSD or ISC licensed.
Exact versions are pinned in `app/package-lock.json`. Icon paths in `app/src/lib/ui/icons.ts` are derived from Lucide (ISC).

## Rust dependencies

Most crates (including `zip`, `flate2` and `roxmltree` used by the GEDZIP and Gramps importers) are MIT and/or
Apache-2.0. Others in the dependency tree: ICU4X crates (Unicode-3.0), `cssparser`/`selectors`/`dtoa-short`/`option-ext`
(MPL-2.0, unmodified), `kamadak-exif`/`mutate_once`/`alloc-*` (BSD), `foldhash`/`zlib-rs` (Zlib), `libloading` (ISC),
`enum-iterator` (0BSD). The optional AI reader adds `reqwest` with rustls (MIT/Apache-2.0; `ring` is Apache-2.0 AND ISC) and `keyring` (MIT/Apache-2.0), which keeps the API key in the operating system keychain. SQLite (bundled via `rusqlite`) is public domain. Exact versions are pinned in `Cargo.lock`;
`cargo metadata` lists each crate's licence.
