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

Windows 接入标准 `HTML Format` 与 `Rich Text Format` 读取。macOS 上以 `x86_64-pc-windows-msvc` 交叉检查桌面 crate 和 CLI 通过，Windows 桌面实屏待验收。Linux 物理桌面及 Wayland 实测、CSS 样式表、内联图片、RTFD 和完整 Office 对象交换继续由 JD-307 追踪。

Linux 使用 GPUI 实际选择的显示后端。X11 需要 PATH 中的 `xclip`（CLIPBOARD selection），Wayland 需要 `wl-clipboard` 提供的 `wl-paste`（常规 selection）。适配器只读取提供方公开的 HTML／RTF MIME 类型，复用共享解析器。辅助程序不可用、合成器拒绝读取、超时或富格式无可用内容时，沿用 GPUI 的纯文本回退。Wayland 的可读性取决于合成器的数据控制协议或 `wl-paste` 的兼容路径。XWayland 的 DISPLAY 不会改变 GPUI 已选择的后端。富格式读取共用 400 毫秒时间预算，JoyDeck 每次接收的辅助程序输出最多 1 MiB。

2026-11-06，Linux x86_64 在 Xvfb 和 Mesa lavapipe 下运行 JoyDeck 桌面构建。独立 Tk 进程提供多格式剪贴板，通过 Ctrl+V 验证画布粘贴的 HTML 优先、RTF 回退、纯文本回退，以及原位 HTML 粘贴。一次画布或原位富文本粘贴只增加一次编辑修订，单步撤销成功，画布重做成功。保存的 PPTX 通过独立 `python-pptx` 和 JoyDeck CLI 读回，验证中文、emoji、粗体、斜体和非法链接剥离。该结果覆盖虚拟 X11 桌面。Wayland 合成器在验证环境无法建立 Unix socket，Wayland、物理 Linux 桌面及 macOS／Windows 桌面复验均未运行。

Linux JoyDeck 产品构建、严格 Clippy 和产品测试通过，默认和 `validation` 两套测试各通过 927 项。三产品单线程回归为 5,831 通过、3 忽略、1 失败。失败来自 JoyWord 的 Linux 安全扩展属性测试。独立探测确认当前环境可设置并直接读取 `security.selinux`，但列举扩展属性时没有返回该名称。

Debug 版从后台打开本轮生成的原生项目会在反序列化 `RunProps` 时触发线程栈保护。上一切片的 Debug 二进制也复现，当前 Release 原生打开通过。该样例用于后续修复和回归。[Debug 重开复现样例](debug-reopen-sample.joydeck)


## RTF 隐藏文字自动回归

2026-11-06，Linux x86_64 无桌面环境，基于主干 `7b37ad4c884706087983d352d4a11148040de02a`。此次仅验证共享 RTF 解析器，不沿用以上历史桌面验收结果。

原解析器忽略 `\v`，将隐藏字符作为可见内容导入。两项新增测试先在原实现失败：`Before{\v secret{nested}}After` 得到 `BeforesecretnestedAfter`，隐藏代码页字符、中文、emoji 和换行也进入输出。修复后隐藏状态随组继承／恢复，`\v0` 和 `\plain` 恢复可见文本；隐藏字符与隐藏段落／软换行不进入导入模型。字符属性含义依据 [Microsoft Text Object Model 的 RTF 对照表](https://learn.microsoft.com/en-us/windows/win32/controls/about-text-object-model)，实现未复制其他编辑器代码。

- `cargo test -p joydeck-format -p joydeck-pptx`：45 + 60 项通过，0 失败、0 忽略，包含两项新增回归。保留现有代码页、字符样式、链接、安全协议和输入预算测试。
- `cargo clippy -p joydeck-format -p joydeck-pptx --all-targets -- -D warnings`、`cargo fmt --all --check`、`python3 scripts/check_docs.py` 均通过。Rust 使用 1.98.1，测试和检查关闭 debug 信息及增量编译。
- 单独 `cargo test -p joydeck-format` 遇到既有依赖配置问题：`zip` 启用 `deflate-flate2` 却未选压缩后端。本轮使用产品已有 `joydeck-pptx` 依赖提供的 `flate2/rust_backend`；未修改依赖或锁文件。
- 首批修复仍把纯隐藏 RTF 当作无可用富文本并回退；后续修复见下节。
- 当前修复未运行 macOS／Windows／物理 Linux／Wayland 系统剪贴板与桌面粘贴、撤销、保存重开验收；JD-307 继续开放。


## 纯隐藏 RTF 的回退边界

同一分支在 `cbc3f6617fe7b752f68b3ef0f09503c16798e91c` 基础上继续修复：有效 RTF 明确启用隐藏属性且无可见内容时，粘贴应消费该输入，不读取来源应用的纯文本替代内容，也不删除当前选区或新建空文本框。这是依据已有隐藏文字过滤行为确定的产品处理，不要求来源应用的不同剪贴板表示互相一致。

新增 `import_rtf_for_paste` 保留三种结果：有可见内容、已解析的隐藏空内容、不可用。原 `import_rtf` 的非空导入契约保留。macOS／Windows 适配器及 Linux 的 RTF MIME 别名路径使用新接口；原位、画布和表格入口消费隐藏空内容后直接返回并提示。Linux 不继续尝试其他 RTF 别名或纯文本。

解析回归覆盖纯隐藏、组作用域恢复、隐藏换行、可见剩余文字、空 RTF、损坏输入及超限输出。损坏、超限、缺失 RTF 仍保持原回退行为；HTML 优先策略保持不变。混合可见／隐藏内容的表格 TSV 路径仍使用来源应用的纯文本，未在本批改变其列分隔语义。上述边界不能视为任意来源剪贴板的完整隐私保证。

当前代码的原生系统剪贴板、选区保持、表格与画布无操作行为仍需真实桌面复验；未宣称完成 JD-307 验收。

验证结果：

- `cargo test -p joydeck-format -p joydeck-pptx`：46 + 60 项通过，0 失败、0 忽略；对应严格 Clippy 通过。
- 临时轻量 Cargo 测试宿主直接以 `#[path]` 包含生产 `native_clipboard_linux.rs`，使用真实 `joydeck-format`、`joydeck-model`、`joydeck-engine` 和 `rustix`；仅把 `joydeck_shell::clipboard_import` 重导出映射到相同的 format 模块，避免引入 GUI 链接。生产模块原有和新增的 4 项测试全部运行通过，覆盖隐藏 RTF 四种 MIME／别名、HTML 优先与引擎撤销重做、普通回退、辅助进程输出／超时预算。宿主未调用实际系统剪贴板。
- 常规桌面 `cargo check` 因环境缺少 `fontconfig.pc` 失败；使用依赖官方支持的 `RUST_FONTCONFIG_DLOPEN=1 cargo check -p joydeck-ui-gpui --tests` 后通过。相同配置下 `cargo clippy -p joydeck-ui-gpui --all-targets -- -D warnings` 通过。此项是 Linux 编译检查，不是桌面运行或其他平台编译结果。
- `cargo fmt --all --check`、`python3 scripts/check_docs.py` 通过。依赖及锁文件未修改。
