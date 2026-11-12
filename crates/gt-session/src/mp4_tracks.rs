//! Tells a video from a sound recording inside an ISO-BMFF container (M4A, MP4, 3GP). The brand in
//! `ftyp` cannot: phone voice recorders write `3gp4`/`mp41` for sound-only files, and the same
//! brands head a video. The track handler types (`moov/trak/mdia/hdlr`: `soun` or `vide`) can.

use crate::audio_len::{boxes, read_at};
use std::fs::File;
use std::path::Path;

/// Most `trak` boxes looked into: a real file has a handful; a hostile one must not cost a read
/// per box.
const MAX_TRACKS: usize = 64;

/// Whether the file has a video track: some `moov/trak/mdia/hdlr` with the handler type `vide`.
/// `moov` is found by seeking over box sizes (64-bit ones included), so one at the end of the
/// file after a big `mdat` is found; only box headers and 12 bytes of each handler are read, and
/// at most [`MAX_TRACKS`] tracks. Anything that cannot be read (no `moov`, a cut or garbled box)
/// answers `false`: an audio-only file from a phone may be written without a readable `moov`
/// (the length reader then finds no length), and refusing on a guess would turn real recordings
/// away. The file is refused only when a video track is actually seen.
pub(crate) fn has_video_track(path: &Path) -> bool {
    let Ok(mut f) = File::open(path) else {
        return false;
    };
    let Ok(total) = f.metadata().map(|m| m.len()) else {
        return false;
    };
    video_in(&mut f, total)
}

fn video_in(f: &mut File, total: u64) -> bool {
    let Some((_, start, end)) = boxes(f, 0, total).into_iter().find(|(k, ..)| k == b"moov") else {
        return false;
    };
    let tracks: Vec<_> = boxes(f, start, end)
        .into_iter()
        .filter(|(k, ..)| k == b"trak")
        .take(MAX_TRACKS)
        .collect();
    for (_, trak, trak_end) in tracks {
        let Some((_, mdia, mdia_end)) = boxes(f, trak, trak_end)
            .into_iter()
            .find(|(k, ..)| k == b"mdia")
        else {
            continue;
        };
        let Some((_, hdlr, hdlr_end)) = boxes(f, mdia, mdia_end)
            .into_iter()
            .find(|(k, ..)| k == b"hdlr")
        else {
            continue;
        };
        // version+flags (4), pre_defined (4), handler_type (4).
        if hdlr_end.saturating_sub(hdlr) >= 12
            && read_at(f, hdlr, 12).is_some_and(|b| b.get(8..12) == Some(b"vide"))
        {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]
    use super::*;

    fn bx(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut out = ((body.len() + 8) as u32).to_be_bytes().to_vec();
        out.extend(kind);
        out.extend(body);
        out
    }

    fn track(handler: &[u8; 4]) -> Vec<u8> {
        let mut hdlr = vec![0u8; 8];
        hdlr.extend(handler);
        hdlr.extend([0u8; 12]);
        bx(b"trak", &bx(b"mdia", &bx(b"hdlr", &hdlr)))
    }

    fn file(handlers: &[&[u8; 4]], big_mdat_first: bool, size64: bool) -> Vec<u8> {
        let mut out = bx(b"ftyp", b"mp41\0\0\0\0isom3gp4");
        if big_mdat_first {
            let body = vec![7u8; 50_000];
            if size64 {
                out.extend(1u32.to_be_bytes());
                out.extend(b"mdat");
                out.extend((16 + body.len() as u64).to_be_bytes());
                out.extend(body);
            } else {
                out.extend(bx(b"mdat", &body));
            }
        }
        let mut moov = bx(b"mvhd", &[0u8; 100]);
        for h in handlers {
            moov.extend(track(h));
        }
        out.extend(bx(b"moov", &moov));
        out
    }

    fn has_video(bytes: &[u8]) -> bool {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("f.bin");
        std::fs::write(&p, bytes).unwrap();
        has_video_track(&p)
    }

    #[test]
    fn a_video_track_is_found_wherever_moov_is() {
        for (big, size64) in [(false, false), (true, false), (true, true)] {
            assert!(has_video(&file(&[b"soun", b"vide"], big, size64)));
            assert!(has_video(&file(&[b"vide"], big, size64)));
            assert!(!has_video(&file(&[b"soun"], big, size64)));
            assert!(!has_video(&file(&[b"soun", b"text"], big, size64)));
        }
        assert!(!has_video(&file(&[], false, false)));
    }

    #[test]
    fn garbage_and_cuts_never_panic() {
        let full = file(&[b"soun", b"vide"], true, true);
        for cut in 0..full.len().min(200) {
            let _ = has_video(&full[..cut]);
        }
        let _ = has_video(&full[..full.len() - 3]);
        assert!(!has_video(b""));
        assert!(!has_video(&[0xFF; 300]));
        // A box bigger than the file, a size below the header, a size of 0, a 64-bit size cut short.
        for bad in [u32::MAX, 3, 0, 1] {
            let mut b = bad.to_be_bytes().to_vec();
            b.extend(b"moov");
            b.extend([0u8; 40]);
            assert!(!has_video(&b));
        }
        // 64-bit size claiming 2^63 bytes.
        let mut b = 1u32.to_be_bytes().to_vec();
        b.extend(b"moov");
        b.extend((1u64 << 63).to_be_bytes());
        assert!(!has_video(&b));
    }
}
