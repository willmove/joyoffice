//! Time production paragraph review on fixed prose with and without editorial findings.
use joyword_proof::{ProofLang, check_grammar_in};
use std::hint::black_box;
use std::time::Instant;

fn main() {
    let paragraphs = [
        (
            "en",
            ProofLang::En,
            "The reader sees a paragraph with clear sentences. The next line contains a quote and a number. ",
        ),
        (
            "cs",
            ProofLang::Cs,
            "Škola stojí na konci ulice. Děti čtou knihu a pozorují svět. ",
        ),
        (
            "pl",
            ProofLang::Pl,
            "Książka leży na stole. Dalej przeczytamy nowe zdanie. Kto to napisał? ",
        ),
        (
            "tr",
            ProofLang::Tr,
            "Okurlar kitabı inceliyor. Bugün güzel bir gün. Masada üç elma ve iki armut var. ",
        ),
        (
            "it-edits",
            ProofLang::It,
            "Questo è un esempio , con una  pausa. apriamo il libro.. ",
        ),
        (
            "ru-edits",
            ProofLang::Ru,
            "Сегодня день хороший ,  но погода меняется. потом мы обсудим работу.. ",
        ),
        (
            "es-questions",
            ProofLang::Es,
            "Consulte https://ejemplo.es/a?b=2. ¿Viene hoy? Mañana también? ",
        ),
        (
            "en-articles",
            ProofLang::En,
            "Our visitor spent an hour at a university. The reader opened a book. ",
        ),
        (
            "pl-clauses",
            ProofLang::Pl,
            "Wiem że czytasz. Z szkoły idziemy we wtorek. ",
        ),
        (
            "ru-clauses",
            ProofLang::Ru,
            "Знаю что он придёт. Ушёл потому что устал. ",
        ),
        (
            "it-elisions",
            ProofLang::It,
            "Un amica sa qual’è la strada. Il Po è lungo. ",
        ),
        (
            "tr-conjunctions",
            ProofLang::Tr,
            "Ali, ve Veli geldi. Bugün okuduk ve yarın yazacağız. ",
        ),
    ];
    for (label, lang, fragment) in paragraphs {
        let text = fragment.repeat(24);
        let expected = check_grammar_in(&text, lang).len();
        let iterations = 500;
        let start = Instant::now();
        for _ in 0..iterations {
            let issues = check_grammar_in(black_box(&text), lang);
            assert_eq!(black_box(issues.len()), expected);
        }
        println!(
            "{label},{iterations},{expected},{}",
            start.elapsed().as_nanos()
        );
    }
}
