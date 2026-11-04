# JoyWord R8 验证

2026-11-04 在 JoyOffice 的 `codex/joyword-r8-integration` 工作树执行。测试输入均为仓库自带样例或其副本。平台结果按本次集成重新记录。

## 旧恢复与 CLI 兼容切片

开发起点为 `c42060e1`。旧行为以 `00fb06872d38bbbfc26f4dbc3404d6810b72aabb` 的独立工作树复现。验证和包内执行结果见 [兼容性证据](evidence/legacy-compat.json)。

| 开发包 | 解包后验证 |
| --- | --- |
| `dist/joyword/r8-compat-final/joyword-0.1.1-windows-x86_64.zip` | Windows x64、PowerShell 7。桌面打开 V3，显示 Unicode 正文、粗体和表格，确认另存后关闭并重启打开同一 DOCX。另存文件与旧快照 SHA-256 一致。JoySheet 显示 XLSX 样例的 Sheet1 及「打印边界」，JoyDeck 显示 PPTX 三页缩略图及第一页。 |
| `dist/joyword/r8-compat-final/joyword-0.1.1-linux-x86_64.tar.gz` | Ubuntu WSL2 x64，WSLg 软件渲染。检查 ELF、动态链接和归档 SHA-256。JoyWord 从 V3 日志绘制首屏，包内 CLI 另存，桌面重开核对正文和表格。JoySheet 显示 XLSX 样例，CLI 读回 B3 为 24。JoyDeck 显示三页 PPTX，运行日志和 CLI 页数一致。 |

两包的 CLI 均验证了 V1 单文件、V2 双槽最高代数、V3 富文本、单槽 CRC 损坏回退、源 DOCX 删除后恢复、另存和重开。截断与 CRC 损坏并存、同代冲突、丢弃记录、快照正文不匹配和覆盖请求均被拒绝。恢复前后逐个比对旧日志、锁文件及仍存在的源 DOCX 的 SHA-256。R8 会话路径另跑 DOCX 编辑／重开、PNG 和普通 PDF 导出、64 个确定性损坏变体，21 个可解析、43 个干净拒绝。

旧版和 Windows 最终包的 schema 1 逐字段对照覆盖 14 个 DOCX，包含最小语料、表格／分页／图片／修订、中文混合媒体和 handbook。文本和报告字段均一致。两包也检查了参数前后位置、`--` 字面文件名、可选兼容字段、UTF-8 截断、错误流与状态码。以最终 Windows 包为子进程重跑了旧 `commands.rs` 中 9 项 `inspect`／`text` 测试，旧 `render`／`convert` 的缺参断言从该次重跑剔除。旧版自身另通过恢复测试和 `visual`、`font_refusal`、`mail_merge`、`exchange` 测试。余下接口迁移见 [R8 交接](../../joyoffice/JOYWORD-R8-HANDOFF.md)。

### 复现命令

在 JoyOffice 根目录用 PowerShell 7 生成旧样例。V2/V3 由旧版写入器和桌面 Office 会话生成，V1 采用旧版解码测试中的编码。

```powershell
$repo = (Get-Location).Path
$baseline = Join-Path $repo 'target/r8-compat-baseline'
git worktree add --detach $baseline 00fb06872d38bbbfc26f4dbc3404d6810b72aabb
New-Item "$baseline/crates/joyword-shell/examples" -ItemType Directory -Force
Copy-Item scripts/joyword/legacy-recovery-fixtures.rs "$baseline/crates/joyword-shell/examples/"
Push-Location $baseline
$env:CARGO_TARGET_DIR = Join-Path $repo 'target/r8-baseline-build'
cargo test --locked -p joyword-cli --test commands -p joyword-shell --test recovery_document --test recovery_formatted_text
cargo run --locked -p joyword-shell --example legacy-recovery-fixtures -- "$repo/target/r8-compat-fixtures"
Remove-Item Env:CARGO_TARGET_DIR
Pop-Location
```

在 Windows 和 Linux 各自执行 `cargo xtask package joyword --out dist/joyword/r8-compat-final`，解包后用 `python scripts/joyword/verify-r8-compat.py CLI FIXTURES NEW_RESULTS` 验证，Windows 加 `--baseline OLD_CLI` 做旧新版对照。运行 `python scripts/joyword/verify-r8.py CLI --mutations 64` 核对 R8 会话和导出。归档路径下配有 SHA-256 文件。

Windows 执行 `cargo xtask check joyword --target x86_64-pc-windows-msvc`，格式检查和生产 Clippy 通过，JoyWord Rust 测试 3,482 通过、2 忽略。跨产品执行 `cargo test --locked --target x86_64-pc-windows-msvc -p joysheet-engine -p joysheet-xlsx -p joydeck-shell -p joydeck-pptx`，962 项通过。文档与边界门禁 `python scripts/check_docs.py`、`python scripts/check_gates.py` 均通过。

## Windows x64

三个原生应用和 CLI 从同一工作区联合编译。三个便携开发包分别解压，桌面应用同时运行。最终归档中的 GUI 可执行文件与执行桌面重开检查的文件哈希一致。

| 应用 | 桌面流程 | 保存及重开结果 |
| --- | --- | --- |
| JoyWord 0.1.1 | 打开含中文、表格和图片的 DOCX，编辑标题并保存测试副本 | 重开显示新中文内容。独立检查 `word/document.xml`、表格和媒体部件均保留。使用仅有旧配置的隔离目录重开，读取 `ui.locale=en`。 |
| JoySheet 0.1.0 | 打开 XLSX，在 C2 输入 `=SUM(800,8)`，计算并保存 | 重开显示 808。独立检查 `sheet1.xml` 同时含原公式和计算值。 |
| JoyDeck 0.8.0 | 打开三页 PPTX，切到第二页，复制幻灯片并保存 | 重开显示四页。独立检查演示文稿 XML 中的页数为四。 |

[Windows 归档与校验值](evidence/windows-archives.json) · [桌面编辑的 OOXML 读回](evidence/windows-ooxml.json) · [测试统计](evidence/test-summary.json) · [依赖核查](evidence/dependency-audit.json)

JoyWord 的格式检查、生产 Clippy 与测试通过，3482 个用例通过，2 个忽略。JoySheet 的引擎／XLSX 和 JoyDeck 的外壳／PPTX 共 962 个回归用例通过。Python 工具测试 152 项，其中 2 项显式跳过旧实现专用验证。当前文档、组件边界、固定依赖和负例门禁通过。

Windows 调试包首次检查发现 JoyWord CLI 主线程栈溢出。修正后从最终包运行 DOCX 往返、PNG 和 PDF 导出、拒绝覆盖及损坏输入，64 个变体均正常解析或干净拒绝。三个 Windows 归档都核对了 SHA-256 和完整性。

## Linux x64

Ubuntu WSL2 联合构建三应用及 CLI。使用 WSLg 和 Mesa 软件 Vulkan 三应用同时启动，JoyWord 完成 DOCX 首屏绘制，JoySheet 初始化工作簿窗口，JoyDeck 打开三页 PPTX。此处覆盖启动与基本显示，Windows 表中的交互编辑验证单独记录。

三个解压后的 Linux 开发包通过 ELF、动态链接与 SHA-256 检查，并在 WSLg 同时启动。JoyWord 绘制已保存 DOCX 的首屏，JoySheet 初始化工作簿窗口，JoyDeck 打开四页 PPTX。JoyWord CLI 通过 DOCX 编辑、保存、重开、PNG 和 PDF 输出及 64 个损坏输入变体。JoySheet CLI 读回 C2 的 808，JoyDeck CLI 读回四张幻灯片。最终 JoyDeck 归档中的 GUI 和 CLI 哈希与冒烟测试文件一致。[Linux 归档与验证结果](evidence/linux-archives.json)

JoySheet 的 Debian 打包在 WSL 的 ext4 内完成，保留 Debian 控制目录权限。使用 `cargo xtask package joysheet --output <ext4 目标目录>` 可复现该路径。Debian 控制元数据为 `joysheet 0.1.0 amd64`。两个平台的 JoySheet 源码包都保留了工作区使用的 AccessKit 路径补丁。

## macOS ARM64

2026-11-04 在 Apple M4 上完成普通 Metal Release 构建和开发包校验。移位后的包内桌面程序创建原生窗口，通过启动参数处理中文空格路径。有效 DOCX 初始化 Reader 和隔离恢复目录，截断输入完成打开作业后未初始化 Reader。解锁后核对首屏的混合文字、表格和三色图片。鼠标定位并用键盘插入文本，撤销后标记消失、重做后恢复。普通保存显示替换确认，取消时源副本散列不变，确认后关窗并重启，独立核对正文、表格和媒体。

包内 CLI 完成混合媒体 DOCX 编辑、另存和重开，独立检查表格与媒体部件、源文件散列和拒绝覆盖。PNG 与普通 PDF 导出通过，独立 PDF 解析及 Poppler 文本读回通过。64 个确定性损坏变体中 21 个可解析，43 个干净拒绝。[本机报告与包指纹](../../joyoffice/validation/20261104-macos/README.md)

2026-11-05 复验覆盖 DOCX 系统文件打开、旧恢复目录扫描、覆盖引导解码和固定渲染金样，见 [macOS 补验](../../joyoffice/validation/20261105-macos-open/README.md)。

同日在 macOS ARM64 执行 `cargo xtask check joyword`，格式检查、生产 Clippy 通过，3,494 项测试通过、2 项忽略。恢复模块 26 项通过。新增用例验证 60 秒配置在第 10 秒不会写入、到期写入，以及关闭周期写入后仍可发现已有草稿。桌面通过当前偏好文件或旧目录回退读取 `autosave.minutes`。本轮使用自动测试验证调度，实屏等待周期尚未复验。

空白文档预设的回归读取了旧目录 A4 和活动目录 Letter，并验证 A4 的 DOCX 保存重开和旧配置字节保留。macOS ARM64 的全量 JoyWord 检查通过 3,495 项，2 项忽略。桌面「新建」共用该构造路径，实屏另存尚未复验。

「用户信息与单位」通过活动配置持久化当前作者名与度量单位。自动回归覆盖旧配置读取、中文作者、毫米设置、未知键保留、受保护文稿拒绝更改作者及配置写入失败时保留原会话。macOS ARM64 的全量 JoyWord 检查通过 3,496 项，2 项忽略。该表单的实屏应用及所有版式面板的单位显示仍待核对。

`cargo xtask package joyword --release --out dist/joyword/r8-macos-preferences` 构建的归档 SHA-256 为 `cd90ab5dfd97672e448d309b08e16e7c90c64f78c0d0a0de1f538483e6fc37c8`。解包到仓库外中文空格路径后，混合媒体 DOCX 往返、拒绝覆盖、PNG、PDF 结构和文本读回均通过。64 个确定性损坏变体中 21 个解析、43 个拒绝。含修订、页眉、图片和额外 XML 部件的 JWREC003 快照恢复后与快照字节一致，两份正文投影不符的日志均拒绝另存。

### 千页 DOCX

2026-11-05 在 Apple M4、24 GiB 内存、macOS 27.2 上用解包后的 ARM64 Release CLI 执行 5 轮。脚本从最小 DOCX 构造固定 1,000 页语料，末段自然换行。每轮打开原件，在末页逐字符追加 `adghk`，保存新 DOCX、重开、检查 OOXML 尾段并渲染末页。每轮页数均为 1,000，原件散列一致。

| 阶段 | 进程耗时中位数 | 峰值 RSS |
| --- | ---: | ---: |
| 打开并返回会话信息 | 61.8 ms | 40.4 MiB |
| 打开、编辑及另存 | 69.1 ms | 43.1 MiB |
| 重开另存稿 | 63.0 ms | 40.9 MiB |
| 打开及渲染末页 PNG | 68.5 ms | 46.5 MiB |

布局中位数为首次 53.1 ms、最后一个字符 0.13 ms。该语料覆盖跨行正文，旧 R6 的 Windows 实屏指标单独保留。macOS 的键盘事件至 GPUI 绘制、物理呈现和跨对象编辑未完成。复现用 `python3 scripts/joyword/verify-r8-long-doc.py CLI NEW_REPORT.json`。[五轮原始样本](evidence/macos-long-doc.json)

## 待验

- macOS 原生打开／另存对话框的完整往返、IME 与读屏待验。DOCX 系统文件打开已完成补验。集成后的 Linux ARM64 原生桌面、信创真机与托管 CI 待验。
- Windows 拼音和读屏沿用独立版证据，R8 包复验、完整 Word/WPS 互通、安装器安装／卸载与文件关联见 [交接任务](../../joyoffice/JOYWORD-R8-HANDOFF.md)。
- 复杂旧恢复语料、固定金样和覆盖引导 fuzz 的迁移，以及剩余 CLI/API 和 Agent 的差距见交接中的接口表。
- JoyDeck 开发包的清单列出了未自动收集到的依赖许可文本，发布验收时仍需补齐对应记录。

