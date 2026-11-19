#![allow(clippy::unwrap_used)]
//! The "as recorded" name kind on the wire and in stored JSON (change-log rows keep records as JSON).

use gt_core::model::NameKind;
use gt_session::NameKindDto;

#[test]
fn name_kind_json_round_trips_and_old_values_still_load() {
    let json = serde_json::to_string(&NameKind::AsRecorded).unwrap();
    assert_eq!(json, "\"AsRecorded\"");
    assert_eq!(
        serde_json::from_str::<NameKind>(&json).unwrap(),
        NameKind::AsRecorded
    );
    // Rows written before the kind existed read as they did.
    for (old, kind) in [
        ("\"Birth\"", NameKind::Birth),
        ("\"Married\"", NameKind::Married),
        (
            "{\"Other\":\"Pen name\"}",
            NameKind::Other("Pen name".into()),
        ),
    ] {
        assert_eq!(serde_json::from_str::<NameKind>(old).unwrap(), kind);
    }
}

#[test]
fn the_wire_word_is_as_recorded() {
    assert_eq!(
        serde_json::to_string(&NameKindDto::AsRecorded).unwrap(),
        "\"as_recorded\""
    );
    assert_eq!(
        serde_json::from_str::<NameKindDto>("\"as_recorded\"").unwrap(),
        NameKindDto::AsRecorded
    );
    assert_eq!(
        serde_json::to_string(&NameKindDto::Professional).unwrap(),
        "\"professional\""
    );
}
