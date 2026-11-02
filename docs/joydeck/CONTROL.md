# JoyDeck control / MCP 0.7.0

启动 `joydeck-cli control --root ABS_DIRECTORY` 或 `joydeck-cli mcp --root ABS_DIRECTORY`。每行一份 UTF-8 JSON，stdout 仅协议，stderr 为诊断。会话拥有独立无界面文稿；不附着 GPUI 编辑器，没有 TCP/外部网络抓取。所选 root 是这个本地进程的授权范围；这是进程控制边界，不是对同权限本机恶意进程的沙箱。

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
| show.start | 可选 1-based page 或已有 custom 名称；要 target，保存不可变文稿快照 |
| show.action | action=next/previous/goto/pause/black/white，goto 带 1-based page |
| show.tick | seconds=0..3600，确定性采样；暂停时冻结，不使用系统时钟 |
| show.status/frame | frame 带新 path 和 scale=.05..4，输出 PNG；此无界面帧不含系统媒体解码层 |

路径必须绝对且规范化后在 root 内；拒绝 `..`、ADS、越界链接、新输出路径已存在等情况。每行最多 1 MiB，响应最多 4 MiB；最多两个并发导出、16 个保留 job。异步编码阶段为 preparing/encoding/finished，尚无可靠百分比。取消在文件发布前生效；发布已经赢得并发竞争时可返回 completed。结束会话会取消并 join 自有任务。

变更请求按 requestId 和精确序列化载荷去重，重试使用完全相同请求；不同参数复用同一 ID 会拒绝。最多保留 128 次变更与有界响应，不淘汰已执行 ID；满后启动新会话。只读轮询不占变更账本。无界面放映动作也受该上限约束，不适合作为无限时高频流。

MCP 固定实现 [2025-11-25 生命周期](https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle)与 [tools](https://modelcontextprotocol.io/specification/2025-11-25/server/tools)：initialize、notifications/initialized、ping、tools/list、tools/call。工具名 joydeck.control，arguments 为上面的 control 请求，返回 content/structuredContent/isError。当前只支持该修订，未声明 2026 新修订或 MCP Tasks；export jobs 是工具内的本地接口。

验收见 [R6/R7](evidence/R6-R7/README.md)。旧 JoyDeck 的 TCP 控制、桌面附着、完整命令/资源浏览及远程协作不在本切片能力声明内。
