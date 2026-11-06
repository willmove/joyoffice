# JoyDeck JD-300 共用文字排版复验

2026-11-05，Apple M4、macOS ARM64。以仓库外中文空格路径中的 Release App 与同包 CLI 验证。原创 [三页 OOXML 样例](mixed.pptx) 由 python-pptx 1.0.2 生成。它包含中英阿拉伯文、组合重音、非 BMP emoji、换行、保存的自动缩放参数、旧缓存页码字段及其后的链接，以及相同显示尺寸的缩放组合和普通文本框。样例引用的 Hack 在验收 Mac 未安装，文稿保留原字体名称，两个渲染入口使用运行时替代结果。

- 窗口在 GPUI 与 CPU 两种模式显示同一混排区域。原位编辑可选中完整段落，用系统键盘插入一个字符，撤销／重做后仍保留全部原有字符。原生另存为写出 v2，包内 CLI 独立读回编辑后的文字和格式，源 PPTX 散列保持一致。[GPUI](01-gpui-mixed-layout.png) · [CPU](02-cpu-mixed-layout.png) · [原位选择](03-inline-mixed-selection.png) · [编辑](04-inline-mixed-edit.png) · [保存](05-saved-mixed-layout.png)
- 旧 R2 包显示正确页码，PDF 搜索层却提取出缓存字段，漏掉后续文字和链接。新版 `pdftotext` 读回 `1 FIELDTAILVISIBLE`，`pdfinfo -url` 读回链接，第一段的多脚本、组合符、emoji 和最后一行也都可提取。
- 普通与缩放组合页的 CPU 像素相同。新版 PDF 中 14 个搜索词的文本、换行和边界坐标一致，最大坐标差为 0 pt。Poppler 在 72 dpi 生成的三页图像与 CPU 同尺寸渲染逐页比较，RGB 通道平均绝对误差为 0.460、0.305、0.305。
- 自动回归逐项比对编辑几何和输出放置的 glyph ID、坐标、行边界和 caret。覆盖双栏、固定 autofit、双向文字、组合字符边界、非均匀组合缩放，以及字段后的软换行链接。

`cargo xtask check joydeck` 通过 906 项测试，失败与忽略均为 0。使用 `cargo xtask package joydeck --release --validation --name joydeck-0.8.0-r3-layout` 打包，解压核对 1,805 项文件散列。同包 CLI 通过 R1 28 项、R2 20 项、R3 20 项及 329 条命令注册对照。[产物与读回数据](verification.json)

本次验证覆盖共用排版与导出。实时字体变化和字节／face／variation 身份由 JD-301 跟踪，旋转／竖排原位输入与其他 DPI 由 JD-302 跟踪。
