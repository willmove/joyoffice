# 验证证据

本目录记录 2026-10-20 验证波次（S1、S2、S3、S5）与其后缺口修复（F1，[#67](https://github.com/willmove/joyoffice/pull/67)）的自动证据。这些早期记录均为自动证据；2026-10-30 新增的 M7／M8 报告另含本机原生 GUI 与 Word／WPS 实际观察，逐项区分。真实打印、其他平台和硬件只按各报告中的实际证据认定。任务状态以 [TASKS](../TASKS.md) 为准，原子准则见 [验收清单](../acceptance/README.md)。

| 文档 | 切片 | 内容 |
| --- | --- | --- |
| [页面场景金样](golden-scenes.md) | S1（[#64](https://github.com/willmove/joyoffice/pull/64)） | 页面场景、PDF 摘要与 CPU 光栅金样 |
| [T103 覆盖引导 fuzz](t103-coverage-fuzz.md) | S2（[#62](https://github.com/willmove/joyoffice/pull/62)） | 以 LLVM 源码区域计数器为反馈的 DOCX 读取器 fuzz 与夜间工作流 |
| [特性样例](feature-fixtures.md) | S3（[#65](https://github.com/willmove/joyoffice/pull/65)） | DOCX-FEAT-* 原创样例的导入、往返与 CLI 投影 |
| [场景排练](scenario-rehearsals.md) | S5（[#63](https://github.com/willmove/joyoffice/pull/63)） | 经命令注册表的无界面场景排练 |
| [验证波次缺口修复](verify-wave-fixes.md) | F1（[#67](https://github.com/willmove/joyoffice/pull/67)） | 上面几份记录里五条已知缺口的根因、修复与回归测试 |
| [第二波验证手册](wave2-manual-verification.md) | 第二波（#89／#91／#90／#92／#93） | T104 生产恢复双代、T202 栏分隔线与章节 PAGE、T204 PDF 内部跳转、T402 授权预览的可执行验证步骤，附手工样例生成方法与逐项可达性说明 |
| [M7 与此前验证收口](m7-20261030.md) | 2026-10-30 | 缓存与 HTML 修复、原生控件语义、全量回归、三级模型测量、便携预览包与发布证据检查；[Windows GUI 与恢复修复](m7-gui-20261030.md) 已记录，完整平台／读屏／签名仍待验收 |

F1 的两条修复有限制：正文页码只覆盖无格式域；`wrapTight`／`wrapThrough` 只是方形近似，不是轮廓绕排。T203 图片关系另报 `not-imported` 的诊断不在 F1 范围，已由 [#69](https://github.com/willmove/joyoffice/pull/69) 修复。

2026-10-21：[#71](https://github.com/willmove/joyoffice/pull/71)（`e8c47764`）把 W09 从固定提交改为检出的 `crates/` 树 id，并去掉 CI 的 `fetch-depth: 0`。[T103 覆盖引导 fuzz](t103-coverage-fuzz.md) 正文里的提交锚点是 2026-10-20 那次活动的记录。麒麟修复会话的机器侧证据由 [#70](https://github.com/willmove/joyoffice/pull/70)（`f7cbeadd`）落库，见 [IMPLEMENTATION](../IMPLEMENTATION.md)「麒麟缺陷修复会话的机器侧证据」，不在本目录。

2026-10-30 后续：[WPS、百页、PDF 与原生可访问性修复](m7-followup-20261030.md)，T208 已 done；以此报告修正前轮对应失败项，保留历史原始记录。

2026-10-30 M8：[本地邮件合并与回归](m8-20261030.md)，T807 已 done；记录模板协议、预算／保护／输出隔离、真实 Windows GUI、Word／WPS 重开与最终构建证据。

2026-10-30 本轮：[焦点、关闭与 EPUB](m8-focus-epub-20261030.md)，窗口句柄与菜单焦点／键盘缺陷关闭，T806 EPUB 第一切片进入 in_progress，PDF/A 和独立阅读器保持未完成。

2026-10-30 后续：[PDF/A 与独立 EPUB 阅读器](m8-archive-20261030.md)，记录 PDF/A-2b 独立合规、NCX 阅读器兼容及格式对话框语义。
