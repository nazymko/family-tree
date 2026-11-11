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

Voice samples on a note (recordings you attached) show up in what an agent reads as a list with each one's title, type and size. The agent never receives the sound itself: asking for a recording returns only those details, so no audio leaves your computer through the agent. An agent cannot attach, replace or remove a recording; only you can, in the note editor.

It can add:

- people (with names, sex, a birth date and place), and families;
- links between people already in the tree, and extra names for a person;
- events, notes, sources and citations;
- one **marker** for brothers and sisters a source mentions without names (the app draws it as a label reading "Some more
  Sisters/Brothers" in the current language, and it has no name or date);
- the file number of a person, so that a second run finds them again;
- **pictures and PDF documents** from a file on this computer, on a person, family, event, source or citation (see
  [Photographed documents](#photographed-documents));
- the **transcription** of a picture: what is written on it, piece by piece, with where each piece is.

It works in **batches**. A batch has a title, such as the name of the file the agent is working from, and ends with the
agent's own report: what it added, what it left unsettled, and questions for the family.

### What it cannot do

- **Change what is already there.** An agent adds. It does not rewrite a name, a date or a note that you or anyone else wrote,
  and it deletes nothing of yours. If a source disagrees with the tree, the agent adds a note that gives both versions. The
  exceptions are its own work: it may correct a note, event, name, citation or piece of a transcription that it wrote itself
  and that nobody has changed since, and it may remove an empty family it made itself in the batch still open, which a refused
  link can leave behind. And one more, below: a confirmed birth or death date.
- **Add a person with no name.** Someone a source knows only by a role, such as somebody's father, goes in a note on the people
  around them.
- **Change whether a living person counts as alive or dead.** The app decides this from dates and relationships, and the
  owner's *Living*/*Deceased* mark overrides it. An agent can never change the result, directly or by accident, so it cannot
  add a death date or burial to a person the app counts as living. It leaves a note and lists the person in its report.
  A death that agrees with what the app already shows is allowed: on a person you marked *Deceased*, or one who is already
  counted as dead because of their age.
- **Fill in an unknown sex on your people.** Sex can be set only on people the agent added itself. On anyone else a note says
  it instead.
- **Enter a date that reads two ways.** A date like `03/04/1901` could be the third of April or the fourth of March, so it is
  refused. The agent writes the month as a word (`3 APR 1901`) or puts the date in a note.
- **Make a second birth parent** in the same role, or an either-or link. When a source says "this parent or that one", the agent
  makes no link and writes a note on the people involved.
- **Connect people a source says are not related.** They stay apart, and the agent lists them in its report.
- **Open the tree file, or change settings, other connections or the rules.**

### Correcting a birth or death date

When you, or a source, have confirmed the exact date of a birth or a death, an agent may set it with the tool
`correct_event_date`, on any person's birth or death event, whoever entered it: it fills an empty date, or replaces one such as
a year alone with the full date. It must give the reason (who confirmed it, or which source says so). Only the date changes;
the place, description and everything else on the event stay as they are.

- The reason is kept as a research note on the event, added in the same change: *"Birth date corrected from 1901 to
  12 MAR 1901. Reason: ..."*. The note is written in English, as the agent's own text is.
- The change is in the **Change log** under the agent's name, with the date before and after, and in the event's history.
  **Undo** takes back the date and the note together.
- It never changes whether someone counts as alive or dead: a date that would is refused, as every other write is.
- When sources disagree and nobody has confirmed which is right, the agent does not correct anything: it adds a note with
  both versions.

### Photographed documents

An agent can attach a picture of a document and write down what is on it.

- `add_media` copies a picture (JPEG, PNG, WebP, GIF or TIFF) or a PDF from a file on this computer into the tree, on a
  person, family, event, source or citation. The file must be a picture that can actually be read, or a whole PDF, of at most
  50 MiB; anything else is refused before it is copied, so the tool cannot pull other files into the tree. Sound recordings
  stay yours to attach. The same file on the same record twice is recognised and not added again.
- `set_media_transcription` stores the text of the picture as pieces in reading order. Each piece can have a box on the
  picture (fractions of its width and height, measured on the picture as shown, already turned upright) and a rotation for
  writing that runs sideways. It can also be one text with no boxes. The agent replaces only the pieces it wrote itself;
  pieces you wrote or corrected stay as they are. `update_own_media_region` changes one of its own pieces.
- In the app, open the picture: **Show text** lays the pieces over it, **Full text** shows them as one text, and you can
  correct, move, resize, turn, add, reorder or delete pieces. Your edits are ordinary edits: Ctrl+Z undoes them and they
  are in the history. Once you change a piece, it is yours and the agent leaves it alone.
- Transcriptions are not exported to GEDCOM (it has no place for them) and are not searched yet.

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
