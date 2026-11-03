use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// New UUID v7 record id.
pub fn new_id() -> Uuid {
    Uuid::now_v7()
}

/// Current time as Unix milliseconds UTC.
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}
