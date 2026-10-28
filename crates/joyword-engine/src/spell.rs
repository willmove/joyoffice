//! Offline spell checking and language proofing tools (T503, EDIT-008).

use joyword_model::{Block, Document, NodeId, Paragraph};
use std::collections::HashSet;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpellIssue {
    pub word: String,
    pub start_offset: usize,
    pub end_offset: usize,
    pub paragraph: NodeId,
    pub suggestions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofingIssue {
    pub message: String,
    pub offset: usize,
    pub length: usize,
    pub paragraph: NodeId,
}

/// Offline dictionary and spell checker with zero external network or AI dependencies.
pub struct OfflineSpellChecker {
    dictionary: HashSet<String>,
    user_dictionary: HashSet<String>,
    ignored_words: HashSet<String>,
}

impl Default for OfflineSpellChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl OfflineSpellChecker {
    /// Initialize with built-in standard vocabulary.
    pub fn new() -> Self {
        let mut dict = HashSet::new();
        // Standard core vocabulary (nouns, verbs, adjectives, function words, technical terms)
        let words = [
            "the",
            "be",
            "to",
            "of",
            "and",
            "a",
            "in",
            "that",
            "have",
            "has",
            "had",
            "having",
            "is",
            "am",
            "are",
            "was",
            "were",
            "been",
            "being",
            "do",
            "does",
            "did",
            "done",
            "i",
            "it",
            "for",
            "not",
            "on",
            "with",
            "he",
            "as",
            "you",
            "do",
            "at",
            "this",
            "but",
            "his",
            "by",
            "from",
            "they",
            "we",
            "say",
            "her",
            "she",
            "or",
            "an",
            "will",
            "my",
            "one",
            "all",
            "would",
            "there",
            "their",
            "what",
            "so",
            "up",
            "out",
            "if",
            "about",
            "who",
            "get",
            "which",
            "go",
            "me",
            "when",
            "make",
            "can",
            "like",
            "time",
            "no",
            "just",
            "him",
            "know",
            "take",
            "people",
            "into",
            "year",
            "your",
            "good",
            "some",
            "could",
            "them",
            "see",
            "other",
            "than",
            "then",
            "now",
            "look",
            "only",
            "come",
            "its",
            "over",
            "think",
            "also",
            "back",
            "after",
            "use",
            "two",
            "how",
            "our",
            "work",
            "first",
            "well",
            "way",
            "even",
            "new",
            "want",
            "because",
            "any",
            "these",
            "give",
            "day",
            "most",
            "us",
            "document",
            "office",
            "editor",
            "word",
            "paragraph",
            "style",
            "table",
            "section",
            "format",
            "layout",
            "font",
            "page",
            "text",
            "heading",
            "footnote",
            "endnote",
            "reference",
            "bookmark",
            "comment",
            "revision",
            "image",
            "shape",
            "equation",
            "formula",
            "table",
            "margin",
            "header",
            "footer",
            "template",
            "view",
            "proof",
            "check",
            "spell",
            "spelling",
            "language",
            "offline",
            "chinese",
            "english",
            "report",
            "test",
            "tests",
            "testing",
            "tested",
            "verification",
            "model",
            "engine",
            "system",
            "true",
            "false",
            "file",
            "open",
            "save",
            "close",
            "read",
            "write",
            "print",
            "screen",
            "number",
            "list",
            "bullet",
            "align",
            "left",
            "right",
            "center",
            "justify",
            "bold",
            "italic",
            "underline",
            "strike",
            "color",
            "size",
            "width",
            "height",
            "border",
            "shading",
            "cell",
            "row",
            "column",
            "data",
            "source",
            "output",
            "input",
            "result",
            "success",
            "error",
            "warning",
            "diagnosis",
            "budget",
            "resource",
            "content",
            "control",
        ];
        for w in words {
            dict.insert(w.to_ascii_lowercase());
        }

        Self {
            dictionary: dict,
            user_dictionary: HashSet::new(),
            ignored_words: HashSet::new(),
        }
    }

    /// Add a custom word to user dictionary.
    pub fn add_word(&mut self, word: &str) {
        self.user_dictionary.insert(word.to_ascii_lowercase());
    }

    /// Ignore a word for the current session.
    pub fn ignore_word(&mut self, word: &str) {
        self.ignored_words.insert(word.to_ascii_lowercase());
    }

    /// Check if a single word is spelled correctly.
    pub fn is_word_valid(&self, word: &str) -> bool {
        let trimmed = word
            .trim_matches(|c: char| !c.is_alphabetic())
            .to_ascii_lowercase();
        if trimmed.is_empty() {
            return true;
        }
        // If word contains non-alphabetic characters (e.g. Chinese characters or numbers), consider valid
        if !trimmed.chars().all(|c| c.is_ascii_alphabetic()) {
            return true;
        }

        self.dictionary.contains(&trimmed)
            || self.user_dictionary.contains(&trimmed)
            || self.ignored_words.contains(&trimmed)
    }

    /// Generate suggestions for a misspelled word based on edit distance.
    pub fn suggest(&self, word: &str, max_suggestions: usize) -> Vec<String> {
        let target = word.to_ascii_lowercase();
        let mut candidates: Vec<(usize, &String)> = Vec::new();

        for dict_word in self.dictionary.iter().chain(self.user_dictionary.iter()) {
            let dist = levenshtein(&target, dict_word);
            if dist <= 2 {
                candidates.push((dist, dict_word));
            }
        }

        candidates.sort_by_key(|&(dist, w)| (dist, w.len()));
        candidates
            .into_iter()
            .take(max_suggestions)
            .map(|(_, w)| w.clone())
            .collect()
    }

    /// Check a paragraph for spelling issues.
    pub fn check_paragraph(&self, paragraph: &Paragraph) -> Vec<SpellIssue> {
        let text = paragraph.raw_text();
        let mut issues = Vec::new();

        for (offset, word) in text.split_word_bound_indices() {
            let cleaned = word.trim_matches(|c: char| !c.is_ascii_alphabetic());
            if cleaned.is_empty() || cleaned.len() < 2 {
                continue;
            }

            if !self.is_word_valid(cleaned) {
                let suggestions = self.suggest(cleaned, 3);
                issues.push(SpellIssue {
                    word: cleaned.to_string(),
                    start_offset: offset,
                    end_offset: offset + word.len(),
                    paragraph: paragraph.id(),
                    suggestions,
                });
            }
        }

        issues
    }

    /// Check all paragraphs in document body.
    pub fn check_document(&self, doc: &Document) -> Vec<SpellIssue> {
        let mut all_issues = Vec::new();
        let body = doc.body();

        for block in body.blocks.iter() {
            if let Block::Paragraph(p) = block {
                all_issues.extend(self.check_paragraph(p));
            }
        }

        all_issues
    }

    /// Check Chinese punctuation and proofing rules.
    pub fn check_chinese_proofing(&self, paragraph: &Paragraph) -> Vec<ProofingIssue> {
        let text = paragraph.raw_text();
        let mut issues = Vec::new();

        // 1. Detect duplicate punctuation (e.g. ，，, 。。, ！！, ？？)
        let dup_punct = ["，，", "。。", "！！", "？？", "、、", "；；"];
        for pat in dup_punct {
            let mut search_start = 0;
            while let Some(pos) = text[search_start..].find(pat) {
                let actual_offset = search_start + pos;
                issues.push(ProofingIssue {
                    message: format!("检测到重复标点符号：{pat}"),
                    offset: actual_offset,
                    length: pat.len(),
                    paragraph: paragraph.id(),
                });
                search_start = actual_offset + pat.len();
            }
        }

        // 2. Detect unclosed book titles 《 ... 》
        let open_count = text.matches('《').count();
        let close_count = text.matches('》').count();
        if open_count != close_count {
            issues.push(ProofingIssue {
                message: format!(
                    "书名号未正确配对（《 出现 {open_count} 次，》 出现 {close_count} 次）"
                ),
                offset: 0,
                length: text.len(),
                paragraph: paragraph.id(),
            });
        }

        issues
    }
}

/// Compute Levenshtein distance between two strings.
fn levenshtein(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let m = a_chars.len();
    let n = b_chars.len();

    let mut dp = vec![vec![0; n + 1]; m + 1];

    for (i, row) in dp.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, val) in dp[0].iter_mut().enumerate() {
        *val = j;
    }

    for i in 1..=m {
        for j in 1..=n {
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else {
                1
            };
            dp[i][j] = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);
        }
    }

    dp[m][n]
}
