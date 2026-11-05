# PDF/A-2b 归档输出

CLI 支持 `convert FILE.docx --to pdfa-2b --archive-year YEAR --font PATH:FACE --output NEW.pdf`。年份由调用者提供，取 1–9999；同一来源、字体和年份输出相同字节。字体使用实际字节与 face，用户需具有相应嵌入许可；不提供系统字体下载或在线转换。

归档沿用屏幕页面场景，启用 ICC、XMP 和 PDF/A-2b 验证。缺字体、不可嵌入、场景无效、遗漏块、验证失败或目标已存在都会拒绝；原 DOCX 不被改写。普通 `--to pdf` 保持原来的无日期默认行为。

导出 JSON 的 `independent_conformance_verified` 为 `false`。可用独立工具验证实际输出，例如 veraPDF `--flavour 2b --format json NEW.pdf`。本轮样例覆盖 PDF/A-2b 归档规则，PDF/UA 与完整结构阅读顺序另行验收。[R8 验证记录](../r8/VALIDATION.md#windows-格式与编辑复验2026-11-05)

桌面「文件」页提供「导出 PDF/A-2b…」。先确认当前 UTC 归档年份和字体嵌入许可，再选择新的 `.pdf` 路径。桌面用当前会话文档快照排版，包括尚未保存的已提交编辑。取消不会写文件，确认或选路径时遇到文档变化需要重新导出。缺字、字体限制、图片解码或归档验证失败时拒绝输出。桌面归档路径已有独立产物校验，实屏验收仍待补。
