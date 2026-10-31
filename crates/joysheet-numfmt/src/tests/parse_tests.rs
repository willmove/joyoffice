use super::*;

#[test]
fn sections_split() {
    assert_eq!(split_sections("0;-0;;@").len(), 4);
    assert_eq!(split_sections("\"a;b\"0").len(), 1);
    assert_eq!(split_sections("\\;0").len(), 1);
    assert_eq!(split_sections("0;0;0;0;0;0").len(), 4);
}

#[test]
fn minutes_vs_months() {
    let s = parse_sections("h:mm");
    assert!(s[0].toks.contains(&Tok::Date(DatePart::Minute(2))));
    let s = parse_sections("m/d");
    assert!(s[0].toks.contains(&Tok::Date(DatePart::Month(1))));
    let s = parse_sections("mm:ss");
    assert!(s[0].toks.contains(&Tok::Date(DatePart::Minute(2))));
}

#[test]
fn commas() {
    let s = &parse_sections("#,##0,")[0];
    assert!(s.thousands);
    assert_eq!(s.scale, 1);
    let s = &parse_sections("0.0,,")[0];
    assert!(!s.thousands);
    assert_eq!(s.scale, 2);
}
