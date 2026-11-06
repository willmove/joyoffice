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

布局中位数为首次 53.1 ms、最后一个字符 0.13 ms。该语料覆盖跨行正文，旧 R6 的 Windows 实屏指标单独保留。复现用 `python3 scripts/joyword/verify-r8-long-doc.py CLI NEW_REPORT.json`。[五轮原始样本](evidence/macos-long-doc.json)

桌面补验在相同千页样例第 1,000 页记录了 15 次真实键盘输入至 GPUI 绘制提交，中位数 16.8 ms，最大值 41.1 ms。Word 16.114.1 和 WPS 12.1.29181 各完成两轮双向正文与结构往返，额外无包关系 XML 的保留仍有差距。物理屏幕呈现、跨对象编辑及同机基线继续待验。[桌面补验](../../joyoffice/validation/20261105-macos-desktop/README.md)

## Windows 兼容性复验（2026-11-05）

在 `099bce42` 派生的 `codex/joyword-r8-completion` 分支使用 Rust 1.98.1、PowerShell 7 和 Windows x64。用 `cargo xtask package joyword` 构建调试开发包，核对 ZIP 校验值后解包到仓库外的中文空格路径，以下 CLI 回归使用包内二进制。[归档和验证证据](evidence/windows-cli-compatibility.json)

| 范围 | 自动验证 |
| --- | --- |
| 产品测试 | `cargo xtask check joyword --target x86_64-pc-windows-msvc` 通过格式检查、生产 Clippy 和 3,487 项 Rust 测试，2 项按原设置忽略。 |
| 旧命令 | `python scripts/joyword/verify-r8.py CLI --mutations 64` 通过现有 DOCX 往返、PNG／PDF 导出及新增的合并、格式交换、显式字体和归档测试。64 个损坏样例中 21 个解析、43 个干净拒绝。旧版与 R8 的第 3 条邮件预览 JSON 逐字节一致，三份合并结果均独立重开。 |
| 恢复与 schema 1 | 包内 CLI 对照 `00fb0687` 的 CLI，14 份 DOCX 的 `inspect` 和 `text` 结果一致。5 条恢复路径读回，4 种损坏或丢弃情形及覆盖请求均拒绝，旧日志及其源文件的散列一致。 |
| 格式交换 | 明确源格式、只读损失计划、未确认时返回 2、隐藏 HTML 拒绝、确认后发布以及不覆盖通过。转换保留段落文字和目标格式支持的基本字符属性。 |
| 显式字体 | PNG 按指定 face 和 144 DPI 导出，报告中的字体散列与实际字节一致。缺字、已有文件拒绝，PDF/A-2b 同输入和年份重复导出字节一致。另用 `r8_render_goldens` 复核现有 144 DPI PNG／PDF 金样，全部匹配。 |
| PDF/A-2b | veraPDF 1.30.2 检查英文样例、中文邮件模板、编号、表格、分页、图片和修订共 7 份产物。每份通过 144 条规则，失败规则与失败检查均为 0。pypdf 6.21.2 严格读回每份 PDF，英文与中文邮件模板核对提取文字。 |
| DOCX 保真 | 两次独立编辑、写出和重开保留 `vendor/opaque.xml` 的原字节与内容类型，不合成关系。当前覆盖无入站或出站关系、合法路径、且位于标准 Word／签名部件目录之外的厂商 XML。 |
| 三产品回归 | `cargo test --locked --target x86_64-pc-windows-msvc -p joysheet-engine -p joysheet-xlsx -p joydeck-shell -p joydeck-pptx` 通过 962 项。 |
| 工具与文档 | `check_docs.py`、`check_gates.py` 和门禁反例通过。Python 共运行 152 项，2 项按原设置跳过。 |

使用千页脚本的同一语料生成器，在 Windows x64 调试 CLI 上执行 5 轮打开、末页逐字编辑、保存、重开和渲染。每轮仍为 1,000 页，源 DOCX 散列一致。进程耗时中位数分别为 193.8、219.5、206.0 和 214.4 ms，最后一个字符的布局中位数为 1.13 ms。本轮采集 CLI 进程耗时，物理呈现和峰值内存待测。[原始样本](evidence/windows-long-doc.json)

Windows 桌面复验时，computer-use 的 Windows 原生管道两次返回 `os error 2`。WSL2 的 `JoyWord-R7-Ubuntu` 实例引用的 `ext4.vhdx` 路径不存在，返回 `ERROR_PATH_NOT_FOUND`。本轮的 Windows IME、读屏、实屏交互及 Linux 原生验收保持待验。之前的原生报告按当时的产物与环境保留。

## Windows 格式与编辑复验（2026-11-05）

在 `69880cc8` 派生的 `codex/joyword-r8-export-compat` 分支使用 Windows x64、Rust 1.98.1 和 PowerShell 7。`cargo xtask check joyword --target x86_64-pc-windows-msvc` 通过格式检查、生产 Clippy 和 3,492 项 Rust 测试，2 项按原设置忽略。`cargo xtask build joyword --target x86_64-pc-windows-msvc` 及文档、边界门禁均通过。

| 范围 | 自动验证 |
| --- | --- |
| EPUB CLI | `python scripts/joyword/verify-r8.py CLI --out NEW_DIR --mutations 64` 验证零写入计划、明确接受损失、输出字节可复现、原件保留、拒绝覆盖、无效日期、修订和未知正文拒绝。对历史 EPUB 书稿及中文邮件模板逐段对照 DOCX 文本，核对图片原字节、ZIP 首项及导航目标。 |
| EPUB 3.3 | EPUBCheck 5.4.0 独立校验 `book.epub` 和 `template.epub`。两份均为零错误、零警告。[EPUBCheck 5.4.0](https://github.com/w3c/epubcheck/releases/tag/v5.4.0) |
| 桌面归档 | 用桌面入口共用的 `ArchivalPdf::prepare` 和 `publish` 从 R8 会话写出测试副本。veraPDF 1.30.2 的 PDF/A-2b 配置通过 144 条规则，失败规则和失败检查均为 0。pypdf 6.21.2 严格读回并核对文本。外壳用例覆盖未保存编辑快照、取消、后缀、拒绝覆盖和读回。 |
| 归档与 EPUB 确认 | GPUI 无头用例确认修改文档或取消后，请求不再有效。实际路径选择前后都复查文档标识、编辑版本和呈现版本。 |
| 查找与替换（JW-406） | GPUI 无头用例从实际根视图触发 Ctrl+F、Ctrl+H 与 F5。跨格式 run 的中文和组合字符可全部替换并一次撤销。 |
| 页面设置（JW-407） | 外壳和引擎用例覆盖度量单位、方向、栏数、无效页边距的原子拒绝、跨节修改时保留各节已有属性、撤销与 DOCX 保存重开。CLI 验证 175% 缩放保持 720 × 540 pt，并核对分栏和页边距的 OOXML 写出。pypdf 独立读回相同的 PDF 页框。 |

同一 CLI 复跑 64 个确定性 DOCX 损坏变体，21 个解析、43 个干净拒绝。

本轮独立校验产物的 SHA-256 如下。

| 产物 | SHA-256 |
| --- | --- |
| `book.epub` | `6abf5512580111f7f681481efb9b26393f36edcc5b93d514702b8f9da146b061` |
| `template.epub` | `1c9400d68ac0c17f756f8bb69d74232fa3850431011437bce68f3825a0315657` |
| `desktop-archive.pdf` | `e1fd7e5447e352c072d68691d540fadc9ec105e414e1a3a7f3aba4d7d33aeab9` |

本轮的 Windows computer-use 原生管道仍返回 `os error 2`，注册的 WSL2 实例仍缺少 `ext4.vhdx`。桌面导出确认、原生文件选择、阅读器显示、新编辑入口的实屏操作及 Linux 原生行为保持待验。

## Windows 系统集成与回退复验（2026-11-05）

在 `d31c0ce3` 派生的 `codex/joyword-r8-system-integration` 分支开发，使用 Windows x64、Rust 1.98.1 和 PowerShell 7。`cargo xtask check joyword --target x86_64-pc-windows-msvc` 的格式检查、生产 Clippy 和 3,495 项 Rust 测试通过，2 项按原设置忽略。文档与边界门禁通过，另有 58 项 Python 门禁用例通过。

| 范围 | 实现与验证 |
| --- | --- |
| JW-414/JW-705 系统打印 | 两页会话选择第 2 页，经 PDF、WinRT 位图和 WPF 分页器得到 1 页，非法页范围拒绝。WinRT 接收设备独立像素，导出后按实际像素复查并在超限时缩小重试。独立辅助进程的系统窗口标题为「打印」，进程在提交前结束。Windows computer-use 原生管道返回 `os error 2`，桌面实屏入口、对话框取消、实际提交和出纸待验。[WinRT 输出尺寸契约](https://learn.microsoft.com/en-us/uwp/api/windows.data.pdf.pdfpagerenderoptions.destinationwidth) |
| JW-515 桌面控制 | 从仓库外启动候选包。拒绝缺少和错误密钥、错误文档、旧版本和文件命令。正确请求写入一次，`joyword-cli mcp --connect 127.0.0.1:PORT` 的 stdio 工具从活动桌面会话读回中文标记。 |
| JW-514 本地 Agent | `agent` feature 提供本地提案。可选构建的 Clippy 与 11 项 GPUI crate 测试通过，覆盖默认关闭、切换开关后拒绝旧代次、拒绝变更选区、确认前只读、阻止直接命令和重复接受、带 AI 作者的修订及一次撤销。网络 Provider 及其流式取消、真实策略／凭据和桌面实屏待验。 |
| JW-806 复杂恢复 | V2 用例在含图片和表格的 DOCX 上重放两处局部编辑，检查媒体保留、DOCX 保存重开及旧日志字节。复杂字段投影和跨结构的文本重放继续单列。 |
| JW-801/JW-812 便携回退 | `python scripts/joyword/verify-r8-rollback.py PREVIOUS.zip CANDIDATE.zip FIXTURES NEW_DIR` 在仓库外的中文路径完成坏包拒绝、换版、V3 快照原样另存和旧版回退。旧 GUI 可启动，旧 CLI 可重开原件和恢复副本，5 个输入文件的 SHA-256 与系统 DOCX 默认关联保持一致。正式安装器、注册、签名与卸载仍待验。 |

旧便携包 SHA-256 为 `78f4a4de98556279164230667a845e247ef26d37627a911738d3bead3132e09a`。候选包由 `cargo xtask package joyword --out dist/joyword/r8-system-verified` 生成，SHA-256 为 `093a284fdee8a7dd45fd287ecc56d0a4de53a9c74741d605537943eca90b4032`。JoyDeck 和 JoySheet 指定模块的联合回归共 965 项通过。共享 WPF 打印辅助程序独立通过选页分页验证。

候选包 CLI 复跑兼容脚本。14 份 DOCX 与 `00fb0687` 的检查结果一致，5 条旧恢复路径成功读回，4 种损坏或丢弃情形及覆盖请求被拒绝。64 个确定性变体中 21 个解析、43 个干净拒绝。

## Windows P0 字段兼容与便携回退（2026-11-06）

在 `0e9bac55` 基线上用 Rust 1.98.1 和 PowerShell 7 构建 Windows x64 调试候选包。使用 `cargo xtask package joyword --out dist/joyword/p0-field-compat-final`。候选 ZIP 的 SHA-256 为 `824eb2befaa86e1849be56ad39ce2528cd5be0a0014b281e354bdf459b2f0c88`。

`cargo xtask check joyword --target x86_64-pc-windows-msvc` 通过格式检查、生产 Clippy 和 3,495 项 Rust 测试，2 项按原设置忽略。文档检查通过。

| 任务 | 当次验证 |
| --- | --- |
| JW-806、JW-808 字段与恢复 | 仓库外中文空格路径的包内 CLI 与旧版 `00fb0687` 比较 14 份既有 DOCX 和 5 份生成的字段 DOCX，schema 1 报告逐字段一致。生成语料含拆分指令、无缓存页码、嵌套字段、非纯文本缓存和未识别简单字段。3 组 V3 字段快照通过原样另存，正文失配均拒绝发布，日志和 DOCX 输入保持原字节。5 条已有旧恢复成功路径、损坏、丢弃、冲突与覆盖拒绝均复跑。 |
| JW-808 格式与桌面 | 包内 `verify-r8.py` 通过 DOCX／PNG／PDF、邮件合并、转换损失确认、PDF/A-2b 和 EPUB 回归。64 个变体中 21 个解析、43 个干净拒绝。用同一轮代码的桌面构建实屏从「文件」确认 EPUB 转换差异并在原生保存框写入副本。独立 XML 读回确认正文、表格和图片，EPUBCheck 5.4.0 返回零错误、零警告。产物 SHA-256 为 `54307b5dbd867d2f3cf7dc291fc269c628a8efc400ec2a1d7e1689f60c1c6ca7`。Windows 实屏打开了系统打印框，取消状态回传、队列提交和出纸尚未验证。 |
| JW-812 便携换版 | 使用已存档的 `78f4a4de…` 旧 ZIP。坏包校验失败。模拟旧目录已移开、候选目录消失，旧 GUI 和 CLI 均恢复。实际切换后，候选包的认证控制和 MCP 读回通过，V3 快照原样另存。主动回退后旧 GUI 启动，旧 CLI 读回原件和恢复副本。5 个输入文件的 SHA-256 与 Windows DOCX 默认关联保持一致。 |

可复跑入口为 `scripts/joyword/verify-r8-compat.py CLI FIXTURES OUT --baseline LEGACY_CLI` 和 `scripts/joyword/verify-r8-rollback.py PREVIOUS.zip CANDIDATE.zip FIXTURES OUT`。旧日志由 `scripts/joyword/legacy-recovery-fixtures.rs` 在 `00fb0687` 生成。

P0 总验收仍开放。正式安装、系统注册与回退，原生归档导出实屏、打印取消结果与实际队列，远程界面操作及 Agent Provider／凭据需继续验证。全仓 `check_gates.py` 在基线的 JoyDeck Linux 剪贴板文件报出进程访问登记和测试断言登记问题。

## 待验

- macOS DOCX 系统打开和原生打开／另存为面板均已实测。macOS IME 与读屏、集成后的 Linux ARM64 原生桌面、信创真机与托管 CI 待验。[原生对话框补验](../../joyoffice/validation/20261105-macos-desktop/README.md#原生文件对话框)
- Windows 拼音和读屏沿用独立版证据，R8 包复验、Word/WPS 的未知部件保真、安装器安装／卸载与文件关联见 [交接任务](../../joyoffice/JOYWORD-R8-HANDOFF.md)。
- 复杂旧恢复语料、旧新版固定金样差分、剩余 CLI/API 和 Agent 的差距见 [宿主任务状态](STATUS.md)。
- JoyDeck 开发包的清单列出了未自动收集到的依赖许可文本，发布验收时仍需补齐对应记录。

