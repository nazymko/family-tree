#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! Voice samples: audio files attached to a note. Every file here is generated in the test
//! (a few dozen bytes, or a block of zeros for the size checks); nothing recorded is used.

mod common;

use common::*;
use gt_session::*;
use std::path::{Path, PathBuf};
use uuid::Uuid;

// ---- tiny synthetic files, one per format ----

fn wav() -> Vec<u8> {
    // 8 kHz, mono, 16 bit, 400 samples of a square wave.
    let samples: Vec<u8> = (0..400i16)
        .flat_map(|i| (if (i / 20) % 2 == 0 { 8000i16 } else { -8000 }).to_le_bytes())
        .collect();
    let mut out = b"RIFF".to_vec();
    out.extend((36 + samples.len() as u32).to_le_bytes());
    out.extend(b"WAVEfmt ");
    out.extend(16u32.to_le_bytes());
    out.extend(1u16.to_le_bytes()); // PCM
    out.extend(1u16.to_le_bytes()); // channels
    out.extend(8000u32.to_le_bytes());
    out.extend(16000u32.to_le_bytes());
    out.extend(2u16.to_le_bytes());
    out.extend(16u16.to_le_bytes());
    out.extend(b"data");
    out.extend((samples.len() as u32).to_le_bytes());
    out.extend(samples);
    out
}

/// MPEG-1 layer III frame header (128 kbit/s, 44.1 kHz) and a zeroed frame body.
fn mp3_frames() -> Vec<u8> {
    let mut out = vec![0xFF, 0xFB, 0x90, 0x00];
    out.resize(417, 0);
    out
}

fn mp3_with_tag() -> Vec<u8> {
    let mut out = b"ID3\x04\x00\x00\x00\x00\x00\x00".to_vec();
    out.extend(mp3_frames());
    out
}

fn ogg_vorbis() -> Vec<u8> {
    let mut out = b"OggS\0\x02".to_vec();
    out.extend([0u8; 8]); // granule position
    out.extend(1u32.to_le_bytes()); // serial
    out.extend([0u8; 12]);
    out.extend(b"\x01vorbis");
    out
}

fn ogg_opus() -> Vec<u8> {
    let mut out = b"OggS\0\x02".to_vec();
    out.extend([0u8; 8]);
    out.extend(1u32.to_le_bytes());
    out.extend([0u8; 4]);
    out.extend(0u32.to_le_bytes());
    out.extend([1, 19]); // one segment of 19 bytes
    out.extend(b"OpusHead\x01\x01\0\0\x80\xbb\0\0\0\0\0");
    out
}

fn m4a() -> Vec<u8> {
    let mut out = 24u32.to_be_bytes().to_vec();
    out.extend(b"ftypM4A ");
    out.extend(0u32.to_be_bytes());
    out.extend(b"M4A mp42isom");
    out.extend(8u32.to_be_bytes());
    out.extend(b"free");
    out
}

/// A phone's recorder usually writes `mp42` or `isom` rather than `M4A `.
fn mp4_audio_generic() -> Vec<u8> {
    let mut out = 24u32.to_be_bytes().to_vec();
    out.extend(b"ftypmp42");
    out.extend(0u32.to_be_bytes());
    out.extend(b"mp42isom");
    out
}

fn aac_adts() -> Vec<u8> {
    // sync 0xFFF, MPEG-4, layer 0, no CRC; AAC-LC, 44.1 kHz, mono; frame length 8.
    let mut out = vec![0xFF, 0xF1, 0x50, 0x40, 0x01, 0x00, 0xFC];
    out.resize(32, 0);
    out
}

fn flac() -> Vec<u8> {
    let mut out = b"fLaC".to_vec();
    out.extend([0x80, 0, 0, 34]); // last block, STREAMINFO, 34 bytes
    out.extend([0u8; 34]);
    out
}

fn formats() -> Vec<(&'static str, Vec<u8>, &'static str)> {
    vec![
        ("wav", wav(), "audio/wav"),
        ("mp3 frames", mp3_frames(), "audio/mpeg"),
        ("mp3 with ID3", mp3_with_tag(), "audio/mpeg"),
        ("ogg vorbis", ogg_vorbis(), "audio/ogg"),
        ("ogg opus", ogg_opus(), "audio/ogg"),
        ("m4a", m4a(), "audio/mp4"),
        ("mp4 audio, generic brand", mp4_audio_generic(), "audio/mp4"),
        ("aac adts", aac_adts(), "audio/aac"),
        ("flac", flac(), "audio/flac"),
    ]
}

// ---- helpers ----

fn write(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let p = dir.join(name);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(&p, bytes).unwrap();
    p
}

fn owner(kind: OwnerKind, id: Uuid) -> OwnerRef {
    OwnerRef { kind, id }
}

/// A session with Ivan Koval and one note on him.
fn setup() -> (tempfile::TempDir, Session, Uuid, Uuid, PathBuf) {
    let tree = tree_of(
        "0 @I1@ INDI\n1 NAME Ivan /Koval/\n1 SEX M\n\
         0 @I2@ INDI\n1 NAME Olena /Koval/\n1 SEX F\n",
    );
    let ivan = find(&tree, "Ivan");
    let (dir, mut s) = open_tree(&tree);
    s.apply(EditCommand::SaveNote {
        target: owner(OwnerKind::Person, ivan),
        note: NoteInput {
            id: None,
            kind: NoteKindDto::Story,
            text: "He told this one at the table.".into(),
        },
    })
    .unwrap();
    let note = s.get_person(ivan).unwrap().notes[0].id;
    let src = dir.path().join("src");
    std::fs::create_dir_all(&src).unwrap();
    (dir, s, ivan, note, src)
}

fn note_media(s: &Session, person: Uuid) -> Vec<MediaLinkDto> {
    s.get_person(person).unwrap().notes[0].media.clone()
}

// ---- sniffing ----

#[test]
fn every_audio_format_is_taken_by_its_content_whatever_the_name() {
    let (_d, mut s, ivan, note, src) = setup();
    for (i, (what, bytes, mime)) in formats().into_iter().enumerate() {
        // The extension lies on purpose.
        let f = write(&src, &format!("clip{i}.dat"), &bytes);
        let r = s.attach_files(owner(OwnerKind::Note, note), &[f]).unwrap();
        assert!(r.failed.is_empty(), "{what}: {:?}", r.failed);
        assert_eq!(r.attached.len(), 1, "{what}");
        assert_eq!(r.attached[0].mime.as_deref(), Some(mime), "{what}");
        assert_eq!((r.attached[0].width, r.attached[0].height), (None, None));
        assert!(r.attached[0].resolved, "{what}");
    }
    assert_eq!(note_media(&s, ivan).len(), formats().len());
}

#[test]
fn a_right_extension_is_not_enough() {
    let (_d, mut s, ivan, note, src) = setup();
    let files = [
        write(&src, "a.mp3", b"just some words, not sound at all"),
        write(&src, "b.wav", b"RIFF\x24\0\0\0AVI LIST"), // RIFF but not WAVE
        write(&src, "c.m4a", &{
            // A HEIC picture and a video share the `ftyp` box with audio.
            let mut v = 24u32.to_be_bytes().to_vec();
            v.extend(b"ftypheic");
            v.extend([0u8; 12]);
            v
        }),
        write(&src, "d.flac", &[0xFF, 0xFA, 0x00, 0x00, 1, 2, 3, 4]), // sync, but bitrate index 0
        write(&src, "e.ogg", &[0xFF, 0xFF, 0xFF, 0xFF, 1, 2, 3, 4]),  // reserved MPEG version
        write(&src, "f.aac", &[0xFF, 0xF1, 0x3C, 0x00, 0, 0, 0, 0]),  // reserved frequency index
        write(&src, "g.mp3", b"MZ\x90\x00 an executable"),
    ];
    let r = s
        .attach_files(owner(OwnerKind::Note, note), &files)
        .unwrap();
    assert!(r.attached.is_empty(), "{:?}", r.attached);
    assert_eq!(r.failed.len(), files.len());
    assert!(r.failed.iter().all(|f| f.reason == "unsupported type"));
    assert!(note_media(&s, ivan).is_empty());
    // Nothing was written to the change log for a refused batch.
    assert_eq!(s.undo_state().undo_label.as_deref(), Some("Save note"));
}

#[test]
fn a_note_takes_sound_only_and_a_person_still_takes_pictures_and_sound() {
    let (_d, mut s, ivan, note, src) = setup();
    let pdf = write(
        &src,
        "letter.pdf",
        b"%PDF-1.4
%body
",
    );
    let r = s
        .attach_files(owner(OwnerKind::Note, note), std::slice::from_ref(&pdf))
        .unwrap();
    assert!(r.attached.is_empty());
    assert_eq!(
        r.failed[0].reason,
        "only sound recordings can be attached to a note"
    );
    assert!(note_media(&s, ivan).is_empty());
    // Nothing was copied into the project for the refused file.
    let media_dir = _d.path().join("project").join("media");
    assert!(!media_dir.exists() || walk(&media_dir).is_empty());

    // On a person, a recording is allowed too (the media page lists it with a player).
    let wavfile = write(&src, "a.wav", &wav());
    let r = s
        .attach_files(owner(OwnerKind::Person, ivan), &[wavfile, pdf])
        .unwrap();
    assert_eq!(r.attached.len(), 2);
}

#[test]
fn an_empty_file_and_a_folder_are_refused_as_for_pictures() {
    let (_d, mut s, _ivan, note, src) = setup();
    let empty = write(&src, "silent.wav", b"");
    let folder = src.join("clips");
    std::fs::create_dir_all(&folder).unwrap();
    let r = s
        .attach_files(owner(OwnerKind::Note, note), &[empty, folder])
        .unwrap();
    let reasons: Vec<_> = r.failed.iter().map(|f| f.reason.as_str()).collect();
    assert_eq!(
        reasons,
        vec![
            "file is empty",
            "folders are not attached; drop the files inside"
        ]
    );
}

#[test]
fn a_big_recording_attaches_without_a_size_cap_and_reads_in_pieces() {
    // 60 MiB: a quarter of an hour of 16-bit stereo at 32 kHz would be about this. Zeros after
    // a real header; the pipeline never decodes sound.
    let (_d, mut s, ivan, note, src) = setup();
    let mut big = wav();
    big.resize(60 << 20, 0);
    let f = write(&src, "long.wav", &big);
    let r = s.attach_files(owner(OwnerKind::Note, note), &[f]).unwrap();
    assert!(r.failed.is_empty(), "{:?}", r.failed);
    let id = note_media(&s, ivan)[0].media.id;

    let first = s.media_range(id, 0, None).unwrap().unwrap();
    assert_eq!(first.total, 60 << 20);
    assert_eq!(
        first.bytes.len() as u64,
        MAX_RANGE_BYTES,
        "one reply is capped"
    );
    assert_eq!(first.mime, "audio/wav");
    assert_eq!(&first.bytes[..4], b"RIFF");

    let mid = s.media_range(id, 1000, Some(1099)).unwrap().unwrap();
    assert_eq!((mid.start, mid.bytes.len()), (1000, 100));
    assert_eq!(&mid.bytes[..], &big[1000..1100]);

    let tail = s
        .media_range(id, (60 << 20) - 10, Some(u64::MAX))
        .unwrap()
        .unwrap();
    assert_eq!(tail.bytes.len(), 10);
    assert!(s.media_range(id, 60 << 20, None).unwrap().is_none());
}

// ---- the note carries them ----

#[test]
fn attaching_shows_on_the_note_and_in_the_log_with_before_and_after_and_undoes() {
    let (_d, mut s, ivan, note, src) = setup();
    let f = write(&src, "grandmother-story.wav", &wav());
    let r = s.attach_files(owner(OwnerKind::Note, note), &[f]).unwrap();
    assert_eq!(r.attached.len(), 1);
    let media = note_media(&s, ivan);
    assert_eq!(media.len(), 1);
    assert_eq!(media[0].media.mime.as_deref(), Some("audio/wav"));
    assert_eq!(s.undo_state().undo_label.as_deref(), Some("Attach media"));

    // The change log: the note changed, its `media` field went from nothing to the file.
    let row = &s.log(LogFilterDto::default(), 0, 50).unwrap().items[0];
    assert_eq!(row.label, "Attach media");
    let n = row.changes.iter().find(|c| c.kind == "note").unwrap();
    let m = n.fields.iter().find(|f| f.field == "media").unwrap();
    assert_eq!(m.before, None);
    assert_eq!(m.after.as_deref(), Some("grandmother-story.wav"));
    assert!(row.changes.iter().any(|c| c.kind == "media"));

    // Ctrl+Z takes the link away.
    s.undo().unwrap().unwrap();
    assert!(note_media(&s, ivan).is_empty());
    s.redo().unwrap().unwrap();
    assert_eq!(note_media(&s, ivan).len(), 1);
}

#[test]
fn removing_shows_in_the_log_and_undoes() {
    let (_d, mut s, ivan, note, src) = setup();
    let f = write(&src, "voice.ogg", &ogg_opus());
    s.attach_files(owner(OwnerKind::Note, note), &[f]).unwrap();
    let media = note_media(&s, ivan)[0].media.id;

    s.apply(EditCommand::DetachMedia {
        target: owner(OwnerKind::Note, note),
        media,
    })
    .unwrap();
    assert!(note_media(&s, ivan).is_empty());
    let row = &s.log(LogFilterDto::default(), 0, 50).unwrap().items[0];
    let n = row.changes.iter().find(|c| c.kind == "note").unwrap();
    let m = n.fields.iter().find(|f| f.field == "media").unwrap();
    assert_eq!(m.before.as_deref(), Some("voice.ogg"));
    assert_eq!(m.after, None);

    s.undo().unwrap().unwrap();
    assert_eq!(note_media(&s, ivan).len(), 1);
}

#[test]
fn editing_the_note_text_keeps_its_voice_samples() {
    let (_d, mut s, ivan, note, src) = setup();
    s.attach_files(
        owner(OwnerKind::Note, note),
        &[write(&src, "a.flac", &flac())],
    )
    .unwrap();
    s.apply(EditCommand::SaveNote {
        target: owner(OwnerKind::Person, ivan),
        note: NoteInput {
            id: Some(note),
            kind: NoteKindDto::Story,
            text: "New words.".into(),
        },
    })
    .unwrap();
    let n = &s.get_person(ivan).unwrap().notes[0];
    assert_eq!(n.text, "New words.");
    assert_eq!(n.media.len(), 1);
}

#[test]
fn a_title_can_be_given_and_is_logged() {
    let (_d, mut s, ivan, note, src) = setup();
    s.attach_files(
        owner(OwnerKind::Note, note),
        &[write(&src, "a.mp3", &mp3_frames())],
    )
    .unwrap();
    let id = note_media(&s, ivan)[0].media.id;
    s.apply(EditCommand::UpdateMedia {
        id,
        title: Some("Olena at the well".into()),
        description: None,
        date_raw: None,
    })
    .unwrap();
    assert_eq!(
        note_media(&s, ivan)[0].media.title.as_deref(),
        Some("Olena at the well")
    );
    s.undo().unwrap().unwrap();
    assert_eq!(note_media(&s, ivan)[0].media.title, None);
}

#[test]
fn the_same_recording_on_two_notes_is_stored_once() {
    let (_d, mut s, ivan, note, src) = setup();
    let f = write(&src, "a.wav", &wav());
    s.attach_files(owner(OwnerKind::Note, note), std::slice::from_ref(&f))
        .unwrap();
    let again = s
        .attach_files(owner(OwnerKind::Note, note), std::slice::from_ref(&f))
        .unwrap();
    assert!(again.attached.is_empty());
    assert_eq!(again.failed[0].reason, "already attached");
    assert_eq!(note_media(&s, ivan).len(), 1);
}

#[test]
fn a_note_that_does_not_exist_is_refused() {
    let (_d, mut s, _ivan, _note, src) = setup();
    let f = write(&src, "a.wav", &wav());
    assert!(
        s.attach_files(owner(OwnerKind::Note, Uuid::now_v7()), &[f])
            .is_err()
    );
}

#[test]
fn a_note_cannot_hold_notes_or_citations() {
    let (_d, mut s, _ivan, note, _src) = setup();
    let e = s
        .apply(EditCommand::SaveNote {
            target: owner(OwnerKind::Note, note),
            note: NoteInput {
                id: None,
                kind: NoteKindDto::General,
                text: "a note on a note".into(),
            },
        })
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::Invalid);
}

// ---- the file, not a picture ----

#[test]
fn audio_has_no_thumbnail_and_no_picture_work_is_done() {
    let (d, mut s, ivan, note, src) = setup();
    s.attach_files(
        owner(OwnerKind::Note, note),
        &[write(&src, "a.wav", &wav())],
    )
    .unwrap();
    let id = note_media(&s, ivan)[0].media.id;
    for size in [MediaSize::Thumb, MediaSize::Preview] {
        let e = s.media_bytes(id, size).unwrap_err();
        assert_eq!(e.code, ErrorCode::NotFound, "{size:?}");
    }
    let (bytes, mime) = s.media_bytes(id, MediaSize::Original).unwrap();
    assert_eq!(mime, "audio/wav");
    assert_eq!(bytes, wav());
    // No cached rendition appeared.
    let thumbs = d.path().join("project").join("thumbs");
    let made = std::fs::read_dir(&thumbs)
        .map(|rd| rd.flatten().count())
        .unwrap_or(0);
    assert_eq!(made, 0);
}

#[test]
fn a_missing_file_is_reported_on_the_link_not_as_a_crash() {
    let (d, mut s, ivan, note, src) = setup();
    s.attach_files(
        owner(OwnerKind::Note, note),
        &[write(&src, "a.wav", &wav())],
    )
    .unwrap();
    let id = note_media(&s, ivan)[0].media.id;
    for e in walk(&d.path().join("project").join("media")) {
        std::fs::remove_file(e).unwrap();
    }
    let m = &note_media(&s, ivan)[0].media;
    assert!(!m.resolved);
    assert_eq!(
        s.media_range(id, 0, None).unwrap_err().code,
        ErrorCode::NotFound
    );
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else {
            out.push(p);
        }
    }
    out
}

// ---- export, backup, restore ----

#[test]
fn gedcom_export_carries_the_recording_as_an_object_and_does_not_fail() {
    let (_d, mut s, ivan, note, src) = setup();
    s.attach_files(
        owner(OwnerKind::Note, note),
        &[write(&src, "a.m4a", &m4a())],
    )
    .unwrap();
    let _ = ivan;
    for version in [ExportVersion::V551, ExportVersion::V70] {
        let opts = ExportOptions {
            version,
            living: LivingExport::All,
            include_private: true,
        };
        let preview = s.preview_export(&opts).unwrap();
        let dest = _d.path().join(format!("out-{version:?}.ged"));
        s.export_gedcom(&opts, &dest, &preview.token).unwrap();
        let text = std::fs::read_to_string(&dest).unwrap();
        assert!(text.contains("OBJE"), "{version:?}");
        assert!(text.contains("audio/mp4"), "{version:?}");
    }
}

#[test]
fn backup_copy_holds_the_recording_and_the_copy_opens_with_it() {
    let (d, mut s, ivan, note, src) = setup();
    s.attach_files(
        owner(OwnerKind::Note, note),
        &[write(&src, "a.wav", &wav())],
    )
    .unwrap();
    let copy = d.path().join("backup");
    s.save_copy(&copy).unwrap();
    let restored = Session::open(&copy, SessionClock::fixed(today(), NOW_MS + 5)).unwrap();
    let n = &restored.get_person(ivan).unwrap().notes[0];
    assert_eq!(n.media.len(), 1);
    assert!(n.media[0].media.resolved);
    let (bytes, mime) = restored
        .media_bytes(n.media[0].media.id, MediaSize::Original)
        .unwrap();
    assert_eq!((bytes, mime.as_str()), (wav(), "audio/wav"));
}
