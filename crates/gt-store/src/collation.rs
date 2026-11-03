//! `icu_root` collation: Unicode root-locale ordering (accents sort next to their base letter).

use icu_collator::options::CollatorOptions;
use icu_collator::{Collator, CollatorPreferences};
use rusqlite::Connection;

pub const COLLATION_NAME: &str = "icu_root";

/// Registers the `icu_root` collation on a connection.
pub(crate) fn register(conn: &Connection) -> Result<(), crate::StoreError> {
    let collator = Collator::try_new(CollatorPreferences::default(), CollatorOptions::default())
        .map_err(|e| crate::StoreError::Corrupt(format!("icu collator unavailable: {e}")))?;
    conn.create_collation(COLLATION_NAME, move |a, b| collator.compare(a, b))?;
    Ok(())
}

thread_local! {
    static COLLATOR: Option<icu_collator::CollatorBorrowed<'static>> =
        Collator::try_new(CollatorPreferences::default(), CollatorOptions::default()).ok();
}

/// Compares two strings with the same ICU root collator the database uses.
pub fn collate(a: &str, b: &str) -> std::cmp::Ordering {
    COLLATOR.with(|c| match c {
        Some(c) => c.compare(a, b),
        None => a.cmp(b),
    })
}
