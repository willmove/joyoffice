# JoyOffice

JoyOffice 是使用 Rust 和 GPUI Kit 的桌面 Office 套件。JoyWord 编辑 DOCX，JoySheet 编辑 XLSX，JoyDeck 编辑 PPTX。三应用在同一仓库维护，共用工具链、锁文件和公共 UI，各自有独立的版本、应用入口和开发包。

| 产品 | 版本 | 开发入口 |
| --- | --- | --- |
| JoyWord | 0.1.1 | [R8 交接、兼容表和后续任务](docs/joyoffice/JOYWORD-R8-HANDOFF.md) |
| JoySheet | 0.1.0 | [开发入口](docs/joysheet/README.md) · [整合记录](docs/joyoffice/JOYSHEET-INTEGRATION.md) |
| JoyDeck | 0.8.0 | [R9 交接](docs/joyoffice/JOYDECK-R9-HANDOFF.md) · [当前状态](docs/joydeck/STATUS.md) |

JoyWord R8 已在这个仓库内替换旧实现，后续工作从 R8 交接开始。旧 Word 的 [M0–M9 规格和验收](docs/joyword/README.md) 可用于核对需求与历史行为。当前开发包与平台的验证范围见各产品的交接文档。

## 构建和运行

工具链固定 Rust 1.98.1，GPUI Kit 固定 0.8.1，GPUI Pre 固定 0.3.10。应用在 `apps/`，产品模块在 `crates/joyword-*`、`crates/joysheet-*` 和 `crates/joydeck-*`，公共模块在 `crates/joyoffice-*`。

```sh
cargo xtask build joyword
cargo xtask build joysheet
cargo xtask build joydeck
cargo xtask run joyword -- assets/samples/mixed-media.docx
cargo xtask run joysheet -- --sample sales
cargo xtask run joydeck -- tests/fixtures/joydeck/r1-preview.pptx
```

## 检查与打包

```sh
python scripts/check_docs.py
python scripts/check_gates.py
cargo xtask check joyword
cargo xtask check joysheet
cargo xtask check joydeck
cargo xtask package joyword
cargo xtask package joysheet
cargo xtask package joydeck
```

开发包在 `dist/` 下按产品分目录。Linux 需要可用的 GPUI 显示与 Vulkan 环境。JoyDeck 的外部 HTML／RTF 粘贴在 X11 上使用 `xclip`，在 Wayland 上使用 `wl-clipboard` 提供的 `wl-paste`。请将对应命令安装到 PATH。macOS 原生构建、签名和文件关联的验证安排见交接文档。正式发布由 [发布工作流](.github/workflows/release.yml) 按产品路由。

- [仓库开发约定](AGENTS.md)
- [Monorepo 整合记录](docs/joyoffice/MONOREPO-RESULT.md)
