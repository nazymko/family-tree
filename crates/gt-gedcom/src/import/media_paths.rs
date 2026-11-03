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

/// Resolve `.` and reject `..` without touching the filesystem.
fn lexical_normalize(p: &Path) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => return None,
            other => out.push(other.as_os_str()),
        }
    }
    Some(out)
}

/// What the purely lexical check says about an absolute candidate.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Lexical {
    /// Component-wise under one of the roots (normalised path).
    Under(PathBuf),
    /// Not under any root (this includes UNC paths that are not under a UNC root).
    Outside,
}

/// Decide, with no filesystem access, whether an absolute candidate lies under one of `roots`.
/// UNC and verbatim-UNC candidates qualify only when lexically under a root (so only for a UNC
/// root): nothing here may make Windows contact a network share named by an untrusted file.
pub(crate) fn lexical_check(cand: &Path, roots: &[PathBuf]) -> Lexical {
    let Some(c) = lexical_normalize(cand) else {
        return Lexical::Outside;
    };
    let under = roots
        .iter()
        .filter_map(|r| lexical_normalize(r))
        .any(|r| !r.as_os_str().is_empty() && c.starts_with(&r));
    if under {
        Lexical::Under(c)
    } else {
        Lexical::Outside
    }
}

fn canonical_roots(ctx: &mut Ctx, roots: &[PathBuf]) -> Vec<PathBuf> {
    ctx.canon_roots
        .get_or_insert_with(|| roots.iter().filter_map(|r| r.canonicalize().ok()).collect())
        .clone()
}

/// After the lexical check: the file must exist and, with symlinks resolved, still be under a
/// canonical root.
fn confirm(ctx: &mut Ctx, roots: &[PathBuf], cand: &Path) -> bool {
    let canon_roots = canonical_roots(ctx, roots);
    cand.canonicalize()
        .is_ok_and(|c| c.is_file() && canon_roots.iter().any(|r| c.starts_with(r)))
}

/// Find the file. No filesystem call is made on a candidate before it passes a lexical check
/// against the media roots and the GEDCOM's own directory (an untrusted file must neither pull
/// in arbitrary local files nor name a network share); a relative path is tried against each of
/// those; last, a unique basename match under the media roots.
pub(crate) fn locate(ctx: &mut Ctx, norm: &str) -> Located {
    let p = Path::new(norm);
    let mut allowed: Vec<PathBuf> = ctx.opts.media_roots.clone();
    allowed.extend(ctx.opts.gedcom_dir.clone());
    let absolute = p.is_absolute() || norm.starts_with("//");
    if absolute {
        if let Lexical::Under(c) = lexical_check(p, &allowed)
            && confirm(ctx, &allowed, &c)
        {
            return Located::Found(c);
        }
    } else if p
        .components()
        .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
    {
        for root in &allowed {
            let cand = root.join(p);
            if matches!(lexical_check(&cand, &allowed), Lexical::Under(_))
                && confirm(ctx, &allowed, &cand)
            {
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
    if absolute {
        Located::OutsideRoots
    } else {
        Located::NotFound
    }
}

pub(crate) fn file_name(norm: &str) -> Option<String> {
    let b = basename(norm);
    (!b.is_empty()).then(|| b.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexical_check_never_needs_the_filesystem() {
        let roots = vec![PathBuf::from("/media/pics")];
        assert_eq!(
            lexical_check(Path::new("//attacker/share/x.jpg"), &roots),
            Lexical::Outside
        );
        assert_eq!(
            lexical_check(Path::new("/media/pics/../etc/x"), &roots),
            Lexical::Outside
        );
        assert!(matches!(
            lexical_check(Path::new("/media/pics/./a/x.jpg"), &roots),
            Lexical::Under(_)
        ));
        let unc = vec![PathBuf::from("//nas/photos")];
        assert!(matches!(
            lexical_check(Path::new("//nas/photos/a.jpg"), &unc),
            Lexical::Under(_)
        ));
        assert_eq!(
            lexical_check(Path::new("//other/photos/a.jpg"), &unc),
            Lexical::Outside
        );
    }
}
