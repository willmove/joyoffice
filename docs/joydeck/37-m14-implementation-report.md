# M14 可运行扩展与开放功能池

2026-10-31。M14原路线为独立立项功能池。本轮交付静态页MP4、离线比较/逐对象接受、标题WebVTT三个扩展；**其余功能池与前序完整阶段继续开放**。

## 静态页视频与标题字幕

独立Swift/AVFoundation AVAssetWriter helper编码H.264 Main MP4，Rust提供BGRA静态页流。默认960宽/15fps/每页2秒；偶数宽320–1920、高≤1080、1–30fps、每页1–30秒，总≤1800帧，编码≤10分钟，单文件≤64MiB。生产者通道容量1；取消/失败杀进程、回收写线程/0700临时目录，不覆盖目标。CLI必须acceptLosses；GUI明确标注“静态页MP4（无音轨/动画）”。不含动画、转场、嵌入视频、声音、录制、旁白或摄像头。

WebVTT按同一静态视频时间轴输出页标题，转义标记、合并换行，私密演讲者备注不输出。当前是独立sidecar标题字幕，不是自动语音转录或MP4内嵌字幕轨。

最终包GUI导出并重新导入/实际播放；另一套AVAssetReader完整解码确认270帧、960×540、18秒、PTS单调、1视频轨/0音轨。首帧与CPU参考平均RGB误差0.907/255。真实编码/probe、运行中取消、已存在目标保护和VTT时间轴/隐私测试通过。[结果](evidence/m14/final-1793404073463954000/m13-m14/verification.json)、[输出面板](evidence/m14/final-1793404073463954000/m13-m14/panel-3.png)、[导出后播放](evidence/m14/final-1793404073463954000/m13-m14/panel-4.png)。

## 离线文稿比较与对象接受

比较按稳定页/对象ID报告文稿设置、页/对象新增删除、顺序、属性、设计/审阅/媒体摘要，最多4096项并标注截断。不比较源PPTX容器和诊断警告；资源按内容hash标识，不把原始媒体bytes放报告。

逐对象接受新增、删除或替换进入原Session事务/history，需目标已有相关图片/版式资源；依赖不满足时整体校验失败。请求校验base/reference SHA-256，比较后修改须重新比较，避免接受旧结果。页面/主题/审阅等差异仅报告。GUI、CLI compare、Dispatcher/MCP document.compare/acceptObject共用实现；重复操作继续采用原有operationId去重。

真实GUI接受标题改动、undo/redo及CPU/GPUI画面通过；CLI比较/接受/撤销同结果，stale guard/删除undo/属性变更单测通过。没有自动合并所有依赖、三方冲突合并、实时协作服务或CRDT。[比较面板](evidence/m14/final-1793404073463954000/m13-m14/panel-1.png)、[接受后画布](evidence/m14/final-1793404073463954000/m13-m14/panel-2.png)。

## 功能池逐项状态

| 原任务 | 本轮实际结果 | 未完成边界 |
| --- | --- | --- |
| T-M14-01 长尾格式 | 保留既有PPTX/native支持与公开损失声明 | 二进制PPT/ODP/宏/加密独立实现、授权语料和验证未完成 |
| T-M14-02 动画设计长尾 | 现有M9/M10基线继续回归 | 全量效果、复杂chartex/SmartArt/3D和母版扩展未实现 |
| T-M14-03 高级文字 | 现有中英文/emoji/IME回归 | 真竖排、公式、warp、拼写字典等未实现 |
| T-M14-04 视频/录制 | 静态页静音MP4可用，取消/再播放通过 | 动画/音轨/转场/嵌入媒体/录制/同步扩展未实现 |
| T-M14-05 硬件互操作 | 稳定CPU像素路径与OS编码helper | 未证明零拷贝、硬件解码/编码路径或跨OS收益 |
| T-M14-06 比较协作 | 离线差异+逐对象接受/undo可用 | 完整依赖迁移、三方冲突/实时协作未实现 |
| T-M14-07 额外平台/插件 | portable headless核心跨目标check/Clippy | 无新增可运行BSD/Web/插件或其他平台GUI产物 |
| T-M14-08 高级输出 | 页面标题VTT时间轨可用 | Tagged PDF/独立阅读顺序/语音字幕等仍待实现 |

因此仅T-M14-04a/06a/08a按声明子集done，不把整池勾为完成，也不将无障碍语义、硬件或正式发布验收移出原阶段。

## 交付与回退

writer保持schema1.10/minReader11，偏好独立schema2，不为导出/比较改写文稿格式。CLI仍可无GPUI构建，macOS helper随包；其他OS的MP4返回明确不支持。已有原生媒体、矢量PDF、打印、MCP边界与前序开放项仍按33–36报告执行。

最终[构建/测试/GUI/包hash索引](evidence/m14/final-verification.json)。开发前源码、M12包与本轮中间包保留，无Git提交或正式发布声明。

接口依据：[Apple AVAssetWriter](https://developer.apple.com/documentation/avfoundation/avassetwriter)。

最终开发包的运行二进制与上述verified目录中的GUI/协议证据逐个SHA-256匹配；本机及跨目标检查、包/源码映射见最终索引。

比较接受新增对象使用保留参考ID的独立事务，拒绝重复ID；新增对象接受后重比为0差异，undo/redo保持ID，打包CLI与独立headless均已实测。无稳定ID的旧版对象明确拒绝比较，需先在编辑器另存为含ID的原生文稿，避免把多对象误并为一项。
