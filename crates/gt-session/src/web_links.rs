//! Web addresses written in a media record or in a source that holds it, so the picture viewer can
//! offer to open the page a scan came from (a FamilySearch record, an archive catalogue entry).

/// Most addresses offered for one picture.
pub(crate) const MAX_LINKS: usize = 3;

/// Every `http://` or `https://` address in `text`, in order. An address ends at whitespace or an
/// angle bracket; punctuation that usually belongs to the sentence (`.,;:)»"'`) is cut off its end.
pub(crate) fn urls_in(text: &str) -> Vec<String> {
    let lower = text.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut from = 0;
    while from < text.len() {
        let rest = &lower[from..];
        let (at, scheme) = match (rest.find("https://"), rest.find("http://")) {
            (Some(a), Some(b)) if b < a => (b, 7),
            (Some(a), _) => (a, 8),
            (None, Some(b)) => (b, 7),
            (None, None) => break,
        };
        let start = from + at;
        let tail = &text[start..];
        let len = tail
            .find(|c: char| c.is_whitespace() || c == '<' || c == '>')
            .unwrap_or(tail.len());
        let url = tail[..len].trim_end_matches(['.', ',', ';', ':', ')', '»', '"', '\'']);
        if url.len() > scheme && !url[scheme..].starts_with('/') {
            out.push(url.to_string());
        }
        from = start + len.max(scheme);
    }
    out
}

/// Addresses from several texts, in order, without repeats, at most [`MAX_LINKS`].
pub(crate) fn collect<'a>(texts: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for t in texts {
        for u in urls_in(t) {
            if !out.contains(&u) {
                out.push(u);
            }
        }
    }
    out.truncate(MAX_LINKS);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_address_in_a_description() {
        let got = collect([
            "Birth record, see https://www.familysearch.org/ark:/61903/1:1:AAAA-BBB1 for the page",
        ]);
        assert_eq!(
            got,
            vec!["https://www.familysearch.org/ark:/61903/1:1:AAAA-BBB1"]
        );
    }

    #[test]
    fn finds_the_address_in_a_source_publication() {
        let got = collect(["Parish book", "Online: http://archive.example.org/book/7"]);
        assert_eq!(got, vec!["http://archive.example.org/book/7"]);
    }

    #[test]
    fn keeps_order_and_drops_duplicates() {
        let got = collect([
            "https://a.example/x https://b.example/y",
            "https://a.example/x",
        ]);
        assert_eq!(got, vec!["https://a.example/x", "https://b.example/y"]);
    }

    #[test]
    fn trims_trailing_punctuation() {
        for (text, want) in [
            ("(https://a.example/p).", "https://a.example/p"),
            ("see https://a.example/p;", "https://a.example/p"),
            ("«https://a.example/p»", "https://a.example/p"),
            ("\"https://a.example/p\",", "https://a.example/p"),
            ("https://a.example/p?x=1:", "https://a.example/p?x=1"),
        ] {
            assert_eq!(urls_in(text), vec![want.to_string()], "{text}");
        }
    }

    #[test]
    fn ignores_other_schemes_and_empty_addresses() {
        assert!(
            collect([
                "ftp://a.example/x",
                "javascript:alert(1)",
                "file:///c:/x",
                "https://",
                "https:///x",
                "mailto:a@b.example"
            ])
            .is_empty()
        );
    }

    #[test]
    fn keeps_at_most_three() {
        let got = collect([
            "https://a.example/1 https://a.example/2 https://a.example/3 https://a.example/4",
        ]);
        assert_eq!(got.len(), 3);
    }

    #[test]
    fn scheme_is_case_insensitive_and_text_stays_as_written() {
        assert_eq!(
            urls_in("HTTPS://A.example/Path"),
            vec!["HTTPS://A.example/Path"]
        );
    }
}
