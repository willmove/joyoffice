# JD-307 macOS 剪贴板与 PPTX 图片复用

2026-11-05，Apple M4／macOS 27.2，1× 实屏。仓库外解压 `joydeck-0.8.0-r3-clipboard.zip`，1,805 项载荷散列一致。归档 SHA-256 为 `bb0c720c218672bdd54f7c740dc075c197a7caf7ff2c2596d5a437d541c1e2fb`。[源文稿](source.pptx) · [目标文稿](destination.pptx) · [外部图片](external-image.png) · [验证数据](verification.json)

| 场景 | 实测结果 |
| --- | --- |
| 富文字与纯文本 | 两个独立 JoyDeck 进程以系统剪贴板传递含 emoji 的粗体文本。目标文稿的一次撤销恢复原文字，重做恢复粗体。独立 AppKit 写入的纯文本也可粘贴并单步撤销。[跨文稿文字](01-rich-text-cross-document.png) |
| 基础对象跨文稿 | 通过 Shift+点选同时复制图片和表格，目标文稿分配新对象 ID，保留表格文字。一次撤销移除两对象，重做一次恢复。尝试剪切不受支持的图表后，源文稿和先前的对象剪贴板均保持。[失败剪切](02-unsupported-cut.png) · [对象粘贴](03-cross-document-objects.png) |
| 系统图片 | 独立 AppKit 进程写入 PNG，经系统剪贴板插入一张图片。一次撤销只移除这张图片，重做恢复。[系统图片](04-native-image.png) |
| 公开格式 | 另存 PPTX 后，python-pptx 核对粗体文字、表格和三张图片。独立 ZIP／XML 读取确认已有图片与粘贴的同内容图片均引用原 `ppt/media/image1.png`，系统 PNG 使用另一部件。源、目标输入的散列均保持。[另存结果](05-saved-transfer.png) |

修复前，相同图片已在内存模型复用，PPTX 保留保存仍把字节相同的图片写成第二个部件。现在按内容类型和字节匹配无依赖的图片，保留原部件和现有关系，为新增对象指向同一部件。[保留保存实现](../../../../crates/joydeck-pptx/src/preserve_content.rs) · [公开格式回归](../../../../crates/joydeck-shell/tests/editing.rs)

JoyDeck 产品检查 920 项通过，0 失败、0 忽略。包内 CLI 复验 R1、R2、R3、R4/R5 的 28、20、20、23 项，命令注册表核对 329 条。

JD-307 继续开放。另用两个独立文稿复现，源富文字中的页内链接保留源页 ID，另存目标原生格式后，这个 ID 在目标文稿中指向无关页面。复现来自前一版验收包，本次图片修复未改链接处理。外部 HTML／RTF、完整 Office 对象互换及复杂对象继续开放。[链接来源](link-source.pptx) · [链接目标](link-dest.pptx)

页内链接误绑定已在后续切片修复。保留本页复现作为修复前证据。[同文稿与跨文稿复验](../R3-CLIPBOARD-LINKS/README.md)
