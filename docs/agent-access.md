# Agent access

Family Tree can let an AI agent that runs on your computer read your family tree and add to it. The agent talks to the app over
MCP (the Model Context Protocol). Every change it makes is marked with its name, can be reviewed, and can be undone.

It is off until you turn it on.

## Connect an agent

1. Open the tree you want the agent to work on. The agent sees only the tree that is open.
2. Go to **Settings**, then **Agent access**, and tick **Let agents on this computer connect**. Read the warning first: a
   connected agent can read everything in the open tree, including living relatives, private notes and pictures, and an agent
   may send what it reads to its AI provider.
3. The port is 8765 unless you change it (any number from 1024 to 65535). If another program already uses it, Settings says
   so; choose another.
4. **Without a key (the default).** Settings shows a ready-made command with no key. For Claude Code:

   ```
   claude mcp add --transport http gene-tree http://127.0.0.1:8765/mcp
   ```

   The agent then works as the built-in **Agent without a key**. All programs without a key share that one identity. They can
   write into, and finish, each other's open batch.
5. **With a key: a name of its own in the log.** Under **Connections**, give the agent a name and press **Create connection**.
   Make one connection per agent. The name is what the log shows next to every change. A window shows the **secret key** and a
   ready-made command. The key is shown once; if you lose it, revoke the connection and make a new one.

   ```
   claude mcp add --transport http gene-tree http://127.0.0.1:8765/mcp --header "Authorization: Bearer <the key>"
   ```

6. To stop programs connecting without a key, turn on **Require a key** in the same section. It applies to the very next
   request.

Any other MCP client that speaks HTTP works the same way: the address is `http://127.0.0.1:<port>/mcp`, requests are
`Content-Type: application/json`, and a key, if any, goes in an `Authorization: Bearer` header.

**Rules agents receive** (a button in the same settings page) shows the instructions every agent is given when it connects.
They are read only.

## What an agent can do

It can read everything in the open tree: people, families, events, places, sources, notes, citations, pictures, the change log
and each record's history. It can also search, and look people up by the number they had in a file.

Voice samples on a note (recordings you attached) show up in what an agent reads as a list with each one's title, type and size. The agent never receives the sound itself: asking for a recording returns only its details (length, sources, clips and transcript), so no audio leaves your computer through the agent. An agent cannot replace or remove a recording, or put one on a note; it can attach a recording file to its source (see *Recordings and clips* below).

It can add:

- people (with names, sex, a birth date and place), and families;
- links between people already in the tree, and extra names for a person;
- events, notes, sources and citations;
- one **marker** for brothers and sisters a source mentions without names (the app draws it as a label reading "Some more
  Sisters/Brothers" in the current language, and it has no name or date);
- the file number of a person, so that a second run finds them again;
- **coordinates of a place** that has none yet, with their source, so that its events show on the Map (see
  [Places on the map](#places-on-the-map));
- **pictures and PDF documents** from a file on this computer, on a person, family, event, source or citation (see
  [Photographed documents](#photographed-documents));
- the **transcription** of a picture: what is written on it, piece by piece, with where each piece is;
- how a picture is **turned** in the app, so that sideways writing reads upright (on its own pictures, or on one nobody has
  turned yet);
- **versions** of one page: files that show the same page in other quality, grouped so the app shows them as one picture
  (never a group you made).

It works in **batches**. A batch has a title, such as the name of the file the agent is working from, and ends with the
agent's own report: what it added, what it left unsettled, and questions for the family.

### What it cannot do

- **Change what is already there.** An agent adds. It does not rewrite a name, a date or a note that you or anyone else wrote,
  and it deletes nothing of yours. If a source disagrees with the tree, the agent adds a note that gives both versions. The
  exceptions are its own work: it may correct a note, event, name, citation or piece of a transcription that it wrote itself
  and that nobody has changed since, and it may remove an empty family it made itself in the batch still open, which a refused
  link can leave behind. It may also take back a parent-child link it made itself (below), and, one more, correct a confirmed
  birth or death date.
- **Add a person with no name.** Someone a source knows only by a role, such as somebody's father, goes in a note on the people
  around them.
- **Change whether a living person counts as alive or dead, beyond one case.** The app decides this from dates and
  relationships, and the owner's *Living*/*Deceased* mark overrides it. An agent can never make anyone count as alive, and
  never changes the status of a person you marked *Living*. It may make someone count as dead only when the tree, with its
  write, shows them more than 100 years old (a birth or baptism over 100 years ago) or as a parent, grandparent and so on of
  someone who counts as dead: for example a father with no dates who gets a son born in 1782. Any other change, such as a
  death date on a person with no such evidence, is refused; the agent leaves a note and lists the person in its report.
  A death that agrees with what the app already shows is allowed too: on a person you marked *Deceased*, or one who is
  already counted as dead because of their age.
  An event of kind `missing` (went missing without trace, for example in a war) is not a death: the agent may add it with
  `add_event`, and it never changes whether the app counts the person as alive. In GEDCOM it is written as a generic event
  with `TYPE Missing`.
- **Fill in an unknown sex on your people.** Sex can be set only on people the agent added itself. On anyone else a note says
  it instead.
- **Enter a date that reads two ways.** A date like `03/04/1901` could be the third of April or the fourth of March, so it is
  refused. The agent writes the month as a word (`3 APR 1901`) or puts the date in a note.
- **Make a second birth parent** in the same role, or an either-or link. When a source says "this parent or that one", the agent
  makes no link and writes a note on the people involved.
- **Connect people a source says are not related.** They stay apart, and the agent lists them in its report.
- **Open the tree file, or change settings, other connections or the rules.**

### Names as a record writes them

A church metric book of the 19th or early 20th century writes names in the old spelling, «Ѳеодоръ Петровъ Вигаданенко»
for a man whose main name in the tree is «Федір Петрович Вигаданенко». An agent that read such a record keeps the
spelling with `add_name` and `kind: "as_recorded"`: given name, patronymic and surname in their own fields, letter for
letter as written (ѣ, ѳ, ѵ, і and the final ъ stay). It never becomes the main name. Then it cites the record on that
name: `add_citation` with `target: {kind: "name", id: <the id add_name returned>}`, the source of the book and the page
or entry. A reading it is not sure of (a smudged letter, two possible surnames) does not go in the name: it goes in a
note, with the reasons.

The person page shows such a name under the main name («Як записано: …», with the source and year), GEDCOM writes it
as a `NAME` with `TYPE as recorded` (5.5.1) or `TYPE OTHER` + `PHRASE as recorded` (7.0), and search finds the person
by it typed in today's letters (`Феодор Петров` finds `Ѳеодоръ Петровъ`). Only letters are mapped, there is no
dictionary of names: `Іоаннъ` is found by `Иоанн` or `Іоанн`, not by `Іван`.

### Correcting a birth or death date

When you, or a source, have confirmed the exact date of a birth or a death, an agent may set it with the tool
`correct_event_date`, on any person's birth or death event, whoever entered it: it fills an empty date, or replaces one such as
a year alone with the full date. It must give the reason (who confirmed it, or which source says so). Only the date changes;
the place, description and everything else on the event stay as they are.

- The reason is kept as a research note on the event, added in the same change: *"Birth date corrected from 1901 to
  12 MAR 1901. Reason: ..."*. The note is written in English, as the agent's own text is.
- The change is in the **Change log** under the agent's name, with the date before and after, and in the event's history.
  **Undo** takes back the date and the note together.
- It changes whether someone counts as alive or dead only as every other write may (above): a date that would do more is
  refused.
- A death date before the same person's birth date, or a birth date after their death date, is refused (when the two
  dates surely do not overlap): the agent leaves a note with both dates instead. The date holds up to 100 characters,
  the reason up to 1000.
- When sources disagree and nobody has confirmed which is right, the agent does not correct anything: it adds a note with
  both versions.

### Verified corrections

An agent may change data it did not create (yours, an import's, another agent's) when it has read a record that shows the
right value. Four tools, each needs `evidence` (`source`: the id of a source in the tree, `page`, `quote`: the record's
own words, optionally `quality` 0-3) and a `reason`, inside an open batch:

- `correct_name`: the given name, patronymic, surnames or kind of any name of a person, and/or `make_preferred` to make an
  existing name the main one (a right spelling that is only an alternative name today). When the main name's text changes,
  the old text stays as an alternative name «як записано», so nothing is lost.
- `correct_event`: date, place, description, age or cause of any event (`correct_event_date` stays as it was).
- `move_child`: a child's link moved to another family that shares at least one parent with the first (from the father alone
  to the father and the mother). The kind of link stays; a note on the child records the move. A family the agent made in the
  same batch that is left empty is removed.
- `correct_note`: the text of any note, yours too. A note with a story needs the new story or `clear_story`.

The record's words are cited on what changed (the name, the event, the child's link, what the note is attached to). The old
value and the reason are in the **Change log** and the history; **Undo** of the batch puts everything back. A correction
never deletes a person and never changes who counts as alive or dead beyond the rule above (a flip is refused). Without a
record the agent actually read, or when sources disagree and none decides, it adds a note instead.

### Taking back a wrong link

When a family or a source shows that a child the agent linked belongs to other parents, the agent can take its own link back
with the tool `remove_own_child_link` (inputs: the batch, the child, the family the child is to leave, and the reason). Only
that child link goes: the person, the family, its partners and its other children stay, and a family left with no children
stays too (the agent can remove it with `remove_own_empty_family` if it made it in the open batch). The agent then links the
child to the right family with `link_people`.

- "Its own" means the link was added by this same connection (the same key, or all programs without a key together) and nobody
  has changed it since. That may be in this batch or in an earlier, closed one of the same connection. A link you made, one
  from an import, another connection's, or one you edited afterwards, is refused: the agent leaves a note and reports it.
- The reason is required. It is kept as a research note on the person, added in the same change: *"Removed from the family of
  A and B (family ...) as a child. Reason: ..."*, in English like the agent's other text.
- It is in the **Change log** under the agent's name ("Remove child from family"), with the family before and after, and in the
  family's history. **Undo** puts the link back and takes the note away.
- It changes whether someone counts as alive or dead only as every other write may (above): a removal that would do more
  is refused.
- Asking again after the link is gone answers that it is already so; a person who was never a child of that family is refused.

### Events of a place

The Places page lists everything that happened in a place and the places under it: births, marriages, deaths and other
events of people in the family tree, and place records about other people, from church books, confession lists,
revisions and censuses. Two tools serve it.

- `add_place_records` (inputs: the batch, the `place`, and up to 1000 `records`) adds records about people who are **not**
  in the tree. A record has a `kind` (`birth`, `marriage`, `death`, `confession`, `revision`, `census` or `other`), a `date`
  as the book writes it (a partial date is fine; the sortable `year` is read from it), `book`, `page`, `rec_id`, a `note`,
  `unsure`, and `people`: for each given name, patronymic, surname, `as_written`, `role`, `estate`, `age` and a note. Put a
  person of the tree in `person` only when sure it is the same person. With `book` and `rec_id` a record is unique: sending
  it again adds nothing, and the answer says `alreadyThere`. These records are a bulk side table: they are not in the change
  log and not undone with the batch.
- `list_place_events` (inputs: the `place`, `kind` or `kinds`, `year_from`, `year_to`, `who` = `all`, `ours` or `others`, a name `query`,
  `offset` and `limit` up to 200) reads the list oldest first, undated last, with the counts by kind and decade.

- `add_place` (inputs: the batch, `name`, optionally `type`, `parent` (a place id), `lat` and `lon` with a `source`, and a `note`) makes a place that is missing. A place of the same name (or an alternative name, ignoring case) under the same parent is never made twice: the answer is `alreadyThere` with the existing id. It is one change in the log and the batch undoes it. An agent has no other way to make a place (`CreatePlace` stays the owner's).
- `merge_places` (inputs: the batch, `keep_id`, `merge_ids`) merges duplicate places into one, in one undoable step: events and media of the merged places, the place records from books and the places under them are moved to `keep_id`, its names, notes, citations and pictures gain those of the merged ones, and the merged places are removed. A research note on the kept place lists what was merged. Refused when `keep_id` is among `merge_ids` or a merged place is an ancestor of `keep_id`. The answer counts what was repointed. The place records move in the same transaction and come back with an undo of the batch.

- `delete_place` (inputs: the batch, `place_id`) deletes a place that nothing refers to: an empty duplicate. It is refused, with the counts, when an event, a place record from a book, a picture, a citation or a place under it refers to the place (merge such a duplicate with `merge_places`). Its own notes go with it. One change in the log; the batch undoes it and the place comes back.
- `move_place` (inputs: the batch, `place_id`, `new_parent_id` = a place id or `null` for the top level) puts a place under another parent. Refused when the new parent is the place itself or one of its descendants. A place already there answers `alreadyThere`.
- `update_place` (inputs: the batch, `place_id`, and any of `name` (renames the primary name), `type`, `add_alt_names`, `remove_alt_names`, `lat` and `lon` with a `source`) edits a place. The primary name cannot be removed; coordinates need a source (kept in a research note) and replace only a point the agent set itself.
- `remove_own_event` (inputs: the batch, `event`, `reason`) takes back an event the agent created itself, in this batch or an earlier one, that nobody has changed since (the same test as `update_own_event`). The event goes with its citations, and a research note on each person or family it was about records what was removed and why. An event the owner, an import or another connection made is refused. A removal that would change whether someone counts as alive or dead is refused (the owner decides that). The batch undoes it and the event comes back with its citations.

### Places on the map

The Map shows an event only when its place has coordinates. An agent can give a place its point with the tool
`set_place_coordinates` (inputs: the batch, the place, `lat` and `lon` in decimal degrees, north and east positive, the
`source`, and optionally a `note`). For example:
`{"batch": "…", "place": "…", "lat": 46.6333, "lon": 32.6167, "source": "GeoNames 702550", "note": "The parish named in the birth records is in this district."}`.

- Only on a place with **no coordinates yet**. Coordinates you set, or anyone else did (an import, another connection), are
  refused and stay as they are: the agent writes a note instead and lists the place in its report. The one exception is
  its own: it may correct a point it set itself, in this batch or an earlier one, as long as nobody has moved or cleared
  it since. Once you change the point, it is yours. A point you cleared is empty again, and an agent may fill it.
- `lat` must be from -90 to 90 and `lon` from -180 to 180; the point is kept to a millionth of a degree, as when you set it
  on the Map yourself. `source` is required (up to 1000 characters): where the coordinates come from, such as
  `GeoNames 702550` or `Wikipedia: Новотроїцьке (Херсонська обл.)`.
- The source is kept as a research note on the place, added in the same change: *"Coordinates 46.633300, 32.616700
  (latitude, longitude). Source: GeoNames 702550"*, followed by the agent's note if it gave one; a correction reads
  *"Coordinates corrected from ... to ..."*. `get_place` shows the note with the point.
- It is the same point the Map's own *Set coordinates* stores, so the Map shows it at once. It is in the **Change log**
  under the agent's name ("Set place coordinates"), and **Undo** takes back the point and the note together.
- The agent is told to set coordinates only when it is sure the place is the very one in the records: the historical
  village in its district and parish, not a namesake elsewhere. When it is not sure, it gives no coordinates and writes a
  note with the candidates instead.
- The same point again answers `alreadyThere` and adds nothing.

### Old maps on the Map

Scanned old maps (two- and three-verst sheets, squares of an atlas) can lie on the Map under the places. An agent can add
one with `add_map_layer` (inputs: the batch, `file_path` of a JPEG, PNG, WebP, GIF or TIFF picture of at most 50 MiB, a
`title`, the `source` it comes from, and optionally `series`, `year`, `visible`, and the placement: four `corners` as
`{"lat": …, "lon": …}` in decimal degrees, top left, top right, bottom right, bottom left of the picture, or
`ozi_map_path`, the OziExplorer `.map` file that came with the scan). `list_map_layers` lists every map on the Map with
its corners and whether it is the agent's own (`yours`). `set_own_map_layer_corners` places one again.

- The picture is copied into the tree's media like any other file and drawn at 70% opacity. Without corners it waits
  for you to place it: the Map's **Old maps** panel shows "not placed yet".
- **An agent's map is laid hidden** unless the agent gives `visible: true` (it is told to do that only when you asked to
  see that map). You choose what the Map shows in the **Old maps** panel: the maps are grouped by series, each series
  folds and has one box that shows or hides all its maps, **Only this** shows one map and hides the others, **Show all**
  and **Hide all** do what they say, a search box finds a map by title or sheet number, and the maps under the middle of
  the view are listed first. Each of these is one change: one line in the **Change log** ("Show old maps", "Hide old
  maps", "Choose the old maps shown") and one step of **Undo**. An agent cannot show or hide your maps.
- The Map loads only what it shows: a hidden map, or one far off the screen, loads no picture at all; a shown one on
  the screen loads its 1600-pixel preview; the original (at most 8192 pixels across) is loaded only for the two largest
  maps on the screen once you zoom in past the preview. Pictures not shown for a while are let go again. While you pan
  or zoom, the maps already drawn are moved as one picture and drawn afresh when the view rests.
- From a `.map` file the app reads the corner lines (`MMPXY` with `MMPLL`), or else the calibration points
  (`Point01`…), both in the datum the file names; a map drawn on the Pulkovo 1942 datum is moved to WGS 84. The picture
  may be the one the file was made for or that picture at another size. **A picture of another shape** (cropped or
  framed differently, such as a site's JPEG next to its `.ozf2`), or one the file's points fall outside of, would land
  kilometres off: an agent is refused ("The .map file does not fit this picture…"), and when you add such a map
  yourself the app says so and opens the corners at the file's guess for you to drag into place before saving.
- A scanned sheet of up to 160 million pixels (a two-verst sheet is about 11 000 x 10 000) gets a preview; the original
  is kept as it is.
- An agent places only a map it added itself and nobody has changed since. Once you move, hide or fade it, it is
  yours. It never changes the order of the maps and never takes one off the Map.
- The same picture again answers `alreadyThere`. It is in the **Change log** under the agent's name ("Lay map on the
  Map", "Change map layer"), and **Undo** takes it back.

### Rewriting a note at your request

When you ask for it (for example, to retell notes as stories), an agent can replace the text of any note, yours included,
with the tool `rewrite_note` (inputs: the batch, the note, the new text, optionally the kind, and the reason). Its own notes
it still changes with `update_own_note`.

- The reason is required (up to 1000 characters); say who asked and for what. The agent is told to keep every fact, name,
  date and doubt of the note and change the wording only.
- The old text stays: the **Change log** and the note's history keep the note as it was, under the agent's name. The log
  entry reads "Rewrite note: ..." with the reason (the log has no other place for it, so there is no extra note). **Undo**
  of the batch puts the old text and kind back.
- Audio clips, citations and media on the note are not touched. The kind stays as it is unless the agent gives one.
- It changes whether someone counts as alive or dead only as every other write may (above). Saying again what the note says answers that it is already so.

### A readable story beside a note

The app shows notes in two views, **Research** (the note as written) and **Story** (readable). An agent can write the
readable retelling of any note, yours included, with the tool `set_note_story` (inputs: the batch, the note, the story). The
note's text is never changed.

- The story keeps every fact, name, date and doubt of the note and leaves the references out (archive marks, pages, record
  numbers, file names, hypothesis tags, FamilySearch ids).
- It fills a note that has no story. A story already there stays as it is. Saying again what it says answers that it is
  already so. It is in the **Change log** under the agent's name, and **Undo** of the batch takes it off.
- A note with no story is shown in the Story view as its own text with the references removed, with a quiet line saying so.

### Photographed documents

An agent can attach a picture of a document and write down what is on it.

- `add_media` copies a picture (JPEG, PNG, WebP, GIF or TIFF) or a PDF from a file on this computer into the tree, on a
  person, family, event, source or citation. **With agent access on, an agent can attach any picture or PDF that your
  Windows user can read from a local drive.** The copy then lives in the tree: it travels with your backups and with every
  copy of the tree you share, so treat it like a picture you attached yourself.
- What it may attach: a picture that can actually be read, of at most 50 MiB, or a whole PDF of at most 8 MiB (the most
  `get_media` sends back; a larger document is refused, attach its pages as pictures). The path must be a plain path on a
  local drive: network shares (`\\server\share`), device paths and paths through a link (symbolic link or junction) are
  refused before anything is opened, so the tool never makes Windows connect to another computer. The batch, the record
  and the date are checked before the file is touched, and a refused call leaves nothing in the tree's media folder. The
  copy is named by what the file is (a PNG is stored as `.png` whatever its name says), and only the bytes actually copied
  count towards the limit. A sound recording goes only on a source (see below). The same file on the same record twice is recognised and
  not added again; a file already in the tree is linked, and the answer carries its id.
- An agent's picture never becomes a person's main picture: you choose that.
- `set_media_transcription` stores the text of the picture as pieces in reading order. Each piece can have a box on the
  picture (fractions of its width and height, measured on the picture as shown, already turned upright) and a rotation for
  writing that runs sideways. It can also be one text with no boxes. The agent replaces only the pieces it wrote itself;
  pieces you wrote or corrected stay as they are. `update_own_media_region` changes one of its own pieces; a piece you
  wrote with text in it it cannot change at all, not even give it a place.
- `set_media_rotation` turns how the app shows a picture: 0, 90, 180 or 270 degrees clockwise, on top of the turn the
  camera stored in the file (its EXIF orientation). An agent may turn a picture it added itself, or one nobody has turned
  yet; once you or anyone else has turned it, it stays as you left it. It is in the log and **Undo** takes it back. Only
  how the picture is shown changes: the places of the transcription stay measured on the picture upright by its EXIF
  orientation, before this turn, and `get_media` sends the agent that same unturned picture, with the turn as a number
  beside it, so the places and the picture always match.
- `group_media_versions` groups files that show the very same page or document in other quality, such as a new scan and
  an older small copy (inputs: the batch, `primary`, and `others`, a list of media ids). The app then shows one picture for
  the group, the primary, with a «2 versions» badge, and the viewer switches between them. The primary is usually the one
  with the most pixels; `get_media` and `list_media` give the width and height, and show a group as `versionPrimary` and
  `versions`. Nothing is deleted or moved: each file keeps its transcription, its links and its history. A file already in
  a group someone else made stays there and the call is refused; the agent may only add files to a group whose primary is
  the one it names. It never takes a file out of a group. The same group again answers `alreadyThere`; **Undo** takes it
  back with the batch.
- `update_own_media` corrects the title, description, date and place of a picture, document or recording (inputs: the
  batch, the media, and any of `title`, `description`, `date`, `place`; at least one). What is left out stays; an empty
  title, description or place takes it away. The date is read like the date of `add_media`, and a date that cannot be
  read is refused. `place` is written as for `add_event`, smallest place first with commas, and is found or created in
  the same places the events use. `add_media` takes the same `place`, and `get_media` returns it. On a media record the
  agent added itself and nobody has changed since, it may change anything; on any other it can only fill what is empty
  (the same rule as for its own citations), and the answer says so when it refuses. The file, its transcription, clips,
  rotation and links stay. It is in the log as "Update media: <title>" with the old values, **Undo** takes it back,
  and saying the same again answers that it is already so.
- In the app, open the picture: **Show text** lays the pieces over it, **Full text** shows them as one text, and you can
  correct, move, resize, turn, add, reorder or delete pieces. Your edits are ordinary edits: Ctrl+Z undoes them and they
  are in the history. Once you change a piece, it is yours and the agent leaves it alone.
- Transcriptions are not exported to GEDCOM (it has no place for them) and are not searched yet.
- A damaged transcription in the tree file never stops the tree from opening: unreadable pieces are left out, a box off
  the picture is pulled onto it, an unknown rotation becomes upright, and the app reports it on its error output.

### Recordings and clips

An agent that works from a recorded interview can attach the recording and point to the stretches where each part of the
story is told.

- `add_media` takes a sound recording too (MP3, WAV, Ogg, M4A, AAC or FLAC, recognised by its content, never by its name),
  of at most 2 GiB, **on a source only**. The limit holds while the file is copied, so a file that grows meanwhile is cut
  off and refused rather than copied whole. The same path checks as for pictures apply, and a recording aimed at anything
  but a source is refused before a byte is copied. This is `add_media` rather than a new tool because it is the same act:
  a file from this computer, checked and copied, linked to one record.
- `add_audio_clip` links a stretch of a recording to a note, citation, event, person or family: `{batch, media, start,
  end, label?, target: {kind, id}}`. `start` and `end` are either milliseconds from the start of the recording as a whole
  number (`65000`), or a string `"m:ss"`, `"mm:ss"` or `"h:mm:ss"`, with optional decimals of a second (`"1:05"`,
  `"01:05.250"`, `"1:02:05"`); a string of digits alone is milliseconds too. The end must be after the start, and both
  within the recording when its length can be read from the file (an unknown length takes any times). The same clip on the
  same record again is answered `alreadyThere` with the clip's id. Example:
  `{"batch": "…", "media": "…", "start": "1:05", "end": 102500, "label": "How the house was built", "target": {"kind": "note", "id": "…"}}`.
- `update_own_audio_clip` changes the times or label of a clip the agent made, or moves it to another record with
  `move_to`. A clip you made or changed stays as it is. Agents cannot delete clips.
- `get_media` on a recording returns its details, its length (`durationMs`, `duration`), the sources it is on, its
  transcript and every clip with the record it is on, `yours: true` on the agent's own; never its bytes. Every note,
  citation, event, person and family the agent reads lists its clips as `audioClips`.
- `set_media_transcription` on a recording takes pieces with `start`, `end` and `speaker` instead of places on a picture:
  a time-coded transcript. Nothing makes one by itself, and a recording has none unless one is written.
- Clips are ordinary links in the tree: **Undo** of the agent's batch takes them away, and with them the recording's link
  to its source if the batch made it; the copy of a file nothing in the tree (or on your Ctrl+Z steps) names any more is
  then deleted from the tree's media folder. Clips are not exported to GEDCOM; the recording is, on its source.

## Review and undo

Everything an agent does is in the **Change log**, with its name. Until you have looked at it, the person is marked with a
small *Agent* badge on cards, rows and tree boxes, and a person's page has a panel *What an agent changed*.

- **Mark reviewed** clears the badge for one change, one batch or one person.
- **Undo** takes back one change. **Undo batch** takes back everything in a batch that is still standing. Both are recorded in
  the log under your name.
- Undo works fact by fact, not by restoring whole records. If you edited something the agent added, for example you changed
  the text of its note, or gave a person it added more facts, that part stays and the app tells you what was kept.
- An agent's changes are separate from your Ctrl+Z: Ctrl+Z never silently removes an agent's work.
- The log filters by author, by batch, and by *Not yet reviewed*. A batch shows its title, the agent, how many changes it made,
  the file it came from and the agent's final report.
- Changes appear on screen as the agent writes. If you have an edit window open on something that changed, the app says so
  before you save, so that you do not overwrite it.

## If the app is closed

The server is part of the app: when the app closes it stops, and the agent's calls fail until you open the app again. If you
left access on, it starts by itself when the app opens, on the same port. Connections and their keys survive; nothing needs
to be created again.

What the agent wrote before then stays in the tree. A batch it never closed shows as *Unfinished* in the log. It is still
valid, and you can review or undo it like any other.

With the app open but no tree open, every tool answers that no tree is open. A batch is stored in the tree it was opened in.
If you open a different tree, writes to the old batch are refused with "This batch is not in the tree that is open now"; the
agent opens a new batch. The old batch is still there, unfinished, when you open the old tree again.

## Security, in plain words

- Access is off until you turn it on. The server listens on this computer only (`127.0.0.1`), so nothing on your network can
  reach it.
- Web pages in your browser are refused in every mode: a foreign `Host`, `Origin` or `Sec-Fetch-Site` is turned away, and a
  request must be `application/json`, which a page cannot send without the server's permission.
- **By default no key is needed. While that is so, any program running on this computer can read and change the open tree,
  including living relatives.** Turn on **Require a key** if you do not fully trust what runs on this computer.
- A wrong, malformed or revoked key is always refused, even when no key is needed.
- A key is made when you create a connection and shown once.
- The app does not keep the key itself, only a one-way fingerprint of it (a SHA-256 hash), and that is stored in the operating
  system's keychain. Keys are never written to a log.
- **Revoke** a connection in Settings and that agent loses access at once. Its name stays in the log of changes.
- The author of a change always comes from the key used, or is **Agent without a key** when there is none; never from anything
  the agent says about itself.
- Text in your tree and in files the agent reads is treated as data. The rules tell the agent never to take instructions from it.
- What an agent reads may go to its AI provider. Turn access off when you do not need it.

## Limits worth knowing

- You cannot ask an agent to add a death date to someone the app counts as living, or to fill the sex of someone you entered.
  Do those yourself in the person editor, then ask the agent to carry on.
- Text is limited: a note, event description, citation text or one piece of a transcription up to 64 KiB, a whole
  transcription up to 1 MiB and 2,000 pieces, a source's text up to 1 MiB, a batch report up to
  64 KiB. For something longer the agent puts it on a source. A whole request (everything the agent sends in one call) may be
  at most 8 MiB.
- A rerun over the same file adds nothing, provided the agent gave each person its number from that file. Notes, events and
  citations that say the same thing are recognised too.
