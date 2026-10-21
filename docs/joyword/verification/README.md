# 验证证据

本目录记录 2026-10-20 验证波次（S1、S2、S3、S5）与其后缺口修复（F1，[#67](https://github.com/willmove/joyoffice/pull/67)）的自动证据。这些都不是人工验收：没有图形界面操作、Word／WPS 对照、真实打印或信创真机。任务状态以 [TASKS](../TASKS.md) 为准，原子准则见 [验收清单](../acceptance/README.md)。

| 文档 | 切片 | 内容 |
| --- | --- | --- |
| [页面场景金样](golden-scenes.md) | S1（[#64](https://github.com/willmove/joyoffice/pull/64)） | 页面场景、PDF 摘要与 CPU 光栅金样 |
| [T103 覆盖引导 fuzz](t103-coverage-fuzz.md) | S2（[#62](https://github.com/willmove/joyoffice/pull/62)） | 以 LLVM 源码区域计数器为反馈的 DOCX 读取器 fuzz 与夜间工作流 |
| [特性样例](feature-fixtures.md) | S3（[#65](https://github.com/willmove/joyoffice/pull/65)） | DOCX-FEAT-* 原创样例的导入、往返与 CLI 投影 |
| [场景排练](scenario-rehearsals.md) | S5（[#63](https://github.com/willmove/joyoffice/pull/63)） | 经命令注册表的无界面场景排练 |
| [验证波次缺口修复](verify-wave-fixes.md) | F1（[#67](https://github.com/willmove/joyoffice/pull/67)） | 上面几份记录里五条已知缺口的根因、修复与回归测试 |

F1 的两条修复有限制：正文页码只覆盖无格式域；`wrapTight`／`wrapThrough` 只是方形近似，不是轮廓绕排。T203 图片关系另报 `not-imported` 的诊断不在 F1 范围，已由 [#69](https://github.com/willmove/joyoffice/pull/69) 修复。

2026-10-21：[#71](https://github.com/willmove/joyoffice/pull/71)（`e8c47764`）把 W09 从固定提交改为检出的 `crates/` 树 id，并去掉 CI 的 `fetch-depth: 0`。[T103 覆盖引导 fuzz](t103-coverage-fuzz.md) 正文里的提交锚点是 2026-10-20 那次活动的记录。麒麟修复会话的机器侧证据由 [#70](https://github.com/willmove/joyoffice/pull/70)（`f7cbeadd`）落库，见 [IMPLEMENTATION](../IMPLEMENTATION.md)「麒麟缺陷修复会话的机器侧证据」，不在本目录。
