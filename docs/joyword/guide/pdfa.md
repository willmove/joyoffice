# PDF/A-2b 归档输出

CLI 支持 `convert FILE.docx --to pdfa-2b --archive-year YEAR --font PATH:FACE --output NEW.pdf`。年份由调用者提供，取 1–9999；同一来源、字体和年份输出相同字节。字体使用实际字节与 face，用户需具有相应嵌入许可；不提供系统字体下载或在线转换。

归档沿用屏幕页面场景，启用 ICC、XMP 和 PDF/A-2b 验证。缺字体、不可嵌入、场景无效、遗漏块、验证失败或目标已存在都会拒绝；原 DOCX 不被改写。普通 `--to pdf` 保持原来的无日期默认行为。

导出 JSON 标明目标与 independent_conformance_verified=false。需要独立工具验证实际输出，例如 veraPDF `--flavour 2b --format json NEW.pdf`；本轮样例的独立验证和真实显示见 [报告](../verification/m8-archive-20261030.md)。该配置不表示 PDF/UA 或完整结构阅读顺序合规。目前尚无桌面 PDF/A 菜单入口。
