//! Old (pre-1918) Cyrillic spelling, for search: a name written «Ѳеодоръ Василіевъ» in a metric
//! book is found by typing «Феодор Василиев» or «Феодор Василіев». Only letters are mapped, there
//! is no dictionary of names: «Іоаннъ» is found by «Иоанн», not by «Іван».

/// Letters that occur only in the old spelling. `і` is not among them: it is a letter of
/// Ukrainian today.
fn is_old_letter(c: char) -> bool {
    matches!(
        c,
        'ѣ' | 'Ѣ'
            | 'ѳ'
            | 'Ѳ'
            | 'ѵ'
            | 'Ѵ'
            | 'ѷ'
            | 'Ѷ'
            | 'ѡ'
            | 'Ѡ'
            | 'ѯ'
            | 'Ѯ'
            | 'ѱ'
            | 'Ѱ'
            | 'ѕ'
            | 'Ѕ'
            | 'ѧ'
            | 'Ѧ'
    )
}

fn is_hard_sign(c: char) -> bool {
    c == 'ъ' || c == 'Ъ'
}

/// True when the text has an old letter, or a word ending in the hard sign («Петровъ»).
pub fn has_old_spelling(s: &str) -> bool {
    let chars: Vec<char> = s.chars().collect();
    chars.iter().enumerate().any(|(i, &c)| {
        is_old_letter(c)
            || (is_hard_sign(c)
                && i > 0
                && chars[i - 1].is_alphabetic()
                && chars.get(i + 1).is_none_or(|n| !n.is_alphabetic()))
    })
}

/// How `ѣ`, `і` and `ѵ` read today: Ukrainian (ѣ, і, ѵ -> і), Russian (ѣ -> е; і, ѵ -> и), and
/// mixed (ѣ -> е; і, ѵ -> і), the way such names are often written now.
const READINGS: [(char, char, char); 3] = [('і', 'і', 'і'), ('е', 'и', 'и'), ('е', 'і', 'і')];

/// The text in today's letters, lower case, in each common reading; empty when the text has no
/// old spelling. A word-final hard sign is dropped; one inside a word stays.
pub fn modern_variants(s: &str) -> Vec<String> {
    if !has_old_spelling(s) {
        return Vec::new();
    }
    let lower: Vec<char> = s.chars().flat_map(char::to_lowercase).collect();
    let mut out: Vec<String> = Vec::new();
    for (yat, i, izhitsa) in READINGS {
        let mut v = String::with_capacity(s.len());
        for (k, &c) in lower.iter().enumerate() {
            match c {
                'ѣ' => v.push(yat),
                'і' => v.push(i),
                'ѵ' | 'ѷ' => v.push(izhitsa),
                'ѳ' => v.push('ф'),
                'ѡ' => v.push('о'),
                'ѯ' => v.push_str("кс"),
                'ѱ' => v.push_str("пс"),
                'ѕ' => v.push('з'),
                'ѧ' => v.push('я'),
                'ъ' if k > 0
                    && lower[k - 1].is_alphabetic()
                    && lower.get(k + 1).is_none_or(|n| !n.is_alphabetic()) => {}
                c => v.push(c),
            }
        }
        if !out.contains(&v) {
            out.push(v);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modern_names_have_no_variants() {
        assert!(modern_variants("Іван Петрович Коваль").is_empty());
        assert!(modern_variants("Марія").is_empty());
        assert!(modern_variants("Anna Smith").is_empty());
        assert!(modern_variants("під'їзд").is_empty());
    }

    #[test]
    fn old_letters_read_in_todays_letters() {
        let v = modern_variants("Ѳеодоръ Петровъ Вигаданенко");
        assert_eq!(v, vec!["феодор петров вигаданенко".to_string()]);
        let v = modern_variants("Соломонида Моисеева Вѣрная");
        assert!(v.contains(&"соломонида моисеева вірная".to_string()));
        assert!(v.contains(&"соломонида моисеева верная".to_string()));
        let v = modern_variants("Іоаннъ Василіевъ");
        assert!(v.contains(&"іоанн василіев".to_string()));
        assert!(v.contains(&"иоанн василиев".to_string()));
        assert_eq!(modern_variants("Сѵмеонъ")[1], "симеон");
    }

    #[test]
    fn a_hard_sign_inside_a_word_stays() {
        assert_eq!(modern_variants("объявленъ"), vec!["объявлен".to_string()]);
    }
}
