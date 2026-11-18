//! Compile an installed Hunspell pair or query a compact dictionary. The same
//! executable interface permits interchanging two builds without an editor.
use joyword_proof::{affix::AffixDict, hunspell};
use std::{
    fs,
    hint::black_box,
    io::{self, BufRead},
    time::Instant,
};

fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [cmd, path] if cmd == "check" => {
            let dict = AffixDict::from_bytes(&fs::read(path).map_err(|e| e.to_string())?)?;
            for line in io::stdin().lock().lines() {
                let word = line.map_err(|e| e.to_string())?;
                println!("{word}\t{}", u8::from(dict.check(&word)));
            }
        }
        [cmd, path, words, repeats] if cmd == "lookup" => {
            let dict = AffixDict::from_bytes(&fs::read(path).map_err(|e| e.to_string())?)?;
            let text = fs::read_to_string(words).map_err(|e| e.to_string())?;
            let queries: Vec<_> = text.lines().collect();
            let repeats = repeats.parse::<usize>().map_err(|e| e.to_string())?;
            let start = Instant::now();
            let mut accepted = 0usize;
            for _ in 0..repeats {
                for &word in &queries {
                    accepted += usize::from(dict.check(black_box(word)));
                }
            }
            println!(
                "{} {} {} {}",
                dict.len(),
                queries.len(),
                accepted,
                start.elapsed().as_nanos()
            );
        }
        [cmd, aff, dic, output] if cmd == "pack" || cmd == "time" => {
            let bytes = |name| fs::read(name).map_err(|e| format!("{name}: {e}"));
            let (rules, stems) = hunspell::decode(&bytes(aff)?, &bytes(dic)?)?;
            if cmd == "pack" {
                let dict = AffixDict::from_hunspell(&rules, &stems, &[])?;
                fs::write(output, dict.to_bytes()?).map_err(|e| e.to_string())?;
            } else {
                let repeats = output.parse::<u32>().map_err(|e| e.to_string())?;
                let start = Instant::now();
                for _ in 0..repeats {
                    black_box(AffixDict::from_hunspell(
                        black_box(&rules),
                        black_box(&stems),
                        &[],
                    )?);
                }
                println!("{}", start.elapsed().as_nanos());
            }
        }
        _ => {
            return Err(
                "expected pack AFF DIC OUTPUT, check COMPACT, lookup COMPACT WORDS REPETITIONS, or time AFF DIC REPETITIONS".into(),
            );
        }
    }
    Ok(())
}
