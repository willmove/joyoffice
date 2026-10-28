use joyword_engine::spell::OfflineSpellChecker;
use joyword_model::{Document, Paragraph, RunProps};
use std::sync::Arc;

#[test]
fn test_offline_spell_check_and_suggestions() {
    let mut checker = OfflineSpellChecker::new();

    // Valid words pass
    assert!(checker.is_word_valid("document"));
    assert!(checker.is_word_valid("paragraph"));
    assert!(checker.is_word_valid("editor"));

    // Misspelled word detected
    assert!(!checker.is_word_valid("paragraf"));

    // Suggestions include correct spelling
    let suggestions = checker.suggest("paragraf", 3);
    assert!(suggestions.contains(&"paragraph".to_string()));

    // Custom dictionary word
    assert!(!checker.is_word_valid("joyplatform"));
    checker.add_word("joyplatform");
    assert!(checker.is_word_valid("joyplatform"));

    // Session ignored word
    assert!(!checker.is_word_valid("unrecognised"));
    checker.ignore_word("unrecognised");
    assert!(checker.is_word_valid("unrecognised"));

    // Paragraph check
    let mut doc = Document::new_blank();
    let p_id = doc.ids().node();
    let mut p = Paragraph::new(p_id);
    let _ = p.push_text(
        "The paragraf has an errror in spelling.",
        None,
        Arc::new(RunProps::default()),
    );

    let issues = checker.check_paragraph(&p);
    assert_eq!(issues.len(), 2);
    assert_eq!(issues[0].word, "paragraf");
    assert!(issues[0].suggestions.contains(&"paragraph".to_string()));
    assert_eq!(issues[1].word, "errror");
    assert!(issues[1].suggestions.contains(&"error".to_string()));
}

#[test]
fn test_chinese_proofing_rules() {
    let checker = OfflineSpellChecker::new();
    let mut doc = Document::new_blank();

    // Duplicate punctuation
    let p1_id = doc.ids().node();
    let mut p1 = Paragraph::new(p1_id);
    let _ = p1.push_text(
        "公文排版规范，，禁止出现连续重复标点。。",
        None,
        Arc::new(RunProps::default()),
    );

    let issues1 = checker.check_chinese_proofing(&p1);
    assert!(issues1.len() >= 2);
    assert!(issues1.iter().any(|i| i.message.contains("，，")));
    assert!(issues1.iter().any(|i| i.message.contains("。。")));

    // Unclosed book title
    let p2_id = doc.ids().node();
    let mut p2 = Paragraph::new(p2_id);
    let _ = p2.push_text(
        "请参阅《公文格式标准 进行核验。",
        None,
        Arc::new(RunProps::default()),
    );

    let issues2 = checker.check_chinese_proofing(&p2);
    assert!(
        issues2
            .iter()
            .any(|i| i.message.contains("书名号未正确配对"))
    );
}
