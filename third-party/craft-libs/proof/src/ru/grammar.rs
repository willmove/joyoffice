//! Russian grammar and punctuation checks: cheap, rule-based and tuned to stay quiet when unsure
//! (a missed comma costs less than a wrong mark on correct text). Messages are in Russian.
//!
//! - **Repeated word** (`в в`), except words Russian doubles on purpose (`да да`, `так так`,
//!   `ну ну`, `ха ха`, `очень очень`).
//! - **Space before punctuation** (`да ,`) and **double spaces**; emoticons (` :)`) and ellipses
//!   are left alone.
//! - **Capital letter at the start of a sentence**, knowing Russian abbreviations (`т. е.`, `г.`,
//!   `руб.`, `ул.`, `и т. д.`), initials, numbers, ellipses and dialogue dashes, after which lower
//!   case is correct.
//! - **Missing comma** before conjunctions and relative pronouns that always open a clause:
//!   `что`, `чтобы`, `поскольку`, `если`, `хотя` (not `хотя бы`) and the forms of `который`, and
//!   before `потому`/`оттого` in `потому что`. Not after a conjunction or particle (`и что`,
//!   `только что`, `даже если`, `так что`, `для того чтобы`), not after a preposition (`за что`,
//!   `в котором`: the comma goes before it) and not inside `что-то`.
//! - **Doubled punctuation** (`,,` — often a typed opening quote — `..`, `;;`).
//!
//! No English rule (such as `a`/`an`) applies to Russian text.

use crate::{Issue, IssueKind, ProofLang, words};

/// Words Russian repeats on purpose: interjections, onomatopoeia and emphatic doubling.
const ALLOWED_REPEATS: &[&str] = &[
    "да",
    "нет",
    "так",
    "ну",
    "ха",
    "хи",
    "хе",
    "ой",
    "ай",
    "эй",
    "ах",
    "ох",
    "эх",
    "ага",
    "угу",
    "тук",
    "кап",
    "бах",
    "бум",
    "тик",
    "так",
    "ля",
    "очень",
    "много",
    "давно",
    "далеко",
    "долго",
    "еле",
    "чуть",
    "тихо",
    "быстро",
    "скорей",
    "скорее",
    "иди",
    "давай",
    "пока",
    "ладно",
    "вот",
    "ещё",
    "еще",
    "всё",
    "все",
    "кто",
    "что",
    "то",
];

/// Abbreviations written with a final period after which a sentence goes on in lower case
/// (lower case, without the period).
const ABBREVIATIONS: &[&str] = &[
    "т",
    "е",
    "д",
    "п",
    "др",
    "пр",
    "см",
    "ср",
    "напр",
    "т.е",
    "т.д",
    "т.п",
    "т.к",
    "т.н",
    "т.о",
    "г",
    "гг",
    "в",
    "вв",
    "н.э",
    "до н.э",
    "руб",
    "коп",
    "тыс",
    "млн",
    "млрд",
    "трлн",
    "ул",
    "пр-т",
    "пер",
    "наб",
    "пл",
    "ш",
    "д",
    "кв",
    "корп",
    "стр",
    "оф",
    "эт",
    "под",
    "обл",
    "р-н",
    "пос",
    "дер",
    "им",
    "св",
    "проф",
    "доц",
    "акад",
    "канд",
    "наук",
    "зав",
    "зам",
    "ред",
    "изд",
    "сост",
    "пер",
    "рис",
    "табл",
    "гл",
    "разд",
    "ст",
    "ч",
    "п",
    "пп",
    "абз",
    "прим",
    "прил",
    "илл",
    "с",
    "сс",
    "т",
    "тт",
    "вып",
    "кн",
    "экз",
    "шт",
    "кг",
    "мин",
    "сек",
    "час",
    "мес",
    "нед",
    "янв",
    "фев",
    "февр",
    "мар",
    "апр",
    "авг",
    "сент",
    "окт",
    "нояб",
    "дек",
    "пн",
    "вт",
    "чт",
    "пт",
    "сб",
    "вс",
    "англ",
    "нем",
    "фр",
    "франц",
    "лат",
    "рус",
    "греч",
    "итал",
    "исп",
    "букв",
    "перен",
    "разг",
    "устар",
    "ок",
    "ок",
    "прибл",
    "мл",
    "ст",
    "тел",
    "моб",
    "факс",
    "эл",
    "почт",
    "вкл",
    "искл",
    "мм",
    "см",
    "дм",
    "км",
    "м",
    "л",
    "мл",
    "т",
    "ц",
    "млн",
    "долл",
    "евро",
    "др",
    "проч",
    "etc",
    "vs",
];

/// Conjunctions and relative pronouns that open a clause: a comma comes before them.
const CLAUSE_OPENERS: &[&str] = &[
    "что",
    "чтобы",
    "поскольку",
    "если",
    "хотя",
    "который",
    "которая",
    "которое",
    "которые",
    "которого",
    "которой",
    "которому",
    "которым",
    "котором",
    "которую",
    "которых",
    "которыми",
];

/// Words after which a clause opener takes no comma of its own: conjunctions (`и что`, `а если`),
/// particles (`только что`, `даже если`, `лишь бы`), interrogatives and the first halves of
/// compound conjunctions (`потому что`, `так что`, `для того чтобы`, `с тем чтобы`, `несмотря на
/// то что`, `тогда как`, `как будто`).
const NO_COMMA_AFTER: &[&str] = &[
    "и",
    "а",
    "но",
    "да",
    "или",
    "либо",
    "ни",
    "не",
    "же",
    "ли",
    "бы",
    "б",
    "то",
    "только",
    "лишь",
    "именно",
    "даже",
    "особенно",
    "потому",
    "оттого",
    "поэтому",
    "так",
    "затем",
    "тем",
    "того",
    // `тем более что`, `при условии что`, `в то время как`.
    "более",
    "условии",
    "время",
    "ввиду",
    "вследствие",
    "несмотря",
    "благодаря",
    "прежде",
    "раньше",
    "мере",
    "пор",
    "чем",
    "будто",
    "словно",
    "точно",
    "как",
    "если",
    "хотя",
    "хоть",
    "чтобы",
    "что",
    "кто",
    "где",
    "когда",
    "почему",
    "зачем",
    "куда",
    "откуда",
    "сколько",
    "разве",
    "неужели",
    "вот",
    "вон",
    "ну",
    "уже",
    "это",
    "просто",
    "чуть",
    "мол",
    "ведь",
    "вроде",
    "тоже",
    "также",
    "ещё",
    "еще",
    "всё",
    "все",
    "почти",
    "вовсе",
    "ой",
    "ах",
    "эх",
    "ох",
    "ага",
    "нет",
    "конечно",
    "наверное",
    "может",
    "пусть",
    "лучше",
    "условии",
    "случае",
    "того",
    "ради",
    "раз",
    "покуда",
    "пока",
    "после",
    "перед",
    "лишь",
    "едва",
    "чуть",
    "дабы",
    "кабы",
    "коли",
    "ежели",
    "абы",
    "вдруг",
    "иначе",
    "причём",
    "причем",
    "притом",
    "зато",
    "однако",
    "тогда",
    "сейчас",
    "теперь",
    "сначала",
    "сперва",
    "иногда",
    "часто",
    "всегда",
    "никогда",
    "т",
    "др",
];

/// Prepositions: a relative pronoun or `что` after one takes the comma before the preposition
/// instead (`дом, в котором`, `то, за что`).
const PREPOSITIONS: &[&str] = &[
    "в",
    "во",
    "на",
    "с",
    "со",
    "к",
    "ко",
    "о",
    "об",
    "обо",
    "от",
    "ото",
    "до",
    "из",
    "изо",
    "за",
    "по",
    "под",
    "подо",
    "над",
    "надо",
    "при",
    "про",
    "без",
    "безо",
    "для",
    "у",
    "через",
    "чрез",
    "между",
    "меж",
    "среди",
    "из-за",
    "из-под",
    "около",
    "возле",
    "вокруг",
    "мимо",
    "против",
    "вместо",
    "кроме",
    "сквозь",
    "вдоль",
    "внутри",
    "вне",
    "после",
    "перед",
    "пред",
    "ради",
    "благодаря",
    "согласно",
    "вопреки",
    "навстречу",
    "насчёт",
    "насчет",
    "ввиду",
    "вследствие",
    "посредством",
    "путём",
    "путем",
    "относительно",
    "касательно",
    "вслед",
    "ко",
    "подле",
    "близ",
    "внутрь",
    "поверх",
];

fn issue(start: usize, end: usize, message: String, suggestions: Vec<String>) -> Issue {
    Issue {
        start,
        end,
        kind: IssueKind::Grammar,
        message,
        suggestions,
        lang: ProofLang::Ru,
    }
}

/// Grammar issues in a paragraph of Russian text, in order.
pub fn check(text: &str) -> Vec<Issue> {
    check_with(text, true)
}

/// [`check`] where `capital_first` says whether the text's first word must start a sentence with a capital
/// (false for a list item, which may be a fragment).
pub fn check_with(text: &str, capital_first: bool) -> Vec<Issue> {
    let ws: Vec<(usize, usize)> = words(text);
    let mut v = Vec::new();
    crate::repeated::visit(
        text,
        ws.iter().copied(),
        ProofLang::Ru,
        ALLOWED_REPEATS,
        |pair| {
            let (first, second) = (pair.first, pair.second);
            v.push(issue(
                pair.range.start,
                pair.range.end,
                format!("Повтор слова: «{second}»"),
                vec![first.to_owned()],
            ));
        },
    );
    crate::editorial::punctuation(text, ProofLang::Ru, &mut v);
    crate::editorial::capitals(
        text,
        &ws,
        ProofLang::Ru,
        capital_first,
        ABBREVIATIONS,
        &mut v,
    );
    missing_commas(text, &ws, &mut v);
    v.sort_by_key(|i| (i.start, i.end));
    v
}

fn missing_commas(text: &str, ws: &[(usize, usize)], v: &mut Vec<Issue>) {
    for k in 1..ws.len() {
        let (Some(&(pa, pb)), Some(&(a, b))) = (ws.get(k - 1), ws.get(k)) else {
            continue;
        };
        let (Some(prev), Some(w), Some(gap)) = (text.get(pa..pb), text.get(a..b), text.get(pb..a))
        else {
            continue;
        };
        if gap.is_empty()
            || !gap.chars().all(|c| c == ' ' || c == '\u{a0}')
            || !prev.chars().all(char::is_alphabetic)
        {
            continue;
        }
        let lw = w.to_lowercase();
        let next = ws
            .get(k + 1)
            .and_then(|&(na, nb)| text.get(na..nb))
            .map(str::to_lowercase);
        let next_gap_plain = ws
            .get(k + 1)
            .and_then(|&(na, _)| text.get(b..na))
            .is_some_and(|g| g.chars().all(|c| c == ' ' || c == '\u{a0}'));
        // `потому что`, `оттого что`: the comma goes before the first word.
        let compound = matches!(lw.as_str(), "потому" | "оттого")
            && next.as_deref() == Some("что")
            && next_gap_plain;
        if !compound && !CLAUSE_OPENERS.contains(&lw.as_str()) {
            continue;
        }
        // `хотя бы` is a particle (`хотя бы раз`).
        if lw == "хотя" && next.as_deref() == Some("бы") {
            continue;
        }
        let p = prev.to_lowercase();
        let p = p.as_str();
        if NO_COMMA_AFTER.contains(&p) || PREPOSITIONS.contains(&p) {
            continue;
        }
        v.push(issue(
            pb,
            b,
            format!("Возможно, пропущена запятая перед «{w}»"),
            vec![format!(", {w}")],
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn messages(t: &str) -> Vec<String> {
        check(t)
            .into_iter()
            .map(|i| format!("{}@{}", i.message, &t[i.start..i.end]))
            .collect()
    }

    fn clean(t: &str) {
        let m = messages(t);
        assert!(m.is_empty(), "{t:?}: {m:?}");
    }

    #[test]
    fn repeated_words() {
        let v = check("Я пошёл в в магазин и и купил хлеб.");
        assert_eq!(v.len(), 2, "{v:?}");
        assert_eq!(v[0].message, "Повтор слова: «в»");
        assert_eq!(v[0].suggestions, ["в"]);
        assert!(
            v.iter()
                .all(|i| i.lang == ProofLang::Ru && i.kind == IssueKind::Grammar)
        );
        clean("Да да, так так. Ха ха! Очень очень давно.");
    }

    #[test]
    fn spaces_and_punctuation() {
        let m = messages("Мы пришли , а он ушёл .  Правда ?");
        assert!(
            m.contains(&"Лишний пробел перед знаком препинания@ ,".to_string()),
            "{m:?}"
        );
        assert!(
            m.contains(&"Лишний пробел перед знаком препинания@ .".to_string()),
            "{m:?}"
        );
        assert!(
            m.contains(&"Лишний пробел перед знаком препинания@ ?".to_string()),
            "{m:?}"
        );
        assert!(m.contains(&"Двойной пробел@  ".to_string()), "{m:?}");
        clean("Отлично :) Правда :-) Число .5 и многоточие ...");
        let m = messages("Он сказал ,,да» и ушёл.. Потом;; конец.");
        assert_eq!(
            m.iter().filter(|x| x.starts_with("Повтор знака")).count(),
            3,
            "{m:?}"
        );
        assert_eq!(
            check("Он сказал ,,да».")
                .iter()
                .find(|i| i.message.starts_with("Повтор знака"))
                .unwrap()
                .suggestions,
            ["«"]
        );
        clean("Ждал… и ждал... до утра.");
    }

    #[test]
    fn sentence_start_capitals() {
        let m = messages("это первое предложение. а это второе! правда? да.");
        assert_eq!(
            m.iter()
                .filter(|x| x.starts_with("Предложение начинается"))
                .count(),
            4,
            "{m:?}"
        );
        assert_eq!(check("мама мыла раму.")[0].suggestions, ["Мама"]);
        // Abbreviations, initials, numbers, ellipses and dialogue dashes.
        clean("Купили фрукты: яблоки, груши и т. д. и овощи.");
        clean("В 1990 г. открылась фирма, т. е. наша компания.");
        clean("Цена 500 руб. за штуку, адрес ул. Ленина, д. 5, кв. 3.");
        clean("Встретил А. С. Пушкина и проф. Иванова.");
        clean("Ждал... и наконец дождался.");
        clean("– Правда? – спросил он. – Да, – ответила она.");
        clean("Он сказал: «Вернусь». Потом ушёл.");
        clean("яблоки,");
    }

    #[test]
    fn missing_commas() {
        let m = messages(
            "Я знаю что он придёт. Это дом который построил Джек. Пришёл чтобы помочь. Ушёл потому что устал. Приходи если сможешь. Пошёл хотя устал.",
        );
        for want in [
            "Возможно, пропущена запятая перед «что»@ что",
            "Возможно, пропущена запятая перед «который»@ который",
            "Возможно, пропущена запятая перед «чтобы»@ чтобы",
            "Возможно, пропущена запятая перед «потому»@ потому",
            "Возможно, пропущена запятая перед «если»@ если",
            "Возможно, пропущена запятая перед «хотя»@ хотя",
        ] {
            assert!(m.contains(&want.to_string()), "{want}: {m:?}");
        }
        assert_eq!(check("Знаю что да.")[0].suggestions, [", что"]);
        // Conjunctions, particles, compound conjunctions and prepositions take no comma there.
        clean("Я знаю, что он придёт, и что останется.");
        clean("Он только что пришёл, так что всё в порядке.");
        clean("Это дом, в котором я живу, и сад, через который иду.");
        clean("Я сделаю это, даже если не успею, лишь бы ты был рад.");
        clean("Пришёл для того чтобы помочь, а не чтобы мешать.");
        clean("Ушёл, потому что устал, и не потому что обиделся.");
        clean("Расскажи хотя бы что-нибудь, за что тебя хвалят.");
        clean("Что ты делаешь? Который час?");
        clean("Кто-то что-то сказал.");
        // craft-libs#495 review: the second half of a compound conjunction.
        clean("Он устал, тем более что шёл долго.");
        clean("Приду, при условии что успею.");
        clean("Он читал, в то время как она спала.");
    }

    #[test]
    fn correct_russian_text_is_clean() {
        clean(
            "Съешь же ещё этих мягких французских булок, да выпей чаю. Мы пришли, когда уже стемнело.",
        );
        clean("Книга, которую я читаю, очень интересная, поэтому я её советую.");
    }

    proptest::proptest! {
        #[test]
        fn never_panics(s in "\\PC{0,80}") {
            let _ = check(&s);
        }

        #[test]
        fn never_panics_on_russian_like_text(s in "[а-яёА-ЯЁ ,.;:!?«»…\\-]{0,60}") {
            for i in check(&s) {
                proptest::prop_assert!(i.start <= i.end && i.end <= s.len());
                proptest::prop_assert!(s.is_char_boundary(i.start) && s.is_char_boundary(i.end));
            }
        }
    }
}
