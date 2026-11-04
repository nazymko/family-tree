//! The AI reader's answer parser, on fixed invented text. No network.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use gt_session::parse_ai_reply;

const GOOD: &str = r#"Here is the result:
```json
{"people":[
 {"id":"p1","given":"Ivan","surname":"Koval","patronymic":"Petrovych","sex":"male","birth":"1901","death":1970,
  "box":{"x":0.1,"y":0.2,"w":0.1,"h":0.05},"confidence":0.9},
 {"id":"p2","given":"Olena","surname":"Koval","box":{"x":0.3,"y":0.2,"w":0.1,"h":0.05},"confidence":0.7}
],
"relations":[
 {"kind":"partner","from":"p1","to":"p2"},
 {"kind":"parent","from":"p1","to":"p9"}
]}
```"#;

#[test]
fn parses_a_valid_answer_with_prose_and_fences() {
    let r = parse_ai_reply(GOOD);
    assert_eq!(r.people.len(), 2);
    assert_eq!(r.people[0].patronymic, "Petrovych");
    assert_eq!(r.people[0].death.as_deref(), Some("1970"));
    assert_eq!(r.people[1].sex, None);
    assert_eq!(r.relations.len(), 1);
    // The relation naming an unknown person is skipped, not fatal.
    assert_eq!(r.skipped, 1);
    assert!(!r.truncated);
}

#[test]
fn skips_malformed_entries_and_counts_them() {
    let t = r#"{"people":[
      {"id":"a","given":"Ann","box":{"x":0.1,"y":0.1,"w":0.1,"h":0.1}},
      {"id":"b","given":"","surname":"","box":{"x":0.1,"y":0.1,"w":0.1,"h":0.1}},
      {"id":"c","given":"Cy"},
      {"id":"d","given":"Di","box":{"x":"left","y":0.1,"w":0.1,"h":0.1}},
      {"id":"a","given":"Dup","box":{"x":0.1,"y":0.1,"w":0.1,"h":0.1}},
      "not an object", 42, {"id":"e","given":"Eve","box":{"x":0.1,"y":0.1,"w":0,"h":0.1}}
    ],"relations":[{"kind":"cousin","from":"a","to":"a"}]}"#;
    let r = parse_ai_reply(t);
    assert_eq!(r.people.len(), 1);
    assert_eq!(r.people[0].id, "a");
    assert_eq!(r.skipped, 5 + 1);
}

#[test]
fn keeps_complete_entries_of_a_truncated_answer() {
    let t = r#"{"people":[
      {"id":"a","given":"Ann","box":{"x":0.1,"y":0.1,"w":0.1,"h":0.1}},
      {"id":"b","given":"Bo","box":{"x":0.2,"y":0.1,"w":0.1,"h":0.1}},
      {"id":"c","given":"Cy","box":{"x":0.3,"y"#;
    let r = parse_ai_reply(t);
    assert_eq!(r.people.len(), 2);
    assert!(r.truncated);
}

#[test]
fn caps_the_number_of_people() {
    let one = |i: usize| {
        format!(r#"{{"id":"p{i}","given":"G{i}","box":{{"x":0.1,"y":0.1,"w":0.1,"h":0.1}}}}"#)
    };
    let all: Vec<String> = (0..520).map(one).collect();
    let t = format!(r#"{{"people":[{}],"relations":[]}}"#, all.join(","));
    let r = parse_ai_reply(&t);
    assert_eq!(r.people.len(), 500);
    assert_eq!(r.skipped, 20);
}

#[test]
fn wrong_types_and_garbage_give_an_empty_result() {
    for t in [
        "",
        "I cannot read this picture.",
        r#"{"people":"none"}"#,
        r#"{"people":{"id":"a"}}"#,
        "[1,2,3]",
    ] {
        let r = parse_ai_reply(t);
        assert!(r.people.is_empty(), "{t}");
        assert!(r.relations.is_empty());
    }
}

#[test]
fn box_values_are_clamped_into_the_picture() {
    let t = r#"{"people":[{"id":"a","given":"Ann","box":{"x":0.9,"y":-1,"w":0.5,"h":2},"confidence":7}]}"#;
    let r = parse_ai_reply(t);
    let p = &r.people[0];
    assert!(p.x + p.w <= 1.0 + 1e-9 && p.y + p.h <= 1.0 + 1e-9 && p.y == 0.0);
    assert_eq!(p.confidence, 1.0);
}
