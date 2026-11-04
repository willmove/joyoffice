# JoyWord R8 交接

后续开发、验证与打包统一在 JoyOffice 仓库进行。工作分支为 `codex/joyword-r8-integration`，替换前基线是 `00fb06872d38bbbfc26f4dbc3404d6810b72aabb`。原独立目录退出开发。独立版导入来源和精确指纹随仓库维护，见 [来源记录](../../licenses/joyoffice/JOYWORD-R8-SOURCE.json)。

## 工作入口

- `apps/joyword`、`apps/joyword-cli` 与 `crates/joyword-*` 是产品代码。桌面和 CLI 使用 `joyword-shell::reader::Reader` 与同一个 `joyword_engine::Session`。GPUI 仅在桌面边界加载。
- GPUI Kit 0.8.1、GPUI Pre 0.3.10 与 AccessKit Windows 补丁沿用宿主版本。使用宿主的 `joyoffice-ui` 与 `joyoffice-opc`。Word 外壳用 OPC 预检，完整 DOCX reader/writer 保有专用包处理。JoyDeck 来源核查引用的 Noto Emoji 资产与 OFL 留在原位置。
- [迁移路线图](../joyword/r8/migration/docs/joyword/PLAN.md)、[112 项任务账本](../joyword/r8/migration/docs/joyword/tasks.json) 和 [R6/R7 笔记](../joyword/r8/migration/docs/joyword/R6-R7.md) 是导入前快照。[独立版平台验证记录](../joyword/r8/migration/evidence/R7/native-validation.json) 中的路径和状态记录当时环境。替换前的 [JoyWord M0–M9 规格](../joyword/README.md) 仍可用于兼容差距核对。
- Windows 使用 PowerShell 7。常用入口为 `cargo xtask check joyword`、`cargo xtask test joyword --headless`、`cargo xtask package joyword`。本机开发包按产品归入 `dist/`。全套验证范围记录在 [R8 验证](../joyword/r8/VALIDATION.md)。

## 接口、数据和系统集成

| 契约 | R8 状态与迁移方式 |
| --- | --- |
| 产品、文件打开 | 保留 `JoyWord`、`joyword` 和产品版本 0.1.1。桌面接受 Unicode 文档路径与 `-- PATH`。Windows 本机的 DOCX 默认处理程序为 `Word.Document.12`。便携包通过桌面「打开」或命令行接收路径，安装器注册和系统默认切换留待发布验收。macOS bundle id 保留 `dev.joyoffice.joyword`。 |
| 偏好 | 配置根仍为 Windows `%APPDATA%/JoyWord`、Linux `$XDG_CONFIG_HOME/joyword` 或 `~/.config/joyword`、macOS `~/Library/Application Support/JoyWord`。可用 `JOYWORD_CONFIG_DIR` 隔离验证。活动文件为 `migration-preview/preferences.conf`，缺失时只读旧根目录的 `preferences.conf`。语言和迁移版主题生效。作者名接入审阅，最近文档接入文件页。作者设置界面、单位、页面预设和旧自动保存间隔的行为继续待接。 |
| 恢复与原件保存 | 桌面「打开」或 `joyword -- PATH.jwr` 可读旧日志，CLI 为 `recover [--] JOURNAL [--save NEW.docx]`。打开任一标准槽名会检查整个目录。日志以只读方式打开，恢复进入独立 R8 编辑会话，用户选择新 DOCX 路径后才写入副本并读回。完整快照正文核对通过且未经编辑时原样另存。当前恢复目录继续使用 `migration-preview/r3-recovery`。启动会检查旧配置目录的已发布日志，用户可从恢复提示显式打开副本。 |
| CLI 与自动化 | `inspect\|text [--max-text-bytes N] [--compatibility] [--] PATH` 恢复 schema 1、UTF-8 字节上限和退出码 0/2/3。R8 的会话检查与原始正文可用 `inspect\|text --session [--] PATH`。新增命令仍可用 `render INPUT OUTPUT.png --page 1`、`convert INPUT OUTPUT.pdf` 和 `run INPUT --cmd COMMAND=JSON --save NEW.docx`。MCP 为有显式目录边界的 stdio，旧桌面验证控制协议与桌面桥接待接。 |
| 格式与 AI | 新引擎包含 DOCX、PDF、图像和若干文本格式的交换，也提供编辑、引用、审阅和合并命令。旧 CLI 的 EPUB、PDF/A-2b、独立 merge/exchange 参数、系统打印完整流程和旧 Agent 桌面集成列为兼容项。`joyword-agent` 保留独立契约测试，旧引擎事务适配器待重接。 |
| 旧验证门禁 | 当前 CI 使用新布局、渲染、内核与保存测试、实际 DOCX 往返、独立 PDF 读回，以及确定性损坏输入。覆盖引导 fuzz、固定金样和旧编辑行为登记对应旧 API，在替换前基线复现。恢复这些门禁是后续任务。 |

### 旧恢复格式

`JWREC001` 为 `recovery.jwr` 的基线 DOCX 与 UTF-8 正文。双槽 `JWREC002` 增加代数、文档版本与丢弃记录，`JWREC003` 再附加完整快照。整条记录用 CRC-32 校验。读取时共享锁只使用目录里现有的锁文件。选择最高有效代数，单槽损坏时显示降级提示，同代冲突、全部双槽损坏、丢弃记录和快照正文不匹配会报错。嵌入的基线或快照足以恢复，源 DOCX 路径无需可用。

V1/V2 以原 DOCX 为基线，保留相同前后文并通过 R8 编辑事务重放正文差异。当前对涉及内联对象、跨表格单元格、已识别的保护或不支持内容的重放给出拒绝诊断。大型包仍受 R8 读入预算限制。V3 按正文投影核对快照。旧模型与 R8 模型对复杂字段和部分对象的表示差异可能导致核对拒绝，此类场景待扩展语料。恢复后继续编辑并保存走当前 DOCX 写入器，首次原样另存可保留完整快照包。

### 其余旧 CLI 接口

| 旧调用 | 可适配项 | 仍需补齐 |
| --- | --- | --- |
| `render FILE --font PATH:FACE --output DIR [--dpi N] [--page N]` | R8 有单页 PNG，`--page` 同为一基页号，DPI 可换算为 `--scale`。 | 指定字体及 face、全页目录输出和文件命名、无覆盖发布、旧报告与固定像素金样。 |
| `convert FILE --to pdf\|pdfa-2b --font PATH:FACE --output FILE.pdf` | 普通 PDF 可用 R8 的 `convert INPUT OUTPUT.pdf`。 | PDF/A-2b 和 `--archive-year`、显式字体与嵌入约束、旧 schema 和确定性输出。 |
| `merge --template DOCX --data CSV [--map FIELD=COLUMN] (--preview-row N\|--output-parent DIR)` | R8 编辑会话已有收件人导入、映射、筛选和合并导出，可接旧参数解析。 | 旧行号、CSV 和列错误契约、逐收件人文件命名与整批安全发布、旧预览和报告。 |
| `exchange FILE --from FORMAT --to FORMAT (--plan\|--output FILE [--accept-losses])` | R8 导入／导出层已有 DOCX、TXT、Markdown、HTML、RTF 和新文件发布。 | 显式源格式、只读计划、损失清单、确认后导出及 schema 1；扩展名转换不足以承接这些契约。 |

旧 `inspect` 的诊断由原导入器生成。本轮已对齐已验证语料的字段值和正文。复杂字段、嵌套兼容标记及未覆盖的诊断分类继续做旧新版差分。

## 已验证的 R8 门禁

- JW-805/807/809/899 的 DOCX ZIP/XML 和 `JWREC001/2/3` 解码已接入覆盖引导 fuzz 与持续 CI 门禁。macOS 复跑 81 次探针无失败。[复验](validation/20261105-macos-open/README.md)
- JW-807 的固定字体字节、face 与 144 DPI PNG／PDF 金样已核对审阅、表格、图片和字段，新增 macOS CI 比对。[复验](validation/20261105-macos-open/README.md)

## 下一步

| 优先级 | 任务 | 完成条件 |
| --- | --- | --- |
| P0 | 旧数据与安全（JW-806/812） | 旧目录发现、已保存的作者名和最近文档已接入。继续复杂旧日志差分、作者设置界面及旧自动保存间隔。 |
| P0 | CLI 与格式兼容（JW-808） | 按上表迁移旧 `render`、`convert`、`merge`、`exchange`，扩展 schema 1 诊断差分，再核对 EPUB、打印、桌面控制和 Agent 授权。 |
| P1 | Word/WPS 双向往返（R5/R6） | 记录应用版本，用含表格、图片、页眉、修订和未知 OOXML 的语料分别往返两次，检查打开、编辑和部件保留。 |
| P1 | 千页 DOCX 性能复验（R5/R6） | 固定语料与硬件环境，记录首屏、跨行／对象编辑重排、输入到绘制延迟及峰值内存，并与旧记录比对。 |
| P1 | 公共基础（JW-803） | 评估 DOCX 包层、CFB/DrawingML/OMML 和字体服务在共享库中的复用。用格式保真与性能回归验证逐项收敛。 |
| P1 | 平台与发布（R7/R8） | macOS ARM64 Release 构建、移位启动、首屏、键鼠编辑及保存重开已验证。DOCX 系统冷启动与运行中打开已通过，原生文件对话框的完整往返继续待验。继续 Linux x64/ARM64 原生、Windows 中文 IME/读屏、高 DPI、Linux 无障碍及安装器、文件关联、签名与卸载。[macOS 记录](validation/20261104-macos/README.md) |

R8 各项功能和平台分别记录证据。独立版的通过记录、联合编译和三应用冒烟均有独立范围。

