-- Schema v6: the transcription of a picture, stored on the media item itself (the text belongs to
-- the picture wherever it is attached). A JSON array of parts in reading order:
-- {"id", "area": {"x","y","w","h"} | null, "rotation": 0|90|180|270, "text"}. NULL when there is none.
-- An older app refuses a v6 project, so the column is never silently dropped by a save.
ALTER TABLE media ADD COLUMN transcription_json TEXT;
