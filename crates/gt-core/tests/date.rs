use gt_core::date::calendar::{Calendar, first_jdn};
use gt_core::date::{Date, DateValue, DayOrder, GedcomVersion, GenDate};
use proptest::prelude::*;

fn d(cal: Calendar, y: i32, m: Option<u8>, day: Option<u8>) -> Date {
    Date {
        calendar: cal,
        year: y,
        bce: false,
        month: m,
        day,
        dual_year: None,
    }
}

fn g(y: i32, m: Option<u8>, day: Option<u8>) -> Date {
    d(Calendar::Gregorian, y, m, day)
}

fn jdn(y: i32, m: u8, day: u8) -> Option<i32> {
    first_jdn(Calendar::Gregorian, y, Some(m), Some(day))
}

type Row = (
    &'static str,
    DayOrder,
    DateValue,
    (Option<i32>, Option<i32>),
);

fn rows() -> Vec<Row> {
    use DateValue::*;
    let dmy = DayOrder::Dmy;
    let mut v: Vec<Row> = vec![
        (
            "12 MAR 1901",
            dmy,
            Exact(g(1901, Some(3), Some(12))),
            (jdn(1901, 3, 12), jdn(1901, 3, 12)),
        ),
        (
            "Bef. 1900",
            dmy,
            Before(g(1900, None, None)),
            (None, jdn(1900, 12, 31)),
        ),
        (
            "aft 1900",
            dmy,
            After(g(1900, None, None)),
            (jdn(1900, 1, 1), None),
        ),
        (
            "between 1900 and 1910",
            dmy,
            Between(g(1900, None, None), g(1910, None, None)),
            (jdn(1900, 1, 1), jdn(1910, 12, 31)),
        ),
        (
            "BET 1900 AND 1910",
            dmy,
            Between(g(1900, None, None), g(1910, None, None)),
            (jdn(1900, 1, 1), jdn(1910, 12, 31)),
        ),
        (
            "1900-1910",
            dmy,
            Between(g(1900, None, None), g(1910, None, None)),
            (jdn(1900, 1, 1), jdn(1910, 12, 31)),
        ),
        (
            "FROM 1900 TO 1910",
            dmy,
            FromTo(g(1900, None, None), g(1910, None, None)),
            (jdn(1900, 1, 1), jdn(1910, 12, 31)),
        ),
        (
            "FROM 1900",
            dmy,
            From(g(1900, None, None)),
            (jdn(1900, 1, 1), None),
        ),
        (
            "TO 1910",
            dmy,
            To(g(1910, None, None)),
            (None, jdn(1910, 12, 31)),
        ),
        (
            "12/03/1901",
            DayOrder::Dmy,
            Exact(g(1901, Some(3), Some(12))),
            (jdn(1901, 3, 12), jdn(1901, 3, 12)),
        ),
        (
            "12/03/1901",
            DayOrder::Mdy,
            Exact(g(1901, Some(12), Some(3))),
            (jdn(1901, 12, 3), jdn(1901, 12, 3)),
        ),
        (
            "1901-03-12",
            dmy,
            Exact(g(1901, Some(3), Some(12))),
            (jdn(1901, 3, 12), jdn(1901, 3, 12)),
        ),
        (
            "MAR 1901",
            dmy,
            Exact(g(1901, Some(3), None)),
            (jdn(1901, 3, 1), jdn(1901, 3, 31)),
        ),
        (
            "CAL 1900",
            dmy,
            Calculated(g(1900, None, None)),
            (jdn(1895, 1, 1), jdn(1905, 12, 31)),
        ),
        (
            "EST 1900",
            dmy,
            Estimated(g(1900, None, None)),
            (jdn(1895, 1, 1), jdn(1905, 12, 31)),
        ),
        (
            "@#DJULIAN@ 4 OCT 1582",
            dmy,
            Exact(d(Calendar::Julian, 1582, Some(10), Some(4))),
            (Some(2299160), Some(2299160)),
        ),
        (
            "JULIAN 4 OCT 1582",
            dmy,
            Exact(d(Calendar::Julian, 1582, Some(10), Some(4))),
            (Some(2299160), Some(2299160)),
        ),
        (
            "@#DFRENCH R@ 1 VEND 1",
            dmy,
            Exact(d(Calendar::FrenchRepublican, 1, Some(1), Some(1))),
            (Some(2375840), Some(2375840)),
        ),
        (
            "@#DHEBREW@ 15 NSN 5784",
            dmy,
            Exact(d(Calendar::Hebrew, 5784, Some(8), Some(15))),
            (Some(2460424), Some(2460424)),
        ),
        (
            "INT 1900 (about then)",
            dmy,
            Interpreted(g(1900, None, None)),
            (jdn(1900, 1, 1), jdn(1900, 12, 31)),
        ),
        ("(during the war)", dmy, PhraseOnly, (None, None)),
        ("garbage text", dmy, Unparsed, (None, None)),
        ("30 FEB 1900", dmy, Unparsed, (None, None)),
        ("", dmy, Unparsed, (None, None)),
    ];
    for s in [
        "abt 1850",
        "c. 1850",
        "circa 1850",
        "~1850",
        "1850?",
        "ABT 1850",
        "ca. 1850",
    ] {
        v.push((
            s,
            dmy,
            About(g(1850, None, None)),
            (jdn(1845, 1, 1), jdn(1855, 12, 31)),
        ));
    }
    v
}

#[test]
fn table() {
    for (input, order, value, (min, max)) in rows() {
        let p = GenDate::parse(input, order);
        assert_eq!(p.raw, input, "raw preserved for {input:?}");
        assert_eq!(p.value, value, "value for {input:?}");
        assert_eq!(p.sort_min, min, "sort_min for {input:?}");
        assert_eq!(p.sort_max, max, "sort_max for {input:?}");
    }
}

#[test]
fn adar_in_non_leap_year() {
    let p = GenDate::parse("1 ADR 5783", DayOrder::Dmy);
    assert!(matches!(p.value, DateValue::Exact(_)), "{:?}", p.value);
    assert_eq!(
        p.sort_min,
        first_jdn(Calendar::Gregorian, 2023, Some(2), Some(22))
    );
    let p = GenDate::parse("1 ADS 5783", DayOrder::Dmy);
    assert_eq!(p.value, DateValue::Unparsed);
}

#[test]
fn phrases() {
    let p = GenDate::parse("INT 1900 (about then)", DayOrder::Dmy);
    assert_eq!(p.phrase.as_deref(), Some("about then"));
    let p = GenDate::parse("(during the war)", DayOrder::Dmy);
    assert_eq!(p.phrase.as_deref(), Some("during the war"));
}

#[test]
fn dual_year() {
    let p = GenDate::parse("1 MAR 1731/32", DayOrder::Dmy);
    let DateValue::Exact(date) = &p.value else {
        panic!("not exact: {:?}", p.value)
    };
    assert_eq!(date.year, 1731);
    assert_eq!(date.dual_year, Some(1732));
    assert_eq!(p.sort_min, jdn(1732, 3, 1));
    assert_eq!(
        p.to_gedcom(GedcomVersion::V70),
        ("1 MAR 1732".to_string(), Some("1 MAR 1731/32".to_string()))
    );
    assert_eq!(p.to_gedcom(GedcomVersion::V551).0, "1 MAR 1731/32");
}

#[test]
fn bce() {
    for s in ["44 B.C.", "44 BCE", "44 bc"] {
        let p = GenDate::parse(s, DayOrder::Dmy);
        let DateValue::Exact(date) = &p.value else {
            panic!("not exact: {s}")
        };
        assert!(date.bce);
        assert_eq!(date.year, 44);
        assert_eq!(p.sort_min, first_jdn(Calendar::Gregorian, -43, None, None));
    }
    let p = GenDate::parse("44 BCE", DayOrder::Dmy);
    assert_eq!(p.to_gedcom(GedcomVersion::V551).0, "44 B.C.");
    assert_eq!(p.to_gedcom(GedcomVersion::V70).0, "44 BCE");
}

#[test]
fn gedcom_round_trip() {
    for (input, order, value, _) in rows() {
        let p = GenDate::parse(input, order);
        let (text, _) = p.to_gedcom(GedcomVersion::V551);
        let again = GenDate::parse(&text, DayOrder::Dmy);
        assert_eq!(again.value, value, "{input:?} -> {text:?}");
    }
}

#[test]
fn v70_output() {
    let p = GenDate::parse("@#DJULIAN@ 4 OCT 1582", DayOrder::Dmy);
    assert_eq!(
        p.to_gedcom(GedcomVersion::V70),
        ("JULIAN 4 OCT 1582".into(), None)
    );
    assert_eq!(
        p.to_gedcom(GedcomVersion::V551),
        ("@#DJULIAN@ 4 OCT 1582".into(), None)
    );
    let p = GenDate::parse("INT 1900 (about then)", DayOrder::Dmy);
    assert_eq!(
        p.to_gedcom(GedcomVersion::V70),
        ("1900".into(), Some("about then".into()))
    );
    assert_eq!(p.to_gedcom(GedcomVersion::V551).0, "INT 1900 (about then)");
    let p = GenDate::parse("BET 1900 AND 1910", DayOrder::Dmy);
    assert_eq!(p.to_gedcom(GedcomVersion::V70).0, "BET 1900 AND 1910");
}

#[test]
fn display_and_sort_key() {
    let p = GenDate::parse("abt 1850", DayOrder::Dmy);
    assert_eq!(p.display(), "about 1850");
    assert_eq!(p.sort_key(), jdn(1845, 1, 1));
    assert_eq!(
        GenDate::parse("12 MAR 1901", DayOrder::Dmy).display(),
        "12 Mar 1901"
    );
    let b = GenDate::parse("BEF 1900", DayOrder::Dmy);
    assert_eq!(b.sort_key(), jdn(1900, 12, 31));
    assert_eq!(GenDate::parse("junk", DayOrder::Dmy).sort_key(), None);
}

proptest! {
    #[test]
    fn parse_never_panics(s in any::<String>()) {
        let p = GenDate::parse(&s, DayOrder::Dmy);
        let _ = p.display();
        let _ = p.to_gedcom(GedcomVersion::V551);
        let _ = p.to_gedcom(GedcomVersion::V70);
    }

    #[test]
    fn parse_never_panics_dateish(s in "[A-Za-z0-9 /@#_().~?,-]{0,40}") {
        let p = GenDate::parse(&s, DayOrder::Mdy);
        let _ = p.to_gedcom(GedcomVersion::V70);
    }
}
