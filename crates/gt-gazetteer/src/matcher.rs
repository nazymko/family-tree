//! Ranked candidates for a place typed as a hierarchy: leaf first ("Ivanivka", "Petrivske district", "Zelena oblast").

use crate::fold::{fold_name, within_one_edit};
use crate::index::{Entry, Gazetteer, Index};
use std::collections::HashSet;

/// Fuzzy matching on a name shorter than this would match too much.
const FUZZY_MIN_LEN: usize = 5;
/// The most rows scored for one query (a very common name).
const MAX_ROWS: usize = 20_000;

#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub geoname_id: u32,
    pub country: String,
    pub name: String,
    /// A Ukrainian (or other Cyrillic) name; empty when none is known.
    pub local: String,
    pub lat: f64,
    pub lon: f64,
    /// Feature code (PPL, PPLA, PPLH, ADM2, ...).
    pub code: String,
    pub population: u32,
    /// The first level division, as the dump names it (empty when unknown).
    pub admin1: String,
    pub admin1_local: String,
    pub admin2: String,
    pub admin2_local: String,
    /// 0..=200, higher is better.
    pub score: u32,
    /// True when the other parts of the typed place agree with the candidate's regions.
    pub region_match: bool,
}

fn name_score(q: &str, q_tokens: &[&str], entry: &Entry) -> u32 {
    let mut best = 0;
    for key in entry.keys() {
        let s = if key == q {
            100
        } else if within_one_edit(key, q) && q.chars().count() >= FUZZY_MIN_LEN {
            65
        } else {
            let k_tokens: Vec<&str> = key.split(' ').collect();
            if q_tokens.iter().all(|t| k_tokens.contains(t)) {
                75
            } else if k_tokens.iter().all(|t| q_tokens.contains(t)) {
                55
            } else if q_tokens.len() == k_tokens.len()
                && q_tokens.iter().zip(&k_tokens).all(|(a, b)| {
                    a == b || (a.chars().count() >= FUZZY_MIN_LEN && within_one_edit(a, b))
                })
            {
                60
            } else {
                0
            }
        };
        best = best.max(s);
    }
    best
}

fn region_keys<'a>(ix: &'a Index, e: &Entry, level: u8) -> Option<&'a Entry> {
    let row = if level == 1 {
        ix.admin1_rows.get(&e.admin1)
    } else {
        ix.admin2_rows.get(&format!("{}.{}", e.admin1, e.admin2))
    }?;
    ix.entries.get(*row as usize)
}

/// Whether a typed part names this division; fuzzy for longer words.
fn part_matches(part: &str, region: &Entry) -> bool {
    region
        .keys()
        .any(|k| k == part || (part.chars().count() >= FUZZY_MIN_LEN && within_one_edit(k, part)))
}

impl Index {
    /// Candidates for `parts` (leaf first), best first, at most `limit`.
    pub fn search(&self, parts: &[&str], limit: usize) -> Vec<Candidate> {
        let Some(leaf) = parts.first() else {
            return Vec::new();
        };
        let q = fold_name(leaf);
        if q.is_empty() {
            return Vec::new();
        }
        let q_tokens: Vec<&str> = q.split(' ').collect();
        let hints: Vec<String> = parts
            .iter()
            .skip(1)
            .map(|p| fold_name(p))
            .filter(|p| !p.is_empty())
            .collect();
        // Rows that share the longest word of the name, exactly or one letter off.
        let Some(word) = q_tokens.iter().max_by_key(|t| t.chars().count()) else {
            return Vec::new();
        };
        let mut token_ids: Vec<u32> = Vec::new();
        if let Some(id) = self.lookup.get(*word) {
            token_ids.push(*id);
        }
        let n = word.chars().count();
        if n >= FUZZY_MIN_LEN
            && let Some(c) = word.chars().next()
        {
            for len in [n - 1, n, n + 1] {
                for id in self.shape.get(&(c, len)).into_iter().flatten() {
                    let Some((tok, _)) = self.vocab.get(*id as usize) else {
                        continue;
                    };
                    if &**tok != *word && within_one_edit(tok, word) {
                        token_ids.push(*id);
                    }
                }
            }
        }
        let mut seen: HashSet<u32> = HashSet::new();
        let mut rows: Vec<u32> = Vec::new();
        'outer: for id in token_ids {
            for r in self
                .vocab
                .get(id as usize)
                .map(|v| v.1.as_slice())
                .unwrap_or_default()
            {
                if seen.insert(*r) {
                    rows.push(*r);
                    if rows.len() >= MAX_ROWS {
                        break 'outer;
                    }
                }
            }
        }
        let mut out: Vec<Candidate> = Vec::new();
        for r in rows {
            let Some(e) = self.entries.get(r as usize) else {
                continue;
            };
            let base = name_score(&q, &q_tokens, e);
            if base == 0 {
                continue;
            }
            let r1 = region_keys(self, e, 1);
            let r2 = region_keys(self, e, 2);
            let mut score = base;
            let mut region_match = false;
            if hints.iter().any(|h| r1.is_some_and(|r| part_matches(h, r))) {
                score += 30;
                region_match = true;
            }
            if hints.iter().any(|h| r2.is_some_and(|r| part_matches(h, r))) {
                score += 20;
                region_match = true;
            }
            score += (((f64::from(e.population) + 1.0).log10() * 2.0) as u32).min(14);
            if e.class == b'P' {
                score += 2;
            }
            out.push(Candidate {
                geoname_id: e.id,
                country: self.country.clone(),
                name: e.name.clone(),
                local: e.local.clone(),
                lat: e.lat,
                lon: e.lon,
                code: e.code.clone(),
                population: e.population,
                admin1: r1.map(|r| r.name.clone()).unwrap_or_default(),
                admin1_local: r1.map(|r| r.local.clone()).unwrap_or_default(),
                admin2: r2.map(|r| r.name.clone()).unwrap_or_default(),
                admin2_local: r2.map(|r| r.local.clone()).unwrap_or_default(),
                score,
                region_match,
            });
        }
        out.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then(b.population.cmp(&a.population))
                .then(a.geoname_id.cmp(&b.geoname_id))
        });
        out.truncate(limit);
        out
    }
}

impl Gazetteer {
    /// The best candidates over every loaded country.
    pub fn search(&self, parts: &[&str], limit: usize) -> Vec<Candidate> {
        let mut all: Vec<Candidate> = self
            .indexes
            .iter()
            .flat_map(|i| i.search(parts, limit))
            .collect();
        all.sort_by(|a, b| b.score.cmp(&a.score).then(b.population.cmp(&a.population)));
        all.truncate(limit);
        all
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::too_many_arguments)]
    fn row(
        id: u32,
        name: &str,
        alts: &str,
        class: &str,
        code: &str,
        a1: &str,
        a2: &str,
        pop: u32,
    ) -> String {
        format!(
            "{id}\t{name}\t{name}\t{alts}\t49.5\t30.5\t{class}\t{code}\tXX\t\t{a1}\t{a2}\t\t\t{pop}\t\t0\tEurope/Kyiv\t2020-01-01"
        )
    }

    /// Two villages with the same name in different districts, a town, a vanished village with an old name.
    fn sample() -> Index {
        let text = [
            row(
                1,
                "Zelena Oblast",
                "Зелена область,Zelena obl",
                "A",
                "ADM1",
                "10",
                "",
                0,
            ),
            row(2, "Synia Oblast", "Синя область", "A", "ADM1", "20", "", 0),
            row(
                3,
                "Petrivske Raion",
                "Петрівський район",
                "A",
                "ADM2",
                "10",
                "1001",
                0,
            ),
            row(
                4,
                "Horishnie Raion",
                "Горішній район",
                "A",
                "ADM2",
                "20",
                "2001",
                0,
            ),
            row(
                10,
                "Ivanivka",
                "Іванівка,Ivanovka",
                "P",
                "PPL",
                "10",
                "1001",
                300,
            ),
            row(11, "Ivanivka", "Іванівка", "P", "PPL", "20", "2001", 900),
            row(
                12,
                "Bila Tserkva",
                "Біла Церква,Belaya Tserkov",
                "P",
                "PPLA",
                "10",
                "1001",
                200_000,
            ),
            row(
                13,
                "Kamianka",
                "Кам'янка,Novoselytsia",
                "P",
                "PPLH",
                "10",
                "1001",
                0,
            ),
            row(14, "Petrivka", "Петрівка", "P", "PPL", "20", "2001", 50),
        ]
        .join("\n");
        Index::from_reader("xx", text.as_bytes()).expect("index")
    }

    #[test]
    fn region_hint_picks_the_right_village() {
        let ix = sample();
        let c = ix.search(&["Іванівка", "Петрівський район", "Зелена область"], 5);
        assert_eq!(c[0].geoname_id, 10);
        assert!(c[0].region_match);
        assert_eq!(c[0].admin1, "Zelena Oblast");
        assert_eq!(c[1].geoname_id, 11);
        let c = ix.search(&["Ivanivka", "Synia oblast"], 5);
        assert_eq!(c[0].geoname_id, 11);
    }

    #[test]
    fn without_hints_the_bigger_place_wins() {
        let ix = sample();
        let c = ix.search(&["Ivanivka"], 5);
        assert_eq!(
            c.iter().map(|c| c.geoname_id).collect::<Vec<_>>(),
            vec![11, 10]
        );
    }

    #[test]
    fn spelling_variants_and_old_names() {
        let ix = sample();
        assert_eq!(
            ix.search(&["Iwaniwka"], 3).first().map(|c| c.geoname_id),
            Some(11)
        );
        assert_eq!(ix.search(&["Ivanovka"], 3).len(), 2);
        assert_eq!(ix.search(&["Novoselytsia"], 3)[0].geoname_id, 13);
        assert_eq!(
            ix.search(&["Белая Церковь"], 3)
                .first()
                .map(|c| c.geoname_id),
            Some(12)
        );
        assert_eq!(ix.search(&["Belaya Tserkov"], 3)[0].geoname_id, 12);
        assert_eq!(ix.search(&["Bila Tserkva"], 3)[0].geoname_id, 12);
        // one letter off in a long name, and the word order of a two-word name does not need to match exactly
        assert_eq!(ix.search(&["Ivanivca"], 3).len(), 2);
        assert!(ix.search(&["Qwertyuiop"], 3).is_empty());
        assert!(ix.search(&[], 3).is_empty());
        assert!(ix.search(&["  "], 3).is_empty());
    }

    #[test]
    fn divisions_are_found_too() {
        let ix = sample();
        let c = ix.search(&["Петрівський район"], 3);
        assert_eq!(c[0].geoname_id, 3);
        assert_eq!(c[0].code, "ADM2");
    }

    #[test]
    fn gazetteer_merges_countries() {
        let g = Gazetteer {
            indexes: vec![sample()],
        };
        assert_eq!(g.search(&["Petrivka"], 2).len(), 1);
        assert!(Gazetteer::default().search(&["Petrivka"], 2).is_empty());
    }

    /// Release-only budget: about 100k rows load in under a second and a search takes under 50 ms.
    #[test]
    #[ignore = "release-only budget check"]
    fn budget_100k_rows() {
        let mut lines = Vec::new();
        lines.push(row(
            1,
            "Zelena Oblast",
            "Зелена область",
            "A",
            "ADM1",
            "10",
            "",
            0,
        ));
        for i in 0..100_000u32 {
            let name = format!("Village{}x{}", i % 977, i / 977);
            lines.push(row(
                100 + i,
                &name,
                &format!("Село{i},Selo{i},Old{i}"),
                "P",
                "PPL",
                "10",
                "1001",
                i % 500,
            ));
        }
        lines.push(row(5, "Ivanivka", "Іванівка", "P", "PPL", "10", "1001", 10));
        let ix = Index::from_reader("xx", lines.join("\n").as_bytes()).expect("index");
        let bytes = ix.to_bytes();
        let t = std::time::Instant::now();
        let ix = Index::from_bytes(&bytes).expect("load");
        let load = t.elapsed();
        let t = std::time::Instant::now();
        let hits = ix.search(&["Ivanivka", "Zelena oblast"], 8);
        let search = t.elapsed();
        let t2 = std::time::Instant::now();
        let _ = ix.search(&["Village12x34", "Zelena oblast"], 8);
        let fuzzy = t2.elapsed();
        eprintln!(
            "load {load:?} search {search:?} fuzzy {fuzzy:?} bytes {}",
            bytes.len()
        );
        assert_eq!(hits[0].geoname_id, 5);
        assert!(load.as_millis() < 1000, "load {load:?}");
        assert!(
            search.as_millis() < 50 && fuzzy.as_millis() < 50,
            "{search:?} {fuzzy:?}"
        );
    }
}
