# JoyOffice 复用清单

宿主路径为 `D:\Coding\EditorProjects\joyoffice`，以下路径均相对此根目录。本次读取的宿主 HEAD 是 `00fb06872d38bbbfc26f4dbc3404d6810b72aabb`。任务编号见 [TASKS.md](TASKS.md)。

复用分为直接引入、抽取后适配、迁移规则与测试三种。复用范围由实际依赖决定；对旧模型耦合的模块，不能直接连到新文档上。

## 公共组件优先引入

| 来源 | 复用内容 | 接入方式 | 任务 |
| --- | --- | --- | --- |
| crates/joyoffice-ui/src/lib.rs | Brand::Word、Palette、metrics、control/card/field/section、图标与对比度测试 | 独立阶段使用固定快照，整合时保留宿主唯一版本；适配 Kit 主题而不另建一套 token | JW-102、JW-803 |
| 根 Cargo.toml、Cargo.lock、rust-toolchain.toml | Rust、GPUI、通用依赖、profile、resolver 和 lint | 以宿主为基准建目标 workspace，锁定所有来源 | JW-003、JW-004、JW-101 |
| third-party/accesskit_windows | 0.34.0 的 HWND/provider 生命周期修复和原许可 | 保留本地 patch 和来源记录，按同样路径组织 | JW-003、JW-702、JW-804 |
| scripts/verify_gpui_pre_provenance.py 和 reviewed deviations | gpui-pre 与指定 Zed GPUI 快照的来源核验 | 复用脚本及审查数据，针对同版本运行 | JW-003、JW-709 |
| crates/joyoffice-opc | 有预算的 ZIP、部件、关系、内容类型、原字节保留与写回 | Wordcraft DOCX 容器通过适配对接；语义仍归 Word 层 | JW-203、JW-204、JW-803 |
| crates/joyoffice-cfb | CFB 容器读取与预算 | 优先用于 .doc 与加密容器适配，保留原文档语义测试 | JW-203、JW-510、JW-803 |
| crates/joyoffice-drawingml、joyoffice-omml | 格式中立的 DrawingML/OMML 能力 | 对比 Wordcraft 类型后迁移映射，避免整包绑定旧 Word 模型 | JW-506、JW-507、JW-803 |
| tools/xtask、scripts/package_release.py 等产品工具 | 产品选择、依赖闭包、工具环境、打包和版本标签路由 | 增量接入新包清单，避免另一套独立发布体系 | JW-103、JW-708、JW-805 |

OPC 的 Cargo.toml 依赖 crc32fast、flate2、quick-xml，包含自己的有界 ZIP 层，不是直接把 Wordcraft 的 zip 3 调用改成 zip 9 调用。替换时要保持读取预算、未知内容、关系定位和导出行为。

## 现 JoyWord 的 UI 和桌面接线

| 来源 | 值得复用的具体内容 | 必须适配的边界 | 任务 |
| --- | --- | --- | --- |
| apps/joyword/src/ui/{controls,file,ribbon,shell,theme}.rs | 外壳、文件页、按钮状态、布局和品牌样式 | 将旧命令 ID/启用态替换为 CommandAdapter；保持键盘焦点与无障碍身份 | JW-105、JW-401 |
| apps/joyword/src/chrome_ui.rs | Ribbon、标尺、字体/段落/表格对话框及按下态交互 | 当前读取旧 model::Justification、TabAlign 等；映射查询结果，不能直接共用旧数据结构 | JW-402 至 JW-413 |
| apps/joyword/src/input_ui.rs | GPUI EntityInputHandler、UTF-16 接口、指针事件、画布输入接线 | 文件与旧 TextInput/Session/布局强耦合；复用接线和测试，重新接内核 IME 协议 | JW-301 至 JW-305 |
| apps/joyword/src/document_canvas.rs | 可见页和邻页调度、异步布局/光栅、缓存预算、取消和迟到结果拒绝 | 将旧 LayoutSnapshot/RasterPage 换成新场景；字节通道、预乘 alpha 和缩放分别验证 | JW-208、JW-209、JW-603 |
| crates/joyword-render/src/requests.rs | RasterTicket/RasterRequestGate 的请求身份和旧结果拒绝逻辑 | 迁入新 render 后改布局键与错误路径；其中 generation exhausted 的 expect 必须改为可诊断结果 | JW-208、JW-603 |
| apps/joyword/src/text_accessibility.rs | 文档结构、文本范围、选区和 AccessKit action 映射 | 节点身份与新模型范围映射；不能仅挂整页图像 | JW-706 |
| apps/joyword/src/linux_files.rs、prompt_ui.rs | Portal 失败后的路径输入与桌面助手退路 | 保持取消、错误和后台执行；不将 Shell/文件访问带入内核 | JW-704 |
| apps/joyword/src/export.rs、epub_ui.rs、exchange_ui.rs、pdfa_ui.rs | 导出参数、状态提示和已提供的格式用户流程 | 输出适配新模型和稳定场景；原功能能力须逐项保留 | JW-414、JW-509 |
| apps/joyword/src/mail_merge_ui.rs | 合并向导、映射、预览与输出交互 | 挂接新内核命令和数据源能力 | JW-508 |
| apps/joyword/src/font_fallback* | UI 字体回退、CJK 接入经验与测试 | UI 字体与文档字体身份分开；不靠名称猜正文 face | JW-205、JW-604 |
| apps/joyword/src/io_validation.rs、tests.rs | 打开/保存竞争、窗口流程和验证入口 | 适配新句柄和状态，保留真实后台入口测试而非假路径 | JW-311、JW-806 |

现 JoyWord 的 main.rs 和 input_ui.rs 集中了大量编辑实现；直接复制 main 再逐个修编译，会把旧引擎耦合一并迁入。应先抽视图和 GPUI 接线，桌面入口保持薄。

## shell 和内核中的可迁移能力

| 来源 | 复用价值 | 建议处理 |
| --- | --- | --- |
| joyword-shell/src/{prefs,l10n,diag}.rs | 配置、语言、日志和诊断的已有规则 | 优先直接复用可解耦部分；旧路径读取与新配置写入分开 |
| joyword-shell/src/{command,chrome,dialogs}.rs | 启用态、Ribbon 描述、对话框草稿、输入校验 | 草稿与视图数据可迁移；执行和模型枚举通过适配重接 |
| joyword-shell/src/storage* | 来源戳、外部修改检查、元数据、锁、原子发布、读回和恢复 | 抽离 ExportPlan/DocumentVersion 依赖，接新 exporter；保留安全保存和故障测试 |
| joyword-shell/src/docx.rs、input.rs、host.rs | 对只读/拒绝、未支持内容、安全导出、组合输入的完整行为契约 | 不整体作为第二套 engine 链接；迁移状态机、校验规则和测试 |
| joyword-shell/src/{print,rich_paste,mail_merge*}.rs | 打印调度、富粘贴预算、合并数据映射 | 通用解析可迁入；文档/事务映射使用新模型 |
| joyword-layout/src/{cjk,slots,compatibility,revision_view}.rs | 中文禁则、字体槽、兼容规则及审阅投影已有经验 | 对比新内核逐条补缺；保留来源和样例，不能同时运行两套分页 |
| joyword-model、joyword-docx、joyword-protect | 保护策略、未知内容保留及模型不变量的已有要求 | 迁移规则与必要算法，并用两来源语料验证；不整体保留旧可编辑模型 |
| joyword-agent | 默认关闭、授权范围、取消代次和提案提交契约；生产依赖为空 | 尽量原样复用独立接口，适配依赖旧 engine 的集成测试 |
| joyword-foundation | 受检查 EMU、ScreenScale 等无依赖小类型 | 按调用方需求保留，与新 point 几何做明确边界换算 |
| joyword-corpus、tests/fixtures、CLI goldens、验证脚本 | 丰富的安全、格式、排版、文本交换和回归基线 | 迁移语义断言；只重建受新模型影响的投影，不删除失败测试掩盖回退 |

还可以审查 JoyDeck 的通用 metafile/字体/渲染工具，确认是否有实际公用能力；如果形成两个产品的共同依赖，再抽入 `joyoffice-*`。不直接引用另一个产品的 app 或 shell。

## 复用流程

1. 记录源路径、提交、文件散列、许可、实际使用方和迁移任务。
2. 确认是否依赖旧 Session、DocumentSnapshot、ExportPlan、PageScene 或 GPUI 具体版本。
3. 引入最小闭包，保留原创图标和通用控件；按生产 no-panic 和预算要求审查。
4. 保留相关语义测试，运行新旧语料的差分和失败场景。
5. 将公共新增能力回归宿主使用方，整合时移除临时重复实现。

可复用模块“已有测试”不意味着迁移后的调用方式已验证；必须复测适配后的路径。复用优先级服务于功能和可靠性，不能靠大面积复制未使用文件提高复用数量。


## R2 已落地

新增原样复用 joyoffice-opc 及中文原创语料；提取并适配 RasterRequestGate、document_canvas 的代次/视口/缓存调度和 input_ui 原生文件框接线。继续使用唯一 joyoffice-ui、宿主 prefs/l10n 与 AccessKit patch。来源文件、宿主 HEAD、原始散列和修改说明见 licenses/joyoffice/SOURCE.json。旧模型强耦合的 CFB、DrawingML、OMML、编辑、恢复和保存接口尚未接到新模型，不能算已复用。

## R3 / R4 增量

R3 已适配原生输入、宿主安全 storage 生产管线和新模型恢复接线。R4 进一步适配宿主 JoyWord chrome_ui 的字段表单/取消语义，以及 JoyDeck format_ui、paragraph_ui、chrome_edit 的 Kit 字体搜索、字号/颜色/行距弹层、混合值留空、文档版本校验和焦点恢复。旧 Session/EMU 不进入新模型；所有应用操作仍执行 Wordcraft 命令。公共 joyoffice-ui 保持原样，字段参数转换放在 Word 专用 shell 中。

R4 样式/对象增量继续适配宿主 JoyWord task pane 与 JoyDeck chrome_nav 的 uniform_list、table_ui 的字段表单、content_ui 的原生路径选择和异步资源导入。保留同版 Kit Input/Dialog/Popover、统一主题和焦点恢复；样式、表格、图片的可编辑数据仍完全在 Wordcraft Session。新增来源散列见 licenses/joyoffice/SOURCE.json，宿主只读。
