//! One line of a GeoNames country dump (`<CC>.txt`, tab separated).

/// A place of the dump that the app keeps: a populated place (class P) or an administrative division (class A).
#[derive(Debug, Clone, PartialEq)]
pub struct GeoRow {
    pub id: u32,
    pub name: String,
    pub ascii: String,
    pub alternates: Vec<String>,
    pub class: u8,
    pub code: String,
    pub admin1: String,
    pub admin2: String,
    pub population: u32,
    pub lat: f64,
    pub lon: f64,
}

/// The feature codes of class A that are kept (the divisions people write in a place text).
const KEPT_ADMIN: &[&str] = &["ADM1", "ADM2", "ADM3", "ADM1H", "ADM2H", "ADM3H"];

/// The row for one line, or `None` for a line that is not a populated place or a kept division, or is malformed.
pub fn parse_line(line: &str) -> Option<GeoRow> {
    let f: Vec<&str> = line.split('\t').collect();
    if f.len() < 15 {
        return None;
    }
    let class = *f.get(6)?.as_bytes().first()?;
    let code = *f.get(7)?;
    match class {
        b'P' => {}
        b'A' if KEPT_ADMIN.contains(&code) => {}
        _ => return None,
    }
    let lat: f64 = f.get(4)?.parse().ok()?;
    let lon: f64 = f.get(5)?.parse().ok()?;
    if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
        return None;
    }
    let name = f.get(1)?.trim().to_string();
    if name.is_empty() {
        return None;
    }
    Some(GeoRow {
        id: f.first()?.parse().ok()?,
        ascii: f.get(2).map(|s| s.trim().to_string()).unwrap_or_default(),
        alternates: f
            .get(3)
            .map(|s| {
                s.split(',')
                    .map(str::trim)
                    .filter(|a| !a.is_empty())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default(),
        name,
        class,
        code: code.to_string(),
        admin1: f.get(10).map(|s| s.to_string()).unwrap_or_default(),
        admin2: f.get(11).map(|s| s.to_string()).unwrap_or_default(),
        population: f.get(14).and_then(|s| s.parse().ok()).unwrap_or(0),
        lat,
        lon,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(class: &str, code: &str) -> String {
        format!(
            "9001\tIvanivka\tIvanivka\tІванівка,Ivanovka,,Иванивка\t50.1234\t30.5678\t{class}\t{code}\tUA\t\t32\t3210\t\t\t415\t\t170\tEurope/Kyiv\t2020-01-01"
        )
    }

    #[test]
    fn parses_a_village() {
        let r = parse_line(&line("P", "PPL")).expect("row");
        assert_eq!(r.id, 9001);
        assert_eq!(r.name, "Ivanivka");
        assert_eq!(r.alternates, vec!["Іванівка", "Ivanovka", "Иванивка"]);
        assert_eq!((r.admin1.as_str(), r.admin2.as_str()), ("32", "3210"));
        assert_eq!(r.population, 415);
        assert!((r.lat - 50.1234).abs() < 1e-9 && (r.lon - 30.5678).abs() < 1e-9);
    }

    #[test]
    fn keeps_divisions_drops_the_rest() {
        assert!(parse_line(&line("A", "ADM2")).is_some());
        assert!(parse_line(&line("A", "PCLI")).is_none());
        assert!(parse_line(&line("H", "STM")).is_none());
        assert!(parse_line(&line("T", "MT")).is_none());
        assert!(parse_line("short\tline").is_none());
        assert!(parse_line("").is_none());
    }

    #[test]
    fn rejects_bad_coordinates() {
        let bad = line("P", "PPL").replace("50.1234", "950.1");
        assert!(parse_line(&bad).is_none());
    }
}
