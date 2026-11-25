-- Schema v13: old scanned maps laid on the Map, under the places. A media record that is such a map
-- carries its layer as JSON: series, year, source, the four corners in millionths of a degree (or
-- none while nobody has placed it), opacity 0-100, whether it is shown, and its place in the stack.
-- NULL on every other file, as on every older row.
-- A GEDCOM export writes the map as an ordinary OBJE record with a note saying where it lies.
-- An older app refuses a v13 project, so the column is never silently dropped by a save.
ALTER TABLE media ADD COLUMN map_layer_json TEXT;
