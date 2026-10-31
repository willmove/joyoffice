# Rust 与 GPUI 目标架构规格

版本：2.0。模块名称是目标边界，不表示当前目录已存在对应代码。实现顺序见 10、11。

## 1 架构决策

桌面 UI、事件循环、窗口和通用控件使用 GPUI/GPUI Kit。领域层使用纯 Rust、可在无窗口条件下测试的独立模型；行为来自 FR 与公开标准。

首先依赖 `gpui-kit` 的 facade 和与它匹配的 `gpui-pre-*` 家族。调研时官方 Kit 文档显示 0.8.1，匹配快照为 0.3.10；`gpui-pre` 是 GPUI 的快照发行包名，不是额外的演示引擎。实施 M0 时锁定经构建验证的完整依赖图与 Cargo.lock，不单独追最新 gpui-pre。[安装与快照关系](https://gpui-kit.com/docs/installation/)

暂不启用 gpui-fast、JavaScript shell、WebView 或其他 UI 引擎。它们是额外选择，会扩大测试和依赖空间。桌面行为与模型均由本项目独立实现。

## 2 目标模块及依赖方向

| 建议 crate / 模块 | 职责 | 可依赖 | 不可依赖 |
|---|---|---|---|
| joydeck-geom、color | 单位、矩阵、路径、颜色 | Rust 基础/几何库 | document、engine、GUI |
| joydeck-document | 稳定 ID、文档树、样式与继承、校验 | geom、color | IO、GUI、渲染后端 |
| joydeck-fonts | 字体源、度量、替代、嵌入权限 | 解析/shaping 依赖 | GUI/系统窗口 |
| joydeck-text、anim | 文字布局、命中；时间轴和状态采样 | document、geom、fonts | GUI、窗口、音频设备 |
| joydeck-native、pptx | 内存字节与中间模型转换、兼容报告 | document、格式库 | 文件对话框、GPUI |
| joydeck-scene | resolved 页面、绘制项、裁剪、命中元数据 | document、text、anim | GPUI、具体 GPU device |
| joydeck-render-cpu | Scene/文档到图像；参考路径 | scene、CPU 绘图库 | GPUI |
| joydeck-export | PDF/图片/大纲等输出，批量任务 | scene、render-cpu、native、pptx | UI 对话框 |
| joydeck-engine | 命令、选择、交互、历史、变更事件 | document、text、anim | GPUI、文件选择器、窗口 |
| joydeck-media | probe、demux/decode、PCM/帧队列 | 格式/codec、时钟抽象 | GPUI、系统窗口 |
| joydeck-services | 文件任务、文档生命周期、资源缓存、恢复、导出调度 | engine、formats、render-cpu、export、platform traits | GPUI Entity 类型 |
| joydeck-platform | 文件选择、系统剪贴板、屏幕、音频、打印、更新等实现 | OS crate、服务契约 | 文档修改逻辑 |
| joydeck-render-gpui | Scene 在 GPUI 上绘制、纹理和 paint adapter | scene、GPUI、可选 CPU fallback | 文档命令实现 |
| joydeck-ui-gpui | 工作区、控件、输入/焦点、窗口和展示状态 | Kit、services、render-gpui | 直接修改 document 字段 |
| joydeck-automation | CLI/控制/MCP dispatcher 和能力限制 | engine、services 契约 | GPUI |
| apps/joydeck、joydeck-cli、xtask | 组合依赖、feature、程序入口、工程检查 | 按需组合 | 将 GUI 依赖泄漏回核心 |

这是目标边界。M1 先在应用内使用 document/import/render/viewer 明确模块实现独立切片，按复杂度再拆 crate。M7 前文件副作用从领域修改拆开，M12 前 CLI 不再经 UI crate 间接加载框架。不能为了分层一次性重构全部核心、导致多个阶段都没有可运行成果。

## 3 状态所有权

### 文档状态

DocumentStore 以 DocumentId 管理打开的文档。每个文档持久状态由一个串行写入者持有，包含不可变 Presentation 快照、内容 revision、saved revision/快照、历史、资源引用和来源兼容报告。修改通过 engine 命令执行。

每个编辑 ViewSession 独立持有当前页、选择、编辑目标、视口、缩放、工具和临时交互。窗口选择和视口从一开始归 ViewSession，避免第二窗口改变第一窗口的输入目标。历史属于文档；撤销后，各 ViewSession 校验失效选择，但不强制复制另一个窗口的全部选择。

### UI 与运行状态

GPUI Entity 管理窗口和 View 生命周期、焦点、弹层、面板尺寸、可观察的文档摘要。GPUI 的实体不能成为原生持久化文档的一部分。面板读不可变 projection；字段编辑形成 draft，提交才触发命令。

ShowSession 持有放映顺序、playhead、媒体同步、临时墨迹、观众屏和演讲者屏；它使用开始放映时的文档快照。放映期间源文档可继续编辑，但默认不热改正在演示的快照；显式“刷新放映内容”在页边界生效，避免演讲画面突然变化。

JobRegistry 持有任务状态、取消 token 和任务句柄。任务结束时按 DocumentId/WindowId/revision 检查投递对象，不能只对“当前活动文档”写结果。

## 4 命令与副作用协议

### 逻辑输入

CommandIntent 包含 command ID、version、目标 DocumentId/ViewSessionId、显式对象或选择快照、参数、来源及 request ID。来源区分 UI、CLI、MCP、恢复重放。命令 schema、可用性、label key、快捷键上下文、是否 undoable 从一个目录生成或查询。

命令 ID 按本项目需求独立定义，见 16；UI 文案使用本地化 key。参数先在 adapter 做结构校验，再由领域层校验边界、引用和约束。向外的 JSON 形式与 Rust 内部类型隔离。程序化命令不隐式打开对话框。

### 修改事务

流程：准备目标 → 解决 IME/交互冲突 → 验证 → 记录 before snapshot → 执行 → 校验结果 → 原子替换当前快照 → 记录历史 → 发布 DocumentChanged。失败恢复 before，不能只恢复当前页而留下媒体/母版半更新。

事务输出包括 changed object IDs、影响的页/母版/主题、是否布局失效、资源变化及警告。初期可保守使用文档 revision 整体失效，M4 引入按页/对象影响集。不得把非内容选择变更递增成内容 revision，否则无意义地触发保存和画布重绘。

命令请求通常可重试的前提是有 request ID 与结果去重；不能把 `slide.new` 的网络重试无条件执行两次。嵌套复合命令在一个最外层事务提交。

### 文件与系统副作用

打开、保存、导出、打印、打开链接和播放媒体是 ServiceRequest。UI 先补齐路径等交互信息，再调用服务。文件写入不装进“文档内存快照回滚”就算完成；服务要有自己的临时文件、取消和成功条件。

CloseDocument 为有阶段的生命周期流程：处理当前输入 → 检查 dirty → 选择保存/放弃/取消 → 等待保存结果 → 释放窗口/文档。自动化未提供 dirty policy 时返回 `confirmation_required`，由调用者补参数，不弹不可见对话框。

## 5 线程与异步边界

| 执行位置 | 工作 | 限制 |
|---|---|---|
| GPUI foreground | 焦点/窗口、事件路由、短命令提交、读取 projection、纹理提交 | 不同步解析 PPTX、大图、PDF、媒体，不等待磁盘 |
| 文档串行执行器 | 保序修改、事务、历史、变更合并 | 单文档一次写入；初期可在 foreground 执行短事务 |
| CPU worker pool | ZIP/XML、排版、CPU 图像、导出、缩略图、解码 | 输入不可变快照和 owned 数据，输出带版本 |
| 音频回调 | 从有界 PCM 队列取样、填充缓冲 | 不锁文档、不分配大块内存、不调用 GPUI |
| 网络/stdio | 解析协议、认证、限流、收发 | 不直接改 Entity；通过 dispatcher 投递 |

仅把函数包进 async 并不能让 CPU 工作离开主线程。GPUI 的 foreground task 与 background task 不同；Task 句柄生命周期会影响取消，因此关键句柄由 JobRegistry/对应 View 保持，后台返回 owned Send 数据再回到前台检查目标。[GPUI 任务模型](https://gpui-kit.com/docs/task/)

耗时命令先用 revision R 快照计算 patch，完成时比较 revision；若文档已变，按操作选择拒绝、重新计算或经明确冲突合并。不能把 R 的整份结果覆盖 R+1。

任务取消采用协作式 token，在 ZIP 成员、页面、布局块和解码帧边界检查。丢弃 GPUI Task 不保证已执行的同步工作立刻停止，更不能回滚已经提交的文件 rename。任务状态区分 queued/running/cancelling/cancelled/succeeded/failed。

## 6 缓存与通知

页面缓存键包含 DocumentId、SlideId、内容/布局/主题/字体资源版本、渲染模式、视口 tile、scale bucket 和动画 sample。不能跨进程持久化 Arc 地址，也不能用全局 revision 使无关文档全部刷新。

变更影响母版时要失效所有引用页；媒体替换要失效所有引用对象；字体到达要重新排版；只移动光标仅重画 overlay。变更通知合并到帧边界，禁止每个对象单独唤醒整个应用。

缓存分 CPU scene/layout、raster、GPU textures 三类，分别记录字节、命中率、命中耗时和淘汰；Document close、device lost 和字体数据库重载有明确清理点。

## 7 历史、内存及资源

独立实现 Arc 快照历史作为起点，必要时再引入结构共享或 patch。默认目标为最多 500 步、历史额外内存估算 256 MiB；两者先到即按完整事务淘汰。一次超预算大操作应事前提示或使用受控策略，不能执行后不可见地失去全部历史。

媒体采用不可变内容标识去重，文档、历史、后台任务都可能持有资源；引用扫描删除媒体时须考虑历史与恢复。原生/OOXML 的 ID 保留为格式层映射；runtime DocumentId 不写回 OOXML。

剪贴板为独立 transferable snapshot，携带必要媒体和源主题，不持有关闭文档的巨大对象图。内部格式版本化，并附 plain text/image 等公开 fallback。

## 8 平台接口边界

必须抽象：文件对话框、系统剪贴板、URL 打开、显示器/全屏、音频输出、打印、文件关联、文件锁/替换、应用目录、更新与签名验证。接口允许 `Unsupported`，UI 据此禁用或提供可解释替代。

GPUI 已提供且满足要求的能力优先使用；缺口再加小型平台 adapter。不能将“框架支持三平台”等同于剪贴板、双屏、读屏、IME 都已由应用验收。

## 9 独立实现与构建边界

- M0 最小工程只含 GPUI 诊断窗口和必要基础；M1 独立实现最小读取/渲染模块。
- media、automation、native print 等使用 feature 或独立 app 隔离，但发布默认功能集固定，不能靠禁用失败功能冒充完整通过。
- 字体来自系统或经审核的通用字体库；依赖检查覆盖 normal/build/dev，禁止未登记的本地外部路径。
- 不在核心使用 GPUI Entity、Pixels、SharedString、Window 或 GUI error 类型；边界转普通 Rust 数据。
- 构建工具检查 crate 依赖方向与禁用 GUI 依赖。实现阶段新增 adapter 不得反向引入领域写逻辑。
- 系统交互的少量 unsafe 如无法避免，限于审核过的 adapter/依赖；自有核心禁止 unsafe，并对 panic/unwrap 等保留严格约束。第三方 GPUI 内部并不因该约束自动成为无 unsafe。

## 10 可观察性

每次交互和任务可关联 command/request/job ID，性能事件含阶段耗时、页/对象数量及字节，不含正文、媒体原始数据、访问 token。诊断可展示 renderer、DPI、缓存、队列、帧耗时、文件告警与构建版本。

日志和兼容报告对用户可导出；上传和遥测默认关闭。最小信息足以复现“打开慢”“字体错位”“保存丢对象”，避免收集文稿内容作为默认诊断。
