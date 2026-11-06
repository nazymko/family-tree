//! Applying a previewed merge: photos first, then a backup, then the whole merge in one database
//! transaction, so a crash leaves the tree as it was or fully merged. Undoing a merge.

use super::dto::MergeResultDto;
use super::plan::{MediaCopy, Plan, sha_of};
use crate::command::UndoReport;
use crate::error::{ApiError, ErrorCode};
use crate::ops::apply_to_tree_tracked;
use crate::session::Session;
use gt_store::{Actor, BackupReason, Batch, ChangeSet, LogFilter, LogMeta};
use std::io::{Read, Write};
use std::path::Path;
use uuid::Uuid;

fn stale() -> ApiError {
    ApiError::new(
        ErrorCode::Stale,
        "The tree changed after the copy was looked at. Look at the copy again.",
    )
}

/// Copies one photo into the tree's `media/` folder, checking its bytes against the hash while
/// copying; the file appears under its final name only when they match.
fn copy_photo(media_root: &Path, tree_dir: &Path, m: &MediaCopy) -> Result<(), ApiError> {
    let rel = m.rel.strip_prefix("media/").unwrap_or(&m.rel);
    let src = media_root.join(rel);
    let dst = tree_dir.join("media").join(rel);
    let dir = dst
        .parent()
        .ok_or_else(|| ApiError::new(ErrorCode::Invalid, "A photo has no place to go."))?;
    let io = |e: std::io::Error| ApiError::new(ErrorCode::Io, format!("Cannot copy a photo: {e}"));
    std::fs::create_dir_all(dir).map_err(io)?;
    if std::fs::symlink_metadata(&src).is_ok_and(|f| f.file_type().is_symlink()) {
        return Err(ApiError::new(
            ErrorCode::Invalid,
            "A photo in the copy is a link, not a file.",
        ));
    }
    let tmp = dir.join(format!(".part-{}", gt_core::ids::new_id()));
    let result = (|| -> Result<(), ApiError> {
        let mut from = std::fs::File::open(&src).map_err(io)?;
        let mut to = std::fs::File::create(&tmp).map_err(io)?;
        let mut h = <sha2::Sha256 as sha2::Digest>::new();
        let mut buf = vec![0u8; 1 << 16];
        loop {
            let n = from.read(&mut buf).map_err(io)?;
            if n == 0 {
                break;
            }
            sha2::Digest::update(&mut h, &buf[..n]);
            to.write_all(&buf[..n]).map_err(io)?;
        }
        to.flush().map_err(io)?;
        let got: String = sha2::Digest::finalize(h)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        if got != m.sha {
            return Err(ApiError::new(
                ErrorCode::Invalid,
                "A photo in the copy does not match its fingerprint, so nothing was merged.",
            ));
        }
        drop(to);
        std::fs::rename(&tmp, &dst).map_err(io)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

impl Session {
    /// Applies the preview made last, if nothing was written since. All or nothing.
    pub fn merge_apply(&mut self, token: Uuid) -> Result<MergeResultDto, ApiError> {
        let Some(plan) = self.pending_merge.take() else {
            return Err(stale());
        };
        let plan: Plan = *plan;
        if plan.token != token || plan.stamp != self.project.log_stamp()? {
            return Err(stale());
        }
        let left_for_you =
            plan.preview.undecided.len() + plan.preview.removed.len() + plan.preview.blocked.len();
        let mut result = MergeResultDto {
            batch: Uuid::nil(),
            name: plan.name.clone(),
            added: 0,
            changed: 0,
            photos_copied: 0,
            left_for_you: u32::try_from(left_for_you).unwrap_or(u32::MAX),
        };
        if plan.sets.is_empty() {
            return Ok(result);
        }
        // Photos first: a failure here changes nothing but harmless extra files.
        for m in &plan.media {
            copy_photo(&plan.copy.media_root, &self.project.dir, m)?;
            result.photos_copied += 1;
        }
        let keep = self.settings.backups_to_keep as usize;
        self.project.backup(BackupReason::BeforeMerge, keep)?;

        let (_, now) = (self.clock.0)();
        let batch = Batch {
            id: gt_core::ids::new_id(),
            title: format!("Merged copy: {}", plan.name),
            source_file: Some(plan.source_name.clone()),
            actor_id: gt_core::ids::new_id(),
            actor_name: plan.name.clone(),
            started: now,
            ended: Some(now),
            report: None,
        };
        let sets: Vec<(ChangeSet, LogMeta)> = plan
            .sets
            .iter()
            .map(|cs| {
                let meta = LogMeta {
                    actor: Actor::Copy {
                        name: plan.name.clone(),
                    },
                    batch: Some(batch.id),
                    people: self.people_of(cs),
                };
                (cs.clone(), meta)
            })
            .collect();
        for (cs, _) in &sets {
            apply_to_tree_tracked(&mut self.tree, cs, &mut self.edit.removed_at);
        }
        let (effective, _) = self.living_maps();
        let living: Vec<(Uuid, bool)> = effective
            .iter()
            .filter(|(id, v)| self.living.get(id) != Some(v))
            .map(|(id, v)| (*id, *v))
            .collect();
        if let Err(e) = self
            .project
            .apply_merge(&batch, &sets, &living, &plan.ledger, now)
        {
            for (cs, _) in sets.iter().rev() {
                apply_to_tree_tracked(&mut self.tree, &cs.inverse(), &mut self.edit.removed_at);
            }
            return Err(e.into());
        }
        for (cs, _) in &sets {
            for ch in &cs.changes {
                if ch.before.is_none() {
                    result.added += 1;
                } else {
                    result.changed += 1;
                }
            }
        }
        result.batch = batch.id;
        self.rebuild();
        Ok(result)
    }

    /// Undoes a merge from the change log, as `undo_batch` does for an agent. What the owner
    /// changed since stays, and is reported. Undone in full, the copy can be looked at again.
    pub fn undo_merge(&mut self, batch: Uuid) -> Result<UndoReport, ApiError> {
        let filter = LogFilter {
            batch: Some(batch),
            ..Default::default()
        };
        let (groups, _) = self.project.log_page(&filter, 0, usize::MAX)?;
        if groups.is_empty()
            || groups
                .iter()
                .any(|g| !matches!(g.actor, Some(Actor::Copy { .. })))
        {
            return Err(ApiError::not_found("merge", batch));
        }
        let report = self.undo_batch(batch)?;
        if report.skipped.is_empty() {
            self.project.forget_merge(batch)?;
        }
        Ok(report)
    }
}

#[allow(dead_code)]
fn _uses(_: &Path) {
    let _ = sha_of;
}
