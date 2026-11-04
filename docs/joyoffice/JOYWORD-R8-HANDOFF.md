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
| 偏好 | 配置根仍为 Windows `%APPDATA%/JoyWord`、Linux `$XDG_CONFIG_HOME/joyword` 或 `~/.config/joyword`、macOS `~/Library/Application Support/JoyWord`。可用 `JOYWORD_CONFIG_DIR` 隔离验证。活动文件为 `migration-preview/preferences.conf`，缺失时只读旧根目录的 `preferences.conf`。语言和迁移版主题生效。作者、近期文件、单位、页面预设和旧自动保存间隔目前保留在配置数据中，对应 UI 与行为待接。 |
| 恢复与原件保存 | 编辑副本先写入并读回核对。替换原件需要确认所有者、ACL 和扩展属性风险。新恢复数据在 `migration-preview/r3-recovery`。旧 `recovery-*.jwr` 和旧恢复工作流使用替换前版本打开后另存 DOCX，再由 R8 接续。旧 journal 的自动迁移和恢复故障演练待验。 |
| CLI 与自动化 | `inspect [--] PATH`、`text [--] PATH` 可用。JSON 结构为新的会话检查结果，旧 schema、`--max-text-bytes`、`--compatibility` 及旧输出选项待做适配。新调用为 `render INPUT OUTPUT.png --page 1`、`convert INPUT OUTPUT.pdf` 和 `run INPUT --cmd COMMAND=JSON --save NEW.docx`。MCP 为有显式目录边界的 stdio，旧桌面验证控制协议与桌面桥接待接。 |
| 格式与 AI | 新引擎包含 DOCX、PDF、图像和若干文本格式的交换，也提供编辑、引用、审阅和合并命令。旧 CLI 的 EPUB、PDF/A-2b、独立 merge/exchange 参数、系统打印完整流程和旧 Agent 桌面集成列为兼容项。`joyword-agent` 保留独立契约测试，旧引擎事务适配器待重接。 |
| 旧验证门禁 | 当前 CI 使用新布局、渲染、内核与保存测试、实际 DOCX 往返、独立 PDF 读回，以及确定性损坏输入。覆盖引导 fuzz、固定金样和旧编辑行为登记对应旧 API，在替换前基线复现。恢复这些门禁是后续任务。 |

## 下一步

| 优先级 | 任务 | 完成条件 |
| --- | --- | --- |
| P0 | 旧数据与安全（JW-806/812） | 用真实旧偏好、旧恢复日志及已保存 DOCX 演练迁入、回退和异常恢复。让作者、近期文件和自动保存设置接入实际会话。校验旧构建回读新 DOCX 的支持范围。 |
| P0 | CLI 与格式兼容（JW-808） | 逐项迁移外部脚本依赖的旧 CLI 参数、JSON schema、退出码和桌面控制协议。按原始语料核对 EPUB、PDF/A、打印、保护、未知 OOXML 保真与 Agent 授权。 |
| P0 | CI 与耐久验证（JW-805/807/809/899） | 在托管 CI 重跑三产品与交叉架构门禁，移植覆盖引导 fuzz、图像金样和行为登记。将原件冲突、恶意 ZIP/XML、长文档与跨产品三开纳入可重复验收。 |
| P1 | 性能与呈现（R5/R6） | 修复字段混排字形差异，用千页样例重测对象和跨行编辑的全量重排、输入到绘制延迟及内存。补齐复杂 DOCX 的 Word/WPS 双向往返。 |
| P1 | 公共基础（JW-803） | 评估 DOCX 包层、CFB/DrawingML/OMML 和字体服务在共享库中的复用。用格式保真与性能回归验证逐项收敛。 |
| P1 | 平台与发布（R7/R8） | 复测 Linux x64 与 ARM64 原生、Windows 中文 IME/读屏、高 DPI、Linux 无障碍。补齐安装器、文件关联、签名与卸载。macOS 原生构建与运行待验。 |

R8 各项功能和平台分别记录证据。独立版的通过记录、联合编译和三应用冒烟均有独立范围。

