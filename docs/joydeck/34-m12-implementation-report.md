# M12 审阅、自动化与输出实施

2026-10-30，macOS ARM64。**M12a/b/c均有可运行开发切片；完整M12仍开放。** 111项Rust测试、实际协议/格式验证和release包按本机范围记录，不代表AT-18/19/11全通过。

## 审阅与文稿检查

独立Review模型：稳定页/对象ID锚点、离线可配置作者、线程/回复/解决、alt text/装饰、独立阅读顺序。`Command::Review`与UI、CLI、MCP共用事务/history。线程≤1024、回复≤128、正文单项≤8192字节、总正文≤1MiB；非法输入回滚，删除锚点在同一事务清理，undo恢复。阅读顺序必须为该页完整且唯一的对象排列，新增对象会补入末尾。

“审阅”页签提供真实操作面板及可定位检查结果。检查顶部可见标题、图片/图表/图示替代文字、低透明文字，检查本身不改文稿。不是完整对比度算法、Tagged PDF、WCAG或读屏认证。评论/阅读顺序目前原生保存；Office legacy/modern评论的解析/重新生成与完整PPTX回归未完成。有损导出明确列出评论/媒体不进入公共格式的损失。

## 自动化

`joydeck-cli`通过`--no-default-features --bin joydeck-cli`真实独立构建；GPUI Kit为optional desktop依赖。commands/describe、inspect、run、render、convert、mcp、serve；退出码0成功/2参数/3读取/4操作或转换/5传输，stdout只放JSON或JSON-RPC，stderr诊断。run是最多128条**顺序独立事务**，遇错中止，不承诺跨请求/文件原子回滚。

独立Dispatcher接protocolVersion/requestId/method/target/params，共享SharedSession单一历史。当前自动化命令page.add/object.add/review.edit及undo/redo，其他领域目录尚未全部开放。每请求≤1MiB、响应≤4MiB、去重≤1024条且≤16MiB；相同ID相同请求回原结果，同ID不同参数拒绝，容量满后拒绝新请求。inspect省略资源bytes/source包，不将正文放日志。

读写roots分开、绝对路径、canonical父目录、禁止..与符号链接逃逸，输出必须是新文件。该边界不是针对恶意本机进程的句柄级OS sandbox。默认无listener；只有用户显式开启输出面板本机控制或运行CLI serve时监听127.0.0.1随机端口。getrandom生成256bit token，0600新建凭据文件，≤8个同步连接/至多8个等待或运行请求；3秒网络读取预算。Windows凭据adapter暂不支持。关闭删除凭据、取消尚未开始请求；已经执行的副作用不会自动重放。断线/30秒超时明确outcome-unknown，只以相同ID重试。

桌面开启时Dispatcher持有相同SharedSession，外部变更进入相同undo，不能远程替换整个文稿。修复macOS accept继承nonblocking导致分包JSON提前报错；真实socket测试覆盖认证拒绝、正确调用、重复不新增页、桌面撤销。

MCP明确锁**2025-11-25** stdio adapter，initialize→notifications/initialized，与2026新版server/discover不混用。tool `joydeck_dispatch`、resource摘要、ping、稳定operationId与传输ID分离；Headless和`--connect /absolute/descriptor.json mcp`远程均有实际客户端验证。没有异步JobRegistry、running取消/进度通知，暂不声称完整MCP/本地控制规格done。

## 输出

原有生成式SVG场景经svg2pdf/pdf-writer转换：可见路径/文字为矢量，字体subset/ToUnicode嵌入，图片及复杂filter按能力局部raster。多页与页书签、受支持内部页链接输出；无Tagged PDF，独立reading_order尚未写入PDF语义。输出单文件≤64MiB、页间取消；库内单页转换尚非可抢占取消。

静态GIF为640宽、每页1秒的页面序列；纯文字/RTF大纲只导出页标题/对象文字，不含媒体/动画/完整表格/图表语义。POTX/PPSX使用真实template/slideshow main content type并沿用PPTX有损边界，不含MP4编码输出。

macOS打印用同一矢量PDF快照与AppKit/PDFKit系统对话框，取消/打印不会写文稿；纸张/范围/方向/缩放由原生面板提供。解锁后修复helper的AppKit启动与初始纸张方向，默认按页面宽高选择横/纵向，显式开放纸张/方向/范围/缩放控件。实际GUI切换方向、页范围2–3、缩放80%后取消，文稿内容/revision/dirty保持相同，临时PDF清理。本机未配置打印机，未提交真实打印作业。Windows/Linux只有PDF导出回退，print adapter未实现。

## 验证证据

- `logs/m12-gui-final-check.log`：111项Rust测试，覆盖旧M0–M10领域/文件回归及新增审阅/自动化/媒体；`logs/m12-gui-final-clippy.log`、`logs/m12-verified-package.log`记录fmt/Clippy/release。
- `docs/evidence/m12/automation.json`：真实stdio客户端对Headless/Remote分别完成初始化、工具/资源、operationId去重、冲突、dirty-new拒绝；未认证socket拒绝、关闭清理。最终包重新验证的哈希以JSON/最终索引为准。
- `docs/evidence/m12/run-1793372269286823000/verification.json`：实际GPUI handlers运行、视频帧进入GPU图片、评论/undo/native重开及输出。**nativeOnly=true，panelsCaptured=0，不算窗口视觉验收。** 两页PDF独立pypdf提取、pdffonts确认嵌入subset/ToUnicode；Poppler渲染对CPU参考平均RGB误差0.476/255。GIF帧数/时长、POTX/PPSX content types独立检查。
- `docs/evidence/m12/headless/vector.pdf`与`vector.png`：CLI独立生成七类图表/图示矢量PDF、中文提取与渲染已人工查看；没有以栅格页伪装矢量。新版书签/链接再随最终包复验。

解锁后真实GUI验收已完成：审阅/媒体/输出面板、实际视频预览与全屏放映、打印预览设置与取消、前序M0–M10和系统拼音回归。修复预览图被flex压缩，检查结果显示页/对象并返回画布定位；开发构建优先选择本次编译helper，避免拾取target上层旧适配器。详见[35验证报告](35-m11-m12-gui-verification.md)。旧native-only运行仍不算GUI证据。

## 交付与开放项

`dist/joydeck-m12-verified-darwin-arm64.zip`为未签名开发包，含CLI与媒体/打印helper；writer schema1.10/minReader11/review-v1/media-v1，旧文件读取/旧程序拒绝新文件有回归。备份与M10包保留。CI入口更新至M12并加headless及macOS helper构建，但远端未执行。

T-M12-11在所声明静态输出范围done；其余父任务仍保留尚未实现或未验证部分：Office评论往返、完整文稿无障碍、异步job/取消与全部命令、PDF阅读顺序、三平台打印/真实打印。M0–M7的统一文字布局、DocumentStore、完整组/母版、授权外部语料等仍为真实开发缺口；没有把它们改记成只等硬件。M13未开始。

公开依据：[MCP 2025-11-25生命周期](https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle)、[svg2pdf 0.13 API](https://docs.rs/svg2pdf/0.13.0/svg2pdf/)。许可证与通用依赖见THIRD_PARTY和licenses。

最终交付证据以[本轮验证索引](evidence/m12/final-verification.json)为准：最终包111项全量/32项headless子集检查通过；媒体helper与10分钟测试hash一致；原生handler/PDF/格式与Headless/Remote MCP实际复验通过。最新真实GUI/格式目录为`evidence/m12/final-gui-1793374588146617000/m11-m12`，nativeOnly=false、3个面板及视频/打印截图通过；前序回归与修复细节见[35](35-m11-m12-gui-verification.md)。
