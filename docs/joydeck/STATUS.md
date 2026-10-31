# JoyDeck 开发状态

当前开发入口为 JoyOffice Monorepo，参见[开发指南](DEVELOPMENT.md)与[迁移计划](../joyoffice/MONOREPO-MIGRATION.md)。[迁移结果及新布局验证](../joyoffice/MONOREPO-RESULT.md)已单独登记。下述数字和证据是迁移前M13/M14历史基线，不改写为新验收结果。

2026-10-31软件任务增量：M12机器接口已覆盖16个已注册领域命令，新增无副作用校验/修订号保护；M14新增显式复制图片依赖并按单事务接受。见[40实施与本轮证据](40-command-and-image-dependencies.md)。解锁后已补齐交付包的前台/像素及新增图片按钮实际点击、保存和单次undo/redo，原锁屏失败记录保留。尚未完成的jobs、版式/主题依赖与三方合并继续开放。

2026-10-31迁移后收尾：已修复比较接受新增对象的图层顺序、媒体元数据差异漏报、远程替换文稿/新连接的ID同步、CLI默认操作ID跨进程冲突，以及恢复/保存命名的同刻度冲突；新增回归并保留原子拒绝/undo/redo。实施与本轮新构建验证见[收尾报告](39-m14-closeout-report.md)，下述历史数字保持原义。完整父阶段与尚未实现的功能池仍开放。

更新：2026-10-31。**M13本机资格化/发布维护，以及M14静态页MP4、离线比较/逐对象接受、标题WebVTT已交付；完整三平台Beta和M14其余功能池仍开放。** 已按实测关闭本机子项，未改变原父任务的完整验收要求。见[任务清单](11-task-backlog.md)、[M13报告](36-m13-implementation-report.md)、[M14报告](37-m14-implementation-report.md)。

本轮增加schema2偏好与最近文件/布局恢复、坏配置隔离与未来版本保护、mac菜单/文件事件/重复打开激活、自包含App、483项SPDX依赖清单与构建/源码hash、脱敏诊断、离线安全暂存/版本选择/回退。视频输出为静态页H.264 Main MP4，无音轨/动画/转场/录制；VTT只含页标题，私密备注不进入字幕。离线比较可逐对象接受更改并撤销，已有资源/版式依赖才可接受，尚非完整三方合并/实时协作。

122项Rust、41项headless子集、fmt/Clippy/release通过；Windows MSVC与Linux x86_64无界面核心跨目标check/Clippy通过，未在目标OS运行或链接GUI。128份原创变体经native/公开读取与python-pptx独立验证1152页/896图表；外部授权语料数仍为0。600秒seeded运行370081次命令、740检查点、100项历史上限通过；命令p95/p99不是显示时延，RSS采样未覆盖启动段。

最终包真实比较/视频导出后播放、24次窗口开关/120次编辑undo、偏好恢复、移出仓库后的中文/空格文件事件、原生菜单与重复打开通过；M0–M12本机原生/画布/输出/放映/打印取消与普通文字/表格系统拼音回归有记录。并发媒体probe目录重名已修复，临时目录统一使用OS随机128bit名称并在创建时设0700。详见[本轮最终索引](evidence/m14/final-verification.json)。

最新[macOS ARM64开发包（旧归档：joydeck-m14-final-darwin-arm64.zip）](../joyoffice/JOYDECK-ARCHIVES.md) · [源码归档（旧归档：joydeck-m14-final-source.tar.gz）](../joyoffice/JOYDECK-ARCHIVES.md) · [开发前回退（旧归档：joydeck-before-m13-m14-source.tar.gz）](../joyoffice/JOYDECK-ARCHIVES.md) · [M12回退包（旧归档：joydeck-m12-verified-darwin-arm64.zip）](../joyoffice/JOYDECK-ARCHIVES.md)。JoyDeck.app已自包含，可独立移动。包中有CLI、媒体/打印/编码三个helper、依赖/SBOM/帮助和hash材料；原生writer保持schema1.10/minReader11，旧程序拒绝新文稿，保留旧文件。开发检查为`cargo xtask check`、`cargo xtask stage M13/M14`；stage不替代GUI/整阶段验收。

正式门禁：有效签名身份0，未正式签名/公证；三平台干净安装/卸载/默认关联、真实外部Office语料、复杂文字/读屏、双屏/DPI/镜像/热插拔/音频设备/睡眠、完整BENCH-SMALL/LARGE与端到端性能、16项notice文本来源及完整再分发审查尚未完成。macOS移位运行不等于干净机安装；跨目标编译不等于目标平台验收。原独立目录的 CI 配置已到 M14；当时无 Git 元数据、未执行远端 CI。Monorepo 验证与新 CI 状态单独记录。

前序仍含真实开发缺口：M0–M2 DocumentStore和完整故障/大文稿/跨平台矩阵；M3统一布局、自动换行与bidi；M4真实device-lost与完整trace；M5完整组/连接符/SVG/自由路径；M6完整母版/本地化/读屏；M7复杂公开格式/未知字段与授权外部语料；M10真实双屏矩阵；M11多轨/ShowClock音频master/设备/A-V/公共媒体；M12 Office评论、完整无障碍、异步jobs/取消、全部命令与PDF阅读顺序/跨平台打印。它们没有被改记为只等硬件。

M14其余长尾格式、全量设计/动画、高级文字、硬件GPU互操作、额外平台/插件、完整合并/协作、Tagged PDF与录制等，继续按独立任务推进。历史实现/证据见[31](31-m9-implementation-report.md)、[32](32-m10-implementation-report.md)、[33](33-m11-implementation-report.md)、[34](34-m12-implementation-report.md)、[35](35-m11-m12-gui-verification.md)。
