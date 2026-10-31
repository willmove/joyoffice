# M13 本机资格化与发布维护实施

2026-10-31，macOS 27.0.1 / Apple M4 / 24GiB，ARM64。已交付本机开发资格化、偏好/原生打开与发布维护子项；**完整三平台 Beta/AT-20 尚未通过**，M0–M12真实开发与设备门禁继续保留。

## 实现

- 偏好schema2：最近文件≤12、窗口大小/缩放/格式栏/备注，schema1迁移，64KiB读取预算、坏配置保留副本后重置、未来schema拒绝覆盖。文件锁协调多窗口/多实例，更新代次避免迟到结果回退UI缓存，0600文件与原子提交。窗口保存使用WindowBounds，避免每次恢复丢失标题栏高度。
- macOS本地file URL事件、中文/空格路径、文件/编辑菜单；已保存原生文件重复打开激活已有窗口。独立进程仍独立编辑，通过文件指纹避免静默覆盖。菜单撤销/重做复用当前焦点的键盘路由。默认文件关联由bundle声明，未修改用户的默认应用选择。
- macOS App改为自包含Mach-O入口，内含CLI、媒体/打印/视频helper、帮助、构建/SBOM/许可材料；从仓库外移动路径实际启动。标准开发ZIP不覆盖旧包。
- 483项目标平台Cargo正常/构建依赖SPDX 2.3清单，经官方JSON schema验证。复制可获得的原始许可，另从Cargo记录的精确VCS提交补齐39项notice并记录来源/hash；16项文本来源缺口仍开放，完整再分发审查未通过。
- 离线release-tool：可信渠道预期SHA256、私有副本解包、路径/重复/符号链接/容量检查、全文件manifest校验、reader兼容门槛、独立版本目录、原子active指针、上一版本保留及回退。正式mac校验需要固定Team ID；无此参数须显式允许开发包。工具不下载或启动程序。
- diagnostics只输出版本/平台/能力；不接收或输出文稿正文、路径、环境和token。帮助说明支持范围、恢复、输出损失、包校验和回退。stage/CI入口扩展到M14；无Git元数据，远端CI未执行。

## 验证

- 128份原创变体，8组文字/几何/表格/图表数据/图片裁剪/组织/审阅/动画；每份native重开、强制公开DrawingML导入和320宽渲染。python-pptx独立检查1152页、896张公共图表、标题和数据、表格/图片及hash。**外部授权文稿数为0**，不替代100份独立外部Office语料。[清单](evidence/m13/qualification-20261031/corpus/manifest.json)、[独立读取](evidence/m13/qualification-20261031/corpus/independent-verification.json)。
- 600.001秒seeded引擎运行，370081次命令、740次原子保存/重开，undo/redo/模型验证，撤销上限100。命令p95 0.136ms、p99 0.1802ms；这是所测模型的命令耗时，不是输入到显示延迟。RSS采样从进程3:32开始，最大观测58464KiB，未测到启动期峰值。[运行](evidence/m13/qualification-20261031/soak.json)、[RSS](evidence/m13/qualification-20261031/rss.json)。
- 122项Rust测试、41项headless子集、fmt/Clippy/release通过；Windows MSVC和Linux x86_64无界面核心完成交叉check/Clippy。交叉检查没有链接GUI或在目标OS运行。
- 最终包24次原生窗口开关、120次编辑/undo、偏好从磁盘恢复及窗口高度不缩小，真实画面/输入回归；App峰值RSS约214MiB。迁移App后系统动态依赖检查、中文空格file事件、重复打开和原生菜单通过；这不是干净机器安装。[本机GUI索引](evidence/m14/final-1793404073463954000/verification.json)、[移动App](evidence/m14/final-1793404073463954000/native-integration/verification.json)。
- 离线更新14项正常/负例检查通过，覆盖暂存、升级、同版本重试、失败保留旧指针、格式门槛、越界/链接/额外文件及损坏回退拒绝。[维护验证](evidence/m13/final-release-verification.json)。
- M4四场景CPU/GPUI像素、release性能及450MiB内存预算；M5–M12原生/公开格式/放映/打印取消、M3/M8系统拼音均复跑。十分钟媒体时钟沿用相同SHA-256 helper的M11记录，不扩称声学/A-V同步认证。

全量回归曾发现媒体probe临时目录按wall clock命名在并发时重名。现统一用OS随机128bit目录名并在创建时设置0700，新增128次并发分配/权限检查；视频编码目录同样使用该分配器，修复后全量回归通过。

## 发布门禁

本机有效签名身份为0；未执行正式签名/公证。Windows/Linux桌面实际链接/GUI、三平台干净安装/卸载/默认关联、读屏/复杂脚本、双屏/DPI/热插拔/音频设备/睡眠、完整BENCH-SMALL/LARGE与端到端p99、外部Office语料、16项notice文本补齐和完整再分发审查仍开放。前序DocumentStore、统一文字布局、完整组/母版、Office评论/媒体往返、异步jobs/PDF阅读顺序等也仍是开发任务。

可运行包及每项结果以[最终索引](evidence/m14/final-verification.json)为准。M12包、开发前源码保留；不把本机切片改记为三平台Beta done。

依据：[SPDX 2.3规范](https://spdx.github.io/spdx-spec/v2.3/)、[官方JSON schema](https://raw.githubusercontent.com/spdx/spdx-spec/v2.3/schemas/spdx-schema.json)、[Cargo许可字段](https://doc.rust-lang.org/cargo/reference/manifest.html#the-license-and-license-file-fields)。

最终开发包的运行二进制与上述verified目录中的GUI/协议证据逐个SHA-256匹配；本机及跨目标检查、包/源码映射见最终索引。

前序收尾还包括导入媒体后裁切时长/音量回显同步，以及导出忙时保留原任务取消token；最终GUI实际重导入18秒MP4显示终点18秒。自动化验证输出增加独立路径，M12历史记录已从开发前归档恢复并与原二进制hash核对。

前序跨文稿粘贴已修复ID分配：同时避开源文稿、目标文稿和本次已分配ID，避免新ID碰撞引起动画target二次改写。确定性双对象/双动画用例和原有设计回归通过。

原生菜单补测发现撤销/重做处理器误注册在“打开”动作内部，导致通过OS文件事件打开后菜单不可用。现改为启动时注册，并延迟到GPUI释放窗口借用后投递焦点按键；回归从中文空格路径打开、添加页并保存，再点击菜单undo/redo/undo并保存，独立读取native页数为10/9/10/9，无需先打开文件对话框。修复前失败日志保留。
