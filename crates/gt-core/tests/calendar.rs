use gt_core::date::calendar::Calendar::*;
use gt_core::date::calendar::{days_in_month, first_jdn, last_jdn, months_in_year};
use proptest::prelude::*;

#[test]
fn known_values() {
    assert_eq!(first_jdn(Gregorian, 2000, Some(1), Some(1)), Some(2451545));
    assert_eq!(
        first_jdn(Gregorian, 1582, Some(10), Some(15)),
        Some(2299161)
    );
    assert_eq!(first_jdn(Julian, 1582, Some(10), Some(4)), Some(2299160));
    assert_eq!(first_jdn(Julian, 2000, Some(1), Some(1)), Some(2451558));
    assert_eq!(first_jdn(Hebrew, 5760, Some(1), Some(1)), Some(2451433));
    assert_eq!(first_jdn(Hebrew, 5784, Some(8), Some(15)), Some(2460424));
    assert_eq!(
        first_jdn(FrenchRepublican, 1, Some(1), Some(1)),
        Some(2375840)
    );
    assert_eq!(
        first_jdn(FrenchRepublican, 4, Some(1), Some(1)),
        Some(2376936)
    );
}

#[test]
fn partial_dates() {
    assert_eq!(
        last_jdn(Gregorian, 1900, Some(2), None),
        first_jdn(Gregorian, 1900, Some(2), Some(28))
    );
    assert_eq!(
        last_jdn(Gregorian, 1900, None, None),
        first_jdn(Gregorian, 1900, Some(12), Some(31))
    );
    assert_eq!(
        first_jdn(Gregorian, 1900, None, None),
        first_jdn(Gregorian, 1900, Some(1), Some(1))
    );
}

#[test]
fn invalid_dates() {
    assert_eq!(first_jdn(Gregorian, 1900, Some(2), Some(29)), None);
    assert_eq!(first_jdn(Gregorian, 1900, Some(13), None), None);
    assert_eq!(first_jdn(Gregorian, 1900, Some(0), None), None);
    assert_eq!(first_jdn(Gregorian, 1900, Some(1), Some(0)), None);
    assert_eq!(first_jdn(Hebrew, 5783, Some(6), Some(1)), None);
    assert_eq!(first_jdn(Hebrew, 0, Some(1), Some(1)), None);
    assert_eq!(first_jdn(FrenchRepublican, 1, Some(13), Some(6)), None);
    assert_eq!(first_jdn(Gregorian, i32::MAX, None, None), None);
}

#[test]
fn month_counts() {
    assert_eq!(months_in_year(Hebrew, 5784), 13);
    assert_eq!(months_in_year(Hebrew, 5783), 12);
    assert_eq!(months_in_year(Gregorian, 2000), 12);
    assert_eq!(months_in_year(FrenchRepublican, 3), 13);
    assert_eq!(days_in_month(FrenchRepublican, 3, 13), Some(6));
    assert_eq!(days_in_month(FrenchRepublican, 4, 13), Some(5));
    assert_eq!(days_in_month(FrenchRepublican, 16, 13), Some(6));
    assert_eq!(days_in_month(FrenchRepublican, 15, 13), Some(5));
    assert_eq!(days_in_month(Hebrew, 5784, 6), Some(30));
    assert_eq!(days_in_month(Hebrew, 5784, 7), Some(29));
    assert_eq!(days_in_month(Julian, 1900, 2), Some(29));
    assert_eq!(days_in_month(Gregorian, 2000, 2), Some(29));
}

#[test]
fn hebrew_year_lengths_are_valid() {
    for y in 1..7000 {
        let a = first_jdn(Hebrew, y, None, None).unwrap_or(0);
        let b = first_jdn(Hebrew, y + 1, None, None).unwrap_or(0);
        let len = b - a;
        assert!(
            [353, 354, 355, 383, 384, 385].contains(&len),
            "year {y} len {len}"
        );
        assert_eq!(last_jdn(Hebrew, y, None, None), Some(b - 1));
    }
}

#[test]
fn french_years_are_contiguous() {
    for y in 1..300 {
        let b = first_jdn(FrenchRepublican, y + 1, None, None);
        assert_eq!(last_jdn(FrenchRepublican, y, None, None), b.map(|v| v - 1));
    }
}

proptest! {
    #[test]
    fn gregorian_roundtrip(y in -4000i32..=4000, m in 1u8..=12, d in 1u8..=31) {
        if let Some(dim) = days_in_month(Gregorian, y, m) {
            prop_assume!(d <= dim);
            let j = first_jdn(Gregorian, y, Some(m), Some(d));
            prop_assert!(j.is_some());
            let next = if d < dim {
                first_jdn(Gregorian, y, Some(m), Some(d + 1))
            } else if m < 12 {
                first_jdn(Gregorian, y, Some(m + 1), Some(1))
            } else {
                first_jdn(Gregorian, y + 1, Some(1), Some(1))
            };
            prop_assert_eq!(j.map(|v| v + 1), next);
        }
    }
}

#[test]
#[allow(clippy::unwrap_used)]
fn gregorian_ymd_and_minus_years() {
    use gt_core::date::calendar::{gregorian_minus_years, gregorian_ymd};
    assert_eq!(gregorian_ymd(2451545), (2000, 1, 1));
    assert_eq!(gregorian_ymd(2299161), (1582, 10, 15));
    let today = first_jdn(Gregorian, 2026, Some(11), Some(3)).unwrap();
    assert_eq!(
        gregorian_minus_years(today, 100),
        first_jdn(Gregorian, 1926, Some(11), Some(3)).unwrap()
    );
    let leap = first_jdn(Gregorian, 2024, Some(2), Some(29)).unwrap();
    assert_eq!(
        gregorian_minus_years(leap, 1),
        first_jdn(Gregorian, 2023, Some(2), Some(28)).unwrap()
    );
}
