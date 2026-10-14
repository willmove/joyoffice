# W02：同一字体与外部字形绘制探针

分配：ZCode Desktop；分支 codex/w02-gpui-font-probe。ADR-010/T003 的独立实验，Mac 优先，不是正式 GPUI 升级。

## 目标

现有 HarfRust/read-fonts 整形和统一场景 PDF 已有实现。核对公开 GPUI API，验证同一字体字节和既有字形位置的屏幕绘制路径，评估 GL/CPU 渲染要求。

## 本轮工作

1. experiments/gpui-font-probe 使用自己的 [workspace]、精确依赖/完整上游 SHA 和独立 Cargo.lock；可路径引用现有公开 layout/PDF API，不升级根 workspace。
2. 比较固定 Zed commit、gpui-pre 或最小补丁路径，记录 API、许可、构建/平台代价。IMPLEMENTATION 的上游线索只当待复核输入。
3. 原创中英文输入明确字体散列/face；屏幕消费已有字形 id/位置，不能系统文字重新整形后声称同一链路。字体不提交，写明来源/许可。
4. 验证系统同名字体冲突。若改写字体 name 表，修改后的字节须一致用于整形/屏幕/PDF，记录原/新散列、face、许可；不能混用两份字体证明同字节。
5. 记录几何证据与缩放/DPI。不能机器比较的给出人工步骤与待验收项，不凭观感宣称通过。
6. 评估候选 Linux GL/CPU 与信创代价；无真机区分调研和实测。交付推荐方案、未证实事项及正式接入补丁清单。

不做完整 IME、断行或正式应用迁移，不改 FontKey/PageScene 等 API。

## 文件边界

只改 experiments/gpui-font-probe/** 和 docs/coordination/reports/W02.md。根依赖、应用、现有 crate、ADR、CI 只读。系统组件安装由用户处理。

## 验证与交接

报告完整 cargo --manifest-path 命令、特性、构建/测试结果、完整上游 SHA、字体身份、图形环境与证据；对独立 manifest 运行适用 fmt/clippy，并运行 python3 scripts/check_docs.py。

API/环境阻塞时也提交最小复现和准确缺口，不宣称 T003/ADR-010 完成。推送自己的分支，等待协调者决定正式来源。
