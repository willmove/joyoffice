//! Adapt JoyWord editorial findings to the original localized editor messages.
use crate::{Issue, IssueKind, ProofLang};
use joyword_proof_rules::{Language, MarkIssue};

struct Messages {
    language: Language,
    before: &'static str,
    extra: &'static str,
    repeated: Option<&'static str>,
    capital: Option<&'static str>,
}

fn messages(lang: ProofLang) -> Option<Messages> {
    let (language, before, extra, repeated, capital) = match lang {
        ProofLang::Cs => (
            Language::Czech,
            "Mezera před interpunkcí",
            "Dvojitá mezera",
            None,
            None,
        ),
        ProofLang::Hr => (
            Language::Croatian,
            "Razmak ispred interpunkcijskog znaka",
            "Dvostruki razmak",
            Some("Ponovljen interpunkcijski znak"),
            Some("Rečenica treba započeti velikim slovom"),
        ),
        ProofLang::Id => (
            Language::Indonesian,
            "Spasi sebelum tanda baca",
            "Spasi ganda",
            None,
            None,
        ),
        ProofLang::It => (
            Language::Italian,
            "Spazio prima della punteggiatura",
            "Spazio doppio",
            Some("Punteggiatura doppia"),
            Some("Maiuscola a inizio frase"),
        ),
        ProofLang::Pl => (
            Language::Polish,
            "Zbędna spacja przed znakiem interpunkcyjnym",
            "Podwójna spacja",
            Some("Powtórzony znak interpunkcyjny"),
            Some("Wielka litera na początku zdania"),
        ),
        ProofLang::Pt => (
            Language::Portuguese,
            "Espaço antes da pontuação",
            "Espaço duplo",
            Some("Sinal repetido"),
            Some("Letra maiúscula no início da frase"),
        ),
        ProofLang::Ru => (
            Language::Russian,
            "Лишний пробел перед знаком препинания",
            "Двойной пробел",
            Some("Повтор знака препинания"),
            Some("Предложение начинается со строчной буквы"),
        ),
        ProofLang::Es | ProofLang::Es419 | ProofLang::EsMx | ProofLang::EsUy => (
            Language::Spanish,
            "Espacio antes de un signo de puntuación",
            "Espacio doble",
            None,
            None,
        ),
        ProofLang::Tr => (
            Language::Turkish,
            "Noktalama işaretinden önce boşluk bırakılmaz",
            "Fazladan boşluk",
            None,
            Some("Cümle büyük harfle başlar"),
        ),
        ProofLang::En | ProofLang::EnGb | ProofLang::EnCa | ProofLang::EnAu => (
            Language::English,
            "Remove the space before the punctuation",
            "Extra space",
            None,
            Some("Capitalize the first word of a sentence"),
        ),
        _ => return None,
    };
    Some(Messages {
        language,
        before,
        extra,
        repeated,
        capital,
    })
}

pub(crate) fn punctuation(text: &str, lang: ProofLang, issues: &mut Vec<Issue>) {
    let Some(labels) = messages(lang) else { return };
    joyword_proof_rules::punctuation(text, labels.language, |mark| {
        let label = match mark.kind {
            MarkIssue::ExtraSpace => labels.extra,
            MarkIssue::BeforeMark => labels.before,
            MarkIssue::AfterOpening => "Espacio después de un signo de apertura",
            MarkIssue::RepeatedMark => {
                let Some(label) = labels.repeated else { return };
                label
            }
        };
        issues.push(Issue {
            start: mark.span.start,
            end: mark.span.end,
            kind: IssueKind::Grammar,
            lang,
            message: label.into(),
            suggestions: mark.replacements.iter().map(|&s| s.to_owned()).collect(),
        });
    });
}

pub(crate) fn capitals(
    text: &str,
    words: &[(usize, usize)],
    lang: ProofLang,
    capital_first: bool,
    abbreviations: &[&str],
    issues: &mut Vec<Issue>,
) {
    let Some(Messages {
        language,
        capital: Some(label),
        ..
    }) = messages(lang)
    else {
        return;
    };
    joyword_proof_rules::sentence_case(
        text,
        words,
        language,
        capital_first,
        abbreviations,
        |span, replacement| {
            issues.push(Issue {
                start: span.start,
                end: span.end,
                kind: IssueKind::Grammar,
                lang,
                message: label.into(),
                suggestions: vec![replacement],
            });
        },
    );
}
