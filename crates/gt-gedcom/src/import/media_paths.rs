//! Locating media files named by `FILE` against the user's media roots.

use super::Ctx;
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

const MAX_DEPTH: usize = 12;
const MAX_ENTRIES: usize = 500_000;

/// Normalise a GEDCOM file path to `/` separators.
pub(crate) fn normalize(path: &str) -> String {
    path.trim().replace('\\', "/")
}

pub(crate) fn is_url(path: &str) -> bool {
    path.contains("://") || path.starts_with("data:")
}

fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn index_roots(roots: &[PathBuf]) -> HashMap<String, Vec<PathBuf>> {
    let mut map: HashMap<String, Vec<PathBuf>> = HashMap::new();
    let mut seen = 0usize;
    for root in roots {
        let mut stack = vec![(root.clone(), 0usize)];
        while let Some((dir, depth)) = stack.pop() {
            let Ok(rd) = std::fs::read_dir(&dir) else {
                continue;
            };
            for e in rd.flatten() {
                seen += 1;
                if seen > MAX_ENTRIES {
                    return map;
                }
                let Ok(ft) = e.file_type() else { continue };
                if ft.is_symlink() {
                    continue;
                }
                if ft.is_dir() {
                    if depth < MAX_DEPTH {
                        stack.push((e.path(), depth + 1));
                    }
                } else if ft.is_file()
                    && let Some(name) = e.file_name().to_str()
                {
                    let list = map.entry(name.to_lowercase()).or_default();
                    if !list.contains(&e.path()) {
                        list.push(e.path());
                    }
                }
            }
        }
    }
    for list in map.values_mut() {
        list.sort();
    }
    map
}

/// `file:///C:/x.jpg`, `file:///home/x.jpg` -> a plain path (percent-decoded). Other `file://`
/// forms (a host part) are returned as `//host/path`, which never lies under a root.
pub(crate) fn file_url_to_path(url: &str) -> Option<String> {
    let rest = url.get(..7).filter(|p| p.eq_ignore_ascii_case("file://"))?;
    let rest = url.get(rest.len()..)?;
    let path = match rest.strip_prefix('/') {
        Some(p) if p.as_bytes().get(1) == Some(&b':') => p.to_string(),
        Some(_) => rest.to_string(),
        None => format!("//{rest}"),
    };
    Some(percent_decode(&path))
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while let Some(&c) = b.get(i) {
        let hex = |k: usize| b.get(k).and_then(|&h| (h as char).to_digit(16));
        if c == b'%'
            && let (Some(h), Some(l)) = (hex(i + 1), hex(i + 2))
        {
            out.push((h * 16 + l) as u8);
            i += 3;
        } else {
            out.push(c);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Outcome of looking for a media file.
pub(crate) enum Located {
    Found(PathBuf),
    /// An absolute path to something outside the GEDCOM's folder and the media roots.
    OutsideRoots,
    /// Several files under the roots share the basename.
    Ambiguous(Vec<PathBuf>),
    NotFound,
}

fn under_roots(p: &Path, roots: &[PathBuf]) -> bool {
    let Ok(canon) = p.canonicalize() else {
        return false;
    };
    roots
        .iter()
        .filter_map(|r| r.canonicalize().ok())
        .any(|r| canon.starts_with(r))
}

/// Find the file. An absolute path is used only when it lies under the GEDCOM's own directory or
/// a media root (an untrusted file must not pull in arbitrary local files); a relative path is
/// tried against each of those; last, a unique basename match under the media roots.
pub(crate) fn locate(ctx: &mut Ctx, norm: &str) -> Located {
    let p = Path::new(norm);
    let mut allowed: Vec<PathBuf> = ctx.opts.media_roots.clone();
    allowed.extend(ctx.opts.gedcom_dir.clone());
    let absolute = p.is_absolute();
    if absolute && p.is_file() && under_roots(p, &allowed) {
        return Located::Found(p.to_path_buf());
    }
    let rel_ok = !absolute
        && p.components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir));
    if rel_ok {
        for root in &allowed {
            let cand = root.join(p);
            if cand.is_file() {
                return Located::Found(cand);
            }
        }
    }
    if !ctx.opts.media_roots.is_empty() {
        let roots = ctx.opts.media_roots.clone();
        let idx = ctx.media_index.get_or_insert_with(|| index_roots(&roots));
        match idx.get(&basename(norm).to_lowercase()).map(Vec::as_slice) {
            Some([one]) => return Located::Found(one.clone()),
            Some(many) if many.len() > 1 => return Located::Ambiguous(many.to_vec()),
            _ => {}
        }
    }
    if absolute && p.is_file() {
        Located::OutsideRoots
    } else {
        Located::NotFound
    }
}

pub(crate) fn file_name(norm: &str) -> Option<String> {
    let b = basename(norm);
    (!b.is_empty()).then(|| b.to_string())
}
