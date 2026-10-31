# 放映媒体与自动化规格

主要阶段为 M7 基本静态放映、M10 动画双屏、M11 媒体和 M12 自动化。所有功能通过同一文档及领域命令工作。

## 1 放映会话

ShowSession 的生命周期：idle → preparing → running ↔ paused → ended；加载/解码失败可产生受控 error 状态，但应允许跳过问题页/媒体或结束放映。准备期间可取消。

进入放映生成不可变 ShowSnapshot，含页面顺序、隐藏页处理、自定义放映、媒体引用、字体与兼容报告。默认编辑器后续修改不热更新到观众画面。M7 先实现静态全屏；M10 逐步加入时间轴，先实现明确的最小转场子集。

| 状态 | 数据 | 确定性规则 |
|---|---|---|
| 当前页 | SlideId、order index、进入页时间 | 模型页索引与放映顺序索引分开 |
| 构建步骤 | click sequence、已触发动画、触发组 | 点击先推进当前页构建，全部结束后再翻页 |
| 播放时间 | monotonic playhead、暂停总量、seek origin | 不用系统日期时间驱动动画 |
| 转场 | 前后页 ID、开始时间、duration、fallback | seek 到指定时间得到确定结果 |
| 临时交互 | 黑/白屏、笔、指针、缩放和导航浮层 | 不隐式写入源文档 |
| 媒体 | clip 状态、master clock、播放策略 | 页切换、返回和循环语义明确 |

返回上一页默认显示其最终构建状态，重新从该页开始则从初始状态；设置项可以覆盖，但不能随机取决于缓存是否存在。页内动作跳转需保留返回栈，设置最大深度，防止自循环不断分配。

## 2 时间轴与动画

独立定义时间轴与采样模型，保持时间采样纯函数。编辑器预览、观众窗口、CPU 导出和未来视频导出使用同一 evaluator。输入时间采用单调时钟转相对时长；测试直接注入虚拟时间。

首批效果清单在 M10 固定：appear/disappear、fade、fly、基础 emphasis、直线 motion path；转场先 none/fade/push/wipe。逐项列 Import/Render/Edit/RoundTrip 能力，未验收的效果不提前开放。

参数包括 duration、delay、start rule（on click/with previous/after previous）、repeat、auto reverse、rewind、对象/段落 target。无效时长、极大 repeat 和缺失目标需拒绝或受控修复。修改动画列表为领域事务，播放位置不是文档修改。

pause 冻结时间与相关媒体；resume 从暂停点继续。睡眠/窗口阻塞造成过大 delta 时，不逐帧补播；按单调目标采样或请求用户恢复，不能瞬间触发大量页跳转。

### Morph

按显式稳定匹配 key 优先，再用经过验证的 ID/name/类型和几何启发式匹配，冲突时降级，不跨页错误合并多个同名对象。先保证位移、缩放、旋转和透明度；图片 crop、路径和逐字变形独立扩展。

Microsoft 公开文档说明可用特定对象名称增强 Morph 匹配。这可以作为交换行为依据，但 JoyDeck 仍需验证同名冲突、缺对象及导入信息缺失时的规则，不推断 PowerPoint 内部算法。[Morph 官方说明](https://support.microsoft.com/en-us/powerpoint/morph-transition-tips-and-tricks)

## 3 观众窗口与演讲者窗口

两个 GPUI 窗口共享 ShowSnapshot 和只读 PlaybackSnapshot，只有 ShowController 修改时间轴。各窗口可因刷新率不同分别绘制，不创建两个互相竞争的播放时钟。

观众窗口只显示页面、临时演示效果和允许的指针；演讲者窗口显示当前/下一页、备注、计时、导航、黑屏和交换屏幕。备注字号和计时是放映偏好，不污染文档。

屏幕选择以平台 display identifier + 当前能力识别，持久化时需容许 ID 改变。默认双屏时给明确的观众屏选择和预览，启动前确认目标不依赖“当前窗口在什么屏”。Microsoft 的公开工作流同样区分观众画面与私有备注。[双屏放映](https://support.microsoft.com/en-us/powerpoint/present-on-multiple-monitors-and-view-speaker-notes-privately)

热插拔策略：暂停或安全转到主屏观众视图；演讲者备注窗口不自动进入可能被镜像的屏幕；弹出可见的屏幕变化提示。镜像模式不能保证私有备注，应回到单屏观众模式并隐藏演讲者备注，等待用户重新选择。

Esc 分层退出导航浮层/放大、结束墨迹工具，最终结束放映；必须可通过显式退出按钮一次结束。PageUp/Down、空格、方向键、B/W 等快捷键在放映 scope 生效，不穿透编辑文档。

## 4 媒体管线

目标链：资源索引 → probe → demux → 后台 decode → bounded frame/PCM queue → resample/mix/clock → OS audio + Scene video image。通用编解码依赖按许可和功能按需引入，M0/M1 不承担完整编译成本。

| 项目 | 首版要求 |
|---|---|
| 基础能力 | WAV/PCM、MP3、经验证的 H.264 MP4 profile；所有 format/profile/位深/音轨以能力表列出 |
| 可选扩展 | AAC/FLAC/ALAC/Ogg/Opus、HEVC/VP9/AV1 按原模块实测开放，不将容器名当编码支持 |
| 播放控制 | play/pause/stop/seek、volume、trim、loop、autoplay、rewind、poster |
| 同步 | 有音轨以音频设备采样时钟为 master；无音轨用 ShowClock；视频允许丢过期帧以追上时钟 |
| 队列 | PCM 建议 0.2–1 秒有界预缓冲，视频建议 3–6 帧；总预算按 09 限制 |
| 失败 | unsupported、probe failed、decode error、device lost 分开；回退海报并允许继续翻页 |
| 编辑 | 裁切/音量等写入模型可撤销；实时播放状态只在 ShowSession |

跨页音频有明确作用域和停止条件；切页不重复启动仍在播放的相同 clip。seek 递增 generation，丢弃旧解码结果及旧音频队列，必要时淡出/淡入避免爆音。音频回调禁止文件读取、解码和文档锁。

软件解码不承诺任意 4K、多轨或复杂编码流畅。硬件解码平台 adapter 是 M14 选项，不能作为 M11 必须依赖的尚不存在能力。主动录制/麦克风/摄像头权限只在相应功能被用户使用时申请。

## 5 CLI 与服务接口

规划 `joydeck-cli` 提供 commands/describe、inspect、run、render、convert、mcp 及连接桌面的 app 功能。M1 可以先有内部 headless 验证工具，公开 CLI 契约 M12 固定。

所有操作接显式 DocumentId 或输入路径；headless UI-only 操作返回 unsupported，不能“成功”但没有效果。stdout 专供结构化结果或 MCP 协议，日志写 stderr。退出码至少区分成功、参数错误、读取失败、兼容/转换失败和内部错误。

脚本批量命令支持明确的事务边界。遇错默认中止并回滚本事务；跨文件副作用不能假装同内存命令一样整体回滚。提供 dry-run/validate 时列出其不执行的副作用。

## 6 本地控制协议

请求结构与窗口框架隔离。传输层使用有界 dispatcher，将 CommandIntent/ServiceRequest 投给相应串行执行者，返回稳定错误和任务句柄。

默认不监听端口。开启控制时仅 loopback 或 OS IPC，使用每实例随机凭据和当前用户权限；凭据不进日志。默认单请求 8 MiB、最多 8 客户端、排队 64 条，复杂操作转 JobRegistry；参数可以调但总内存/并发仍受控。

请求包括 protocolVersion、requestId、method、target、params；响应包含 result 或 error，可选 jobId。已完成 requestId 的可变更操作返回先前结果；同 ID 不同参数拒绝。超时分 queued-cancelled、running-outcome-unknown 和 completed；不能让客户端把未知执行结果当作安全重试。

engine.commands、document.inspect、command.execute、show.start/end、job.status/cancel 是主要稳定方法；ui.inspect/screenshot 属调试能力，截图需对应真实 GPUI 窗口，不以页面 render 冒充窗口截图。坐标方法显式标注 document pt 或 logical screen px，不能沿用旧协议多行重复定义造成歧义。

## 7 MCP

保留 Headless/Remote 两种后端，方法由相同命令目录生成。调研时官方入口重定向到 2026-07-28，该版改为请求级版本/能力元数据与 `server/discover`，移除了旧 initialize 握手。M12 必须选择并记录实际支持的版本：新版本按新协商规则验收；若兼容旧客户端，则在版本化 adapter 中保留其初始化流程，不能将两种协议混用。[官方规范](https://modelcontextprotocol.io/specification/2026-07-28)、[协议变更](https://modelcontextprotocol.io/specification/2026-07-28/changelog)

测试工具/资源列表和调用、所选版本的结果元数据、错误、取消及通知。应用文件读写 roots 是服务配置，与已变化的 MCP Roots 功能不是同一概念。可变更工具使用显式 operationId 去重，与 JSON-RPC 传输 request ID 分离，适应客户端重新发送新请求的行为。

UI、CLI、MCP 对同一命令必须得到相同文档结果和可用性原因。工具 description 可以本地化，机器字段/ID 不随语言改变；需要文件路径的操作不得通过偷偷弹 UI 对话框补参数。

自动化读/写 roots 分离，路径按真实文件系统 canonicalize，拒绝逃逸和不允许的协议。roots 是应用访问边界，不是抵御本机恶意进程的完整 OS sandbox；需要更强隔离时使用受限进程和句柄级访问方案。

## 8 媒体及自动化验收重点

同一页在编辑器预览、放映和时间采样输出中有相同对象状态。媒体 seek/暂停/循环至少连续重复 20 次，检查旧帧和队列泄漏；设备切换后不崩溃。

CLI 脚本和 UI 分别构造同一文稿，对比规范化模型；远程重复请求不得增加重复页；未认证请求不能读文件或改文稿；MCP 断开不阻止桌面关闭。测试版本、样本和失败日志随阶段记录。


## M10 已实现及未验收范围（2026-10-30）

ShowController虚拟/monotonic时钟、appear/fade/fade-out/fly/move/zoom/spin、点击/与前/之后、重复/反向/rewind、显式段落、none/fade/push/wipe及Morph几何/透明度/同资源裁剪子集已实现。窗口预览与观众复用确定性采样器。演示者的当前画面取实际观众采样，下一页/备注独立；单一时钟代次移交。激光/墨迹保留副本或放弃、黑白屏、原生内部链接/自定义顺序/循环/排练/自动翻页已有测试与真实GUI证据。

M10软件子项done，完整双屏/DPI/镜像/热插拔、真实睡眠唤醒、完整性能矩阵与AT-12仍开放。动态1024/静态2048宽CPU帧、33ms调度不是60fps达标声明；公开PPTX链接/自定义放映设置及全量效果不支持。细节与证据见 [32](32-m10-implementation-report.md)；M11媒体及M12自动化切片已实施，范围和未完成门禁见[33](33-m11-implementation-report.md)、[34](34-m12-implementation-report.md)。
