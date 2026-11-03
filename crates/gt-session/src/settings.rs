//! Project settings kept in the `meta` table, and the in-memory settings of a session.

use crate::dto::{DayOrder, NameOrder, Settings, Theme};
use crate::error::ApiError;
use gt_store::Project;
use uuid::Uuid;

pub const KEY_LIVING_MAX_AGE: &str = "living_max_age";
pub const KEY_HOME_PERSON: &str = "home_person";

/// Reads the project-level values; the rest start at their defaults (the app layer owns them).
pub fn load(project: &Project) -> Result<Settings, ApiError> {
    let mut s = Settings::default();
    if let Some(age) = project
        .meta_get(KEY_LIVING_MAX_AGE)?
        .and_then(|v| v.trim().parse::<u32>().ok())
    {
        s.living_max_age = age;
    }
    s.home_person = project
        .meta_get(KEY_HOME_PERSON)?
        .and_then(|v| Uuid::parse_str(v.trim()).ok());
    Ok(s)
}

/// A partial update; `None` leaves a field alone. `home_person: Some(None)` clears it.
#[derive(Debug, Clone, Default)]
pub struct SettingsPatch {
    pub day_order: Option<DayOrder>,
    pub name_order: Option<NameOrder>,
    pub theme: Option<Theme>,
    pub backups_to_keep: Option<u32>,
    pub living_max_age: Option<u32>,
    pub home_person: Option<Option<Uuid>>,
}
