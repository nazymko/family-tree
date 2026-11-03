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

fn index_roots(roots: &[PathBuf]) -> HashMap<String, PathBuf> {
    let mut map = HashMap::new();
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
                    map.entry(name.to_lowercase()).or_insert_with(|| e.path());
                }
            }
        }
    }
    map
}

/// Find the file: an absolute path that exists, else relative to each root, else by basename.
pub(crate) fn locate(ctx: &mut Ctx, norm: &str) -> Option<PathBuf> {
    let p = Path::new(norm);
    if p.is_absolute() && p.is_file() {
        return Some(p.to_path_buf());
    }
    let rel_ok = !p.is_absolute()
        && p.components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir));
    if rel_ok {
        for root in &ctx.opts.media_roots {
            let cand = root.join(p);
            if cand.is_file() {
                return Some(cand);
            }
        }
    }
    if ctx.opts.media_roots.is_empty() {
        return None;
    }
    let roots = ctx.opts.media_roots.clone();
    let idx = ctx.media_index.get_or_insert_with(|| index_roots(&roots));
    idx.get(&basename(norm).to_lowercase()).cloned()
}

pub(crate) fn file_name(norm: &str) -> Option<String> {
    let b = basename(norm);
    (!b.is_empty()).then(|| b.to_string())
}
