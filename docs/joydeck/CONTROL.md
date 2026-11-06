# JoyDeck control / MCP（R9-UI2）

启动 `joydeck-cli control --root ABS_DIRECTORY` 或 `joydeck-cli mcp --root ABS_DIRECTORY`。每行一份 UTF-8 JSON，stdout 仅协议，stderr 为诊断。CLI control／MCP 会话默认拥有独立无界面文稿；桌面附着仅通过下节显式选项启用，没有 TCP/外部网络抓取。所选 root 是这个本地进程的授权范围；这是进程控制边界，不是对同权限本机恶意进程的沙箱。

control 请求如下，输出含 requestId 和 result 或 error。文稿 ID 在 open/new 时产生、为 JavaScript 安全整数；选择和模型修改都递增控制 revision，engineRevision 单列。保存也递增控制 revision。显式保存后才持久化无界面编辑；断连不自动保存。

```json
{"protocolVersion":"joydeck-control-1","requestId":"open-1","method":"document.open","params":{"path":"C:\\Work\\input.pptx"}}
```

后续操作携带上次返回的 documentId/revision。例如 ID 123、revision 0：

```json
{"protocolVersion":"joydeck-control-1","requestId":"select-1","method":"command.execute","target":123,"expectedRevision":0,"params":{"command":{"command":"select","page":1,"shapes":[]}}}
```

接口集合：

| 方法 | 参数/行为 |
| --- | --- |
| engine.commands | 返回实际支持命令、方法、预算和控制边界 |
| document.new/open | open 带 path；替换脏文稿必须显式 dirtyPolicy=discard |
| document.status/inspect | 状态与兼容报告/对象；inspect 要 target |
| command.execute | command 为当前引擎的有类型命令；要求 target/expectedRevision |
| document.save | path 为新 .pptx/.joydeck；要求 target/expectedRevision；保存后模型标记已保存 |
| document.exchange | path 和 mode=preserve/regenerate；要求 target/expectedRevision；导出副本不改变源/dirty |
| document.close | 要求 target/expectedRevision；脏文稿要求 dirtyPolicy=discard |
| export.start | path 和 options（format/pages/scale/dpi/layout/transparent/seconds/fps）；返回 jobId |
| jobs.get/cancel | jobId；轮询 state/phase/cancelRequested/result/error；无需文稿 target |
| jobs.release | jobId；释放已结束的任务记录和工作线程，已发布文件仍保留；运行中返回错误 |
| show.start | 可选 1-based page 或已有 custom 名称；要 target，保存不可变文稿快照 |
| show.action | action=next/previous/goto/pause/black/white，goto 带 1-based page |
| show.tick | seconds=0..3600，确定性采样；暂停时冻结，不使用系统时钟 |
| show.status/frame | frame 带新 path 和 scale=.05..4，输出 PNG；此无界面帧不含系统媒体解码层 |

路径必须绝对且规范化后在 root 内；拒绝 `..`、ADS、越界链接、新输出路径已存在等情况。每行最多 1 MiB，响应最多 4 MiB，最多两个并发导出、16 个保留任务。单文件导出报告 preparing、encoding、publishing、finished 阶段。PNG／JPEG 和 GIF 另返回 `progress`，含 completed、total、unit，单位分别是 pages 和 frames。单文件发布前取消可阻止发布，目录导出取消会清理本次已写入的图片。发布已经赢得并发竞争时可返回 completed。结束会话会取消并 join 自有任务。

变更请求按 requestId 和精确序列化载荷去重，重试使用完全相同请求；不同参数复用同一 ID 会拒绝。最多保留 128 次变更与有界响应，不淘汰已执行 ID；满后启动新会话。只读轮询不占变更账本。无界面放映动作也受该上限约束，不适合作为无限时高频流。

MCP 固定实现 [2025-11-25 生命周期](https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle)与 [tools](https://modelcontextprotocol.io/specification/2025-11-25/server/tools)：initialize、notifications/initialized、ping、tools/list、tools/call。工具名 joydeck.control，arguments 为上面的 control 请求，返回 content/structuredContent/isError。当前只支持该修订，未声明 2026 新修订或 MCP Tasks；export jobs 是工具内的本地接口。

验收基线见 [R6/R7](evidence/R6-R7/README.md)，当前增量见 [R9-UI2](evidence/R9-UI2/README.md)。旧 TCP 控制、完整命令／资源浏览及远程协作仍未声明支持。

## R9-UI2：能力协商与增量方法

先调用 `engine.commands`：commandSchemaVersion=2、modelSchema=joydeck-v2；`legacyCommandSchemaSupported=false` 表示完整旧 schema 未覆盖，`legacyCommandSubset` 另列受支持的 11 个旧字符串 ID。原有 schemaVersion/commandId 包装不兼容。旧索引从 0 起；schema 2 的 select/at 页码从 1 起，不能混用。完整 16 项对照与未覆盖项见 [兼容矩阵](evidence/R9-UI2/legacy-command-matrix.csv)。

| 新增／恢复方法 | 参数及限制 |
| --- | --- |
| command.validate | 与 execute 相同的 params；支持 schema 2 或广告的旧字符串子集；target/expectedRevision；真实引擎快照验证，无副作用 |
| document.undo/redo | target/expectedRevision；返回真实历史状态，沿用重放契约 |
| document.check | target；当前文稿兼容 warnings、slides、media，不是完整 Office 保真证书 |
| document.compare | target，params.path 为 root 内的参考输入；不改文稿，返回 baseSha256/referenceSha256/differences/truncated |
| document.acceptObject | target/expectedRevision，params 为 page（从 1 起）、object（报告中的数值 ID）、baseSha256、referenceSha256；只接受 acceptable=true 的基础对象差异；一项一事务，两摘要及当前内容均校验 |

比较按页序／对象 ID 匹配。页面／文稿属性、资源字节和对象顺序会据实报告，但不支持直接接受。复杂对象不能以“未发现差异”掩盖资源变化；截断结果由 truncated=true 标识，不是完整结果。接受一次后重比；对已变化文稿复用旧摘要会拒绝。CLI 同时提供 `joydeck-cli compare BASE REFERENCE`。

旧字符串参数示例（target/revision 取当前响应，不可照抄示例值）：

```json
{"protocolVersion":"joydeck-control-1","requestId":"notes-1","method":"command.execute","target":123,"expectedRevision":4,"params":{"command":"document.design","action":{"type":"notes","value":[0,"演讲备注"]}}}
```

只支持 document.design 的 notes，table.edit 的行列结构／尺寸／TSV／合并拆分，以及 review.edit 的 add/reply/resolve/accessibility；未知 action/field 原子拒绝。transaction.batch 最多 256 项、嵌套深度最多 16，不能继承旧 1000 项上限；嵌套修订守卫拒绝，使用外层 expectedRevision。当前旧批次在起始快照解析已有引用，新增线程后立即按新 ID 回复等依赖查询应拆为后续请求，尚不等于旧 v1 的完整顺序解析。

## 显式绑定桌面

运行 `joydeck.exe INPUT.pptx --control-stdio=ABS_DIRECTORY`，stdin/stdout 使用同一 JSON 行协议。仅该初始窗口安装控制端；默认普通 GUI 不监听，后开窗口不接管 stdin。它在 GUI 线程临时借用该窗口的唯一 Editor，能力返回 attachedDesktop=true、transport=desktop stdio；不会创建影子可编辑模型。

new/open/close 必须通过该窗口 GUI 操作，控制请求明确拒绝。GUI 切换文稿产生新 documentId 并清理旧比较／重放／作业状态；GUI 改动和选择也会推进控制 revision。加载、保存、组合输入或拖动期间返回 busy，使用新 requestId 重试。排队最多一项、30 秒超时、1 MiB 请求与已有输出/root/重放预算继续生效。关闭绑定窗口后不重定向到其他窗口。附着端尚未提供桌面 MCP 分发／TCP 入口。

真实传输和最终 GUI 验证分别登记于 [R9-UI2](evidence/R9-UI2/README.md)，机器协议测试不替代实际窗口编辑验收。
