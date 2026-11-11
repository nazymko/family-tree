//! How long a sound recording lasts, read from its container's headers with a few small reads and
//! seeks: never the whole file, so a recording of any size answers at once.
//!
//! Exact for WAV (data size and byte rate), FLAC (STREAMINFO), M4A (`moov/mvhd`), Ogg Vorbis and
//! Opus (the last page's granule position) and MP3 with a Xing/Info or VBRI header. A plain MP3
//! without one gets an estimate from its first frame's bit rate, which is right for constant bit
//! rate and only roughly right otherwise. Raw AAC has no length to read and stays unknown.

use crate::media::{Kind, sniff};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// A recording's length, and whether it was read (`exact`) or estimated from a bit rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Length {
    pub(crate) ms: u64,
    pub(crate) exact: bool,
}

/// The length of the recording at `path`, or `None` when it cannot be told.
pub(crate) fn length(path: &Path) -> Option<Length> {
    let mut f = File::open(path).ok()?;
    let total = f.metadata().ok()?.len();
    let head = read_at(&mut f, 0, 16)?;
    let exact = |ms| Some(Length { ms, exact: true });
    match sniff(&head)? {
        Kind::Wav => exact(wav(&mut f, total)?),
        Kind::Flac => exact(flac(&mut f)?),
        Kind::M4a => exact(m4a(&mut f, total)?),
        Kind::Ogg => exact(ogg(&mut f, total)?),
        Kind::Mp3 => mp3(&mut f, total),
        _ => None,
    }
}

fn read_at(f: &mut File, at: u64, len: usize) -> Option<Vec<u8>> {
    f.seek(SeekFrom::Start(at)).ok()?;
    let mut buf = Vec::with_capacity(len);
    f.take(len as u64).read_to_end(&mut buf).ok()?;
    Some(buf)
}

fn le32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

fn be32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

fn be64(b: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_be_bytes(b.get(at..at + 8)?.try_into().ok()?))
}

fn ms_of(units: u64, per_second: u64) -> Option<u64> {
    if per_second == 0 {
        return None;
    }
    u64::try_from(u128::from(units) * 1000 / u128::from(per_second)).ok()
}

/// RIFF chunks after `WAVE`: the byte rate from `fmt `, the size of `data` (cut to the file, as a
/// recorder that was stopped mid-write leaves a size it never reached).
fn wav(f: &mut File, total: u64) -> Option<u64> {
    let mut at = 12u64;
    let mut byte_rate = None;
    for _ in 0..64 {
        let h = read_at(f, at, 8)?;
        if h.len() < 8 {
            return None;
        }
        let size = u64::from(le32(&h, 4)?);
        match h.get(0..4)? {
            b"fmt " => byte_rate = Some(u64::from(le32(&read_at(f, at + 8, 16)?, 8)?)),
            b"data" => {
                let data = size.min(total.saturating_sub(at + 8));
                return ms_of(data, byte_rate?);
            }
            _ => {}
        }
        at += 8 + size + (size & 1);
    }
    None
}

/// STREAMINFO, always the first metadata block: sample rate (20 bits) and total samples (36).
fn flac(f: &mut File) -> Option<u64> {
    let b = read_at(f, 8, 34)?;
    let rate = (u64::from(*b.get(10)?) << 12)
        | (u64::from(*b.get(11)?) << 4)
        | (u64::from(*b.get(12)?) >> 4);
    let samples = (u64::from(*b.get(13)? & 0x0F) << 32) | u64::from(be32(&b, 14)?);
    if samples == 0 {
        return None;
    }
    ms_of(samples, rate)
}

/// The boxes from `at` to `end`: `(type, body start, body end)`.
fn boxes(f: &mut File, mut at: u64, end: u64) -> Vec<([u8; 4], u64, u64)> {
    let mut out = Vec::new();
    while at + 8 <= end && out.len() < 4096 {
        let Some(h) = read_at(f, at, 16) else { break };
        let (Some(size), Some(kind)) = (be32(&h, 0), h.get(4..8)) else {
            break;
        };
        let Ok(kind) = <[u8; 4]>::try_from(kind) else {
            break;
        };
        let (body, size) = match size {
            0 => (at + 8, end - at),
            1 => match be64(&h, 8) {
                Some(s) => (at + 16, s),
                None => break,
            },
            s => (at + 8, u64::from(s)),
        };
        if size < 8 || at.checked_add(size).is_none_or(|e| e > end) {
            break;
        }
        out.push((kind, body, at + size));
        at += size;
    }
    out
}

/// `moov/mvhd`: duration in units of its timescale. `moov` is often at the end of the file, after
/// the sound; only box headers are read on the way.
fn m4a(f: &mut File, total: u64) -> Option<u64> {
    let (_, start, end) = boxes(f, 0, total)
        .into_iter()
        .find(|(k, ..)| k == b"moov")?;
    let (_, body, _) = boxes(f, start, end)
        .into_iter()
        .find(|(k, ..)| k == b"mvhd")?;
    let b = read_at(f, body, 32)?;
    let (scale, duration) = match b.first()? {
        0 => (be32(&b, 12)?, u64::from(be32(&b, 16)?)),
        1 => (be32(&b, 20)?, be64(&b, 24)?),
        _ => return None,
    };
    if duration == 0 || duration == u64::from(u32::MAX) || duration == u64::MAX {
        return None;
    }
    ms_of(duration, u64::from(scale))
}

/// Ogg: the codec from the first packet, the length from the granule position of the last page.
fn ogg(f: &mut File, total: u64) -> Option<u64> {
    let first = read_at(f, 0, 27 + 255 + 64)?;
    let segments = usize::from(*first.get(26)?);
    let packet = first.get(27 + segments..)?;
    let (rate, skip) = if packet.starts_with(b"\x01vorbis") {
        (u64::from(le32(packet, 12)?), 0)
    } else if packet.starts_with(b"OpusHead") {
        let pre_skip = u16::from_le_bytes(packet.get(10..12)?.try_into().ok()?);
        (48_000, u64::from(pre_skip))
    } else {
        return None;
    };
    // The last page starts in the last 64 KiB (a page holds at most 65 307 bytes).
    let tail_len = total.min(65_536 + 27);
    let tail = read_at(f, total - tail_len, tail_len as usize)?;
    let mut granule = None;
    let mut i = 0;
    while let Some(off) = tail.get(i..)?.windows(4).position(|w| w == b"OggS") {
        let p = i + off;
        if tail.get(p + 4) == Some(&0)
            && let Some(g) = tail.get(p + 6..p + 14)
        {
            let g = u64::from_le_bytes(g.try_into().ok()?);
            if g != u64::MAX {
                granule = Some(g);
            }
        }
        i = p + 4;
    }
    let samples = granule?.checked_sub(skip)?;
    if samples == 0 {
        return None;
    }
    ms_of(samples, rate)
}

/// MPEG audio layer III: frame count from a Xing/Info or VBRI header, else bytes over the first
/// frame's bit rate (an estimate).
fn mp3(f: &mut File, total: u64) -> Option<Length> {
    let mut start = 0u64;
    let tag = read_at(f, 0, 10)?;
    if tag.starts_with(b"ID3") {
        let size = tag
            .get(6..10)?
            .iter()
            .fold(0u64, |acc, b| (acc << 7) | u64::from(b & 0x7F));
        let footer = if tag.get(5)? & 0x10 != 0 { 10 } else { 0 };
        start = 10 + size + footer;
    }
    // The first frame: within a little of where the tag ends (some writers pad).
    let window = read_at(f, start, 4096 + 200)?;
    let (pos, h) = (0..4096).find_map(|i| {
        let h = window.get(i..i + 4)?;
        frame(h).map(|fr| (i, fr))
    })?;
    let frame_at = pos;
    let side = match (h.mpeg1, h.mono) {
        (true, false) => 32,
        (true, true) => 17,
        (false, false) => 17,
        (false, true) => 9,
    };
    let spf = if h.mpeg1 { 1152 } else { 576 };
    let xing = window.get(frame_at + 4 + side..frame_at + 4 + side + 12);
    if let Some(x) = xing.filter(|x| x.starts_with(b"Xing") || x.starts_with(b"Info"))
        && be32(x, 4)? & 1 == 1
    {
        let frames = u64::from(be32(x, 8)?);
        return ms_of(frames * spf, h.rate).map(|ms| Length { ms, exact: true });
    }
    let vbri = window.get(frame_at + 36..frame_at + 36 + 18);
    if let Some(v) = vbri.filter(|v| v.starts_with(b"VBRI")) {
        let frames = u64::from(be32(v, 14)?);
        return ms_of(frames * spf, h.rate).map(|ms| Length { ms, exact: true });
    }
    let audio = total.saturating_sub(start + frame_at as u64);
    ms_of(audio * 8, h.bitrate).map(|ms| Length { ms, exact: false })
}

struct Frame {
    mpeg1: bool,
    mono: bool,
    rate: u64,
    bitrate: u64,
}

fn frame(h: &[u8]) -> Option<Frame> {
    let [a, b, c, d] = *h else { return None };
    if a != 0xFF || b & 0xE0 != 0xE0 || (b >> 1) & 3 != 1 {
        return None;
    }
    let version = (b >> 3) & 3; // 3 = MPEG-1, 2 = MPEG-2, 0 = MPEG-2.5
    let (bi, ri) = (usize::from(c >> 4), usize::from((c >> 2) & 3));
    if version == 1 || bi == 0 || bi == 15 || ri == 3 {
        return None;
    }
    const V1: [u64; 15] = [
        0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320,
    ];
    const V2: [u64; 15] = [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160];
    const RATES: [u64; 3] = [44_100, 48_000, 32_000];
    let mpeg1 = version == 3;
    let div = match version {
        3 => 1,
        2 => 2,
        _ => 4,
    };
    Some(Frame {
        mpeg1,
        mono: d >> 6 == 3,
        rate: RATES.get(ri)? / div,
        bitrate: if mpeg1 { V1.get(bi)? } else { V2.get(bi)? } * 1000,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    /// Synthetic recordings of a known length, built here; nothing recorded is used.
    pub(crate) fn wav(ms: u64) -> Vec<u8> {
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
        out.extend(b"LIST");
        out.extend(4u32.to_le_bytes());
        out.extend(b"INFO");
        out.extend(b"data");
        out.extend(data.to_le_bytes());
        out.resize(out.len() + data as usize, 0);
        out
    }

    pub(crate) fn flac(ms: u64) -> Vec<u8> {
        let rate = 44_100u64;
        let samples = rate * ms / 1000;
        let mut out = b"fLaC".to_vec();
        out.extend([0x80, 0, 0, 34]); // last block, STREAMINFO, 34 bytes
        out.extend([0x10, 0, 0x10, 0, 0, 0, 0, 0, 0, 0]);
        out.push((rate >> 12) as u8);
        out.push((rate >> 4) as u8);
        out.push(((rate & 0x0F) << 4) as u8 | 0x01); // + 1 channel bits and bps
        out.push(0xF0 | ((samples >> 32) as u8 & 0x0F));
        out.extend((samples as u32).to_be_bytes());
        out.extend([0u8; 16]);
        out
    }

    fn mp4_box(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut out = ((body.len() + 8) as u32).to_be_bytes().to_vec();
        out.extend(kind);
        out.extend(body);
        out
    }

    /// `ftyp`, a block of sound (`mdat`), then `moov` with `mvhd` at the end, as phones write it.
    pub(crate) fn m4a(ms: u64, padding: usize) -> Vec<u8> {
        let mut out = mp4_box(b"ftyp", b"M4A \0\0\0\0M4A mp42isom");
        out.extend(mp4_box(b"mdat", &vec![0u8; padding]));
        let mut mvhd = vec![0u8; 4]; // version 0, flags
        mvhd.extend(0u32.to_be_bytes());
        mvhd.extend(0u32.to_be_bytes());
        mvhd.extend(1000u32.to_be_bytes());
        mvhd.extend((ms as u32).to_be_bytes());
        mvhd.extend([0u8; 80]);
        let moov = mp4_box(b"moov", &mp4_box(b"mvhd", &mvhd));
        out.extend(moov);
        out
    }

    fn ogg_page(granule: u64, packet: &[u8], first: bool) -> Vec<u8> {
        let mut out = b"OggS\0".to_vec();
        out.push(if first { 2 } else { 0 });
        out.extend(granule.to_le_bytes());
        out.extend(1u32.to_le_bytes());
        out.extend(0u32.to_le_bytes());
        out.extend(0u32.to_le_bytes());
        out.push(1);
        out.push(packet.len() as u8);
        out.extend(packet);
        out
    }

    pub(crate) fn ogg_vorbis(ms: u64, padding: usize) -> Vec<u8> {
        let mut id = b"\x01vorbis".to_vec();
        id.extend(0u32.to_le_bytes());
        id.push(1);
        id.extend(44_100u32.to_le_bytes());
        id.extend([0u8; 14]);
        let mut out = ogg_page(0, &id, true);
        out.extend(ogg_page(1000, &[0u8; 200], false));
        out.resize(out.len() + padding, 0);
        out.extend(ogg_page(44_100 * ms / 1000, &[0u8; 100], false));
        out
    }

    pub(crate) fn ogg_opus(ms: u64) -> Vec<u8> {
        let mut out = ogg_page(0, b"OpusHead\x01\x01\x38\x01\x80\xbb\0\0\0\0\0", true);
        out.extend(ogg_page(312 + 48 * ms, &[0u8; 50], false));
        out
    }

    /// MPEG-1 layer III, 128 kbit/s, 44.1 kHz: a Xing header when `frames` is given, else plain
    /// frames whose length comes from the bit rate.
    pub(crate) fn mp3(ms: u64, xing: bool) -> Vec<u8> {
        let mut out = b"ID3\x04\x00\x00\x00\x00\x00\x0A".to_vec();
        out.extend([0u8; 10]);
        let frames = 44_100 * ms / 1000 / 1152;
        let frame_len = 417;
        let mut first = vec![0xFF, 0xFB, 0x90, 0x00];
        first.resize(frame_len, 0);
        if xing {
            first[36..40].copy_from_slice(b"Xing");
            first[40..44].copy_from_slice(&1u32.to_be_bytes());
            first[44..48].copy_from_slice(&(frames as u32).to_be_bytes());
            out.extend(&first);
        } else {
            // 128 kbit/s: 16 000 bytes a second.
            let bytes = 16 * ms;
            while (out.len() as u64) < 20 + bytes {
                out.extend(&first);
            }
            out.truncate(20 + bytes as usize);
        }
        out
    }

    fn len_of(bytes: &[u8]) -> Option<Length> {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a");
        std::fs::write(&p, bytes).unwrap();
        length(&p)
    }

    #[test]
    fn reads_the_length_of_each_kind() {
        let exact = |ms| Some(Length { ms, exact: true });
        assert_eq!(len_of(&wav(1500)), exact(1500));
        assert_eq!(len_of(&flac(2250)), exact(2250));
        assert_eq!(len_of(&m4a(61_000, 5000)), exact(61_000));
        assert_eq!(len_of(&ogg_vorbis(3000, 70_000)), exact(3000));
        assert_eq!(len_of(&ogg_opus(4000)), exact(4000));
        let x = len_of(&mp3(10_000, true)).unwrap();
        assert!(x.exact && x.ms.abs_diff(10_000) < 30, "{x:?}");
        let cbr = len_of(&mp3(2000, false)).unwrap();
        assert!(!cbr.exact && cbr.ms.abs_diff(2000) < 30, "{cbr:?}");
    }

    #[test]
    fn a_damaged_or_cut_file_has_no_length_and_never_panics() {
        assert_eq!(len_of(b"RIFF\0\0\0\0WAVE"), None);
        assert_eq!(len_of(b"fLaC"), None);
        let mut m = m4a(1000, 10);
        m.truncate(60);
        assert_eq!(len_of(&m), None);
        assert_eq!(len_of(&ogg_vorbis(0, 0)), None);
        assert_eq!(
            len_of(b"\xFF\xF1\x50\x80\0\0\0"),
            None,
            "raw AAC has no length to read"
        );
        // A data size past the end of a cut recording counts what is there.
        let mut w = wav(1000);
        w.truncate(w.len() - 8000);
        assert_eq!(
            len_of(&w),
            Some(Length {
                ms: 500,
                exact: true
            })
        );
        for cut in 0..80 {
            for b in [
                wav(100),
                flac(100),
                m4a(100, 4),
                ogg_opus(100),
                mp3(100, true),
            ] {
                let _ = len_of(&b[..cut.min(b.len())]);
            }
        }
    }
}
