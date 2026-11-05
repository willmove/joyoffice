# JD-305 文字超链接

2026-11-05，Apple M4／macOS 27.2，1× 实屏。仓库外解压 ARM64 Release 验收包 `joydeck-0.8.0-r3-text-links-desktop.zip`，1,805 项载荷散列一致。归档 SHA-256 为 `85c810fe8cd9199345960e87a052279d01820e1314fc6f60f0b9c83f98c0d662`。[原创三页样例](text-links.pptx) · [验证数据](verification.json)

在原位选择已有文字，通过「插入 → 链接」或 Cmd/Ctrl+K 添加网页、邮件或 `#页码` 链接及屏幕提示。光标在已有链接内时，编辑或移除覆盖它跨不同字符样式的连续范围。页码在应用时解析为稳定的幻灯片 ID。放映端以同一文字排版产生的点击区域识别文字链接，并沿用形状遮挡与可见性判断。[DrawingML hlinkClick](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.hyperlinkonclick?view=openxml-3.0.1) · [MS-OI29500 页内跳转](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oi29500/a65b76db-6abc-4989-8cd1-baa9a3500f6f)

| 路径 | 实测结果 |
| --- | --- |
| 选择与输入校验 | 选中跨粗体、斜体的 `Plan`，相邻 emoji 和原有链接保持。拒绝文件协议和超界页码，修订保持 0。新增链接一次提交，原生撤销和重做各一步。[协议校验](01-unsafe-destination.png) · [页码校验](01b-invalid-slide-number.png) · [混合选区](02-web-link.png) |
| 编辑与保存 | 原生另存网页链接 PPTX，独立 XML 和 python-pptx 验证地址、提示、分段字符格式、原有链接及两处未知 XML 扩展。将光标放回链接并改为邮件地址，原生 `.joydeck` 经包内 CLI 读回，内嵌原件逐字节一致。 |
| 页内跳转 | 改为 `#3` 后，PPTX 关系指向第 3 页部件。独立进程重开，在全屏放映点击 `Plan`，从第 1 页直接跳到第 3 页。自动回归另验证重排后的稳定目标和实际文字命中。[放映首屏](03-internal-link-in-show.png) · [跳转结果](04-link-destination.png) |
| 移除 | 重新打开带页内链接的 PPTX，移除 `Plan` 的链接后另存。独立 XML 核对目标文字已解除链接，其他链接、格式及未知扩展保留，输入文件散列保持。[移除结果](05-removed-link.png) |

JoyDeck 产品检查 917 项通过，0 失败、0 忽略。包内 CLI 通过 R1、R2、R3、R4/R5 的 28、20、20、23 项复验以及 329 条命令注册表核对。

JD-305 仍开放。插入新的链接显示文字、动态字段和其余格式命令继续推进。涉及正文重写的组合编辑和无法安全映射的源链接 XML 使用 JoyDeck 原生格式保存。外部链接的地址和持久化已验证，真实点击放映复测覆盖页内跳转。
