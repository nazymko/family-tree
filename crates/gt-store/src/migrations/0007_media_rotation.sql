-- Schema v7: how a picture is shown, turned clockwise on top of its EXIF orientation: 0, 90, 180 or
-- 270. NULL when nobody has turned it (shown as 0). The transcription's areas stay on the picture
-- upright by EXIF, before this turn. Not exported to GEDCOM.
-- An older app refuses a v7 project, so the column is never silently dropped by a save.
ALTER TABLE media ADD COLUMN rotation INTEGER;
