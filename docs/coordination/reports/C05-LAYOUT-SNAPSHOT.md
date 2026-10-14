# C05：文档排版快照、单节分页与命中（2026-10-14）

- 执行者：当前 Codex 协调者，单人执行。
- 任务卡：[C05](../tasks/C05-LAYOUT-SNAPSHOT.md)。
- 基线：01e4b6d28682c02ce55dd10c33ce9e6447f1c8c3（C04）。
- 分支：codex/coordination-layout-snapshot。
- 独立目录：/Users/willm/.codex/worktrees/joyoffice-coordination/joyoffice；启动时干净。
- 主机：Apple M4／24 GiB，macOS 27.0.1 arm64；Rust 1.98.1、edition 2024。
- interface_ready：true，限显式单字体预览 profile、受支持的正文流与 cluster 边界命中。
- implementation_verified：true，19 项新增 layout 测试、PDF 集成、独立多页读回、相关 release 与完整默认成员回归通过。
- acceptance_complete：false；未接 GPUI，无完整字体／Word profile，GUI／IME／Office 验收暂缓；T105 保持 in_progress。

## 实际变更

新增 layout::document，不修改 model／engine／OPC／字体／整形／PageScene 公共接口。只读 DocumentSnapshot 经真实 FontKey、自动 Unicode 断行与实际单节页框生成 LayoutSnapshot。SingleFontProfile 明确字号、固定行盒、行内基线及 FeatureSet；不根据字体名称猜测字节或 Word 度量。layout 仅新增现有 model 依赖；engine 仅为 layout／PDF 的测试或示例依赖，PDF 同样只增现有 model 测试依赖。Cargo.lock 仅 4 条内部边，无新增／升级外部包。

页面尺寸／页边距／gutter 使用实际绝对 Measure；Start／Center／End 对齐、段前后距和 page_break_before 支持，跨页延续不重复段前距。间距采用明确的预览累加规则，不是 Word 的段间距折叠／页顶抑制。页内全部 EMU，不因缩放或 PDF 输出重排。过宽不可断文本沿用有界诊断，缺字保留原文并定位；过高行显式失败。

LayoutKey 包含 DocumentVersion、相关内容 SHA-256、完整 FontKey 和 profile。内容指纹流式覆盖故事／段落身份、文本、页框和相关段落属性，避免建立巨大模型转储；预检校验使用中的样式。ensure_current 在后台结果交接时还检查同版本手工修改的模型，不允许用版本碰巧相同的其他文档。撤销恢复文本也产生新版本，旧快照仍被拒绝。指纹检查是 O(n) 交接检查，不是每字绘制操作。

每行提供段落、连续 byte／scalar 源范围、页面／场景 run、行盒与 cluster caret stops。hit_test 返回真实 model::Position，可驱动 engine 事务；caret 以 Before／After 表达 wrap 边界亲和性。相邻行盒使用半开边界，页面外点选安全夹到端点；零 advance 有确定性几何等价选择。连字／组合簇内部没有伪造的光标位置，调用者需先取支持的 cluster 边界。selection_rects 覆盖跨行／跨页／跨段落标记与空段落；反向、失效节点或簇内部范围明确失败。

段落／文本／run／元数据／页面／行／字形／shaping 调用／整形字节／诊断／选区矩形均有不可放宽的硬上限，调用者可降低。全局整形预算随段落消耗，不逐段归零。共享单调取消标记在预检、候选 shaping 之间、拟合与放置循环检查；单次已进入的底层 shaping 仍按既有段长上限运行，不能从内部强制中断。预取消和 clone 可见性有自动测试，运行中的实际取消延迟未测量。取消或任何失败只销毁候选，不返回成功半份快照，不修改文档或既有场景。

表格／其他块、行内对象与 break、额外 story／section、字体／字符格式或尚未解析的样式、未知属性、域／标注／修订投影等明确拒绝，不静默扁平化。未接 GPUI，不把此预览 profile 当作完整 T105；显式行内分页符、混合字体、四槽字体／回退、Word 中文规则、增量重排、屏幕绘制／IME 仍待实现。

## 测试与独立验证

以下 Cargo 命令均设置 CARGO_TARGET_DIR=/Users/willm/Coding/joyoffice/target，源码来自本独立 worktree；字体相关命令均设置 JOYWORD_TEST_FONT='/System/Library/Fonts/Supplemental/Songti.ttc:0' 和 JOYWORD_REQUIRE_TEST_FONT=1，实际执行，未跳过。

| 命令 | 实际结果 |
| --- | --- |
| cargo test --locked | 全默认成员 200 passed，0 failed／ignored |
| cargo test --locked --release -p joyword-layout -p joyword-pdf | 50 passed：layout 45，PDF 5；0 failed／ignored |
| cargo clippy --workspace --all-targets --features dev-runtime-shaders --locked -- -D warnings | passed；既有 block／proc-macro-error2 future-incompat 提示仍在 |
| cargo fmt --all -- --check | passed |
| cargo check --locked -p joyword-layout -p joyword-pdf --all-targets --target x86_64-pc-windows-msvc | passed；交叉检查，不是 Windows 运行 |
| cargo check --locked -p joyword-layout -p joyword-pdf --all-targets --target aarch64-unknown-linux-gnu | passed；交叉检查，不是真机运行 |
| cargo build -p joyword --features dev-runtime-shaders --locked | passed；开发着色器，不是发行构建或 GUI 验收 |
| python3 -m unittest discover -s scripts/tests -v | 13 passed，含 4 项新增核对器反例测试 |
| python3 scripts/check_docs.py | passed：82 个需求、58 项任务、9 个登记用例；不替代兼容验收 |
| git diff --check | passed |

19 项 layout 测试覆盖精确页面边界、连续源映射、真实 engine 插入／撤销后重排与陈旧拒绝、同版本内容变化、字体字节／profile 变化、wrap 亲和性、公共行边界、空段落、零 advance、emoji／组合／连字簇、跨页选区和段落标记、未知／反向范围、段前后与居中／末端对齐、段前分页、续页不重复段前距、全局预算不重置、失败不修改原文／旧快照、缺字诊断有界、过高行／极端页框、预取消／共享标记与缩放不改变 EMU。固定种子 0x0c05_2026 的 64 轮 engine 编辑／撤销，每轮逐段重建原文、核对全部源范围与每个命中几何。

PDF 新增集成测试从真实 DocumentSnapshot 分页，检查多页场景文字完整、连续两次输出字节确定、PDF 不修改模型或场景。document_flow 示例使用 20 段项目原创中文合同文本，并先经 engine 提交“测试：”文字，输出同一快照的 PDF 与预期几何。CI unified-pdf 已加入该示例与 Poppler 多页独立读回；本提交的远程 CI 尚待推送后单独核验，不能沿用 C04 通过结论。

本机实际命令：

```sh
cargo run --locked -p joyword-pdf --example document_flow -- /System/Library/Fonts/Supplemental/Songti.ttc 0 /Users/willm/Coding/joyoffice/target/c05-document-flow
PATH=/Users/willm/.cache/codex-runtimes/codex-primary-runtime/dependencies/native/poppler/poppler/bin:$PATH /Users/willm/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/bin/python3 scripts/verify_document_pdf.py /Users/willm/Coding/joyoffice/target/c05-document-flow --glyph-baselines
/Users/willm/.cache/codex-runtimes/codex-primary-runtime/dependencies/bin/override/pdftoppm -scale-to 1000 -png /Users/willm/Coding/joyoffice/target/c05-document-flow/scene.pdf /Users/willm/Coding/joyoffice/target/c05-document-flow/page
```

结果：2 页、28 行、551 字形、0 诊断；40 次 shaping、2,329 个整形字节。Poppler 全页框／逐页行数／文字与断点／可见起止 x／相对基线检查 0 错误；pdfplumber 独立读取全部 551 个字符的绝对基线，最大偏差约 0.000002 pt。Poppler 的边框不包含首尾空格，因此预期可见宽度排除这些空格，实际原文／PDF 仍完整保留；没有放宽容差。两页 PNG 均已实际查看，无裁切、重叠或缺字。预览位于忽略的 target 下，不提交系统字体、PDF 或 PNG。

字体 Songti.ttc face 0，SHA-256 dd2f0628d302f322a2e9c3cf3253d5e73f418275e1f1421c84a7c65ce5a87777，仅本地使用；确定性小字体测试使用仓库 OFL fixture。运行环境字体／Poppler 与 CI 的 Noto 需要分别记录，不当作同一参考 profile。

## GUI、来源与下一步

用户随后明确选择“先继续后台开发”，已暂停 GUI。此前 macOS 屏幕捕获及辅助功能权限 false，WPS 12.1.29181（build 29181）只收到打开 C03 原创副本的请求，无法观察，不是 WPS 打开成功证据。未绕过权限，也未记录 JoyWord 中文输入、Word/WPS 往返或人工分页验收通过。

规范与通用库来源见 [PROVENANCE](../../joyword/PROVENANCE.md)。下一优先项是选择正式 GPUI 字形路径并把真实模型／场景接入画布与 T102 输入；同时扩展字体／样式与分页规则。T103 原件覆盖保存、文件锁／元数据和 T104 恢复亦未完成，保留产品验收状态，不缩减原始目标。
