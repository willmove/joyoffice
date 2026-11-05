# JoyOffice 三产品联合回归与 macOS 开发包

2026-11-05，PR #126 的代码提交 `ce8bd67` 在 Apple M4 / macOS 27.2 完成三产品构建、检查、测试和本机开发包验证。状态按 JD-905 记录，合并 CI 见 [PR #126](https://github.com/willmove/joyoffice/pull/126)。

| 本机检查 | 通过 | 失败 | 忽略 |
| --- | ---: | ---: | ---: |
| `cargo xtask check joydeck` | 899 | 0 | 0 |
| `cargo xtask check joyword` | 3,497 | 0 | 2 |
| `cargo xtask check joysheet` | 1,400 | 0 | 1 |
| `cargo test --workspace --locked --features joydeck/validation,joysheet/automation` | 5,791 | 0 | 3 |

使用各产品的 `cargo xtask package --release` 路由构建并移位到仓库外。Deck 使用验收特性。包内 CLI 分别完成 DOCX inspect、Deck R1 的 28 项调用及 Sheet 的 CSV → XLSX → 读回与求和。Sheet 的 `SUM(B2:B3)` 返回 12。三个归档散列核对一致，Deck 的 1,805 项、Sheet 的 2,558 项包内清单校验通过，Word 的 942 个 ZIP 文件项通过 CRC。JoyDeck CLI 的 normal/build 依赖闭包无 GPUI 或其他产品。[归档和产物指纹](verification.json)

Windows 和 Linux 实屏及正式发行资格由各自任务跟踪。
