# joyword-proof

First-party multilingual proofing crate for JoyWord: spelling, Hunspell affix/compound morphology, ranked suggestions, localized editorial/grammar rules, hyphenation, and installed language-pack loading.

## Provenance and Data Licensing

- Core editorial/lexical rules, affix syntax, compact dictionary indexing and writing, stem ranking, and pack integration are maintained first-party in JoyOffice (`crates/joyword-proof` and `crates/joyword-proof-rules`).
- Embedded language data (`third-party/craft-i18n/language-data` — SCOWL English dictionaries, Moby Hyphenator II, hyph-utf8 patterns, and word supplements) and retained MIT/Apache-2.0 language vocabularies/localized messages keep their full upstream license texts and provenance records under `licenses/craft-i18n/` and `licenses/craft-libs/`.
