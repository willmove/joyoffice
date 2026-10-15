# C04：自动 Unicode 默认断点（2026-10-14）

- 执行者：当前 Codex 协调者，单人执行。
- 任务卡：[C04](../tasks/C04-UNICODE-WRAP.md)。
- 基线：574c44f3f9e4b9a93766ce7fa7f3dd8aa8ecb05e（C03）。
- 分支：codex/coordination-unicode-wrap。
- 目录：/Users/willm/.codex/worktrees/joyoffice-coordination/joyoffice；启动时干净。
- 主机：Apple M4／24 GiB，macOS 27.0.1 arm64，Rust 1.98.1。
- interface_ready：true，仅限单字体 LTR、Unicode 15.0 默认规则的自动行拟合。
- implementation_verified：true，8 项新增真实字体测试及完整默认成员回归通过。
- acceptance_complete：false，Word 中文规则、混合格式／字体回退、分页／命中和 GUI 尚未完成；T105 保持 in_progress。

## 行为

新增 layout_paragraph_auto；调用者不再需要手工生成合法断点。精确使用原锁文件已有 unicode-linebreak 0.1.5，按 Unicode 15.0.0 UAX #14 默认规则生成内部候选，再与整段 shaping cluster 起点求交。剩余文本的 cluster 再校验、候选行独立重整形、非单调宽度选择、超宽／缺字诊断全部复用既有 fitter。显式 layout_paragraph 的签名、严格断点拒绝和测试保持不变。

两个入口共用同一内部请求，候选生成受 max_text_bytes／max_breakpoints 限制，所有探测与候选重整形只使用一份 shaping／字节／行数预算；没有错误重试导致预算归零的路径。原文每个字节和标量仍被连续覆盖，不裁剪首尾或重复空白。空段落返回一空行；NBSP、Word Joiner、组合序列和 emoji ZWJ 服从默认规则及整形簇约束。

保留既有控制字符、hard separator、tab 与 bidi 拒绝；自动入口还明确拒绝 discretionary soft hyphen。原因是断在 SHY 后需生成可见连字符及其定位，本轮尚未实现，不能静默断行却漏显示连字符。没有将 Unicode 默认规则等同于固定 Word 的禁则、语言定制、词典分词或自动连字。

无新外部包、无依赖升级；Cargo.lock 只有 layout→unicode-linebreak 的一行新增边。未改字体、整形或 PageScene 公共 API。line_scene 示例可用 auto 参数演示中文自动断行；旧显式模式保留。通用库来源与规范条款见 [PROVENANCE](../../joyword/PROVENANCE.md)。

## 验证

所有 Cargo 命令使用 CARGO_TARGET_DIR=/Users/willm/Coding/joyoffice/target；源码来自本独立 worktree。

| 命令（除已说明的 target 目录外） | 实际结果 |
| --- | --- |
| JOYWORD_REQUIRE_TEST_FONT=1 JOYWORD_TEST_FONT='/System/Library/Fonts/Supplemental/Songti.ttc:0' cargo test -p joyword-layout --locked | 26 项通过：10 项既有单元、8 项既有字体测试、8 项新增字体测试 |
| JOYWORD_REQUIRE_TEST_FONT=1 JOYWORD_TEST_FONT='/System/Library/Fonts/Supplemental/Songti.ttc:0' cargo test --locked --quiet | 全默认成员 180 项通过，0 failed／ignored，实际使用字体 |
| JOYWORD_REQUIRE_TEST_FONT=1 JOYWORD_TEST_FONT='/System/Library/Fonts/Supplemental/Songti.ttc:0' cargo test -p joyword-layout --release --locked --quiet | 26 项通过 |
| cargo clippy --workspace --all-targets --features dev-runtime-shaders --locked -- -D warnings | passed；既有 future-incompat 提示仍在 |
| cargo fmt --all -- --check | passed |
| python3 scripts/check_docs.py | passed：82 个需求、58 项任务、9 个登记用例 |
| python3 -m unittest discover -s scripts/tests -v | 9 passed |
| git diff --check | passed |

新增测试：中文开／闭标点默认禁则；NBSP 与 Word Joiner 不拆；组合字符和 emoji ZWJ 不拆；固定 fi 连字和显式路径完全相同的结果／预算统计；首尾和重复空白；文本／候选／shaping／整形字节／行数预算独立拒绝；已有输入拒绝及 soft hyphen；8 种宽度的确定性与全部源映射连续性。各行还与独立重新 shape 的结果比较，保证没有截取整段字形假装独立行。

`cargo run -p joyword-layout --example line_scene --locked -- /System/Library/Fonts/Supplemental/Songti.ttc 0 auto` 实际产生 4 个场景 run、0 诊断；43 次 shaping、2,462 个整形字节。源范围覆盖 0..109 bytes、0..49 scalars，120 pt 可用宽度下全部行不超宽。示例的固定 18 pt 基线和 A4 只是场景实验，不是 Word 分页规则。

字体：Songti.ttc face 0，SHA-256 dd2f0628d302f322a2e9c3cf3253d5e73f418275e1f1421c84a7c65ce5a87777，仅本机使用；fi／AV 使用仓库固定 OFL 字体，没有系统字体入库。

## GUI 与后续

用户在本轮授权从北京时间约 18:27 开始独占桌面 30 分钟。实际检测屏幕捕获权限和辅助功能权限均为 false，screencapture 返回 could not create image from display；已请求用户开启系统权限，没有绕过权限或记录 GUI 通过。WPS 安装版本为 12.1.29181（build 29181）；已向其发送打开原创 C03 副本的本地请求，但无法观察内容，因此不算 WPS 验收。JoyWord 当前仍无文档输入入口，不能宣称真实 IME 已通过。

下一步补齐 T105 的单节分页、快照版本键和命中数据，再接 T102 画布与 IME。固定 Word 规则、四槽字体和语言定制仍需后续实测。C04 的远程 CI 另按源码提交记录，不沿用 C03 通过结论。

## 后续核验的远程 CI

Actions [38551299360](https://github.com/willmove/joyoffice/actions/runs/38551299360) 绑定 C04 源码 01e4b6d28682c02ce55dd10c33ce9e6447f1c8c3：8／8 作业成功（macOS、Windows、x86／ARM Linux 构建、ARM／LoongArch QEMU、统一 PDF 与依赖检查）。此结果未替代 GUI 或真机验收。用户随后明确选择“先继续后台开发”，GUI 暂缓，继续 C05。
