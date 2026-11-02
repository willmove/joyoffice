# Third party sources

JoyDeck uses the JoyOffice MIT-licensed public UI, workspace layout, engineering launchers and packaging/provenance foundations from commit `a973f75d7d58ab8dd5f05b0e3db5f0c53137d57d`. Copyright (c) 2026 willmove. The original license is retained in `licenses/joyoffice/LICENSE-MIT`; file paths, original hashes and adaptations are recorded in `licenses/joyoffice/source-map.json`.

GPUI Kit 0.8.1, its component/assets layers and the GPUI pre 0.3.10 snapshot are Cargo dependencies. Their actual selected packages and licenses are recorded in the development package SBOM. Imported sources are not represented as independently authored clean-room implementations.

The welcome preview in `tests/fixtures/joydeck/welcome.json` is original content licensed under the repository MIT license. Runtime system fonts are not bundled. No ArtCraft/DeckCraft trademarks or product artwork are imported.

The color, geometry and presentation-model libraries are adapted from the supplied DeckCraft 0.14.1 snapshot, Copyright (c) 2026 ArtCraft Team and the DeckCraft contributors, under the MIT option of its MIT OR Apache-2.0 license. Original notices and both license texts are retained in `licenses/deckcraft`. `licenses/deckcraft/file-map.json` records the actual imported files, original snapshot hashes and local namespace/formatting adaptations. The broader snapshot inventory does not imply that all upstream files are imported.
