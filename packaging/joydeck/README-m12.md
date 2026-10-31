# JoyDeck M11/M12 macOS ARM64 开发预览

双击 `JoyDeck.app`，或运行 `./joydeck --authoring-demo`。默认是空白文稿。
这是未签名/未公证开发包；完整 M11/M12 尚未验收。本机已验证审阅/媒体/输出窗口、实际视频预览及全屏放映、原生打印设置和取消；本机没有打印机，未提交打印作业。

- 审阅：作者、页/对象评论、回复、解决、替代文字、装饰与独立阅读顺序、可定位检查；原生保存与撤销。公开PPTX的新评论输出和完整无障碍检查未完成。
- 媒体：在媒体面板填写本地文件绝对路径并导入。PCM/WAV、MP3、H.264 Baseline/Main 8-bit ≤1080p30 的 MP4（可含AAC）；单资源≤8MiB，总量≤32MiB。选择媒体后设置裁切秒数/音量/循环/自动播放；预览独立于文稿undo。自动播放视频在放映中居中覆盖画布；一次一项，跨页仅音频。设备切换/音轨混合、完整A/V同步标定和三平台媒体adapter仍开放。
- 输出：新文件路径、矢量PDF、纯文字/RTF大纲、每页1秒静态GIF、POTX/PPSX；后两项继承PPTX有损范围。macOS系统打印使用原生对话框，可取消；实际打印/三平台验收未完成。
- 自动化：`./joydeck-cli commands`；`--read-root /absolute/read --write-root /absolute/write inspect /absolute/file.joydeck`；`convert /absolute/input /absolute/output.pdf`。参数选项放命令前，输出默认不覆盖。
- MCP：`./joydeck-cli --read-root /absolute/read --write-root /absolute/write mcp`。只实现2025-11-25 stdio，先initialize再notifications/initialized。joydeck_dispatch工具参数包括稳定operationId、method、target、params。资源为当前文稿摘要。
- 本机控制默认关闭。输出面板填写读取/写入根目录，点击开启；状态栏显示0600凭据文件路径。`./joydeck-cli --connect /absolute/descriptor.json mcp`连接当前桌面文稿，共享撤销。不要共享含token的凭据文件。同步串行执行，至多8连接；断开/超时后结果未知，只以相同requestId/operationId重试；不支持异步job/cancel。

writer schema1.10/minReader11/review-v1/media-v1，读取旧1.0–1.8。旧程序拒绝新文件，请保留原文件和M10回退包。

验证脚本包含真实窗口、原生handler、格式和MCP测试。需要未锁屏桌面及Pillow/numpy/pypdf/Poppler；`--native-only`只验证原生状态与格式，不能作为窗口截图通过证据。10分钟音频基准只证明AVPlayer位置与host时钟关系，不是声学/音视频同步认证。

前序真实双屏、跨平台、统一文字布局、完整组/母版、外部语料等门禁继续开放。详见源码中的STATUS和33/34实施报告。未建立Git提交，无正式发布承诺。
