//! Text mappings for roles, name kinds and parent relationships (both directions).

use gt_core::model::{NameKind, ParentRel, Role};

/// Role from an `ASSO.RELA` (5.5.1) or `ASSO.ROLE` (7.0) text.
pub fn role_from_text(t: &str) -> Role {
    match t.trim().to_ascii_uppercase().as_str() {
        "PRINCIPAL" => Role::Principal,
        "WITN" | "WITNESS" => Role::Witness,
        "GODP" | "GODPARENT" => Role::Godparent,
        "CLERGY" => Role::Clergy,
        "OFFICIATOR" | "OFFICIANT" => Role::Officiator,
        "INFORMANT" => Role::Informant,
        "PARENT" => Role::Parent,
        "FATH" | "FATHER" => Role::Father,
        "MOTH" | "MOTHER" => Role::Mother,
        "CHIL" | "CHILD" => Role::Child,
        "SPOU" | "SPOUSE" => Role::Spouse,
        "HUSB" | "HUSBAND" => Role::Husband,
        "WIFE" => Role::Wife,
        "FRIEND" => Role::Friend,
        "NGHBR" | "NEIGHBOR" => Role::Neighbor,
        "MULTIPLE" => Role::Multiple,
        _ => Role::Other(t.trim().to_string()),
    }
}

/// `(value, phrase)` for a role line; 7.0 uses the enumerated codes plus `OTHER`/`PHRASE`.
pub fn role_text(r: &Role, v70: bool) -> (String, Option<String>) {
    let (code, word) = match r {
        Role::Principal => ("OTHER", "Principal"),
        Role::Informant => ("OTHER", "Informant"),
        Role::Witness => ("WITN", "Witness"),
        Role::Godparent => ("GODP", "Godparent"),
        Role::Clergy => ("CLERGY", "Clergy"),
        Role::Officiator => ("OFFICIATOR", "Officiator"),
        Role::Parent => ("PARENT", "Parent"),
        Role::Father => ("FATH", "Father"),
        Role::Mother => ("MOTH", "Mother"),
        Role::Child => ("CHIL", "Child"),
        Role::Spouse => ("SPOU", "Spouse"),
        Role::Husband => ("HUSB", "Husband"),
        Role::Wife => ("WIFE", "Wife"),
        Role::Friend => ("FRIEND", "Friend"),
        Role::Neighbor => ("NGHBR", "Neighbor"),
        Role::Multiple => ("MULTIPLE", "Multiple"),
        Role::Other(s) => {
            return if v70 {
                ("OTHER".into(), Some(s.clone()))
            } else {
                (s.clone(), None)
            };
        }
    };
    match (v70, code) {
        (true, "OTHER") => ("OTHER".into(), Some(word.into())),
        (true, _) => (code.into(), None),
        (false, _) => (word.into(), None),
    }
}

pub fn name_kind_from_text(t: &str) -> NameKind {
    match t.trim().to_ascii_lowercase().as_str() {
        "birth" | "maiden" => NameKind::Birth,
        "married" => NameKind::Married,
        "aka" => NameKind::Aka,
        "religious" => NameKind::Religious,
        "nickname" => NameKind::Nickname,
        "immigrant" => NameKind::Immigrant,
        "professional" => NameKind::Professional,
        _ => NameKind::Other(t.trim().to_string()),
    }
}

/// `(TYPE value, PHRASE)` for a name kind; `None` for the default (`Birth`).
pub fn name_kind_text(k: &NameKind, v70: bool) -> Option<(String, Option<String>)> {
    let std = |s: &str| {
        if v70 {
            (s.to_ascii_uppercase(), None)
        } else {
            (s.to_string(), None)
        }
    };
    let other = |s: &str| {
        if v70 {
            ("OTHER".to_string(), Some(s.to_string()))
        } else {
            (s.to_string(), None)
        }
    };
    Some(match k {
        NameKind::Birth => return None,
        NameKind::Married => std("married"),
        NameKind::Aka => std("aka"),
        NameKind::Immigrant => std("immigrant"),
        NameKind::Professional => std("professional"),
        NameKind::Religious => other("religious"),
        NameKind::Nickname => other("nickname"),
        NameKind::Other(s) => other(s),
    })
}

pub fn parent_rel_from_text(t: &str) -> ParentRel {
    match t.trim().to_ascii_lowercase().as_str() {
        "birth" | "natural" => ParentRel::Birth,
        "adopted" => ParentRel::Adopted,
        "step" => ParentRel::Step,
        "foster" => ParentRel::Foster,
        "guardian" => ParentRel::Guardian,
        "sealed" | "sealing" => ParentRel::Sealed,
        "unknown" => ParentRel::Unknown,
        _ => ParentRel::Other(t.trim().to_string()),
    }
}

/// Word used in `_FREL`/`_MREL`.
pub fn parent_rel_text(r: &ParentRel) -> String {
    match r {
        ParentRel::Birth => "Natural".into(),
        ParentRel::Adopted => "Adopted".into(),
        ParentRel::Step => "Step".into(),
        ParentRel::Foster => "Foster".into(),
        ParentRel::Guardian => "Guardian".into(),
        ParentRel::Sealed => "Sealed".into(),
        ParentRel::Unknown => "Unknown".into(),
        ParentRel::Other(s) => s.clone(),
    }
}
