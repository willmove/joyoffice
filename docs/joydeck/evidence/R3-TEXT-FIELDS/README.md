# JD-305 动态文字字段

2026-11-05，Apple M4／macOS 27.2，1× 实屏。仓库外解压 ARM64 Release 验收包 `joydeck-0.8.0-r3-text-fields.zip`，1,805 项载荷散列一致。归档 SHA-256 为 `2c41798e2fa1081aa84af078b3bbfd7c45a6468fc602782ceb7a41f3711d1650`。[原创三页样例](text-fields.pptx) · [验证数据](verification.json)

选中文本框后，从「插入 → 字段」插入动态幻灯片编号或日期／时间。已有文字选区整体替换，只选中文本框时追加到框尾。日期／时间提供 13 种格式，短日期使用简体中文区域设置，其余格式使用英语。原位光标和选区以实屏排版定位，源缓存长短与实时显示不同时，方向键和删除仍以整块字段操作。[DrawingML 字段](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.field?view=openxml-3.0.1) · [MS-OI29500 日期格式](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oi29500/209a8afb-4ce6-4ad9-ad6b-f18da263502e)

| 路径 | 实测结果 |
| --- | --- |
| 导入与原位编辑 | 原始 PPTX 缓存 `99999`，首屏显示 `1`。从 Ribbon 追加第二个编号，使用实际方向键跨过原字段并键入 `!`，模型确认符号出现在原字段之后。画布显示 `No. 1! \| New: 1`。[导入](01-imported-live-number.png) · [光标编辑](02-atomic-number-caret.png) |
| 日期与历史 | 从日期选单选择短日期，插入 `datetime1`、语言 `zh-CN`、值 `2026/11/5`。一次 Backspace 删除整块字段，原生撤销一步恢复。[日期选单](03-date-format-menu.png) · [插入结果](04-dynamic-date.png) |
| 另存与重开 | 原生 `.joydeck` 内嵌原件逐字节一致。另存 PPTX 后，独立 XML 核对原编号字段的规范化 XML、GUID 和扩展属性一致，新增两字段各有唯一 ID。包内 CLI 读回日期；独立桌面进程重开仍显示字段。[重开](05-reopened-pptx.png) |
| 页序变更 | 通过包内 CLI 将首张移至末尾，独立桌面进程重开并点击第 3 张。截屏文字识别确认两个编号均显示 `3`。输入 PPTX 的 SHA-256 保持一致。[重排结果](06-reordered-live-numbers.png) |

JoyDeck 产品检查 919 项通过，0 失败、0 忽略。新增自动回归覆盖 UTF-16 范围内的整体替换及字段实时显示伸缩的坐标映射。包内 CLI 通过 R1、R2、R3、R4/R5 的 28、20、20、23 项复验以及 329 条命令注册表核对。

JD-305 继续开放。固定日期、整份文稿的页脚与应用到全部页面尚待实现，缩略图悬停标题仍取原文缓存。含公式、缓存为空或跨行的原有字段沿用原位编辑保护。涉及段落结构或无法无损映射的组合编辑可保存为 `.joydeck`。
