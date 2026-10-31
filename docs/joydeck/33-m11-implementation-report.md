# M11 媒体实施与验证

2026-10-30，macOS ARM64开发预览。**本机媒体软件切片已实现；完整M11未done。** 原有M10真实双屏门禁与M0–M7开发缺口保留。

## 已实现

- 独立Swift/AVFoundation进程，不引入FFmpeg或第三方演示引擎。probe实际读取音视频轨、时长、codec、avcC profile和fps，拒绝不支持/损坏内容。启用PCM/WAV、MP3、H.264 Baseline/Main 8-bit MP4（允许AAC），≤1920×1080/30fps/1小时；High/HEVC等不提前开放。
- 单资源≤8MiB、文稿媒体≤32MiB、≤64clip/asset。原始bytes按SHA-256独立存放media/<hash>.bin；元数据与clip参数进入模型。导入后台探测有10秒超时，晚到结果检查session/revision；删除页面清理锚点与未引用资源，undo恢复。
- 原生媒体面板可导入、选择、裁切、音量、循环、自动播放、跨页音频，参数原子撤销。播放位置/解码帧不写入文档。预览有play/pause/seek/stop，adapter随Player销毁退出并清理0700临时目录。
- AVPlayer处理系统音频输出及媒体时钟，AVPlayerItemVideoOutput提供真实解码BGRA。后台读线程只保留最新一帧；显示≤960宽、≤960²像素；共享帧与临时磁盘帧有界。seek递增generation并在Rust与helper两端丢弃旧代次，切页停止非跨页clip；放映暂停/继续联动。
- 原生放映每页选首个自动播放clip；视频居中覆盖画布，cross-slide仅音频。保留相同跨页音轨，不随切页重新启动；当前不是多轨混合/任意对象矩形视频布局。

## 验证与证据

`tests/playback.rs`覆盖PCM导入/资源hash/非法参数回滚/undo/native重开，MP3实际播放，H.264 Main + AAC 1080p30真实解码、暂停、连续20次seek和循环、帧长度预算、损坏媒体受控失败。新增自建媒体的生成源在apps/joydeck/native，来源说明见tests/fixtures/README.md及THIRD_PARTY。

`docs/evidence/m11/audio-clock-bench.json`记录实际600秒音轨运行、17k以上时钟采样及RSS。最终运行约599.83秒、17,996次采样，最大偏差60.35ms、adapter RSS 45,536KiB，helper哈希与最终包一致；测量是AVPlayer位置对host monotonic时钟，**不是声学端到端延迟或音视频同步漂移标定**。首轮约600秒、最大偏差86ms；最终helper已重跑。

解锁后最终包真实窗口已复验：视频预览与暂停帧像素比较、全屏自动播放视频截图、暂停/停止、媒体native保存重开通过。修复纵向flex布局将预览图压缩到零高度的问题，预览固定保留165pt高度。旧锁屏native-only运行仅保留为历史。详见[GUI验证报告](35-m11-m12-gui-verification.md)。

## 未完成门禁

T-M11-02完整多轨mix/可观测PCM回调/音频master联动整个ShowClock未实现；当前由系统AVPlayer独立保持媒体内部时钟。T-M11-03视频指定矩形/编辑画布海报、M11-04多clip与完整重访/跨页策略仍待。设备显式切换、拔除/断音频/恢复、完整A/V drift标定、设备矩阵和长会话BENCH-MEDIA仍待。公开PPTX媒体导入/写回与timing尚未实现；原PPTX/native源包闭包保留不等于媒体可播放。Windows/Linux媒体adapter未实现。

因此T-M11-01–07父任务保留开放；下面任务清单的a子项按实测范围done，AT-13不整体验收。Swift编译有macOS27弃用提示，当前兼容API可以构建，不等于已验证所有SDK/OS。无麦克风、摄像头或录制权限申请。

## 运行与回退

最终包`dist/joydeck-m12-verified-darwin-arm64.zip`含joydeck、joydeck-cli、两个原生helper、许可证和自建fixture；双击JoyDeck.app。源码回退`dist/joydeck-before-m11-m12-source.tar.gz`与M10分发包保留。当前writer schema1.10/minReader11，旧程序拒绝新文稿；保留旧文件。无Git元数据，未创建提交；未签名/公证/干净机器安装。

接口依据：[Apple AVPlayerItemVideoOutput](https://developer.apple.com/documentation/avfoundation/avplayeritemvideooutput)。

最终交付证据以[本轮验证索引](evidence/m12/final-verification.json)为准：最终包111项全量/32项headless子集检查通过；媒体helper与10分钟测试hash一致；原生handler/PDF/格式与Headless/Remote MCP实际复验通过。最新真实GUI/格式目录为`evidence/m12/final-gui-1793374588146617000/m11-m12`，nativeOnly=false、3个面板及视频/打印截图通过；前序回归与修复细节见[35](35-m11-m12-gui-verification.md)。
