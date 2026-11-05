# JD-307 外部 HTML 与 RTF 粘贴

2026-11-05，Apple M4／macOS 27.2，1× 实屏。仓库外解压 ARM64 Release 验收包 `joydeck-0.8.0-r3-external-rich-verified.zip`，1805 项载荷散列一致。归档 SHA-256 为 `e9d41d150d75a8d0e804ba0602271c15d5324f76cc4e7484a681f9af58b0662c`。[输入文稿](fixture.pptx) · [HTML 样例](web-fragment.html) · [RTF 样例](office-fragment.rtf) · [验证数据](verification.json)

外部文字粘贴按 HTML、RTF、纯文本顺序取可用数据。原位粘贴修改当前选区，画布空白处粘贴创建可编辑文本框。内部富文字、对象、图片和表格 TSV 继续沿用各自路径。HTML 只提取剪贴板片段与支持的静态样式，RTF 处理组内格式、常用代码页和 UTF-16 单元。网页与邮件链接经过现有协议白名单。解析在内存中完成，只处理剪贴板输入，受输入大小、嵌套深度、输出文字与 run 数量约束。[AppKit 剪贴板格式](https://developer.apple.com/documentation/appkit/nspasteboard/pasteboardtype) · [Windows CF_HTML](https://learn.microsoft.com/en-us/windows/win32/dataxchg/html-clipboard-format) · [HTML 分词标准](https://html.spec.whatwg.org/multipage/parsing.html#tokenization) · [Microsoft RTF 规范引用](https://learn.microsoft.com/en-us/openspecs/sharepoint_protocols/ms-wordswcf/a9968e4b-4603-41a5-bfa6-280d4e4d0685)

| 路径 | 实测结果 |
| --- | --- |
| HTML 原位粘贴 | 独立 AppKit 进程同时写入 HTML 和故意不同的纯文本。Cmd+V 选择 HTML，保留中文、emoji、粗体、斜体、字族、字号、颜色、下划线和网页地址。脚本文字隐藏，文件链接的标签仍显示。单步撤销与重做通过。[HTML 编辑](01-html-inline.png) |
| RTF 画布粘贴 | 系统剪贴板仅写入 RTF。Cmd+V 创建一个文本框，保留字体、字重、颜色、软换行、段落和链接。撤销恢复原文稿，重做恢复文本框。独立 python-pptx 检查另存的公开 PPTX。[RTF 画布](02-rtf-canvas.png) |
| 保存与重开 | 通过原生另存分别写出 `.joydeck` 和 PPTX。两份文稿在各自独立的读取进程里重开，模型与画布上的文本、格式和地址保持。输入 PPTX 散列保持。[原生 HTML 重开](03-native-html-reopen.png) · [公开 RTF 重开](04-public-rtf-reopen.png) |
| 回退 | HTML 只有不可见内容时选择同一次复制中的 RTF。RTF 缺失闭合组时选择系统提供的纯文本，分别产生一次修订。[回退结果](05-fallback-order.png) |

JoyDeck 产品检查 924 项通过，0 失败、0 忽略。三产品联合验证通过 5,824 项。新增自动回归覆盖 CF_HTML 字节偏移、格式和安全链接、RTF 中文代码页与 emoji、损坏与超限输入。包内 CLI 验证 R1、R2、R3、R4/R5 的 28、20、20、23 次调用和 329 条命令注册表。同一 Release 包重复验证纯文本、内部富文字、PNG、跨文稿对象、失败剪切和公开 PPTX 图片复用。

Windows 接入标准 `HTML Format` 与 `Rich Text Format` 读取。macOS 上以 `x86_64-pc-windows-msvc` 交叉检查桌面 crate 和 CLI 通过，Windows 桌面实屏待验收。Linux 原生富格式、CSS 样式表、内联图片、RTFD 和完整 Office 对象交换继续由 JD-307 追踪。

Debug 版从后台打开本轮生成的原生项目会在反序列化 `RunProps` 时触发线程栈保护。上一切片的 Debug 二进制也复现，当前 Release 原生打开通过。该样例用于后续修复和回归。[Debug 重开复现样例](debug-reopen-sample.joydeck)
