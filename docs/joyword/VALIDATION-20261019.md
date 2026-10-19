# 2026-10-19 验证审计（Windows 与统信 UOS）

本轮从干净的 `origin/main`（`26f46c3f2582de88cb09012487def0744788f9c7`）创建 `codex/validation-20261019`。范围是核对待验清单、执行当前环境可运行的自动验证、修复验证发现的问题，并补充完成证据。本轮未合并。

## 当前结论

任务现为 **12 done／17 in_progress／37 planned／0 blocked，共 66 项**。本轮补齐自动验证、两个渲染缺陷、Windows GUI 保存／IME／剪贴板／撤销证据，以及 T210 的真实窗口对照；T210 与 T211 因此标记 `done`。其余任务仍有明确未完成条款，未把局部通过计成整项 done。M0 退出条件和 T107／T206／T304／T403／T704 发布门槛仍未通过。

本文先记录 Windows 审计，再补充 2026-10-19 统信 UOS Desktop 20 Professional 会话。UOS 侧新增窗口、缩放、中文剪贴板、视图开关、`kill -9` 恢复、CLI PDF 与双 GUI 阅读器证据，并发现和修复一处 tiny-skia 栅格断言；Fcitx 与系统文件对话框仍未打通。

## Windows GUI 实测（2026-10-19）

用户在明确约定时段并授权鼠标键盘后，Computer Use 以隔离测试 profile 启动 `JoyWord · Preview`，16 个步骤均有窗口截图与按时间记录在 [GUI 观察](validation/20261019/gui/observations.jsonl)，汇总、散列和 ACL 差异在 [GUI 汇总](validation/20261019/gui/summary.json)。

| 路径 | 实测结果 |
| --- | --- |
| 打开本地 DOCX | DOCX-MIN-001 隔离副本正文正确显示，状态栏为第 1 页／共 1 页、80% 缩放 |
| `Ctrl+S` 风险确认 | 原生提示列出 ownership、ACL、扩展属性／其他命名流不可观测；按钮为取消、默认“另存副本”、显式“确认替换原件”；提示打开时原文件散列未变 |
| 默认另存副本 | 写出 `saved-copy.docx`，中文插入可由 CLI 读回，原文件散列保持 `7F234D…8CFB` |
| 显式替换原件 | 中文修改可读回；创建时间保持 `2026-10-19T04:52:00.964938Z`，`Zone.Identifier` 保持 `ZoneId=3` |
| ACL 边界 | SID owner 与 ACE 内容相同，但 raw SDDL 的 DACL 从 `D:` 变为 `D:AI`；不能宣称精确 ACL 标志保留 |
| 系统中文 IME | 物理键组成 `nihao`，候选窗跟随光标，候选 1 为“你好”；Esc 取消无文档变化，空格提交“你好”，一次 Ctrl+Z 撤掉两个字 |
| 连续输入撤销 | 三次物理 `abc` 合并为一步撤销；系统剪贴板 `Ctrl+C/V` 粘贴为独立一步撤销 |
| 连续保存 | 第二次保存成功；保存后撤销只撤掉新文字，撤销后再保存也成功，三轮均用 CLI 独立读回 |
| 窗口／CPU render | 把 80% 截图页宽归一后，DOCX-MIN-001 单行正文与 CPU PNG 的水平边界差 ≤2.23 px、垂直边界差 ≤6.32 px；图像见 [窗口截图](validation/20261019/gui/01-open-local.png) 与 [CPU render](validation/20261019/DOCX-MIN-001/cpu.png) |

这些步骤不覆盖慢速打开窗口、保存中关闭 dirty 提示、真实崩溃、两个 GUI PDF 阅读器或系统打印。完成上述证据后 Computer Use 原生管道中断；重置 node session 并执行一次恢复后仍返回 `native pipe is unavailable`，因此没有继续操作窗口。

## 环境与证据边界

- Windows 11 build 26300，x86_64；Intel Core Ultra 9 285H；物理内存 33,752,997,888 bytes。
- `rustc 1.98.1 (48a229cea 2026-09-01)`；`cargo 1.98.1 (797e8a9bc 2026-08-05)`；PowerShell 7。GUI 使用 Windows 11 当前会话、系统中文 IME、隔离 `test` profile 和 `simsun.ttc` face 0。
- 字体为本机 `C:/Windows/Fonts/simsun.ttc`，face 0，SHA-256 `1526ac24375f51f6eb73bc2d3f8072dbe4a80a3a65217677c9d9a84f67dab2ab`；没有提交系统字体。
- PDF 文字独立读回：Git 随附 Poppler `pdftotext -raw -enc UTF-8`、pdfplumber、pypdf；PDF 光栅由 bundled Poppler `pdftoppm -r 96 -png` 生成。
- 基线健壮性矩阵在修改生产 crate **之前**执行：production binding 为 `aef1b8ddce63aac90ba54e5340984c205a90d810`，与当时 crates 树相同；81/81，43 ok、38 rejected，0 crash/timeout/invariant/protocol。该结果不覆盖本轮后来的 PDF／CPU 修复，也不是覆盖引导 fuzz。Windows 无 Job Object 内存上限，summary 如实记录 `enforced=false`，不能据此宣称硬内存预算验证完成。
- 本机缺少 `cargo-deny`，本轮未运行依赖许可／公告检查。没有新增或升级依赖。Linux、macOS、信创与新版远端 CI 没有在本轮重跑。

## 已执行的验证

| 验证 | 本轮结果 | 限制 |
| --- | --- | --- |
| 基线 `cargo test --locked` 默认成员 | 652 passed、0 failed、0 ignored | 基线二进制；不能替代 GUI；部分可选工具路径会在测试内部跳过，最终 PDF 单独补验 |
| `cargo test --locked -p joyword` | 38 passed | 无头应用适配器测试 |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 通过（含本轮两项修复） | 本机 Windows |
| `cargo fmt --all -- --check`、架构／panic／文件系统门禁 | 通过 | 反例检查预期报告三项违规并通过反例判定 |
| `python -m unittest discover -s scripts/tests` | 53 passed | 工具逻辑 |
| recovery-journal 实验 | 15 passed | 故障注入与日志恢复；不能替代断电 |
| docx-robustness Rust 测试 | 3 passed | 修改生产 crate 之前的固定绑定 |
| docx-robustness Python driver | 12 tests，1 skipped | Windows 不支持的 POSIX 进程树断言跳过 |
| 五个 DOCX-MIN 样例 inspect／render／PDF | 各两次，逐字节可复现，原文件散列不变 | 详见 JSON，未知块按声明计数省略，不是完整兼容 |
| inspect UTF-8 截断 | `--max-text-bytes 1` 返回 3、`truncated=true` | 不把截断当完整输出 |
| 修复后 `cargo test --locked -p joyword-render -p joyword-pdf` | 48 passed、0 failed、0 ignored；含两项新回归 | Poppler raw 已在 PATH，无头验证 |
| 五个样例 CPU／PDF 光栅 | 5/5，逐行墨迹边界最多 1 px，每个深色像素在双向 1 px 邻域内有覆盖 | 96 DPI、宋体 face 0；不是窗口对照，也不是 0.01 pt 场景几何证明 |
| 五个样例 PDF 文字 | Poppler raw 与 inspect 声明正文逐字一致；pdfplumber 与 pypdf 一致 | 独立读回，不等于两个 GUI 阅读器人工验收 |
| Windows GUI 保存与编辑 | 16 个步骤、16 张截图；默认副本、显式替换、连续保存、IME、剪贴板和多来源撤销均通过 | 只覆盖 DOCX-MIN-001 隔离副本；原始 ACL 精确标志与未实现功能仍未验 |
| 4000 段长文档 CLI 排版 | 4000 段／160000 字符可导入，排版超过 64 页硬预算并明确拒绝输出 | 证明超过 64 页，不证明精确百页；没有 GUI 首末段编辑 |

本地完整运行目录：`target/validation-20261019/`。精简、可随仓库保留的日志、JSON、脚本及散列见 [本轮证据](validation/20261019/evidence.json)。测试计数按各次运行独立列出，不叠加为产品覆盖率。

## 验证发现与修复

1. **PDF 合成粗体重复可检索文字（T204／T210）**：DOCX-MIN-002 视觉标题只有一次，但 PDF 两次 `draw_glyphs` 共用字体的 ToUnicode；第二次即使给空 cluster range／空文本，仍被三个独立读取器解释为重复标题。保留第一次文本绘制，偏移加粗副本改画轮廓，几何与字重不变。回归 `synthetic_bold_contributes_searchable_text_only_once` 用 Poppler raw 顺序核对单次文字，避免普通读回的空间去重掩盖重复。
2. **CPU 亚像素下划线消失（T203／T204／T205／T210）**：DOCX-MIN-004 的 PDF 有蓝色链接下划线，CPU PNG 在 96 DPI 没有，墨迹边界比较报 2 px 差异。矩形关闭抗锯齿导致薄于一像素且未覆盖像素中心的线消失；改用覆盖率抗锯齿及专用矩形光栅器（一般路径光栅器也会截掉亚像素横边），不改变场景 EMU 坐标。回归 `subpixel_underlines_remain_visible_between_pixel_centres` 在五种亚像素位置检查 0.2 pt 蓝线可见。

## 统信 UOS Desktop 20 Professional 补充验证（2026-10-19）

环境：UOS Desktop 20 Professional（VERSION_ID 20，eagle），内核 4.19.0-amd64-desktop，x86_64，glibc 2.28，海光 C86 7390 四 vCPU 的 KVM 虚机，X11 2560×1440，无 Vulkan 硬件；GPU 为 llvmpipe/VMware 软件路径。使用机内 SwiftShader ICD（`/opt/namiwork/vk_swiftshader_icd.json`）与软件 EGL 启动 GPUI。工具链 Rust 1.98.1（clippy、rustfmt）；GUI 字体 Noto Sans CJK face 2，PDF 字体 Noto Serif CJK face 2。完整记录、截图、PDF 与日志见 [UOS 报告](../../tests/fixtures/reference/uos/uos_report.json)。

| 验证 | 结果 | 限制 |
| --- | --- | --- |
| `cargo build -p joyword --locked` | 在补本地 fontconfig pkg-config 后裸命令成功，启动 `JoyWord · Preview`，中文页面与功能区完整渲染 | UOS 的 `libfontconfig1-dev` 与已装 freetype 版本冲突；也可用 `RUST_FONTCONFIG_DLOPEN=1`；软件渲染，不代表 GPU 性能 |
| `cargo test --locked --no-fail-fast` | 681 passed、2 failed、0 ignored | 两项失败为内核 4.19 jiffy 时间粒度：同长度快速改写与硬链接别名写入未改变 mtime/ctime |
| `cargo test -p joyword --locked` | 38 passed | 无头应用适配器 |
| fmt／clippy／docs／gates | 全部通过 | 文档检查 84 需求、66 任务、14 Markdown、36 行为条目 |
| Python 工具测试 | 40 passed、2 failed、11 errors | 失败集中在依赖 git ≥2.28 的 `--initial-branch` 与 partial clone 辅助流程；本机 git 2.20.1 |
| GUI 缩放 | 80%→90%→100% 页宽 635→713→793 px，Ctrl+0 复位 | 单显示器 1x，未验 Retina／多显示器 |
| GUI 编辑／剪贴板 | ASCII 键入与系统剪贴板中文粘贴提交，撤销和状态栏计数正确 | 仅验证已实现文本路径 |
| GUI 视图开关 | 格式标记、隐藏文字、域代码连续切换；格式标记画出空格点和段落标记 | 补验中发现并修复栅格断言 |
| 系统 IME | Fcitx 4.2.9.32.49 + 搜狗输入法在运行，但 `nihao` 以 ASCII 直通，无候选框 | 中文输入仅经剪贴板可用；T003／T005 未完成 |
| 文件对话框 | Ctrl+O／Ctrl+S／Ctrl+Shift+S 均无系统选择/保存对话框 | 本次会话未运行 xdg-desktop-portal 主服务；GUI 打开、保存、导出与百页 GUI 被阻塞 |
| `kill -9` 恢复 | 日志无哨兵正文；重启记录 `recovery.restored kind=document count=1`，正文恢复 | 进程终止不等于断电 |
| CLI PDF | DOCX-SAMPLE-001 输出 1 页可检索 PDF，Poppler 五段逐字一致 | 系统打印未验 |
| 双 GUI 阅读器 | qpdfview 高亮搜索「远山」；WPS PDF 12.1.2.1128 报告「精确查找 (1)」 | 真实打印机仍未验 |

补验发现与修复：切换隐藏文字时 debug 构建在 tiny-skia 0.11.4 `scan/hairline_aa.rs:124` 断言。原因是页面场景存在 1–2 px 宽、左边缘为分数像素的矩形，覆盖率矩形光栅器内部整像素宽度归零。`joyword-render::rasterize` 现将亚 2 px 宽矩形吸附到整像素边界并至少保留 1 px，不改变 EMU 场景坐标，亚像素下划线的既有可见性回归仍通过；新增 `narrow_fractional_rects_keep_their_pixels_without_panicking`。修复后三个视图开关连续切换无崩溃。UOS 侧 `confirmed_replacement_copies_a_user_extended_attribute` 通过，但上面两项时间粒度测试失败仍按未完成记录。

## 性能冒烟（自动，无真实窗口）

Release、同一宋体 face 0、20 页／725 段：打开启动小样例 7 次中位 0.108 ms；首次全文排版 14.271 ms；输入提交 5 次中位 0.011 ms，全文重排 13.414 ms；增量输入重排中位 8.492 ms，5 次各整形 1 段；首段改动增量 8.557 ms、整形 1 段，对照全量整形 725 段。造样例 1078.336 ms 单独列出，没有藏进启动计时。运行期间机器存在其他构建任务，未采集电源／温控档案，数据仅作为回归冒烟，不证明 QUALITY 的 p95、实际呈现、GPU 帧时间、百页 GUI 或 1.0 性能预算通过。

## 下一轮优先验收

| 任务 | 当前缺口／可执行步骤 | 环境 |
| --- | --- | --- |
| T109 | 慢打开期间只读／取消，以及保存进行中关闭 dirty 提示；普通连续保存已有 GUI 证据 | 独立测试副本；大文件需可观测慢路径 |
| T110 | 精确 ACL 标志、其他命名流、真实 ACL 边界、非 NTFS／网络盘；默认副本和显式替换已通过；UOS jiffy 时间粒度盲区仍存在 | Windows NTFS 与另列存储环境；UOS 需内核/文件系统补验 |
| T209 | 拖动、自动更正与审阅模式边界；连续键入、IME、粘贴和保存后撤销已有 GUI 证据 | 未实现能力先开发 |
| T208 | 用超过 64 页长文档在 GUI 编辑首段／末段并核对页码；当前长文档未获得精确页数 | GUI；独立副本 |
| T204 | 双 GUI 阅读器已在 UOS 通过；系统打印对话框及真实打印仍未验 | 需打印机与平台打印通道 |
| T200–T203、T205、T207 | 按各项人工条款验证已实现子集；剩余功能先实现 | Word 2024／固定 WPS 互操作对照 |
| T103 | WPS 样例、覆盖引导 fuzz；既有固定矩阵只算变异冒烟 | 需固定工具版本／许可审查的覆盖引擎 |
| T104 | 断电恢复；进程终止和故障注入已有证据 | 受控可断电测试环境 |
| T001／T003／T005 | Windows 腿与 cargo-deny、Retina／多显示器、国产 CPU+GPU 与 UOS Fcitx／文件对话框 | UOS x86_64 虚机已补验；信创真机按用户决策非阻塞待补验 |
| T300 | 壳层审阅命令、表格内修订等先实现，再做 Word／WPS 修订交换 | 不把已有库测试计成完整产品验收 |

## 完整未完成任务与人工验收条款

以下从当前 TASKS 逐项摘录。`planned` 表示工作未开始，不是“只差测试”；自动验证的完整定义与完成证据仍以 [TASKS](TASKS.md) 为准。

| 任务 | 状态 | 人工验收 |
| --- | --- | --- |
| T001 工具链、依赖锁与三端 CI | in_progress | Mac、麒麟与 UOS 已核对；Windows 参考环境与 cargo-deny 本机复核仍缺；CI 配置不算成功运行证据。 |
| T003 字体、IME 与统一 PDF 技术验证 | in_progress | Windows IME 已通过；UOS Fcitx 未接入；多显示器 DPI；PDF 双阅读器已在 UOS 通过。 |
| T005 信创平台技术验证 | in_progress | UOS x86_64 虚机已启动窗口、缩放并经剪贴板输入中文；国产 CPU+GPU 物理机与 IME 仍待补验，非阻塞。 |
| T103 DOCX 最小语义与保留读写 | in_progress | 打开自建与固定 Word／WPS 样例，编辑支持区域、保存、重新打开。 |
| T104 撤销、本地日志与恢复 | in_progress | 强制结束测试进程后恢复，比较最后完整事务和正式保存文件。 |
| T109 打开与保存的并发安全 | in_progress | 慢速打开大文档期间输入与取消；保存期间连续编辑后关闭提示正确。 |
| T110 原件保存的元数据观测与用户选择路径 | in_progress | 普通本地文档保存原件并重开；含 ACL 或隔离标记的文档在替换前列出损失项且默认仍为另存。 |
| T107 M1 发布门槛 | planned | 在 Mac 上完成 M1 工作流，并用 Word 与 WPS 打开保存结果。 |
| T200 样式、编号与编辑便利 | in_progress | 制作多级标题与列表，格式刷、查找替换和撤销。 |
| T201 基础表格编辑 | in_progress | 制作合同表格，插行、合并、拆分、撤销、保存重开。 |
| T202 页面与页眉页脚 | in_progress | 改变页边距与方向，检查页码、页眉及缩放后分页。 |
| T203 图片与富文本剪贴板 | in_progress | Word／WPS／浏览器双向复制列表、表格、图片，比较粘贴选项。 |
| T204 统一 PDF 与平台打印 | in_progress | 真实打印或平台打印到文件；UOS 上 qpdfview／WPS PDF 双阅读器已完成。 |
| T205 功能区、标尺与状态栏 | in_progress | 只用功能区和对话框完成报告排版，切换编辑标记与缩放。 |
| T206 M2 发布门槛 | planned | 在 Mac 上按报告工作流完成制作、跨应用粘贴、导出 PDF 与打印。 |
| T207 中文基础排版 | in_progress | 打开中文 Word 与 WPS 生成的合同和报告，对照参考 PDF 检查断行与分页。 |
| T208 段落缓存与增量重排 | in_progress | 百页文档分别编辑首段与末段，观察响应与分页正确性。 |
| T209 撤销组与输入来源区分 | in_progress | 连续打字、IME 提交、拖动与粘贴混合操作后逐步撤销，每步粒度符合预期。 |
| T300 文字与移动修订 | in_progress | 在 Word 与 JoyWord 交换文字修订，逐项和批量接受／拒绝。 |
| T301 格式与表格结构修订 | planned | 对含表格的合同增删行、改样式并交换审阅。 |
| T302 批注线程与现代扩展 | planned | 跨段批注后移动或删除正文，交换 Word 并检查线程。 |
| T303 比较与合并 | planned | 比较两份合同并在 Word 审阅结果，检查格式和表格差异；与本地历史版本比较后另存恢复。 |
| T304 审阅交换发布门槛 | planned | 按语料执行跨软件接受／拒绝和批注回复，保存证据。 |
| T400 Provider 与关闭基础 | planned | 首次启动离线编辑，配置服务后开启／关闭，检查入口与任务。 |
| T401 受限工具与授权 | planned | 对合同选区提问，验证引用和实际发送内容不超范围。 |
| T402 修改预览与 AI 修订 | planned | 选区改写／格式调整，逐项接受并整体撤销。 |
| T403 取消、竞争与离线门槛 | planned | 在流式生成和提交前关闭，继续普通输入、保存及打印。 |
| T500 复杂分页与长文档 | planned | 百页报告改首段／末段、改变方向与图文绕排，观察定位与响应。 |
| T501 注释、域与引用体系 | planned | 长报告插入章节与图表后更新目录、页码和交叉引用。 |
| T502 公式、形状和表单内容 | planned | 编辑带公式和表单的报告，与 Word 交换并修改。 |
| T503 语言工具与视图 | planned | 离线检查中英文拼写，切换草稿、大纲和阅读视图并核对文档未改变。 |
| T504 中文高级版式与公文 | planned | 公文、拼音和中文版式样例对照与编辑，在 Word 与 WPS 中检查。 |
| T600 RTF 与 Strict OOXML 交换 | planned | 与 Word／WPS 交换带样式、脚注、修订的 RTF 与 Strict 文件。 |
| T601 HTML、Markdown 与文本 | planned | 导入网页片段／Markdown，导出文本后核对丢失说明。 |
| T602 旧版 DOC 导入 | planned | 固定 Word 生成旧 DOC，导入后与原稿语义和版面比较。 |
| T603 PDF 导航、标签与页面图片 | planned | 两个 PDF 阅读器检查导航、可检索性与结构。 |
| T604 加密、保护与活动内容 | planned | 打开受保护和宏文档，编辑安全区域、另存并检查状态。 |
| T605 WPS 文字原生格式导入 | planned | 用固定版本 WPS 生成含样式、表格、批注和修订的 .wps，导入后与 WPS 中的显示和语义比较。 |
| T700 三端及信创产品验收 | planned | 逐平台运行新建、编辑、交换、打印、恢复和关闭 AI。 |
| T701 输入法与可访问性 | planned | VoiceOver、Narrator、Linux 辅助技术和中文 IME（含信创系统的 Fcitx 输入法）真实操作。 |
| T702 性能、故障和恢复 | planned | 在固定硬件反复运行长文档场景，核对原始样本与统计。 |
| T703 安装、签名和升级 | planned | 干净机器安装、升级与回退，确认用户文档未改动。 |
| T704 1.0 发布门槛与公开矩阵 | planned | 按公开说明从安装到合同审阅执行端到端验收。 |
| T800 DOC 写出 | planned | 保存旧 DOC 并在指定 Word 版本继续编辑。 |
| T801 图表与 SmartArt 原生编辑 | planned | 编辑真实图表数据和布局，Word 重开检查。 |
| T802 稀有域与嵌入对象 | planned | 对特殊文档操作并确认每个对象的实际支持等级。 |
| T803 高级签名与复杂模板 | planned | 用独立验证器检查签名，在 Word 检查模板实例。 |
| T804 PDF 重建与 OCR | planned | 将重建结果与原 PDF 并排审校，确认用户能发现不确定区域。 |
| T805 ODT 文件交换 | planned | 与 LibreOffice／WPS 交换带样式、脚注、修订的文件。 |
| T806 EPUB 与归档 PDF | planned | EPUB 阅读器和 PDF/A 验证器检查导航、可检索性与结构。 |
| T807 邮件合并 | planned | CSV 合并生成多份信件，检查字段和取消后原模板未被覆盖。 |
| T808 纵排、双向与复杂文字 | planned | 中英阿文混排与纵排样例对照与编辑。 |
| T900 实时协作规格入口 | planned | 明确多用户审阅与离线冲突场景，不在本轮声称已实现。 |
| T901 套件共享边界规格入口 | planned | 检查跨应用资源／主题交换工作流，保持各自内容语义。 |
