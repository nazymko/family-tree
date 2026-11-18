//! Godparents written by an agent: a repeat is no second godparent, and an agent takes off only a
//! godparent it added itself and nobody has changed since.

use super::{AgentCtx, AgentWrite, Written, item_is_own, refused};
use crate::command::EditCommand;
use crate::error::ApiError;
use crate::ops::godparent::same_name;
use crate::ops::plan_with;
use crate::session::Session;
use uuid::Uuid;

impl Session {
    /// The godparent this addition would add again: the same person, or the same written name,
    /// on the same event. Answered as `alreadyThere`, naming that godparent.
    pub(super) fn godparent_repeat(&self, cmd: &EditCommand) -> Option<Uuid> {
        let EditCommand::AddGodparent { event, godparent } = cmd else {
            return None;
        };
        let e = self.tree.events.get(event)?;
        e.godparents
            .iter()
            .find(|g| match (godparent.person, godparent.name.as_deref()) {
                (Some(p), None) => g.person == Some(p),
                (None, Some(n)) if !n.trim().is_empty() => {
                    g.person.is_none() && g.name.as_deref().is_some_and(|o| same_name(o, n))
                }
                _ => false,
            })
            .map(|g| g.id)
    }

    /// Takes off a godparent this connection added (in any batch), when nobody else has changed
    /// it since. Gone already: `AlreadyThere`. Anyone else's: refused.
    pub fn remove_own_godparent(
        &mut self,
        event: Uuid,
        godparent: Uuid,
        ctx: &AgentCtx,
        batch: Uuid,
    ) -> Result<(AgentWrite, Vec<Uuid>), ApiError> {
        self.check_batch(ctx, batch)?;
        let Some(e) = self.tree.events.get(&event) else {
            return Err(ApiError::not_found("event", event));
        };
        let there = e.godparents.iter().any(|g| g.id == godparent);
        let rows: Vec<Written> = self
            .project
            .last_writers(event)?
            .into_iter()
            .map(|(by, _)| by)
            .zip(self.project.rows_of(event)?)
            .collect();
        let path = ["Event".to_string(), "godparents".to_string()];
        let id = serde_json::json!(godparent);
        if !there {
            let was_there = rows.iter().any(|(_, c)| {
                c.before
                    .as_ref()
                    .and_then(crate::revert::stripped)
                    .and_then(|(json, _)| {
                        let list = super::at_path(&json, &path)?.as_array()?.clone();
                        super::find_item(&list, &id).map(|_| ())
                    })
                    .is_some()
            });
            if was_there {
                return Ok((AgentWrite::AlreadyThere { record: godparent }, Vec::new()));
            }
            return Err(ApiError::not_found("godparent", godparent));
        }
        if !item_is_own(&rows, &path, &id, ctx.id) {
            return Err(refused(
                "Only a godparent you added yourself, which nobody has changed since, can be \
                 taken off. Leave a note on the event and list it in your report.",
            ));
        }
        let cmd = EditCommand::RemoveGodparent { event, godparent };
        let (_, now) = (self.clock.0)();
        let (cs, focus) = plan_with(&self.tree, &cmd, now, self.day_order(), None)?;
        self.pass_and_store(&cmd, cs, focus, ctx, batch)
    }
}
