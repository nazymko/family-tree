use gt_gedcom::ansel::decode_ansel;
use gt_gedcom::{DetectedEncoding, decode_payload, detect};

#[test]
fn utf8_bom() {
    assert_eq!(
        detect(b"\xEF\xBB\xBF0 HEAD\r\n"),
        DetectedEncoding::Utf8 { bom: true }
    );
}

#[test]
fn utf16le_by_content() {
    let bytes: Vec<u8> = "0 HEAD\r\n"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    assert_eq!(detect(&bytes), DetectedEncoding::Utf16Le);
}

#[test]
fn ansel_acute() {
    assert_eq!(decode_ansel(&[0xE2, 0x65]), "\u{e9}");
    let file = b"0 HEAD\r\n1 CHAR ANSEL\r\n0 @I1@ INDI\r\n1 NAME M\xE8uller\r\n";
    assert_eq!(detect(file), DetectedEncoding::Ansel);
    assert_eq!(
        decode_payload(b"M\xE8uller", &DetectedEncoding::Ansel),
        "M\u{fc}ller"
    );
}

#[test]
fn char_ansel_but_utf8_body() {
    let file = "0 HEAD\r\n1 CHAR ANSEL\r\n0 @I1@ INDI\r\n1 NAME Müller\r\n".as_bytes();
    assert_eq!(detect(file), DetectedEncoding::Utf8 { bom: false });
}

#[test]
fn windows1252_fallback() {
    let file = b"0 HEAD\r\n1 NAME M\xFCller\r\n";
    assert_eq!(detect(file), DetectedEncoding::Windows1252);
    assert_eq!(
        decode_payload(b"\xFC", &DetectedEncoding::Windows1252),
        "\u{fc}"
    );
}
