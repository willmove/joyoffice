# R6/R7 0.7.0 文件输出、放映与控制切片

2026-11-03，Windows x86_64 MSVC release 开发包：`joydeck-0.7.0-r6-r7-windows-x86_64-release-e5aab1f38b.zip`，位于仓库 `dist/joydeck`，28,135,426 字节。解压整个目录运行 joydeck.exe；操作与限制见 [包说明](../../../../packaging/joydeck/README-r6-r7.md)，接口见 [CONTROL](../../CONTROL.md)。完整阶段仍按 [任务清单](../../TASKS.md) 推进。

## 产物与基线

- ZIP SHA-256：`72efc300c5878229a316611871d88b12366bea4bc5d566ad9f0071e5dbf39c29`。
- 构建源码指纹：`e5aab1f38badc5bd5983d2e73e95039e331aa94d4e437e1800b8c7746e4b7ae7`。
- [包清单](package.json)、[最终构建日志](logs/accepted-package.txt)、[验收汇总](accepted-verification.json)、[源码与 Git 对象逐字节核对](source-index-verification.json)。784 项 SHA256SUMS 校验通过，桌面主线程栈 8 MiB。
- [最终仓库外运行记录](gui/final/run.json)：中文空格目录运行，最终桌面 EXE `62efececc529060a8f870bf7b643f5d9bf36146a0461d7c1ee191bf32d5d1691`；输入原件未变，所有验收窗口与原生 helper 均已退出。
- Rust 1.98.1、GPUI Kit 0.8.1/pre 0.3.10 不变。Krilla 官方 crates.io 0.8.2、关闭默认 feature，仅 raster-images，与 JoyWord 一致，无 craft-libs 全局 patch。新增 6 项 registry 依赖的版本/来源/checksum 全部与固定 JoyOffice lock 一致，见 [依赖增量](dependency-delta.json)。
- JoyOffice 基线 a973f75d7d58ab8dd5f05b0e3db5f0c53137d57d 保持 clean；DeckCraft 755 文件无变化，见 [上游只读检查](upstream-read-only.json)。私有旧 DTO/格式 31 文件只参与一次性转换，不进入新编辑状态。来源、许可与适配指纹已登记。

未签名开发包，实际 normal/build SBOM 455 包；22 项上游包缺少可直接收集的许可全文（包括本次官方 Krilla），明细在 package.json。系统字体与 codec 不分发，正式发行审计仍待完成。Windows helper 使用本机 .NET Framework/WinForms/WPF/WinRT，源、编译器、SDK 和二进制指纹随包登记。

## 可运行切片

| 范围 | 当次行为与边界 |
| --- | --- |
| 旧文件 | 基础 schema 1.0–1.11 DTO/能力验证，实际测试 1.0 与 1.11；通过公开 OOXML 接入新模型。v2 附带精确原件、转换警告、旧页/对象 ID 到公开对象的映射。旧媒体、审阅/辅助信息、放映链接和未知/未来必需特性明确拒绝；旧设计/字段解析为结果，未重建完整继承。 |
| PPTX 交换 | preserve 定向保留源部件；regenerate 显式生成支持子集的 PPTX/POTX/PPSX 副本并提示损失。未知部件保留对照、签名/宏拒绝与原件保护均有测试。 |
| PDF | CPU 图形加准确定位的可检索字形子集/ToUnicode、链接和书签，备注与讲义。字体集合索引/variation 保留、嵌字许可检查、溢出/预算/取消报错。非全矢量、非 PDF/A/标记 PDF。 |
| 图片/文本/视频 | PNG/JPEG、既有动画采样 GIF、提纲/RTF、静态 MP4 与标题 VTT。MP4 不包含对象动画、媒体、音频或备注；VTT 不含演讲者备注。输出新路径、页范围、大小/像素预算与取消回滚。 |
| 打印 | 自有快照 helper 打开实际 Windows 系统打印窗口，已验证取消返回 0/cancelled 和清理，无实体打印/Spooler 作业提交。冷启动/前台切换较慢，需激活系统窗口；此体验问题保留在 JD-606。当前打印仅幻灯片。 |
| 放映 | 独立观众/演示者、隐藏/既有自定义/范围/循环顺序、既有动画与转场时刻采样、暂停/黑白/全屏、临时墨迹/激光。备注仅演示者窗口。共享 CPU 状态渲染，未声明 R4 GPUI 对象场景覆盖所有放映内容。 |
| 媒体 | 每页一个顶层本地内嵌片段，Windows WPF 异步解码/播放（帧交付上限 640×360、约 10Hz）、默认静音、暂停/seek/循环/基本裁剪。暂停 seek 在解码纹理更新后返回，最终桌面实际像素已验证变化。多片段、旋转/翻转、跨页、音频设备与完整音画同步仍待资格化。 |
| 控制 | 版本化独立无界面 control/MCP 2025-11-25 stdio；选择也递增修订，文稿 ID、目标/修订校验、重放、新路径/根限制、2 并发/16 保留导出 jobs、取消与 EOF 清理。不是桌面附着控制，无 TCP；无界面编辑需显式保存。 |

## 自动验证

[最终产品检查](logs/accepted-product-check-final.txt)：860 项 Rust 测试、5 项 Python 测试，fmt、严格 Clippy、依赖/CLI 隔离、261 项引入源码指纹和文档门禁均通过。早期 slice/prototype 日志保留排错过程，不代替最终验收。

同一最终 CLI 二进制已完成 95 次主回归：[R6/R7 32 次](accepted-r6-r7-final.json)、[R3 20 次](accepted-r3.json)、[R4/R5 23 次](accepted-r4-r5.json)、[R2 20 次](accepted-r2.json)。R2 历史阶段断言仅在执行内存副本改为 R6-R7，原行为断言未改。最终 UI/媒体修正后 CLI hash 与前轮一致，并重跑 R6/R7 与 decoder 验证。MCP 最终 38 条响应，覆盖初始化、工具发现、真实 PPTX 编辑/保存、重复去重、过期修订、导出 jobs/取消、确定性放映与黑屏。

[5 次附加故障调用与 3 个 stdio 生命周期场景](accepted-faults.json)：带签名/宏部件哨兵的故障包拒绝保留但可显式再生（不代表签名验证或真实宏互操作验收）、旧媒体拒绝迁移、真实存在的越界输入拒绝、1 MiB 过长行关闭、EOF 取消导出。[5 次 DeckCraft 导入/项目重开/公开交换/PDF](accepted-deckcraft.json) 通过。R2–R5 保存、撤销重做、强杀恢复、文字/表格、图片/图表、未知 XML 和像素对照持续通过。

[独立交付检查](accepted-delivery-final.json)：pypdf 校验三页 PDF、三页备注、单页三合一讲义，中文可检索、字体均嵌入且有 ToUnicode、各 3 个书签与实际 URL。Poppler 72 DPI 图形对 CPU 参考的平均通道误差 1.967029/255（阈值 5），已目视核对 [幻灯片](pdf/slides.png)、[备注](pdf/notes.png)、[讲义](pdf/handouts-3.png)。独立 ISO BMFF 读取确认 MP4 仅视频轨，WPF 实际解码播放/暂停/seek，输出帧均有非空画面且 seek 前后不同。

## 实际最终桌面

[gui/final](gui/final/run.json) 的 20 张截图与原生窗口记录对应最终 e5aab1f38b 包。此前 gui/accepted 是 a5525eabdb 的联调记录，其中暂停 seek 未刷新被发现并修复，不把旧失败帧作为最终通过证据。

- 800×600 深/浅色工作区、可见输出表单；桌面备注 PDF 实际生成。联调同时验证 DPI 999 原位报错、保留已填内容；最终同一表单逻辑复测输出成功。
- 系统打印窗口实际显示，点击取消；[结果](gui/final/print-result.json) 为 exit 0/cancelled，未点击打印。
- [演示者备注与未播放](gui/final/04-presenter-stopped.png)、[超过 5 秒片长后的循环播放](gui/final/06-presenter-playing-loop.png)、暂停及 seek 后新帧 [像素验证](gui/final/paused-seek-pixels.json)。
- 墨迹、清除、激光、黑白屏隐藏注记、F11 实际全屏、右方向键跳过隐藏第二页、Esc 配对退出；正文未变，换页/退出后 helper 不残留。
- 旧 1.11 真实文件在桌面另存新的 v2 并重开；[附件/ID 独立读回](gui/final/migration-readback.json) 精确匹配旧原件，转换警告仍显示。
- 真实 PowerPoint 淡入动画：[触发前](gui/final/17-motion-before.png)、[中间](gui/final/18-motion-middle.png)、[完成](gui/final/19-motion-completed.png)，[像素亮度单调变化](gui/final/motion-pixels.json) 验证，不用静态占位代替动画。

## 后续范围

完整 R4–R7/329 命令覆盖仍进行中。旧媒体/审阅迁移、全部设计继承/复杂 OOXML、动画编辑/反向转场/触发矩阵、GIF 图片/组合时序、排练持久化、真实多屏/DPI/睡眠/设备/完整音画同步、附着桌面的控制、实体打印和 macOS/Linux 原生验收仍在任务清单。当前打印窗口置前体验与系统时长未知时的媒体资格化需要继续改进。没有以本轮基础切片宣称整阶段完成。
