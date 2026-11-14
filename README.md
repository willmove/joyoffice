# JoyOffice

[English](#english) · [简体中文](#chinese) · [Downloads](#download-and-install) · [下载](#下载与安装)

<a id="english"></a>

JoyOffice is a desktop office suite built with **Rust and GPUI Kit**. Its applications, **JoyWord**, **JoySheet**, and **JoyDeck**, share the suite version with the first-party workspace libraries and tools. Each product includes a native desktop app and a headless CLI.

This overview reflects the repository as of **2026-11-14**.

## Download and install

Choose the **v0.1.4 preview** for your operating system and processor. Each portable archive includes the desktop app and CLI.

| OS and processor | [JoyWord](https://github.com/willmove/joyoffice/releases/tag/joyword-v0.1.4) | [JoySheet](https://github.com/willmove/joyoffice/releases/tag/joysheet-v0.1.4) | [JoyDeck](https://github.com/willmove/joyoffice/releases/tag/joydeck-v0.1.4) |
| --- | --- | --- | --- |
| Windows x64 | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.4/joyword-0.1.4-windows-x86_64-setup.exe) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.4/joyword-0.1.4-windows-x86_64.zip) | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-windows-x86_64-release-setup.exe) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-windows-x86_64-release.zip) | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.4/joydeck-0.1.4-windows-x86_64-release-setup.exe) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.4/joydeck-0.1.4-windows-x86_64-release.zip) |
| Windows ARM64 | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.4/joyword-0.1.4-windows-aarch64-setup.exe) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.4/joyword-0.1.4-windows-aarch64.zip) | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-windows-aarch64-release-setup.exe) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-windows-aarch64-release.zip) | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.4/joydeck-0.1.4-windows-aarch64-release-setup.exe) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.4/joydeck-0.1.4-windows-aarch64-release.zip) |
| macOS Apple Silicon | [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.4/joyword-0.1.4-macos-aarch64.zip) | [DMG](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-macos-aarch64-release.dmg) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-macos-aarch64-release.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.4/joydeck-0.1.4-macos-aarch64-release.zip) |
| macOS Intel | [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.4/joyword-0.1.4-macos-x86_64.zip) | [DMG](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-macos-x86_64-release.dmg) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-macos-x86_64-release.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.4/joydeck-0.1.4-macos-x86_64-release.zip) |
| Linux x64 | [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.4/joyword-0.1.4-linux-x86_64.tar.gz) | [DEB](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-linux-x86_64-release.deb) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-linux-x86_64-release.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.4/joydeck-0.1.4-linux-x86_64-release.zip) |
| Linux ARM64 | [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.4/joyword-0.1.4-linux-aarch64.tar.gz) | [DEB](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-linux-aarch64-release.deb) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-linux-aarch64-release.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.4/joydeck-0.1.4-linux-aarch64-release.zip) |

- **Windows** All three apps offer Setup EXE installers with app-local runtime DLLs, Start menu and uninstall entries. Alternatively, extract the ZIP and run `joyword.exe`, `joysheet.exe`, or `joydeck.exe`; the original Word/Deck ZIPs require a system-provided Visual C++ v14 runtime.
- **macOS** Extract the ZIP and open the included `.app`, or move it to Applications. For the JoySheet DMG, mount it and drag `Joysheet.app` to Applications.
- **Linux** These packages are built on Ubuntu 24.04. Extract and run the app in a desktop session. On Ubuntu 24.04, install the JoySheet DEB with `sudo apt install ./filename.deb` and launch it from the app menu. Other distributions may run the portable archives if they provide compatible libraries.

These previews ship without publisher signatures or macOS notarization. Each product’s release page lists SHA-256 checksums, runtime requirements, and known limits.

## Products and current capabilities

| Product | App / CLI version | Primary files | Current development baseline |
| --- | --- | --- | --- |
| <img src="assets/brand/joyword.svg" width="24" height="24" alt=""> **JoyWord** | **0.1.4** | DOCX, OpenDocument, legacy DOC import | R8 implementation integrated into JoyOffice. [Handoff and compatibility](docs/joyoffice/JOYWORD-R8-HANDOFF.md) · [Status](docs/joyword/r8/STATUS.md) |
| <img src="assets/brand/joysheet.svg" width="24" height="24" alt=""> **JoySheet** | **0.1.4** | XLSX | Integrated M8-preview6 feature baseline with subsequent fixes; [developer guide](docs/joysheet/README.md) · [status](docs/joysheet/STATUS.md) |
| <img src="assets/brand/joydeck.svg" width="24" height="24" alt=""> **JoyDeck** | **0.1.4** | PPTX, `.joydeck` v2 | R9 integration of the DeckCraft/GPUI refactor; [handoff](docs/joyoffice/JOYDECK-R9-HANDOFF.md) · [status](docs/joydeck/STATUS.md) |

### JoyWord — documents

- Paginated editing with character/paragraph formatting, styles, tables, images, sections, columns, headers, footers, and page fields.
- Find and replace, outline navigation, comments, tracked changes, comparison, and mail merge.
- Open and save DOCX, import supported legacy DOC files, and exchange supported ODT, OTT and FODT content. The File workspace offers dedicated export choices, including PDF, PNG, PDF/A-2b, EPUB and two Word XML formats. [Format and validation details](docs/joyword/r8/STATUS.md)
- Shared desktop/CLI editing sessions, recovery drafts, legacy journal recovery, and authenticated access to selected active-document operations through a local control bridge and MCP.
- The ribbon follows the observed Windows Word grouping, with working command menus, clipboard controls, paragraph numeric controls, and explicit disabled states for unavailable features. See the [product status](docs/joyword/r8/STATUS.md) for implementation and validation scope.
- Recent work strengthens imported theme/font/color bindings, unknown XML retention, recovery and preference safety, and complex-object compatibility regression coverage.

The optional `agent` build feature provides local selection proposals with preview and undoable acceptance. Provider, credential, and broader desktop integration remain open. See [R8 validation](docs/joyword/r8/VALIDATION.md) for tested workflows and remaining conditions.

### JoySheet — spreadsheets

- A virtualized grid with frozen panes, hidden rows/columns, zoom, range selection, cell editing, and a shared formula-bar draft.
- Formula evaluation, function completion, reference highlighting, number formats, cell styles, sorting, filtering, tables, conditional formatting, and data validation.
- Charts, pictures, shapes, sparklines, pivot-table workflows, analysis/solver forms, comments, protection, and named ranges.
- XLSX workflows, additional spreadsheet readers and text/data exchange, PDF export/preview, multi-window workbook views, recovery, scripts, CLI, and MCP.
- Password prompts for protected XLSX files support retry and cancel. Save As writes an unencrypted copy and preserves the source. Windows testing also covered copying integers, decimals and text both ways with Excel. [Validation](docs/joysheet/STATUS.md)
- The Excel-style ribbon includes Drawing, Help and File backstage; supported structured-object editing, page-break dragging and formula trace arrows have scoped validation.

Format and UI coverage varies by workflow. The [migration matrix](docs/joysheet/MIGRATION.md) distinguishes implemented features, automated checks, and remaining native acceptance work.

### JoyDeck — presentations

- A slide workspace with Ribbon controls, thumbnails, canvas selection and transforms, notes, slide browsing, zoom, and multiple views sharing one editable document.
- Text boxes, character/paragraph formatting, rotated/vertical text, Unicode/IME input, tables, images, hyperlinks, and slide/date fields.
- PPTX and native v2 save/reopen, undo/redo, recovery, basic comparison/review, and explicit migration of legacy `.joydeck` v1 files.
- Slide-show and presenter windows; PDF, image, GIF, outline, and other scoped exports; static MP4 output on verified Windows/macOS paths; CLI, JSON control, and MCP.
- The presentation workspace can fill, update and clear footer text with undo. PPTX input now allows up to 500 MiB subject to ZIP and XML budgets. [Validation and limits](docs/joydeck/STATUS.md)
- Updated File backstage, Up/Down slide navigation and slide-show rendering at the actual window pixel size and DPI improve everyday use.

Full master/theme editing, complex content and media, the complete command/UI matrix, and old automation parity remain in progress. See the [task list](docs/joydeck/TASKS.md) and [UI parity record](docs/joydeck/UI-PARITY.md).

All three apps provide shared light/dark theme presets and double-click ribbon collapse. See the [v0.1.4 Windows validation](docs/joyoffice/validation/20261113-v014/README.md) for this release's tested scope and remaining platform checks.

## Architecture and repository layout

The suite shares one Cargo workspace, lockfile, and toolchain while keeping each product's document model and editing engine independent. Product applications do not depend on one another. GPUI stays at the desktop boundary so CLI and core tests can run without the GUI dependency.

```text
apps/
  joyword/           joyword-cli/
  joysheet/          joysheet-cli/
  joydeck/           joydeck-cli/
crates/
  joyword-*          Document model, editing, layout, formats, rendering, shell, UI
  joysheet-*         Workbook model, formulas, calculation, formats, charts, UI
  joydeck-*          Slide model, editing, text, animation, formats, rendering, UI
  joyoffice-*        Shared UI, OPC, CFB, DrawingML, and OMML infrastructure
tools/xtask/         Product-scoped engineering commands
scripts/            Validation, compatibility, packaging, and platform tools
docs/               Architecture, product guides, tasks, and validation evidence
assets/             Application assets and sample documents
tests/fixtures/     Registered format and regression inputs
licenses/           Third-party notices, source records, and license texts
```

- **Word:** the engine owns edits; layout reads document state; screen and PDF consumers share page-scene data. Desktop and CLI use the same `Session` through `joyword-shell`.
- **Sheet:** one engine worker owns the session; UI views consume immutable snapshots through bounded queues and revision checks.
- **Deck:** `joydeck-model`/`joydeck-engine` own the editable model and commands; format/rendering modules are separated from shell IO and GPUI. `joydeck-legacy` is a private read-only migration bridge.
- **Shared code:** public modules stay product-neutral. Further format/font consolidation proceeds incrementally, with consumer regression checks.

See [module boundaries](docs/joyoffice/ARCHITECTURE.md). Engineering rules require bounded untrusted inputs, preservation or explicit refusal of unsupported content, and no silent loss of unknown XML.

## Build and run

### Prerequisites

The repository pins **Rust 1.98.1**, **edition 2024**, **GPUI Kit 0.8.1**, and **GPUI Pre 0.3.10**. Engineering commands require **Python 3.11 or newer**; set `JOYOFFICE_PYTHON` to select an interpreter if needed. Use the committed `Cargo.lock`.

| Platform | Native development requirements |
| --- | --- |
| Windows | PowerShell 7, Visual Studio C++ build tools and Windows SDK, CMake. Product wrappers help locate the Visual Studio CMake installation. |
| macOS | Xcode with the Metal compiler available through `xcrun --find metal`. Distribution builds use the normal Metal path. |
| Linux | Clang, CMake, pkg-config, font and X11/Wayland development libraries, Vulkan, OpenSSL, and ALSA development packages. GUI execution needs a working display and graphics driver. |

Exact Linux package lists are maintained in [Word CI](.github/workflows/ci.yml), [Sheet CI](.github/workflows/ci-joysheet.yml), and [Deck CI](.github/workflows/ci-joydeck.yml). Install appropriate CJK fonts for Chinese text. Deck's external HTML/RTF clipboard adapter uses `xclip` on X11 or `wl-paste` from `wl-clipboard` on Wayland; the relevant command must be on `PATH`. Sheet's macOS/Linux PDF preview uses Poppler's `pdftoppm`.

### Get the source and launch a product

```sh
git clone https://github.com/willmove/joyoffice.git
cd joyoffice
cargo fetch --locked

cargo xtask build joyword
cargo xtask build joysheet
cargo xtask build joydeck

cargo xtask run joyword -- assets/samples/mixed-media.docx
cargo xtask run joysheet -- --sample sales
cargo xtask run joydeck -- tests/fixtures/joydeck/r1-preview.pptx
```

Choose the product you need; building all three is optional. Add `--release` to a product build or run command for the optimized profile. Use the `xtask` run entry point so required Deck platform helpers are prepared.

## CLI and automation

`--headless` selects the product CLI for build/run and excludes desktop packages from product checks. It also checks that the selected normal/build dependency closure does not include GPUI.

```sh
cargo xtask build joyword --headless
cargo xtask build joysheet --headless
cargo xtask build joydeck --headless

cargo xtask run joyword --headless -- inspect --session assets/samples/mixed-media.docx
cargo xtask run joysheet --headless -- eval "SUM(1,2,3)" --json
cargo xtask run joydeck --headless -- inspect tests/fixtures/joydeck/r1-preview.pptx
```

Word supports inspection, rendering, conversion, mail merge, recovery, and MCP; Sheet exposes workbook inspection, formula evaluation, conversion, scripts, and MCP; Deck exposes inspection, rendering, editing scripts, migration, export, JSON control, and MCP.

Automation access is explicit and product-specific. Word's headless MCP requires authorized read/write roots; its desktop bridge uses an authenticated loopback connection. Deck's headless control/MCP requires an authorized root, and attaching control to a desktop view is opt-in. Query advertised capabilities and command schemas before editing: Deck's retained `joydeck-control-1` envelope uses command schema **2**, with only an advertised subset of legacy string commands. Protocol details: [Word handoff](docs/joyoffice/JOYWORD-R8-HANDOFF.md), [Sheet control](docs/joysheet/CONTROL.md), [Deck control](docs/joydeck/CONTROL.md).

## Checks and development workflow

```sh
python3 scripts/check_docs.py
python3 scripts/check_gates.py

cargo xtask check joyword
cargo xtask check joysheet
cargo xtask check joydeck
```

`check` runs product-scoped formatting, strict Clippy, and tests for the product packages and shared libraries they use. Deck additionally checks imported sources, dependencies, and refactor documentation. `cargo xtask test <product>` runs the selected tests; `cargo xtask check <product> --headless` checks the non-GUI scope. Fetch the locked dependency graph before checks that inspect it offline.

For shared-library, lockfile, resolver, or build/feature changes, run all affected products and the combined feature regression:

```sh
cargo clippy --workspace --locked --all-targets --features joydeck/validation,joysheet/automation -- -D warnings
cargo test --workspace --locked --features joydeck/validation,joysheet/automation
```

Native acceptance hooks for Deck and Sheet use the explicit `--validation` option; ordinary builds do not enable those hooks. Follow [AGENTS.md](AGENTS.md) for branch conventions, module boundaries, exact dependency pins, `unsafe_code = forbid`, and validation requirements. Develop all three products here; archived standalone directories and old milestone reports serve as historical references.

## Packaging and releases

```sh
cargo xtask package joyword --release
cargo xtask package joysheet --release
cargo xtask package joydeck --release
```

Native development packages are written under `dist/joyword/`, `dist/joysheet/`, and `dist/joydeck/`. Packaging runs on the host platform and includes product-specific binaries, resources, build/source fingerprints, and dependency/license metadata. Cross-target build/check supports `--target`; packaging is currently native-only, and headless-only packages are not implemented.

All first-party workspace packages inherit the version from `[workspace.package]` in `Cargo.toml`. Products retain separate release tags (`joyword-v*`, `joysheet-v*`, `joydeck-v*`), and historical `v*` tags route to Word. JoyDeck previously used `0.8.0` for its refactor builds. When moving one of those installations to `0.1.4`, select the new package explicitly because a semantic-version comparison ranks `0.1.4` below `0.8.0`. [Release workflow](.github/workflows/release.yml)

Each desktop package carries its product logo and platform icon. See the [icon maintenance guide](docs/joyoffice/BRANDING.md) for the Windows executable resource, macOS app bundle, and Linux desktop launcher.

Development packages and successful CI checks do not establish formal distribution readiness. Exact-version license-text gaps, signing/notarization, installers, file associations, upgrade/rollback, and device validation remain tracked per product; strict packaging gates can refuse output while requirements are unmet.

## Platform evidence and next work

| Platform | Recorded scope and remaining work |
| --- | --- |
| Windows | Development-package and native editing checks exist for all three products. Full system IME, screen-reader, high-DPI/multi-monitor, printer/device, and installer acceptance remain scenario-specific. |
| macOS ARM64 | Release apps have been built and relocated, with file-open and editing/save/reopen evidence. Deck has additional system input and text/table checks. Signing/notarization and the remaining native matrix are open. |
| Linux and other architectures | Headless and selected desktop results are recorded separately; Sheet includes UOS 25 X11 basic GUI coverage. Current Deck Wayland/X11 desktop qualification and the complete cross-platform matrix remain open. CI/QEMU results are distinct from hardware acceptance. |

Representative records: [macOS three-product validation](docs/joyoffice/validation/20261104-macos/README.md), [native dialogs and Word/WPS round trips](docs/joyoffice/validation/20261105-macos-desktop/README.md), [recent editing/export checks](docs/joyoffice/validation/20261106-p2/README.md), and [remaining work and Linux CLI rollback](docs/joyoffice/validation/20261106-remaining-work/README.md). UOS/Kylin and other domestic-platform hardware follow-up is recorded when available and does not block ongoing development; untested hardware stays explicitly unverified.

Current priorities are Word legacy-data/format compatibility and native distribution, Deck daily-editing/UI parity and complex-content/automation compatibility, and Sheet broader interoperability, native interaction, and performance qualification. Historical Word M0–M9 and Deck M0–M14 results describe their earlier implementations and do not automatically qualify the current ones.

## Documentation

| Area | Starting points |
| --- | --- |
| Repository | [Development rules](AGENTS.md) · [Architecture](docs/joyoffice/ARCHITECTURE.md) · [Monorepo history](docs/joyoffice/MONOREPO-RESULT.md) |
| JoyWord | [R8 handoff](docs/joyoffice/JOYWORD-R8-HANDOFF.md) · [Current status](docs/joyword/r8/STATUS.md) · [Validation](docs/joyword/r8/VALIDATION.md) |
| JoySheet | [Developer guide](docs/joysheet/README.md) · [Status](docs/joysheet/STATUS.md) · [Feature matrix](docs/joysheet/MIGRATION.md) · [Integration results](docs/joyoffice/JOYSHEET-RESULT.md) |
| JoyDeck | [R9 handoff](docs/joyoffice/JOYDECK-R9-HANDOFF.md) · [Developer guide](docs/joydeck/DEVELOPMENT.md) · [Tasks](docs/joydeck/TASKS.md) · [UI parity](docs/joydeck/UI-PARITY.md) · [Command inventory](docs/joydeck/COMMANDS.csv) |

## License and attribution

The root project is licensed under [MIT](LICENSE). Imported code and assets retain their original licenses and notices: see [WordCraft source records](licenses/wordcraft/SOURCE.json), [DeckCraft attribution](licenses/deckcraft/ATTRIBUTION-upstream.md), [GridCraft/Sheet attribution](docs/joysheet/ATTRIBUTION.md), and [GPUI notices](licenses/gpui/NOTICE.md). The root license does not replace these component-specific terms. System fonts are not committed or redistributed as project assets.

---

<a id="chinese"></a>

# JoyOffice（简体中文）

[English](#english) · [简体中文](#chinese)

JoyOffice 是使用 **Rust 和 GPUI Kit** 构建的桌面 Office 套件。**JoyWord**、**JoySheet**、**JoyDeck** 与仓库内第一方库和工具共用套件版本。每款产品都提供原生桌面应用和无界面 CLI。

本文概况截至 **2026-11-14**。

## 下载与安装

选择与你的系统和处理器匹配的 **v0.1.4 预览版**。每个便携包都包含桌面应用和 CLI。

| 系统与架构 | [JoyWord](https://github.com/willmove/joyoffice/releases/tag/joyword-v0.1.4) | [JoySheet](https://github.com/willmove/joyoffice/releases/tag/joysheet-v0.1.4) | [JoyDeck](https://github.com/willmove/joyoffice/releases/tag/joydeck-v0.1.4) |
| --- | --- | --- | --- |
| Windows x64 | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.4/joyword-0.1.4-windows-x86_64-setup.exe) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.4/joyword-0.1.4-windows-x86_64.zip) | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-windows-x86_64-release-setup.exe) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-windows-x86_64-release.zip) | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.4/joydeck-0.1.4-windows-x86_64-release-setup.exe) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.4/joydeck-0.1.4-windows-x86_64-release.zip) |
| Windows ARM64 | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.4/joyword-0.1.4-windows-aarch64-setup.exe) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.4/joyword-0.1.4-windows-aarch64.zip) | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-windows-aarch64-release-setup.exe) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-windows-aarch64-release.zip) | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.4/joydeck-0.1.4-windows-aarch64-release-setup.exe) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.4/joydeck-0.1.4-windows-aarch64-release.zip) |
| macOS Apple 芯片 | [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.4/joyword-0.1.4-macos-aarch64.zip) | [DMG](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-macos-aarch64-release.dmg) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-macos-aarch64-release.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.4/joydeck-0.1.4-macos-aarch64-release.zip) |
| macOS Intel | [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.4/joyword-0.1.4-macos-x86_64.zip) | [DMG](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-macos-x86_64-release.dmg) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-macos-x86_64-release.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.4/joydeck-0.1.4-macos-x86_64-release.zip) |
| Linux x64 | [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.4/joyword-0.1.4-linux-x86_64.tar.gz) | [DEB](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-linux-x86_64-release.deb) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-linux-x86_64-release.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.4/joydeck-0.1.4-linux-x86_64-release.zip) |
| Linux ARM64 | [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.4/joyword-0.1.4-linux-aarch64.tar.gz) | [DEB](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-linux-aarch64-release.deb) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-linux-aarch64-release.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.4/joydeck-0.1.4-linux-aarch64-release.zip) |

- **Windows** 三个产品均可使用 Setup EXE 安装程序，携带 app-local 运行库并提供开始菜单及卸载入口；也可解压 ZIP 后运行 `joyword.exe`、`joysheet.exe` 或 `joydeck.exe`，其中原 Word/Deck ZIP 需系统提供 Visual C++ v14 运行库。
- **macOS** 解压 ZIP 后打开随附的 `.app`，也可把它拖入「应用程序」。JoySheet DMG 挂载后可将 `Joysheet.app` 拖入「应用程序」。
- **Linux** 这些包在 Ubuntu 24.04 上构建。解压后在图形会话中运行对应程序。JoySheet DEB 可在 Ubuntu 24.04 上用 `sudo apt install ./文件名.deb` 安装，再从应用菜单打开。其他发行版可尝试使用便携包，需满足运行库要求。

预览包尚无发行者签名或 macOS 公证。各产品发布页提供 SHA-256 校验文件、运行要求及兼容范围。

## 产品与当前能力

| 产品 | 应用／CLI 版本 | 主要文件 | 当前开发基线 |
| --- | --- | --- | --- |
| <img src="assets/brand/joyword.svg" width="24" height="24" alt=""> **JoyWord** | **0.1.4** | DOCX、OpenDocument、旧版 DOC 导入 | 已整合到 JoyOffice 的 R8 实现。[交接与兼容表](docs/joyoffice/JOYWORD-R8-HANDOFF.md) · [状态](docs/joyword/r8/STATUS.md) |
| <img src="assets/brand/joysheet.svg" width="24" height="24" alt=""> **JoySheet** | **0.1.4** | XLSX | 已整合的 M8-preview6 功能基线及后续修复；[开发指南](docs/joysheet/README.md) · [状态](docs/joysheet/STATUS.md) |
| <img src="assets/brand/joydeck.svg" width="24" height="24" alt=""> **JoyDeck** | **0.1.4** | PPTX、`.joydeck` v2 | DeckCraft/GPUI 重构实现的 R9 整合；[交接](docs/joyoffice/JOYDECK-R9-HANDOFF.md) · [状态](docs/joydeck/STATUS.md) |

### JoyWord — 文档

- 分页编辑，支持字符／段落格式、样式、表格、图片、分节、分栏、页眉页脚和页码字段。
- 查找替换、大纲导航、批注、修订、比较和邮件合并。
- 打开和保存 DOCX，导入受支持的旧版 DOC，读写受支持的 ODT、OTT 和 FODT 内容。「文件」工作区提供独立导出入口，包含 PDF、PNG、PDF/A-2b、EPUB 和两种 Word XML。[格式与验证记录](docs/joyword/r8/STATUS.md)
- 桌面与 CLI 共用编辑会话，提供恢复草稿、旧日志恢复，以及通过本地认证控制桥和 MCP 操作活动文档的部分能力。
- 功能区按实测 Windows Word 分组，接通真实命令菜单、剪贴板与段落数值控件，并明确禁用尚未提供的功能。实现与验证范围见[产品状态](docs/joyword/r8/STATUS.md)。
- 近期加强导入主题与字体／颜色绑定、未知 XML 保留、恢复与偏好安全，以及复杂对象兼容性回归覆盖。

可选 `agent` 构建特性提供本地选区提案、预览和可撤销的接受操作。Provider、凭据及更广的桌面集成仍开放。已验证流程与剩余条件见 [R8 验证](docs/joyword/r8/VALIDATION.md)。

### JoySheet — 电子表格

- 虚拟化网格，支持冻结窗格、隐藏行列、缩放、范围选择、单元格编辑和与公式栏共享的草稿。
- 公式计算、函数补全、引用高亮、数字格式、单元格样式、排序、筛选、表格、条件格式和数据验证。
- 图表、图片、形状、迷你图、透视表流程、分析／求解表单、批注、保护与名称管理。
- XLSX 工作流、其他电子表格格式读取及文本／数据交换、PDF 导出／预览、工作簿多窗口视图、恢复、脚本、CLI 和 MCP。
- 打开受密码保护的 XLSX 可输入密码、重试或取消。「另存为」生成未加密副本并保留源文件。Windows 已与 Excel 实测双向复制整数、小数和文本。[验证记录](docs/joysheet/STATUS.md)

不同工作流的格式和 UI 覆盖程度有所区别。[迁移矩阵](docs/joysheet/MIGRATION.md)分别记录已实现功能、自动检查和仍待完成的原生验收。

### JoyDeck — 演示文稿

- 包含 Ribbon、缩略图、画布选择与变换、备注、幻灯片浏览和缩放的编辑工作区，多个视图可共享同一可编辑文稿。
- 文本框、字符／段落格式、旋转／竖排文字、Unicode／输入法、表格、图片、超链接和幻灯片编号／日期字段。
- PPTX 与原生 v2 格式保存重开、撤销重做、恢复、基础比较／审阅，以及旧 `.joydeck` v1 文件的显式迁移。
- 放映和演示者窗口；PDF、图片、GIF、大纲等限定范围的导出；已验证 Windows／macOS 路径上的静态 MP4 输出；CLI、JSON control 和 MCP。
- 支持补建、更新和清空页脚文字，可撤销。PPTX 文件读取上限提高至 500 MiB，仍受 ZIP 和 XML 安全预算约束。[验证及限制](docs/joydeck/STATUS.md)
- 重做文件后台，接入上下键翻页，放映画面按窗口实际像素与 DPI 渲染。

完整母版／主题编辑、复杂内容和媒体、全部命令／UI 矩阵及旧自动化等价仍在推进。详见[任务清单](docs/joydeck/TASKS.md)和[界面功能对照](docs/joydeck/UI-PARITY.md)。

三产品均提供共享亮暗主题预设和双击收展功能区。[v0.1.4 Windows 验证](docs/joyoffice/validation/20261113-v014/README.md)记录本版实测范围及未覆盖的平台验收。

## 架构与仓库布局

套件共用一个 Cargo workspace、锁文件和工具链，各产品独立维护文档模型与编辑引擎，应用之间不互相依赖。GPUI 限于桌面边界，CLI 和核心测试可以在不依赖 GUI 的情况下运行。

```text
apps/
  joyword/           joyword-cli/
  joysheet/          joysheet-cli/
  joydeck/           joydeck-cli/
crates/
  joyword-*          文档模型、编辑、布局、格式、渲染、外壳与 UI
  joysheet-*         工作簿模型、公式、计算、格式、图表与 UI
  joydeck-*          幻灯片模型、编辑、文字、动画、格式、渲染与 UI
  joyoffice-*        公共 UI、OPC、CFB、DrawingML 与 OMML 基础设施
tools/xtask/         按产品选择的工程命令
scripts/            验证、兼容性、打包和平台工具
docs/               架构、产品指南、任务与验证证据
assets/             应用资产与样例文档
tests/fixtures/     已登记的格式与回归输入
licenses/           第三方声明、来源记录与许可正文
```

- **Word：** 引擎负责编辑，布局只读文档状态；屏幕和 PDF 消费共用页面场景数据。桌面与 CLI 通过 `joyword-shell` 使用同一 `Session`。
- **Sheet：** 单一引擎 worker 持有会话，UI 通过有界队列和修订检查消费不可变快照。
- **Deck：** `joydeck-model`／`joydeck-engine` 持有可编辑模型与命令；格式／渲染模块与外壳 IO、GPUI 分离。`joydeck-legacy` 是私有只读迁移桥。
- **公共代码：** 公共模块保持产品中立，格式和字体能力继续逐步收敛，并验证实际使用方。

详见[模块边界](docs/joyoffice/ARCHITECTURE.md)。工程约定要求限制不可信输入预算，对不支持内容保留原始数据或明确拒绝，不静默丢失未知 XML。

## 构建和运行

### 环境要求

仓库固定 **Rust 1.98.1**、**edition 2024**、**GPUI Kit 0.8.1** 和 **GPUI Pre 0.3.10**。工程命令需要 **Python 3.11 或以上**，必要时可用 `JOYOFFICE_PYTHON` 指定解释器。依赖使用已提交的 `Cargo.lock`。

| 平台 | 原生开发要求 |
| --- | --- |
| Windows | PowerShell 7、Visual Studio C++ 构建工具及 Windows SDK、CMake。产品包装脚本可协助定位 Visual Studio 自带的 CMake。 |
| macOS | 安装 Xcode，并确保 `xcrun --find metal` 可找到 Metal 编译器。分发构建使用正常 Metal 路径。 |
| Linux | Clang、CMake、pkg-config，字体与 X11／Wayland 开发库，以及 Vulkan、OpenSSL、ALSA 开发包。运行 GUI 需要可用的显示与图形驱动环境。 |

准确的 Linux 包清单随 [Word CI](.github/workflows/ci.yml)、[Sheet CI](.github/workflows/ci-joysheet.yml) 和 [Deck CI](.github/workflows/ci-joydeck.yml) 维护。中文显示需安装适当的 CJK 字体。Deck 的外部 HTML／RTF 剪贴板适配在 X11 使用 `xclip`，在 Wayland 使用 `wl-clipboard` 提供的 `wl-paste`，对应命令须在 `PATH` 中。Sheet 的 macOS／Linux PDF 预览使用 Poppler 的 `pdftoppm`。

### 获取源码并启动产品

```sh
git clone https://github.com/willmove/joyoffice.git
cd joyoffice
cargo fetch --locked

cargo xtask build joyword
cargo xtask build joysheet
cargo xtask build joydeck

cargo xtask run joyword -- assets/samples/mixed-media.docx
cargo xtask run joysheet -- --sample sales
cargo xtask run joydeck -- tests/fixtures/joydeck/r1-preview.pptx
```

按需选择产品即可，无需构建全部三款。产品构建或运行命令可增加 `--release` 使用优化配置。使用 `xtask` 运行入口，以准备 Deck 所需的平台辅助程序。

## CLI 与自动化

`--headless` 在构建／运行时选择产品 CLI，在产品检查中排除桌面包，同时检查所选普通／构建依赖闭包不包含 GPUI。

```sh
cargo xtask build joyword --headless
cargo xtask build joysheet --headless
cargo xtask build joydeck --headless

cargo xtask run joyword --headless -- inspect --session assets/samples/mixed-media.docx
cargo xtask run joysheet --headless -- eval "SUM(1,2,3)" --json
cargo xtask run joydeck --headless -- inspect tests/fixtures/joydeck/r1-preview.pptx
```

Word 提供检查、渲染、转换、邮件合并、恢复和 MCP；Sheet 提供工作簿检查、公式求值、转换、脚本和 MCP；Deck 提供检查、渲染、编辑脚本、迁移、导出、JSON control 和 MCP。

自动化须显式启用，并遵循各产品边界。Word 的无界面 MCP 需要授权读写根目录，桌面桥使用认证后的本地回环连接。Deck 的无界面 control／MCP 需要授权根目录，控制桌面视图也须显式开启。编辑前先查询已声明的能力和命令 schema：Deck 保留的 `joydeck-control-1` envelope 使用命令 schema **2**，旧字符串命令仅支持已声明的子集。协议详见 [Word 交接](docs/joyoffice/JOYWORD-R8-HANDOFF.md)、[Sheet 控制](docs/joysheet/CONTROL.md)和 [Deck 控制](docs/joydeck/CONTROL.md)。

## 检查与开发流程

```sh
python3 scripts/check_docs.py
python3 scripts/check_gates.py

cargo xtask check joyword
cargo xtask check joysheet
cargo xtask check joydeck
```

`check` 对产品所属包和实际使用的公共库运行格式检查、严格 Clippy 和测试。Deck 还检查导入来源、依赖与重构文档。`cargo xtask test <product>` 运行所选测试；`cargo xtask check <product> --headless` 检查无 GUI 范围。离线检查依赖图前，先获取锁定的依赖。

修改公共库、锁文件、resolver 或构建／feature 配置时，运行所有受影响产品及联合 feature 回归：

```sh
cargo clippy --workspace --locked --all-targets --features joydeck/validation,joysheet/automation -- -D warnings
cargo test --workspace --locked --features joydeck/validation,joysheet/automation
```

Deck 和 Sheet 的原生验收入口使用显式 `--validation` 选项，普通构建不启用这些验收入口。分支约定、模块边界、依赖精确固定、`unsafe_code = forbid` 和验证要求见 [AGENTS.md](AGENTS.md)。三款产品都在本仓库开发，已归档的独立目录和旧里程碑报告用于历史查证。

## 打包与发布

```sh
cargo xtask package joyword --release
cargo xtask package joysheet --release
cargo xtask package joydeck --release
```

原生开发包分别输出到 `dist/joyword/`、`dist/joysheet/` 和 `dist/joydeck/`。打包在宿主平台执行，包含所属产品二进制、资源、构建／源码指纹及依赖／许可元数据。跨目标构建／检查可使用 `--target`；当前打包仅支持原生平台，尚未实现纯 headless 包。

所有第一方工作区包默认继承根 `Cargo.toml` 中的 `[workspace.package]` 版本。产品保留各自的发布标签 `joyword-v*`、`joysheet-v*`、`joydeck-v*`，历史 `v*` 标签映射到 Word。从 JoyDeck 历史重构包 `0.8.0` 迁到 `0.1.4` 时须显式选择新包，语义版本比较会把 `0.1.4` 排在 `0.8.0` 之前。[发布工作流](.github/workflows/release.yml)

各产品桌面包均带有产品 LOGO 和平台图标。Windows 可执行文件资源、macOS 应用包和 Linux 桌面入口的维护方式见 [产品标识与图标](docs/joyoffice/BRANDING.md)。

开发包和 CI 通过不代表已具备正式发行资格。精确版本的许可正文缺口、签名／公证、安装器、文件关联、升级回退及设备验收按产品跟踪；要求尚未满足时，严格打包门禁可能拒绝生成产物。

## 平台证据与后续工作

| 平台 | 已记录范围与剩余工作 |
| --- | --- |
| Windows | 三款产品均有开发包与原生编辑检查。完整系统输入法、读屏、高 DPI／多显示器、打印设备及安装验收仍按具体场景推进。 |
| macOS ARM64 | 已构建并移位运行 Release 应用，有文件打开、编辑、保存重开的证据。Deck 另有系统输入和文字／表格检查。签名／公证与其余原生矩阵仍开放。 |
| Linux 与其他架构 | 无界面和部分桌面结果分别记录；Sheet 包含 UOS 25 X11 基础 GUI 覆盖。当前 Deck 的 Wayland／X11 桌面资格化和完整跨平台矩阵仍开放。CI／QEMU 结果与硬件验收分开记录。 |

代表性记录：[macOS 三产品验证](docs/joyoffice/validation/20261104-macos/README.md)、[原生对话框与 Word／WPS 往返](docs/joyoffice/validation/20261105-macos-desktop/README.md)、[近期编辑／导出检查](docs/joyoffice/validation/20261106-p2/README.md)、[剩余工作与 Linux CLI 回退](docs/joyoffice/validation/20261106-remaining-work/README.md)。UOS／麒麟等信创真机在条件具备时补验，不阻塞持续开发；未实测硬件明确保留为未验证。

当前重点是 Word 的旧数据／格式兼容与原生分发，Deck 的日常编辑／UI 对齐及复杂内容／自动化兼容，以及 Sheet 更广的互操作、原生交互和性能验收。旧 Word M0–M9、旧 Deck M0–M14 结果描述替换前实现，不能直接作为当前实现的验收结论。

## 文档导航

| 范围 | 入口 |
| --- | --- |
| 仓库 | [开发约定](AGENTS.md) · [架构](docs/joyoffice/ARCHITECTURE.md) · [Monorepo 历史](docs/joyoffice/MONOREPO-RESULT.md) |
| JoyWord | [R8 交接](docs/joyoffice/JOYWORD-R8-HANDOFF.md) · [当前状态](docs/joyword/r8/STATUS.md) · [验证](docs/joyword/r8/VALIDATION.md) |
| JoySheet | [开发指南](docs/joysheet/README.md) · [状态](docs/joysheet/STATUS.md) · [功能矩阵](docs/joysheet/MIGRATION.md) · [整合结果](docs/joyoffice/JOYSHEET-RESULT.md) |
| JoyDeck | [R9 交接](docs/joyoffice/JOYDECK-R9-HANDOFF.md) · [开发指南](docs/joydeck/DEVELOPMENT.md) · [任务](docs/joydeck/TASKS.md) · [界面对照](docs/joydeck/UI-PARITY.md) · [命令清单](docs/joydeck/COMMANDS.csv) |

## 许可与来源

项目根目录采用 [MIT 许可](LICENSE)。导入代码和资产保留各自原始许可与声明，详见 [WordCraft 来源记录](licenses/wordcraft/SOURCE.json)、[DeckCraft 归属](licenses/deckcraft/ATTRIBUTION-upstream.md)、[GridCraft／Sheet 归属](docs/joysheet/ATTRIBUTION.md)和 [GPUI 声明](licenses/gpui/NOTICE.md)。根许可不替代这些组件自身的条款，系统字体不作为项目资产提交或再分发。
