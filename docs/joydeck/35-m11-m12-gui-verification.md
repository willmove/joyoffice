# M11/M12 解锁后 GUI 与收尾验证

2026-10-30，macOS ARM64、一个物理显示器。最终包为 [joydeck-m12-verified-darwin-arm64.zip](../dist/joydeck-m12-verified-darwin-arm64.zip)。本机软件子项已按实测更新为 done；完整 M11/M12、前序未实现功能和多平台/硬件门禁保持开放。

## 本次修复

- 媒体预览图处于纵向 flex 滚动面板时被压缩到零高度；固定保留165pt，使解码画面实际可见。
- 打印 helper 完成 AppKit 启动后再显示面板，控件可被系统辅助功能读取；依据第一页宽高选择初始横/纵方向，开放纸张、范围、方向与缩放控件。
- 审阅检查结果显示页号/对象位置，点击后切回画布；开发构建优先使用本次编译 helper，避免上层 target 残留适配器覆盖新版本。
- macOS 截图带显示器 ICC 配置，原脚本直接比较其 RGB 数值和 sRGB 参考图。统一转换到 sRGB 后比较，原像素阈值保持不变。原 M9 失败记录保留，修正后平均误差 GPUI 0.227/255、CPU 0.471/255，平坦区0.0435/255，均通过。

## 最终包实测

| 项目 | 结果与证据 |
| --- | --- |
| 审阅、媒体、输出面板 | 3个真实窗口截图；评论/回复/解决/undo、native重开和输出通过，[记录](evidence/m12/final-gui-1793374588146617000/m11-m12/verification.json) |
| 视频预览与放映 | 实际 H.264 视频暂停帧与截图比较平均RGB误差3.281/255；全屏自动播放截图已查看，[媒体](evidence/m12/final-gui-1793374588146617000/m11-m12/panel-1.png)、[放映](evidence/m12/final-gui-1793374588146617000/m11-m12/video-show.png) |
| 打印设置/取消 | 默认横向，切换纵/横、页范围2–3、缩放80%，显示A4纸张控件；点击Cancel后文稿内容/revision/dirty保持相同，[设置截图](evidence/m12/final-gui-1793374588146617000/m11-m12/print-settings.png)、[状态断言](evidence/m12/final-gui-1793374588146617000/m11-m12/print-unchanged.json) |
| M0–M4 原生回归 | M0/M1/M2/M3/M4全通过，[日志](evidence/m12/regression-1793374344236729000/m0-m4.log) |
| M4 性能/图形/内存 | 4种CPU/GPUI场景像素、release渲染及450MiB内存预算通过，观测RSS 211.11MiB，[记录](evidence/m12/final-gui-1793374588146617000/m4/verification.json) |
| M5/M6/M7/M8 | 原生窗口、CPU/GPUI像素、剪贴板/文件/全屏、公开格式独立读取及表格回归全通过，[本轮记录](evidence/m12/final-gui-1793374588146617000/verification.json) |
| M9/M10 | 7类公共图表/工作簿、CPU/GPUI画面、放映计时/暂停/黑白屏/导航、快照、焦点恢复及单屏窗口时钟移交通过，[记录](evidence/m12/m9-m10-icc-verified/verification.json) |
| 系统拼音 | 普通文字和表格的候选窗、提交、取消、单次撤销、native重开通过；使用System Events合成键盘事件，[文字](evidence/m3/pinyin-1793374407716800000/result.json)、[表格](evidence/m8/pinyin-1793374416051958000/result.json) |
| 自动/无界面 | 111项Rust、fmt/Clippy、32项headless子集通过；独立无默认features构建/命令执行，无GPUI正常依赖；真实Headless/Remote MCP通过 |
| 分发/回退 | 80项文件哈希全部匹配，源码归档包含根目录tests与媒体fixture；媒体helper与原600秒时钟测试哈希相同；旧分发包和开发前源码保留 |

全部路径、二进制及ZIP哈希见 [最终验证索引](evidence/m12/final-verification.json)。M0–M4、M9/M10与系统拼音曾以同SHA-256的前一包目录运行；当前包只更新QA脚本/说明，运行二进制完全相同。M4–M8和M11/M12在当前包复跑；不覆盖旧失败证据。

本机未配置打印机，未提交真实打印作业。原native-only运行仍不是GUI通过证据；普通文字/表格拼音由系统输入法与合成事件验证，不声称物理键盘或跨平台测试。单屏窗口验证不代替真实双屏/DPI/镜像/热插拔/睡眠唤醒。

## 可复现入口

使用带Pillow/numpy/pypdf/python-pptx的Python和Poppler，在未锁屏桌面运行；GUI脚本依次执行，避免抢焦点：

```sh
cargo xtask check
python3 scripts/verify-m11-m12.py --binary dist/joydeck-m12-verified-darwin-arm64/joydeck
python3 scripts/benchmark-m4.py --binary dist/joydeck-m12-verified-darwin-arm64/joydeck
python3 scripts/verify-m9-m10.py --binary dist/joydeck-m12-verified-darwin-arm64/joydeck
python3 scripts/smoke-macos-ime.py --binary dist/joydeck-m12-verified-darwin-arm64/joydeck
python3 scripts/smoke-macos-ime.py --binary dist/joydeck-m12-verified-darwin-arm64/joydeck --table
python3 scripts/verify-automation.py --binary dist/joydeck-m12-verified-darwin-arm64/joydeck-cli
```

实际执行的完整参数记录在各verification.json中，`--native-only`明确跳过截图/打印/放映截图。检查最终包失败时不应把native-only成功替换为GUI成功。

## 仍开放

M11：多轨混音/完整ShowClock音频master、视频矩形/海报、多clip策略、设备切换/拔除/恢复、完整音视频同步及公共PPTX媒体往返、Windows/Linux适配。M12：Office评论解析/重新输出、完整对比度/读屏、异步jobs/取消及余下领域命令、PDF独立阅读顺序/Tagged语义、真实打印及跨平台适配。M0–M7的统一文字布局、DocumentStore、完整组/母版、外部授权语料等仍需开发；M10真实双屏矩阵仍需验证。

T-M11-07a、T-M12-03a、T-M12-10a以本机已声明子集完成；父任务的完整验收不改为done。无Git元数据，未创建提交；开发包未签名/公证，未作干净机器安装。
