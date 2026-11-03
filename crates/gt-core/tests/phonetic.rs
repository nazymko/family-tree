use gt_core::phonetic::{daitch_mokotoff, soundex};

#[test]
fn soundex_known_values() {
    assert_eq!(soundex("Robert").as_deref(), Some("R163"));
    assert_eq!(soundex("Rupert").as_deref(), Some("R163"));
    assert_eq!(soundex("Ashcraft").as_deref(), Some("A261"));
    assert_eq!(soundex("Tymczak").as_deref(), Some("T522"));
}

#[test]
fn soundex_folds_diacritics() {
    assert_eq!(soundex("Müller"), soundex("Muller"));
    assert_eq!(soundex("Müller").as_deref(), Some("M460"));
    assert_eq!(soundex("Łukasz"), soundex("Lukasz"));
}

#[test]
fn soundex_non_latin_is_none() {
    assert_eq!(soundex("Иван"), None);
    assert_eq!(soundex(""), None);
    assert_eq!(soundex("123 !?"), None);
}

#[test]
fn daitch_mokotoff_known_values() {
    assert!(daitch_mokotoff("Moskowitz").contains(&"645740".to_string()));
    let a = daitch_mokotoff("Auerbach");
    assert!(a.contains(&"097400".to_string()));
    assert!(a.contains(&"097500".to_string()));
}

#[test]
fn daitch_mokotoff_folds_and_rejects() {
    assert_eq!(daitch_mokotoff("Müller"), daitch_mokotoff("Muller"));
    assert!(daitch_mokotoff("Иван").is_empty());
    assert!(daitch_mokotoff("").is_empty());
}
