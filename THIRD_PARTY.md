# Third party sources

JoyDeck uses the JoyOffice MIT-licensed public UI and workspace layout implementation from commit `a973f75d7d58ab8dd5f05b0e3db5f0c53137d57d`. Copyright (c) 2026 willmove. The original license is retained in `licenses/joyoffice/LICENSE-MIT`; file paths, original hashes and adaptations are recorded in `licenses/joyoffice/source-map.json`.

GPUI Kit 0.8.1, its component/assets layers and the GPUI pre 0.3.10 snapshot are Cargo dependencies. Their actual selected packages and licenses are recorded in the development package SBOM. Imported sources are not represented as independently authored clean-room implementations.

The welcome preview in `tests/fixtures/joydeck/welcome.json` is original content licensed under the repository MIT license. Runtime system fonts are not bundled. No ArtCraft/DeckCraft trademarks or product artwork are imported.

DeckCraft source migration is tracked by `docs/joydeck/TASKS.md`; its existing source inventory remains a planning reference until particular files are imported and attributed.
