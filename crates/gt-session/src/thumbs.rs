//! Thumbnails and previews: bounded decoding, EXIF orientation, resizing and the on-disk cache
//! (`<project>/thumbs/<sha256>-<size>.jpg`, JPEG quality 82, regenerable at any time).

use crate::error::{ApiError, ErrorCode};
use fast_image_resize::{PixelType, Resizer, images::Image};
use image::{
    DynamicImage, ExtendedColorType, ImageDecoder, ImageEncoder, ImageError, ImageReader, Limits,
    codecs::jpeg::JpegEncoder, metadata::Orientation,
};
use std::io::Cursor;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

/// Longest side of a thumbnail in pixels.
pub const THUMB_PX: u32 = 256;
/// Longest side of a preview in pixels.
pub const PREVIEW_PX: u32 = 1600;
/// Images with more pixels than this are stored but never decoded.
pub const MAX_PIXELS: u64 = 100_000_000;
const MAX_SIDE: u32 = 32_768;
const MAX_ALLOC: u64 = 1024 * 1024 * 1024;
const JPEG_QUALITY: u8 = 82;
static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbSize {
    /// 256 px longest side.
    Thumb,
    /// 1600 px longest side.
    Preview,
}

impl ThumbSize {
    pub fn px(self) -> u32 {
        match self {
            ThumbSize::Thumb => THUMB_PX,
            ThumbSize::Preview => PREVIEW_PX,
        }
    }

    fn label(self) -> &'static str {
        match self {
            ThumbSize::Thumb => "thumb",
            ThumbSize::Preview => "preview",
        }
    }
}

pub(crate) const TOO_LARGE: &str = "image too large to preview";

fn too_large() -> ApiError {
    ApiError::new(ErrorCode::Invalid, TOO_LARGE)
}

fn limits() -> Limits {
    let mut l = Limits::default();
    l.max_image_width = Some(MAX_SIDE);
    l.max_image_height = Some(MAX_SIDE);
    l.max_alloc = Some(MAX_ALLOC);
    l
}

fn map_image_err(e: ImageError) -> ApiError {
    match e {
        ImageError::Limits(_) => too_large(),
        _ => ApiError::new(ErrorCode::Invalid, "cannot read image"),
    }
}

/// Most bytes of a file ever handed to the EXIF parser (it reads a whole TIFF into memory).
pub(crate) const EXIF_READ_CAP: u64 = 16 * 1024 * 1024;

/// Parses EXIF from at most `EXIF_READ_CAP` bytes of `src`.
pub(crate) fn read_exif_capped<R: std::io::Read>(src: R) -> Option<exif::Exif> {
    use std::io::Read;
    let mut buf = Vec::new();
    src.take(EXIF_READ_CAP).read_to_end(&mut buf).ok()?;
    exif::Reader::new()
        .read_from_container(&mut Cursor::new(buf))
        .ok()
}

/// EXIF orientation tag (1..=8) of an image container, when present.
pub(crate) fn exif_orientation<R: std::io::Read>(src: R) -> Option<u8> {
    let exif = read_exif_capped(src)?;
    let f = exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY)?;
    let v = f.value.get_uint(0)?;
    u8::try_from(v).ok().filter(|v| (1..=8).contains(v))
}

/// Header-only pixel size, as stored (no orientation, nothing decoded).
pub(crate) fn raw_dims(path: &Path) -> Option<(u32, u32)> {
    let mut reader = ImageReader::open(path).ok()?.with_guessed_format().ok()?;
    reader.limits(limits());
    reader.into_dimensions().ok()
}

/// Pixel size of an image as displayed (EXIF orientation applied), reading headers only.
pub(crate) fn image_dims(path: &Path) -> Option<(u32, u32)> {
    let (w, h) = raw_dims(path)?;
    let file = std::fs::File::open(path).ok()?;
    let turned = exif_orientation(file).is_some_and(|o| o >= 5);
    Some(if turned { (h, w) } else { (w, h) })
}

fn flatten_to_rgb(img: &DynamicImage) -> (u32, u32, Vec<u8>) {
    if img.color().has_alpha() {
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        let mut out = Vec::with_capacity(w as usize * h as usize * 3);
        for p in rgba.pixels() {
            let a = u16::from(p.0[3]);
            for c in &p.0[..3] {
                let v = (u16::from(*c) * a + 255 * (255 - a) + 127) / 255;
                out.push(u8::try_from(v).unwrap_or(255));
            }
        }
        (w, h, out)
    } else {
        let rgb = img.to_rgb8();
        let (w, h) = rgb.dimensions();
        (w, h, rgb.into_raw())
    }
}

fn resized(w: u32, h: u32, rgb: Vec<u8>, longest: u32) -> Result<(u32, u32, Vec<u8>), ApiError> {
    let side = w.max(h);
    if side <= longest {
        return Ok((w, h, rgb));
    }
    let scale = f64::from(longest) / f64::from(side);
    let scaled = |v: u32| ((f64::from(v) * scale).round() as u32).max(1);
    let (nw, nh) = (scaled(w), scaled(h));
    let internal = || ApiError::new(ErrorCode::Internal, "cannot resize image");
    let src = Image::from_vec_u8(w, h, rgb, PixelType::U8x3).map_err(|_| internal())?;
    let mut dst = Image::new(nw, nh, PixelType::U8x3);
    Resizer::new()
        .resize(&src, &mut dst, None)
        .map_err(|_| internal())?;
    Ok((nw, nh, dst.into_vec()))
}

/// Decodes `bytes` (with limits), applies EXIF orientation, scales down to `size` (never up)
/// and encodes a JPEG.
pub fn render(bytes: &[u8], size: ThumbSize) -> Result<Vec<u8>, ApiError> {
    render_px(bytes, size.px())
}

/// Like [`render`] with an explicit longest side in pixels (EXIF orientation applied, never scaled up).
pub fn render_px(bytes: &[u8], px: u32) -> Result<Vec<u8>, ApiError> {
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| ApiError::new(ErrorCode::Invalid, "cannot read image"))?;
    let mut decoder = reader.into_decoder().map_err(map_image_err)?;
    let (w, h) = decoder.dimensions();
    if u64::from(w) * u64::from(h) > MAX_PIXELS {
        return Err(too_large());
    }
    decoder.set_limits(limits()).map_err(map_image_err)?;
    let img = DynamicImage::from_decoder(decoder).map_err(map_image_err)?;
    let (w, h, rgb) = flatten_to_rgb(&img);
    drop(img);
    let (w, h, rgb) = resized(w, h, rgb, px)?;
    let mut out = DynamicImage::ImageRgb8(
        image::RgbImage::from_raw(w, h, rgb)
            .ok_or_else(|| ApiError::new(ErrorCode::Internal, "bad pixel buffer"))?,
    );
    if let Some(o) = exif_orientation(Cursor::new(bytes)).and_then(Orientation::from_exif) {
        out.apply_orientation(o);
    }
    let rgb = out.to_rgb8();
    let mut buf = Vec::new();
    JpegEncoder::new_with_quality(&mut buf, JPEG_QUALITY)
        .write_image(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            ExtendedColorType::Rgb8,
        )
        .map_err(|_| ApiError::new(ErrorCode::Internal, "cannot encode thumbnail"))?;
    Ok(buf)
}

fn valid_sha(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Path of a cached rendition; `None` when the stored hash is not a plain hex digest (so it can
/// never name anything outside the cache) or fails the lexical check.
pub(crate) fn cache_path(
    project: &Path,
    sha: Option<&str>,
    size: ThumbSize,
) -> Option<std::path::PathBuf> {
    let sha = sha.filter(|s| valid_sha(s))?.to_ascii_lowercase();
    let root = project.join("thumbs");
    let p = root.join(format!("{sha}-{}.jpg", size.label()));
    gt_gedcom::is_lexically_under(&p, &[root]).then_some(p)
}

/// The rendition from the cache, or generated from `bytes()` and cached (best effort).
pub(crate) fn cached_render(
    project: &Path,
    sha: Option<&str>,
    size: ThumbSize,
    bytes: impl FnOnce() -> Result<Vec<u8>, ApiError>,
) -> Result<Vec<u8>, ApiError> {
    let cache = cache_path(project, sha, size);
    if let Some(c) = &cache
        && let Ok(hit) = std::fs::read(c)
        && !hit.is_empty()
    {
        return Ok(hit);
    }
    let out = render(&bytes()?, size)?;
    if let Some(c) = cache {
        let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let tmp = c.with_extension(format!("tmp{}-{n}", std::process::id()));
        if std::fs::create_dir_all(c.parent().unwrap_or(project)).is_ok()
            && std::fs::write(&tmp, &out).is_ok()
            && std::fs::rename(&tmp, &c).is_err()
        {
            let _ = std::fs::remove_file(&tmp);
        }
    }
    Ok(out)
}
