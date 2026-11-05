# Agent access

Gene Tree can let an AI agent that runs on your computer read your family tree and add to it. The agent talks to the app over
MCP (the Model Context Protocol). Every change it makes is marked with its name, can be reviewed, and can be undone.

It is off until you turn it on.

## Connect an agent

1. Open the tree you want the agent to work on. The agent sees only the tree that is open.
2. Go to **Settings**, then **Agent access**, and tick **Let agents on this computer connect**. Read the warning first: a
   connected agent can read everything in the open tree, including living relatives, private notes and pictures, and an agent
   may send what it reads to its AI provider.
3. The port is 8765 unless you change it (any number from 1024 to 65535). If another program already uses it, Settings says
   so; choose another.
4. Under **Connections**, give the agent a name and press **Create connection**. Make one connection per agent. The name is
   what the log shows next to every change.
5. A window shows the **secret key** and a ready-made command. The key is shown once. Copy the command now; after you close
   the window nobody can show the key again. If you lose it, revoke the connection and make a new one.
6. Run the command in a terminal. For Claude Code it looks like this (your port and key will differ):

   ```
   claude mcp add --transport http gene-tree http://127.0.0.1:8765/mcp --header "Authorization: Bearer <the key>"
   ```

   Any other MCP client that speaks HTTP works the same way: the address is `http://127.0.0.1:<port>/mcp` and the key goes in
   an `Authorization: Bearer` header.

**Rules agents receive** (a button in the same settings page) shows the instructions every agent is given when it connects.
They are read only.

## What an agent can do

It can read everything in the open tree: people, families, events, places, sources, notes, citations, pictures, the change log
and each record's history. It can also search, and look people up by the number they had in a file.

It can add:

- people (with names, sex, a birth date and place), and families;
- links between people already in the tree, and extra names for a person;
- events, notes, sources and citations;
- one **marker** for brothers and sisters a source mentions without names (the app draws it as *&lt;Some more Sisters/Brothers&gt;* in the
  current language, and it has no name or date);
- the file number of a person, so that a second run finds them again.

It works in **batches**. A batch has a title, such as the name of the file the agent is working from, and ends with the
agent's own report: what it added, what it left unsettled, and questions for the family.

### What it cannot do

- **Change what is already there.** An agent adds. It does not rewrite a name, a date or a note that you or anyone else wrote,
  and it deletes nothing. If a source disagrees with the tree, the agent adds a note that gives both versions. The one
  exception is its own work: it may correct a note, event, name or citation that it wrote itself and that nobody has changed
  since.
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

With the app open but no tree open, every tool answers that no tree is open. If you open a different tree, an agent's open
batch belongs to the old one and writes to it are refused; the agent starts a new batch.

## Security, in plain words

- The server listens on this computer only (`127.0.0.1`). Nothing on your network, and no web page in your browser, can reach
  it: requests with a foreign `Host` or `Origin` are turned away.
- Every request needs a secret key. The key is made when you create the connection and shown once.
- The app does not keep the key itself, only a one-way fingerprint of it (a SHA-256 hash), and that is stored in the operating
  system's keychain. Keys are never written to a log.
- **Revoke** a connection in Settings and that agent loses access at once. Its name stays in the log of changes.
- The author of a change always comes from the key used, never from anything the agent says about itself.
- Text in your tree and in files the agent reads is treated as data. The rules tell the agent never to take instructions from it.
- What an agent reads may go to its AI provider. Turn access off when you do not need it.

## Limits worth knowing

- You cannot ask an agent to add a death date to someone the app counts as living, or to fill the sex of someone you entered.
  Do those yourself in the person editor, then ask the agent to carry on.
- Text is limited: a note, event description or citation text up to 64 KiB, a source's text up to 1 MiB, a batch report up to
  64 KiB. For something longer the agent puts it on a source.
- A rerun over the same file adds nothing, provided the agent gave each person its number from that file. Notes, events and
  citations that say the same thing are recognised too.
