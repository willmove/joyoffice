# JoyWord 当前状态

2026-11-04。R7 交付 Windows x64、Linux x64 与 Linux ARM64 的 Release 开发包。千页末段已折行时，Windows 同机输入至 GPUI 提交中位数由 122 ms 降至 37 ms。Windows 与 Linux x64 通过实包编辑、保存和新进程重开。ARM64 交叉包通过 QEMU CLI 编辑、保存重开及 PNG 渲染。共享界面沿用 JoySheet。macOS 暂缓。[最新 Release 对照和验收](../../evidence/R6-wrapped/README.md)

## R6 和 R7 本轮

- 视口用二分查找定位，后台按可见页和邻页调度，跨缩放级别共用有上限的缓存。缩放保持页面锚点。编辑移除了同步全文展开及一次 IME 中间刷新，千页全文重分页和输入延迟仍需优化。
- 复用 JoySheet 标题栏、功能区、主题、命令搜索和状态栏样式。活动段落与选区经 AccessKit 暴露给 Windows UI Automation，完整文档导航和读屏应用待验。
- 产品检查 3,412 项通过、2 项忽略。最终缩放和缓存调整后复跑全仓 fmt、UI Clippy 和 8 项 UI 测试。锁定依赖及隔离宿主候选 JoyWord/CLI 编译检查通过。
- 交付包、跨平台状态、性能数据与限制分别见 [R6/R7 说明](R6-R7.md)、[R6 验证](../../evidence/R6/README.md) 和 [R7 验证](../../evidence/R7/README.md)。

## R5 本轮

- 引用：目录、大纲级别、脚注尾注、来源/引文/书目、题注/表目录、交叉引用、索引/引文目录、书签及域更新。
- 审阅：批注/回复/编辑/解决/删除、修订投影与窗格、接受/拒绝、导航和 DOCX 比较/合并；部分复杂故事范围比较仍未完整实现。
- 邮件：收件人有界导入、工作表选择、筛选排序、字段及预览、合并 DOCX 副本；32 MiB 展开估算和 1000 条记录限制。会话数据源不随 DOCX 持久化，不发送邮件。
- 交换：多格式打开及 DOCX/PDF/TXT/Markdown/HTML/RTF/ODT/LaTeX/Word XML 副本导出；CLI PNG。实包十种输出及可导入格式重开、拒绝覆盖通过。
- 自动化：会话宏录制/回放，失败恢复编辑快照，禁止桌面宏外部 IO；MCP stdio 独立内核入口要求显式文件根目录，实包协议与越界拒绝通过。无默认网络监听、无桌面实时桥接、无 VBA。
- GPUI Kit 标题栏、功能区大图标与分组、任务窗格复用宿主组件；完成常用结构对齐，完整视觉一致性继续推进。
- 三产品宿主隔离 canary 65 成员/935 包解析和 JoyWord 构建检查通过。GPUI Kit 0.8.1 / GPUI 0.3.10 及公共依赖保持一致；原宿主不改写。
- 产品闭包 fmt、Clippy 与 3,411 项测试通过（2 忽略）。范围与边界见 [R5.md](R5.md)，证据见 [R5 验证](../../evidence/R5/README.md)。

## 已实现

- Rust 1.98.1、edition 2024；GPUI Kit 0.8.1 / gpui-pre 0.3.10、AccessKit patch 与 JoyOffice 基线一致。应用、包和模块使用 joyword。
- 完整 Wordcraft geom/model/fonts/layout/engine/docx/doc/formats/render/pdf/metafile 闭包，单一 Session、651 项实际注册命令、撤销/typing/格式/序列化内核。UI 已连接文本编辑、原生纯文本剪贴板和保存。
- 继续复用 joyoffice-ui、prefs/l10n、产品工程工具；新增原样 OPC 容器预算预检，以及宿主请求身份、视口/邻页缓存调度和原生文件框接线。
- 真实 DOCX 后台打开、全文分页、CPU 光栅 → GPUI 页面；正文、中文混排、粗斜体、表格、图片、页眉页脚；翻页、滚动和 50%–200% 缩放。
- 打开失败保留旧文档，取消代次和迟到结果拒绝；64 MiB 输入、256 MiB 解包、单页 4 MP、总缓存 16 MP 预算。同步导入/全文排版尚不能在阶段内部抢占取消。
- 字体/布局沿用宿主 HarfRust/Skrifa；ZIP9、双 digest/HMAC、stock Krilla 私有适配，未引入全局 PDF fork 或 egui。
- 固定非 UI craft-libs 来源及 craft-i18n 默认语言数据；13 个 OFL 字体内嵌。中文使用本机安装字体，未复制系统字体。
- CLI inspect/commands/run/render/convert PDF、PNG、TXT；命令通过同一 Session。输出只写新路径，拒绝覆盖已有文件。
- UI 线程拥有唯一 Reader/Session；后台光栅接收不可变场景快照，不与实时编辑争抢 Session 锁。光标、选区和预编辑下划线独立绘制。
- 复用宿主 EntityInputHandler 注册、失焦接线和 UTF-16 换算；预编辑更新不累加历史，取消恢复原文/选区/redo/dirty，提交形成一次撤销。过期文档输入回调被拒绝。
- 单击、拖选、双击单词、三击段落、键盘移动/扩选、Enter/删除/Tab、Undo/Redo。原生输入是普通字符的唯一写入通道。
- 迁入宿主来源戳、锁、元数据、同步临时文件、读回校验、原子新文件发布与原件替换。SaveJob 绑定文档/版本，迟到保存不清除更新的 dirty；另存拒绝覆盖，原件替换默认要求明确的元数据确认。
- 新建/打开/关闭保护未保存会话。恢复工作线程写入 migration-preview/r3-recovery，预编辑不写恢复快照，旧目录不改写。
- R4：粗体/斜体/下划线/删除线/上下标、字体搜索、独立中文字体、字号、颜色/高亮；对齐、列表及编号样式、缩进、行距、段前后间距、边框/底纹和分页标志；正文/标题 1/2 样式及清除格式。
- 功能区和快捷键共用引擎命令；选区和撤销后刷新混合格式、选中态及不可用原因。表单复用 Kit Input/Dialog，绑定文档/版本/选区，整体校验后只提交改动字段，取消保留原文档，一次应用一次撤销。

查找替换、原生图片/富文本粘贴、页面设置、页眉页脚、最近文件、AI/MCP、打印尚未接入。加密 DOCX 没有密码对话框。完整旧 CLI 参数兼容与 parity 仍待迁移。恢复已接线，但原生崩溃后恢复全链尚未验收。

- R4 增量：完整样式面板（搜索、虚拟列表、新建/应用/修改/删除段落和字符样式、继承/后续、按选区更新、恢复继承、快速样式）；单次/连续格式刷、Esc 结束。
- 插入网格/表单、表格行列增删、选择、合并拆分、属性/边距/边框/底纹/对齐/自动调整/均分及列宽/整体缩放和移动。图片异步导入/替换、选择、大小/裁剪/位置/环绕/旋转/替代文字、颜色/校正/透明度/重置。
- 对象拖动仅预览，在松开时经唯一 Session 提交一次，Esc/离开视口取消；文档身份/revision 变化拒绝迟到编辑。图片导入有 32 MiB、16 MP 和 128 MiB 解码预算，并绑定文档/选区/版本。

## 验证

R4 第一轮完整产品 fmt、Clippy 和 3,360 项测试通过、2 项来源忽略；新增混合格式、字段校验、局部段落 patch、连续输入格式、DOCX 读回和 GPUI 格式路由回归。后续字体目录/数值显示修复的 UI Clippy 与 3 项 UI 测试也通过。R2 的字体服务/校对库另 517 项通过、2 项来源忽略，本轮未修改这些外部组件。

R3 已在仓库外的中文及空格路径启动桌面包，验证中文和 emoji 文字编辑、可见选区、Ctrl+Z/Y、关闭取消、原件安全替换及另存。GPUI 集成测试覆盖组合更新/提交/取消和旧文档回调。R7 最终包补验了 Windows 原生拼音候选窗和提交后的 DOCX 重开。[R3 记录](../../evidence/R3/README.md) · [R7 记录](../../evidence/R7/README.md)

R4 在仓库外实际点击功能区，完成中西文字体、字号、B/I/U、字色、对齐、段落缩进/间距、一次撤销重做、另存及独立 OOXML 检查；最终包继续通过字体搜索、无效字号/取消、字母编号、高亮、安全保存和新进程重开。DOCX/PDF 样本、截图和原始日志见 [R4 记录](../../evidence/R4/README.md)。

临时宿主副本解析通过：Word/Deck/Sheet 共 63 成员、933 包，复用宿主唯一 UI/OPC 和 patch；候选 Word 与 CLI cargo check 通过。Deck/Sheet 本轮未编译。宿主目录保持只读；165 个原基线文件及 HEAD 校验未变。现 Git 分支 codex/joyword-gpui-r1；尚未提交/推送。

本轮完整产品 fmt、Clippy 和 3,369 项测试通过、2 项来源忽略；新增样式继承/字符样式、格式刷、表格和图片的 DOCX 往返，以及 GPUI 取消/过期/一次撤销拖动测试。最终新包在仓库外校验 31 项 SHA-256 和源码指纹；CLI 编辑保存重开、独立 OOXML/PDF/PNG 检查通过。用户解锁后已完成实际样式新建/修改/搜索、格式刷撤销重做、表格合并拆分/底纹/列宽拖动、图片原生导入/大小/裁剪/环绕/替代文字/手柄缩放；最终包修复 Kit Root 截获 Tab，正反导航和中文输入验证通过。另存后退出并用最终包新进程重开，独立 OOXML/PDF 检查通过，原件未变。详见 [样式及对象证据](../../evidence/R4-authoring/README.md)。

## 构建和运行

```powershell
cargo xtask check joyword --headless
cargo xtask check joyword
cargo xtask build joyword
cargo xtask run joyword
cargo xtask package joyword
python scripts/check_dependencies.py
python scripts/check_plan.py --verify-sources
```

Windows 使用 pwsh.exe。package 生成 dist/joyword/R7 下的开发包，包含桌面、CLI、样例和许可，记录 Cargo 真实产物、源码指纹和 SHA-256。Windows x64、Linux x64 和 Linux ARM64 已生成 Release ZIP。ARM64 真机桌面、签名及独立 Linux 桌面资格待验。[Release 开发包和校验](../../evidence/R6-wrapped/README.md)

## 接续任务

1. R6 扩展跨行和复杂文档的增量排版，补齐系统级呈现帧时间、内存和显存对照，以及原生字形和复杂图形验证。
2. R7 继续 ARM64 真机和独立 Linux 桌面验收，补齐 Windows 多屏/DPI、真实读屏、系统打印、安装器和签名。macOS 暂缓。
3. 按 R2–R5 各阶段记录补齐 CLI 兼容、恢复、剪贴板、复杂对象、查找替换、页面设置和混排域字形等任务。
4. R8 完成宿主旧 Word 附属 crate API 映射与三产品编译/测试，再执行替换。
