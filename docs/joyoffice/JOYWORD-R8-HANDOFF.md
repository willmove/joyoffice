# JoyWord R8 交接

后续开发、验证与打包统一在 JoyOffice 仓库进行。初次整合使用 `codex/joyword-r8-integration`，替换前基线是 `00fb06872d38bbbfc26f4dbc3404d6810b72aabb`。原独立目录已归档。独立版导入来源和精确指纹随仓库维护，见 [来源记录](../../licenses/joyoffice/JOYWORD-R8-SOURCE.json)。

## 工作入口

- `apps/joyword`、`apps/joyword-cli` 与 `crates/joyword-*` 是产品代码。桌面和 CLI 使用 `joyword-shell::reader::Reader` 与同一个 `joyword_engine::Session`。GPUI 仅在桌面边界加载。
- GPUI Kit 0.8.1、GPUI Pre 0.3.10 与 AccessKit Windows 补丁沿用宿主版本。使用宿主的 `joyoffice-ui` 与 `joyoffice-opc`。Word 外壳用 OPC 预检，完整 DOCX reader/writer 保有专用包处理。JoyDeck 来源核查引用的 Noto Emoji 资产与 OFL 留在原位置。
- 在 [宿主任务状态](../joyword/r8/STATUS.md) 更新本仓库的进展。[迁移路线图](../joyword/r8/migration/docs/joyword/PLAN.md)、[112 项任务账本](../joyword/r8/migration/docs/joyword/tasks.json) 和 [R6/R7 笔记](../joyword/r8/migration/docs/joyword/R6-R7.md) 是导入前快照。[独立版平台验证记录](../joyword/r8/migration/evidence/R7/native-validation.json) 中的路径和状态记录当时环境。替换前的 [JoyWord M0–M9 规格](../joyword/README.md) 仍可用于兼容差距核对。
- Windows 使用 PowerShell 7。常用入口为 `cargo xtask check joyword`、`cargo xtask test joyword --headless`、`cargo xtask package joyword`。本机开发包按产品归入 `dist/`。全套验证范围记录在 [R8 验证](../joyword/r8/VALIDATION.md)。

## 接口、数据和系统集成

| 契约 | R8 状态与迁移方式 |
| --- | --- |
| 产品、文件打开 | 保留 `JoyWord`、`joyword` 和产品版本 0.1.1。桌面接受 Unicode 文档路径与 `-- PATH`。Windows 本机的 DOCX 默认处理程序为 `Word.Document.12`。便携包通过桌面「打开」或命令行接收路径，安装器注册和系统默认切换留待发布验收。macOS bundle id 保留 `dev.joyoffice.joyword`。 |
| 偏好 | 配置根仍为 Windows `%APPDATA%/JoyWord`、Linux `$XDG_CONFIG_HOME/joyword` 或 `~/.config/joyword`、macOS `~/Library/Application Support/JoyWord`。可用 `JOYWORD_CONFIG_DIR` 隔离验证。活动文件为 `migration-preview/preferences.conf`，缺失时只读旧根目录的 `preferences.conf`。语言和迁移版主题生效。作者名接入审阅，最近文档接入文件页。开发工具的「用户信息与单位」保存作者和度量单位，新会话读取这两项。各版式面板中的度量字段仍需逐项核对。桌面自动恢复按 `autosave.minutes` 写入草稿，默认 10 分钟，0 停止周期写入且可继续发现既有草稿。桌面新建空白页使用 `new.page` 的 A4 或 Letter 预设。 |
| 恢复与原件保存 | 桌面「打开」或 `joyword -- PATH.jwr` 可读旧日志，CLI 为 `recover [--] JOURNAL [--save NEW.docx]`。打开任一标准槽名会检查整个目录。日志以只读方式打开，恢复进入独立 R8 编辑会话，用户选择新 DOCX 路径后才写入副本并读回。完整快照正文核对通过且未经编辑时原样另存。当前恢复目录继续使用 `migration-preview/r3-recovery`。启动会检查旧配置目录的已发布日志，用户可从恢复提示显式打开副本。 |
| CLI 与自动化 | `inspect\|text [--max-text-bytes N] [--compatibility] [--] PATH` 恢复 schema 1、UTF-8 字节上限和退出码 0/2/3。R8 会话检查用 `inspect\|text --session [--] PATH`。无界面 MCP 继续使用显式目录边界。桌面可通过 `--control PORT` 启用认证后的活动文档操作，详见下文。 |
| 格式、打印和 Agent | CLI 已接入显式字体的 PDF/A-2b 和独立 `merge`、`exchange`、`epub`。桌面从当前会话导出 EPUB 和 PDF/A-2b 副本。Windows 文件页可选页后调用系统打印。可选 `agent` 构建支持本地选区提案、预览和修订事务。真实打印队列、其他平台打印、远端 Provider 与策略/凭据接入仍待验。 |
| 旧验证门禁 | 当前 CI 使用新布局、渲染、内核与保存测试、实际 DOCX 往返、独立 PDF 读回、确定性损坏输入、覆盖引导 fuzz 和固定渲染金样。旧编辑行为登记对应替换前 API，在历史基线复现。 |

### 旧恢复格式

`JWREC001` 为 `recovery.jwr` 的基线 DOCX 与 UTF-8 正文。双槽 `JWREC002` 增加代数、文档版本与丢弃记录，`JWREC003` 再附加完整快照。整条记录用 CRC-32 校验。读取时共享锁只使用目录里现有的锁文件。选择最高有效代数，单槽损坏时显示降级提示，同代冲突、全部双槽损坏、丢弃记录和快照正文不匹配会报错。嵌入的基线或快照足以恢复，源 DOCX 路径无需可用。

V1/V2 以原 DOCX 为基线，在段落边界保持且各处都有共同前后文时，最多逐段重放 128 个局部差异。表格单元格和内联对象可保留在独立编辑之间。涉及对象本身、跨单元格替换、保护或已识别的未知正文仍拒绝。V3 先核对当前正文投影。遇到字段表示差异时，可以用旧版内联标记投影精确复核已覆盖的结构。匹配后首次原样另存完整快照包。其他未覆盖的对象或正文差异仍可能导致拒绝。

### 桌面控制、打印与本地提案

- 桌面用 `joyword --control PORT -- INPUT.docx` 显式开启，仅监听 `127.0.0.1`。调用者和应用共用环境变量 `JOYWORD_CONTROL_KEY`，配置为 32–128 位可打印 ASCII，建议使用新生成的随机密钥。TCP 沿用一行 JSON 的 `id/method/params/key` 和 `id/ok/result|error`。先调用 `capabilities` 查询范围，再由 `document.inspect` 取 `doc` 和 `version`。编辑请求必须附上这两项。MCP 客户端入口为 `joyword-cli mcp --connect 127.0.0.1:PORT`，`execute` 可在命令参数中提供这两项。当前允许插入文字、撤销／重做、全选及移动到文首／文末。文件访问、远程界面操作及自动生成密钥文件留待后续。
- Windows 的「文件 → 打印」从当前页面场景生成所选页，单次最多 32 页。PowerShell 7 启动 Windows 打印对话框，打印机和设备选项在系统对话框中选择。返回取消、队列提交或驱动错误，队列提交与实际出纸分别验收。
- 用 `cargo build --locked -p joyword --features agent` 构建本地提案入口。默认关闭，显式启用后可对单个普通正文段落中的选区录入替换稿。确认前显示前后文字，接受时核对文档、版本、选区、授权代次和有效期。接受落为一条可撤销的编辑事务，修订作者为 `JoyWord AI`。该入口使用本地录入的提案，Provider 与凭据集成仍开放。

### 已迁入的旧 CLI 接口

| 旧调用 | 宿主行为 | 尚未验收 |
| --- | --- | --- |
| `render FILE --font PATH:FACE --output DIR [--dpi N] [--page N]` | 按实际字体字节及 face 布局，默认 144 DPI，`--page` 从 1 开始。输出 `page-N.png` 和 schema 1 报告，拒绝缺字及覆盖。 | 旧引擎与 R8 的像素差分矩阵和更多复杂字体组合。 |
| `convert FILE --to pdf\|pdfa-2b --font PATH:FACE --output FILE.pdf` | 使用显式字体，PDF/A-2b 要求 `--archive-year YYYY`。字体不允许嵌入、缺字或写入失败时拒绝发布。 | 归档合规按实际产物独立验证，继续扩展字体和语料矩阵。 |
| `merge --template DOCX --data CSV [--map FIELD=COLUMN] (--preview-row N\|--output-parent DIR)` | `{{field}}` 模板、严格 UTF-8 CSV、从 1 开始的预览及每条一份 `letter-NNNN.docx`。在独立新目录生成，出错清理已写文件。 | 桌面 R5 使用 Word MERGEFIELD 工作流。旧模板的更多故事范围、复杂格式和跨平台复跑待验。 |
| `exchange FILE --from FORMAT --to FORMAT (--plan\|--output FILE [--accept-losses])` | 支持 DOCX、TXT、Markdown、HTML、RTF 的段落子集。只读计划报告损失；拒绝未确认损失、隐藏内容和覆盖。 | 复杂结构及格式往返差分、桌面损失确认。 |
| `epub FILE (--plan\|--output NEW.epub --accept-losses)` | 输出 EPUB 3.3 重排正文、标题导航、简单表格及内嵌 PNG／JPEG。保留 schema 1 计划、显式接受转换损失和禁止覆盖。支持 `--title`、`--language`、`--modified`。 | 更多 DOCX 差分语料、真实阅读器和桌面导出面板。[EPUB 指南](../joyword/guide/epub.md) |

旧 `inspect` 的诊断由原导入器生成。Windows 候选包已对齐 19 份语料的 schema 1 报告，包括拆分指令的复杂页码字段、无缓存页码、嵌套字段、非纯文本缓存和未识别简单字段。嵌套兼容标记及其他诊断分类继续做旧新版差分。[本轮验证](../joyword/r8/VALIDATION.md#windows-p0-字段兼容与便携回退2026-11-06)

## 已验证的 R8 门禁

- JW-805/807/809/899 的 DOCX ZIP/XML 和 `JWREC001/2/3` 解码已接入覆盖引导 fuzz 与持续 CI 门禁。macOS 复跑 81 次探针无失败。[复验](validation/20261105-macos-open/README.md)
- JW-807 的固定字体字节、face 与 144 DPI PNG／PDF 金样已核对审阅、表格、图片和字段，新增 macOS CI 比对。[复验](validation/20261105-macos-open/README.md)

## 下一步

| 优先级 | 任务 | 完成条件 |
| --- | --- | --- |
| P0 | 旧数据与安全（JW-806/812） | Windows 便携包已实测坏包拒绝、切换中失败复原和主动回退。V3 已覆盖多种字段快照的精确核对。继续其他对象与旧数据差分、偏好实屏及正式安装与文件关联回退。 |
| P0 | CLI 与格式兼容（JW-808） | 19 份 schema 1 报告与旧 CLI 一致。Windows EPUB 实屏导出与独立验证通过，已实屏打开系统打印框。继续原生归档导出、打印取消结果与队列提交、远程界面操作及 Agent Provider／凭据。 |
| P1 | 常用编辑（JW-406/JW-407） | 已有查找替换、跳页和页面设置入口。继续查找范围、结果列表、大纲、高级分栏与分节和原生交互验证。 |
| P1 | Word/WPS 双向往返（R5/R6） | macOS 上 Word 和 WPS 各通过两轮正文、表格、图片、页眉和修订往返。JoyWord 已自动验证保留合规的独立厂商 XML，Word 与 WPS 的普通保存仍有移除该部件的实测记录。[实机证据](validation/20261105-macos-desktop/README.md#word-和-wps-双向往返) |
| P1 | 千页 DOCX 性能复验（R5/R6） | macOS ARM64 Release CLI 已通过 5 轮打开、末页编辑、保存重开和渲染。桌面首屏和末页 15 次键盘输入至 GPUI 提交已测。继续物理呈现、跨对象编辑和同条件基线比较。[千页验证](../joyword/r8/VALIDATION.md#千页-docx) |
| P1 | 公共基础（JW-803） | 评估 DOCX 包层、CFB/DrawingML/OMML 和字体服务在共享库中的复用。用格式保真与性能回归验证逐项收敛。 |
| P1 | 平台与发布（R7/R8） | macOS ARM64 Release 构建、移位启动、首屏、键鼠编辑及保存重开已验证。DOCX 系统冷启动与运行中打开、原生打开和另存为面板的取消及确认均通过。继续 Linux x64/ARM64 原生、Windows 中文 IME/读屏、高 DPI、Linux 无障碍及安装器、文件关联、签名与卸载。[macOS 对话框补验](validation/20261105-macos-desktop/README.md#原生文件对话框) |

R8 各项功能和平台分别记录证据。独立版的通过记录、联合编译和三应用冒烟均有独立范围。

