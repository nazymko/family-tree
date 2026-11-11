#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! Recordings on sources, clips of them where the story is told, and a recording's time-coded
//! transcript. Every recording here is built in the test: silence of a known length in each
//! container; nothing recorded is used.

mod common;

use common::*;
use gt_session::recording::RecordingDto;
use gt_session::*;
use std::path::{Path, PathBuf};
use std::time::Instant;
use uuid::Uuid;

// ---- synthetic recordings of a known length ----

fn wav(ms: u64) -> Vec<u8> {
    let rate = 8000u32;
    let data = (u64::from(rate) * 2 * ms / 1000) as u32;
    let mut out = b"RIFF".to_vec();
    out.extend((36 + data).to_le_bytes());
    out.extend(b"WAVEfmt ");
    out.extend(16u32.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(rate.to_le_bytes());
    out.extend((rate * 2).to_le_bytes());
    out.extend(2u16.to_le_bytes());
    out.extend(16u16.to_le_bytes());
    out.extend(b"data");
    out.extend(data.to_le_bytes());
    // A quiet saw tooth, so the file is not all zeros.
    out.extend((0..data / 2).flat_map(|i| (((i % 64) as i16 - 32) * 8).to_le_bytes()));
    out
}

fn mp4_box(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut out = ((body.len() + 8) as u32).to_be_bytes().to_vec();
    out.extend(kind);
    out.extend(body);
    out
}

/// `ftyp`, a block standing for the sound, then `moov/mvhd` at the end, as a phone writes it.
fn m4a(ms: u64) -> Vec<u8> {
    let mut out = mp4_box(b"ftyp", b"M4A \0\0\0\0M4A mp42isom");
    out.extend(mp4_box(b"mdat", &[0u8; 4000]));
    let mut mvhd = vec![0u8; 12];
    mvhd.extend(1000u32.to_be_bytes());
    mvhd.extend((ms as u32).to_be_bytes());
    mvhd.extend([0u8; 80]);
    out.extend(mp4_box(b"moov", &mp4_box(b"mvhd", &mvhd)));
    out
}

fn ogg_page(granule: u64, packet: &[u8], first: bool) -> Vec<u8> {
    let mut out = b"OggS\0".to_vec();
    out.push(if first { 2 } else { 0 });
    out.extend(granule.to_le_bytes());
    out.extend(1u32.to_le_bytes());
    out.extend([0u8; 8]);
    out.push(1);
    out.push(packet.len() as u8);
    out.extend(packet);
    out
}

fn ogg_vorbis(ms: u64) -> Vec<u8> {
    let mut id = b"\x01vorbis".to_vec();
    id.extend(0u32.to_le_bytes());
    id.push(1);
    id.extend(44_100u32.to_le_bytes());
    id.extend([0u8; 14]);
    let mut out = ogg_page(0, &id, true);
    out.extend(ogg_page(44_100 * ms / 1000, &[0u8; 100], false));
    out
}

/// MPEG-1 layer III with a Xing header giving the frame count.
fn mp3(ms: u64) -> Vec<u8> {
    let frames = 44_100 * ms / 1000 / 1152;
    let mut first = vec![0xFF, 0xFB, 0x90, 0x00];
    first.resize(417, 0);
    first[36..40].copy_from_slice(b"Xing");
    first[40..44].copy_from_slice(&1u32.to_be_bytes());
    first[44..48].copy_from_slice(&(frames as u32).to_be_bytes());
    let mut out = b"ID3\x04\x00\x00\x00\x00\x00\x00".to_vec();
    out.extend(first);
    out
}

fn flac(ms: u64) -> Vec<u8> {
    let rate = 44_100u64;
    let samples = rate * ms / 1000;
    let mut out = b"fLaC".to_vec();
    out.extend([0x80, 0, 0, 34]);
    out.extend([0x10, 0, 0x10, 0, 0, 0, 0, 0, 0, 0]);
    out.push((rate >> 12) as u8);
    out.push((rate >> 4) as u8);
    out.push(((rate & 0x0F) << 4) as u8 | 0x01);
    out.push(0xF0 | ((samples >> 32) as u8 & 0x0F));
    out.extend((samples as u32).to_be_bytes());
    out.extend([0u8; 16]);
    out
}

// ---- helpers ----

fn me() -> AgentCtx {
    AgentCtx {
        id: Uuid::from_u128(7),
        name: "Interview listener".into(),
    }
}

fn other_agent() -> AgentCtx {
    AgentCtx {
        id: Uuid::from_u128(8),
        name: "Another listener".into(),
    }
}

fn write(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, bytes).unwrap();
    p
}

fn at(kind: OwnerKind, id: Uuid) -> OwnerRef {
    OwnerRef { kind, id }
}

struct World {
    dir: tempfile::TempDir,
    s: Session,
    ivan: Uuid,
    olena: Uuid,
    source: Uuid,
    note: Uuid,
    citation: Uuid,
    birth: Uuid,
    family: Uuid,
}

/// Ivan and Olena Koval, married; Ivan's birth, a story note and a citation of an interview.
fn world() -> World {
    let tree = tree_of(
        "0 @I1@ INDI\n1 NAME Ivan /Koval/\n1 SEX M\n1 BIRT\n2 DATE 1920\n\
         1 SOUR @S1@\n2 PAGE side A\n1 NOTE He told this one at the table.\n1 FAMS @F1@\n\
         0 @I2@ INDI\n1 NAME Olena /Koval/\n1 SEX F\n1 FAMS @F1@\n\
         0 @F1@ FAM\n1 HUSB @I1@\n1 WIFE @I2@\n\
         0 @S1@ SOUR\n1 TITL Interview with Ivan Koval, 1987\n",
    );
    let ivan = find(&tree, "Ivan");
    let olena = find(&tree, "Olena");
    let source = *tree.sources.keys().next().unwrap();
    let note = *tree.notes.keys().next().unwrap();
    let citation = *tree.citations.keys().next().unwrap();
    let birth = *tree.events.keys().next().unwrap();
    let family = *tree.families.keys().next().unwrap();
    let (dir, s) = open_tree(&tree);
    World {
        dir,
        s,
        ivan,
        olena,
        source,
        note,
        citation,
        birth,
        family,
    }
}

/// The same project opened again: nothing the old session read about files is kept.
fn reopen(w: World) -> World {
    let path = w.s.project_dir();
    let World { dir, s, .. } = w;
    drop(s);
    let s = Session::open(&path, SessionClock::fixed(today(), NOW_MS)).unwrap();
    World { dir, s, ..w }
}

impl World {
    /// Attaches a recording to the source as the owner; its media id.
    fn record(&mut self, name: &str, bytes: &[u8]) -> Uuid {
        let f = write(self.dir.path(), name, bytes);
        let r = self
            .s
            .attach_files(at(OwnerKind::Source, self.source), &[f])
            .unwrap();
        assert!(r.failed.is_empty(), "{:?}", r.failed);
        r.attached[0].id
    }

    fn clip(
        &mut self,
        target: OwnerRef,
        media: Uuid,
        start_ms: u64,
        end_ms: u64,
        label: Option<&str>,
    ) -> Result<EditResult, ApiError> {
        self.s.apply(EditCommand::AddAudioClip {
            target,
            media,
            start_ms,
            end_ms,
            label: label.map(str::to_string),
        })
    }

    fn recording(&self, media: Uuid) -> RecordingDto {
        self.s.get_recording(media).unwrap()
    }
}

// ---- lengths, read from headers only ----

#[test]
fn each_kind_of_recording_goes_on_a_source_with_its_length() {
    let mut w = world();
    for (name, bytes, ms) in [
        ("talk.wav", wav(1500), 1500),
        ("talk.m4a", m4a(61_000), 61_000),
        ("talk.ogg", ogg_vorbis(3000), 3000),
        ("talk.flac", flac(2250), 2250),
    ] {
        let id = w.record(name, &bytes);
        assert_eq!(w.recording(id).media.duration_ms, Some(ms), "{name}");
    }
    let id = w.record("talk.mp3", &mp3(10_000));
    let ms = w.recording(id).media.duration_ms.unwrap();
    assert!(ms.abs_diff(10_000) < 30, "{ms}");
    // The source lists them; a picture is not asked for its length.
    let src = w.s.source_dto(w.source).unwrap();
    assert_eq!(src.media.len(), 5);
    assert!(src.media.iter().all(|l| l.clip.is_none()));
}

#[test]
fn a_whole_recording_still_goes_on_a_note_and_nowhere_else_but_a_source() {
    let mut w = world();
    let f = write(w.dir.path(), "a.wav", &wav(500));
    for kind in [
        OwnerKind::Person,
        OwnerKind::Event,
        OwnerKind::Family,
        OwnerKind::Citation,
    ] {
        let id = match kind {
            OwnerKind::Person => w.ivan,
            OwnerKind::Event => w.birth,
            OwnerKind::Family => w.family,
            _ => w.citation,
        };
        let r =
            w.s.attach_files(at(kind, id), std::slice::from_ref(&f))
                .unwrap();
        assert_eq!(
            r.failed[0].reason, "sound recordings can only be attached to a note or a source",
            "{kind:?}"
        );
    }
    let r =
        w.s.attach_files(at(OwnerKind::Note, w.note), std::slice::from_ref(&f))
            .unwrap();
    assert!(r.failed.is_empty());
}

// ---- clips ----

#[test]
fn clips_go_where_the_story_is_told_and_are_read_there() {
    let mut w = world();
    let rec = w.record("interview.m4a", &m4a(600_000));
    let (ivan, note, citation, birth, family) = (w.ivan, w.note, w.citation, w.birth, w.family);
    w.clip(
        at(OwnerKind::Note, note),
        rec,
        65_000,
        102_500,
        Some("The table"),
    )
    .unwrap();
    w.clip(at(OwnerKind::Citation, citation), rec, 1_000, 9_000, None)
        .unwrap();
    w.clip(
        at(OwnerKind::Event, birth),
        rec,
        200_000,
        230_000,
        Some("Born in winter"),
    )
    .unwrap();
    w.clip(
        at(OwnerKind::Person, ivan),
        rec,
        0,
        4_000,
        Some("  Hello  "),
    )
    .unwrap();
    w.clip(
        at(OwnerKind::Family, family),
        rec,
        300_000,
        360_000,
        Some("The wedding"),
    )
    .unwrap();

    let p = w.s.get_person(ivan).unwrap();
    let only = |links: &[MediaLinkDto]| -> ClipDto {
        assert_eq!(links.len(), 1, "{links:?}");
        assert_eq!(links[0].media.id, rec);
        links[0].clip.clone().unwrap()
    };
    let c = only(&p.notes[0].media);
    assert_eq!(
        (c.start_ms, c.end_ms, c.label.as_deref()),
        (65_000, 102_500, Some("The table"))
    );
    assert_eq!(only(&p.citations[0].media).label, None);
    assert_eq!(only(&p.events[0].media).start_ms, 200_000);
    assert_eq!(only(&p.media).label.as_deref(), Some("Hello"), "trimmed");
    assert!(!p.media[0].primary, "a clip is never the main picture");
    assert_eq!(p.media[0].media.duration_ms, Some(600_000));

    let r = w.recording(rec);
    assert_eq!(r.sources.len(), 1);
    assert_eq!(
        r.sources[0].title.as_deref(),
        Some("Interview with Ivan Koval, 1987")
    );
    let starts: Vec<u64> = r.clips.iter().map(|c| c.clip.start_ms).collect();
    assert_eq!(
        starts,
        [0, 1_000, 65_000, 200_000, 300_000],
        "in time order"
    );
    for c in &r.clips {
        assert_eq!(
            c.person.as_ref().map(|p| p.id),
            Some(ivan),
            "{:?}",
            c.target
        );
    }
    let by_kind = |k: OwnerKind| r.clips.iter().find(|c| c.target.kind == k).unwrap();
    assert_eq!(
        by_kind(OwnerKind::Note).title.as_deref(),
        Some("He told this one at the table.")
    );
    assert_eq!(
        by_kind(OwnerKind::Citation).title.as_deref(),
        Some("Interview with Ivan Koval, 1987, side A")
    );
    assert_eq!(
        by_kind(OwnerKind::Event).event_kind.as_deref(),
        Some("birth")
    );
    assert_eq!(
        by_kind(OwnerKind::Family).title.as_deref(),
        Some("Ivan Koval & Olena Koval")
    );
    let _ = w.olena;

    // The same stretch twice on one record is refused; on another record it is a second clip.
    let e = w
        .clip(
            at(OwnerKind::Note, note),
            rec,
            65_000,
            102_500,
            Some("The table"),
        )
        .unwrap_err();
    assert_eq!(e.message, "That clip is already on this record.");
    w.clip(
        at(OwnerKind::Person, ivan),
        rec,
        65_000,
        102_500,
        Some("The table"),
    )
    .unwrap();
    assert_eq!(w.recording(rec).clips.len(), 6);
}

#[test]
fn a_clip_must_fit_its_recording_and_go_on_a_record_that_tells_a_story() {
    let mut w = world();
    let rec = w.record("short.wav", &wav(2_000));
    let note = at(OwnerKind::Note, w.note);
    let msg = |r: Result<EditResult, ApiError>| r.unwrap_err().message;
    assert_eq!(
        msg(w.clip(note, rec, 500, 500, None)),
        "A clip must end after it starts."
    );
    assert_eq!(
        msg(w.clip(note, rec, 900, 500, None)),
        "A clip must end after it starts."
    );
    assert_eq!(
        msg(w.clip(note, rec, 1_000, 2_001, None)),
        "The recording lasts 0:02; a clip must end by then."
    );
    w.clip(note, rec, 1_000, 2_000, None).unwrap();
    let long = "x".repeat(201);
    assert!(msg(w.clip(note, rec, 0, 10, Some(&long))).contains("200 characters"));
    assert_eq!(
        msg(w.clip(at(OwnerKind::Source, w.source), rec, 0, 10, None)),
        "A clip goes on a note, citation, event, person or family."
    );
    // A picture has no clips.
    let png = write(w.dir.path(), "p.png", PNG);
    let pic =
        w.s.attach_files(at(OwnerKind::Person, w.ivan), &[png])
            .unwrap()
            .attached[0]
            .id;
    assert_eq!(
        msg(w.clip(note, pic, 0, 10, None)),
        "Only a sound recording has clips."
    );
}

#[test]
fn a_missing_file_keeps_its_clips_and_takes_any_times() {
    let mut w = world();
    let rec = w.record("gone.wav", &wav(2_000));
    let note = at(OwnerKind::Note, w.note);
    w.clip(note, rec, 0, 1_000, Some("Before")).unwrap();
    let file = w.s.media_original_path(rec).unwrap();
    std::fs::remove_file(&file).unwrap();
    // A fresh session: nothing cached about the file.
    let mut w = reopen(w);
    let r = w.recording(rec);
    assert!(!r.media.resolved);
    assert_eq!(r.media.duration_ms, None);
    assert_eq!(r.clips.len(), 1);
    // Its length cannot be read, so any stretch is taken; the stored type says it is sound.
    w.clip(note, rec, 5_000, 99_000, None).unwrap();
    assert_eq!(w.recording(rec).clips.len(), 2);
}

#[test]
fn a_clip_is_edited_moved_and_deleted_as_one_item_and_each_step_undoes() {
    let mut w = world();
    let rec = w.record("talk.ogg", &ogg_vorbis(120_000));
    let note = at(OwnerKind::Note, w.note);
    let person = at(OwnerKind::Person, w.ivan);
    w.clip(note, rec, 1_000, 2_000, Some("first")).unwrap();
    let id = w.recording(rec).clips[0].clip.id;
    let edit = |s: &mut Session, target, start_ms, end_ms, label: &str, move_to| {
        s.apply(EditCommand::UpdateAudioClip {
            target,
            clip: id,
            start_ms,
            end_ms,
            label: Some(label.into()),
            move_to,
        })
    };
    let r = edit(&mut w.s, note, 1_500, 4_000, "nudged", None).unwrap();
    assert_eq!(r.label, "Edit clip");
    let c = &w.recording(rec).clips[0];
    assert_eq!(
        (c.clip.id, c.clip.start_ms, c.clip.end_ms),
        (id, 1_500, 4_000)
    );
    assert_eq!(c.clip.label.as_deref(), Some("nudged"));
    assert!(edit(&mut w.s, note, 1_500, 999_000, "too long", None).is_err());

    edit(&mut w.s, note, 1_500, 4_000, "nudged", Some(person)).unwrap();
    let r = w.recording(rec);
    assert_eq!(
        (r.clips.len(), r.clips[0].clip.id),
        (1, id),
        "the same clip, moved"
    );
    assert_eq!(r.clips[0].target, person);
    assert!(w.s.get_person(w.ivan).unwrap().notes[0].media.is_empty());

    let r =
        w.s.apply(EditCommand::DeleteAudioClip {
            target: person,
            clip: id,
        })
        .unwrap();
    assert_eq!(r.label, "Delete clip");
    assert!(w.recording(rec).clips.is_empty());
    // The recording stays on its source.
    assert_eq!(w.s.source_dto(w.source).unwrap().media.len(), 1);

    w.s.undo().unwrap();
    assert_eq!(w.recording(rec).clips[0].target, person);
    w.s.undo().unwrap();
    assert_eq!(w.recording(rec).clips[0].target, note);
    w.s.undo().unwrap();
    assert_eq!(
        w.recording(rec).clips[0].clip.label.as_deref(),
        Some("first")
    );
    w.s.undo().unwrap();
    assert!(w.recording(rec).clips.is_empty());

    let e =
        w.s.apply(EditCommand::DeleteAudioClip {
            target: note,
            clip: id,
        })
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::NotFound);
}

#[test]
fn deleting_the_recording_takes_every_clip_of_it_and_undo_brings_them_back() {
    let mut w = world();
    let rec = w.record("talk.wav", &wav(5_000));
    w.clip(at(OwnerKind::Note, w.note), rec, 0, 1_000, None)
        .unwrap();
    w.clip(at(OwnerKind::Person, w.ivan), rec, 0, 1_000, None)
        .unwrap();
    w.clip(at(OwnerKind::Person, w.ivan), rec, 2_000, 3_000, None)
        .unwrap();
    w.s.apply(EditCommand::DeleteMedia { id: rec }).unwrap();
    let p = w.s.get_person(w.ivan).unwrap();
    assert!(p.media.is_empty() && p.notes[0].media.is_empty());
    w.s.undo().unwrap();
    assert_eq!(w.recording(rec).clips.len(), 3);
}

#[test]
fn clips_survive_saving_and_opening_the_tree() {
    let mut w = world();
    let rec = w.record("talk.wav", &wav(5_000));
    w.clip(at(OwnerKind::Event, w.birth), rec, 250, 4_750, Some("Born"))
        .unwrap();
    let before = w.recording(rec).clips;
    let w = reopen(w);
    assert_eq!(w.recording(rec).clips, before);
}

// ---- a large file ----

#[test]
fn a_large_recording_is_measured_and_read_in_pieces_never_whole() {
    let mut w = world();
    // About 5.8 MB: six minutes of 8 kHz mono 16-bit sound.
    let ms = 6 * 60 * 1000;
    let rec = w.record("long.wav", &wav(ms));
    let started = Instant::now();
    assert_eq!(w.recording(rec).media.duration_ms, Some(ms));
    assert!(
        started.elapsed().as_millis() < 500,
        "{:?}",
        started.elapsed()
    );
    let total = w.s.media_size(rec).unwrap();
    assert!(total > 5_000_000);
    // A piece near the end: what a seek to 5:55 asks for.
    let offset = 44 + 16 * 355_000;
    let piece =
        w.s.media_range(rec, offset, Some(offset + 999))
            .unwrap()
            .unwrap();
    assert_eq!(
        (piece.start, piece.bytes.len(), piece.total),
        (offset, 1000, total)
    );
    // An open-ended ask is cut to one window.
    let open = w.s.media_range(rec, 0, None).unwrap().unwrap();
    assert_eq!(open.bytes.len() as u64, gt_session::media::MAX_RANGE_BYTES);
    // A clip in the last seconds fits; past the end does not.
    w.clip(
        at(OwnerKind::Note, w.note),
        rec,
        ms - 5_000,
        ms,
        Some("The end"),
    )
    .unwrap();
    assert!(
        w.clip(at(OwnerKind::Note, w.note), rec, ms - 5_000, ms + 1, None)
            .is_err()
    );
}

// ---- a recording's transcript ----

#[test]
fn a_recording_takes_a_time_coded_transcript_and_a_picture_does_not() {
    let mut w = world();
    let rec = w.record("talk.wav", &wav(10_000));
    let piece =
        |start: Option<u64>, end: Option<u64>, speaker: Option<&str>, text: &str| TextRegionInput {
            id: None,
            area: None,
            rotation: None,
            text: text.into(),
            start_ms: start,
            end_ms: end,
            speaker: speaker.map(str::to_string),
        };
    w.s.apply(EditCommand::SetMediaTranscription {
        id: rec,
        regions: vec![
            piece(
                Some(0),
                Some(2_500),
                Some("Interviewer"),
                "Where were you born?",
            ),
            piece(
                Some(2_500),
                Some(7_000),
                Some("Ivan"),
                "In the village, in winter.",
            ),
            piece(None, None, None, "(laughter)"),
        ],
        known: None,
    })
    .unwrap();
    let t = &w.recording(rec).media.transcription;
    assert_eq!(t.len(), 3);
    assert_eq!((t[1].start_ms, t[1].end_ms), (Some(2_500), Some(7_000)));
    assert_eq!(t[1].speaker.as_deref(), Some("Ivan"));
    assert_eq!((t[2].start_ms, t[2].speaker.as_deref()), (None, None));

    let refuse = |s: &mut Session, id, regions| {
        s.apply(EditCommand::SetMediaTranscription {
            id,
            regions,
            known: None,
        })
        .unwrap_err()
        .message
    };
    assert_eq!(
        refuse(&mut w.s, rec, vec![piece(Some(5), Some(5), None, "a")]),
        "A piece of a transcript must end after it starts."
    );
    assert!(refuse(&mut w.s, rec, vec![piece(Some(5), None, None, "a")]).contains("both"));
    let mut placed = piece(None, None, None, "a");
    placed.area = Some(TextAreaInput {
        x: 0.1,
        y: 0.1,
        w: 0.2,
        h: 0.2,
    });
    assert!(refuse(&mut w.s, rec, vec![placed]).contains("no picture"));
    let png = write(w.dir.path(), "p.png", PNG);
    let pic =
        w.s.attach_files(at(OwnerKind::Person, w.ivan), &[png])
            .unwrap()
            .attached[0]
            .id;
    assert_eq!(
        refuse(&mut w.s, pic, vec![piece(Some(0), Some(5), None, "a")]),
        "Times and speakers belong to the transcript of a sound recording."
    );
}

// ---- agents ----

#[test]
fn an_agent_attaches_a_recording_to_its_source_and_clips_it_and_undoing_the_batch_leaves_nothing() {
    let mut w = world();
    let b = w.s.begin_batch(&me(), "Interview", None).unwrap();
    let file = write(w.dir.path(), "interview.ogg", &ogg_vorbis(90_000));
    let dir = w.s.project_dir();
    // Only a source takes it, refused before anything is copied.
    let e = prepare_agent_file(&dir, &file, OwnerKind::Person)
        .err()
        .unwrap();
    assert!(e.message.contains("source"), "{}", e.message);
    let prepared = prepare_agent_file(&dir, &file, OwnerKind::Source).unwrap();
    let (write, _, rec) =
        w.s.attach_as_agent(
            at(OwnerKind::Source, w.source),
            prepared,
            &MediaAbout::default(),
            &me(),
            b,
        )
        .unwrap();
    assert!(matches!(write, AgentWrite::Done(_)));
    let stored = w.s.media_original_path(rec).unwrap();
    assert!(stored.is_file());

    let add = |s: &mut Session, target, start_ms, end_ms, label: &str| {
        s.apply_as_agent(
            EditCommand::AddAudioClip {
                target,
                media: rec,
                start_ms,
                end_ms,
                label: Some(label.into()),
            },
            &me(),
            b,
        )
    };
    let note = at(OwnerKind::Note, w.note);
    assert!(matches!(
        add(&mut w.s, note, 1_000, 5_000, "Born").unwrap(),
        AgentWrite::Done(_)
    ));
    let id = w.recording(rec).clips[0].clip.id;
    assert_eq!(
        add(&mut w.s, note, 1_000, 5_000, "Born").unwrap(),
        AgentWrite::AlreadyThere { record: id }
    );
    assert!(add(&mut w.s, note, 1_000, 95_000, "too long").is_err());
    // Its own clip it may change; the owner's it may not, nor delete any.
    let mine = w.s.apply_as_agent(
        EditCommand::UpdateAudioClip {
            target: note,
            clip: id,
            start_ms: 1_200,
            end_ms: 5_000,
            label: Some("Born, winter".into()),
            move_to: None,
        },
        &me(),
        b,
    );
    assert!(matches!(mine.unwrap(), AgentWrite::Done(_)));
    w.clip(at(OwnerKind::Person, w.ivan), rec, 0, 900, Some("owner's"))
        .unwrap();
    let owners = w
        .recording(rec)
        .clips
        .iter()
        .find(|c| c.target.kind == OwnerKind::Person)
        .unwrap()
        .clip
        .id;
    let e =
        w.s.apply_as_agent(
            EditCommand::UpdateAudioClip {
                target: at(OwnerKind::Person, w.ivan),
                clip: owners,
                start_ms: 0,
                end_ms: 800,
                label: None,
                move_to: None,
            },
            &me(),
            b,
        )
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::AgentRefused, "{}", e.message);
    let e =
        w.s.apply_as_agent(
            EditCommand::DeleteAudioClip {
                target: note,
                clip: id,
            },
            &me(),
            b,
        )
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::AgentRefused);
    // Another agent's clip is not this one's either.
    let b2 =
        w.s.begin_batch(&other_agent(), "Second look", None)
            .unwrap();
    let e =
        w.s.apply_as_agent(
            EditCommand::UpdateAudioClip {
                target: note,
                clip: id,
                start_ms: 0,
                end_ms: 800,
                label: None,
                move_to: None,
            },
            &other_agent(),
            b2,
        )
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::AgentRefused);
    w.s.end_batch(&other_agent(), b2, "nothing").unwrap();
    // The owner takes the clip off its stretch first, so the batch can undo whole.
    w.s.apply(EditCommand::DeleteAudioClip {
        target: at(OwnerKind::Person, w.ivan),
        clip: owners,
    })
    .unwrap();
    w.s.end_batch(&me(), b, "One recording, one clip.").unwrap();

    let report = w.s.undo_batch(b).unwrap();
    assert!(report.skipped.is_empty(), "{:?}", report.skipped);
    assert!(
        w.s.get_person(w.ivan).unwrap().notes[0].media.is_empty(),
        "the clip went"
    );
    assert!(
        w.s.source_dto(w.source).unwrap().media.is_empty(),
        "the link went"
    );
    assert!(!w.s.tree().media.contains_key(&rec), "the record went");
    assert!(
        !stored.exists(),
        "and its copy, which nothing names any more"
    );
}

#[test]
fn undoing_a_batch_keeps_a_copy_the_owner_can_still_bring_back() {
    let mut w = world();
    let bytes = wav(1_000);
    // The owner attached the same recording and deleted it: Ctrl+Z can bring it back.
    let rec = w.record("same.wav", &bytes);
    w.s.apply(EditCommand::DeleteMedia { id: rec }).unwrap();
    let b = w.s.begin_batch(&me(), "Interview", None).unwrap();
    let file = write(w.dir.path(), "again.wav", &bytes);
    let prepared = prepare_agent_file(&w.s.project_dir(), &file, OwnerKind::Source).unwrap();
    let (_, _, again) =
        w.s.attach_as_agent(
            at(OwnerKind::Source, w.source),
            prepared,
            &MediaAbout::default(),
            &me(),
            b,
        )
        .unwrap();
    let stored = w.s.media_original_path(again).unwrap();
    w.s.end_batch(&me(), b, "done").unwrap();
    w.s.undo_batch(b).unwrap();
    assert!(stored.exists(), "the owner's undo still names it");
    w.s.undo().unwrap();
    assert!(w.s.media_original_path(rec).is_ok());
}

const PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0xF8, 0xFF, 0xFF, 0x3F,
    0x00, 0x05, 0xFE, 0x02, 0xFE, 0xA7, 0x35, 0x81, 0x84, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E,
    0x44, 0xAE, 0x42, 0x60, 0x82,
];

fn _unused(_: &Path) {}
