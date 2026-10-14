# C02：保守 DOCX 编辑导出（2026-10-14）

- 执行者：当前 Codex 协调者，单人执行；没有启动其他桌面 Agent 或后台监控。
- 主机：Apple M4／24 GiB，macOS 27.0.1，arm64；Rust 1.98.1。
- 任务卡：[C02](../tasks/C02-DOCX-EXPORT.md)。
- 启动目录：/Users/willm/Coding/joyoffice；main 干净。
- 完整基线：b48ca9b426b80c220f0b2518d1e0ad363e07afea。
- 交付分支：codex/coordination-docx-export。
- interface_ready：true，仅限当前保守文字子集的内存导出计划与已验证字节。
- implementation_verified：true，17 项新增真实链路测试、100 轮确定性 Unicode 编辑、本地全默认成员及 release 回归通过。
- acceptance_complete：false，没有 GUI、固定 Word/WPS、文件系统保存或阶段人工验收；T103/T106 保持 in_progress。

## 行为和边界

新增 joyword-docx::plan_export 和不可变 ExportPlan；shell 的 DocxSession::export_plan 使用原导入预算。导出器从原包重新导入可信基线，不能依靠调用者修改的 Editability 绕过限制。对照节点、顺序、来源、属性、样式表、settings、section、标注、资源与 story，拒绝不在文字补丁子集内的修改。

支持现有正文段落、现有字符格式边界内的文字插入／删除；支持默认字符属性空段落输入、整段清空、相邻同格式 token 合并、原有 tab 与 CDATA 输入。输出文字进行 XML 转义和 xml:space 保留，tab 写独立元素；不把模型内部对象标记写入正文。

段落／run 的原始标签和属性、pPr／rPr／section、其间注释及所有其他 OPC 部件和关系保留精确解压字节。文字叶子上有未知属性、未知 namespace 声明、注释或 PI 时明确拒绝。任何导入只读原因／锁定、数字签名包的编辑、结构变化、字符格式边界变化、段落格式和对象修改均拒绝。DOTX／DOCM／DOTM 可以精确原包复制；其编辑导出未开放。

计划包含版本、目标类型、变更与保留部件，以及 ZIP 元数据重建警告。原样复制保留 ZIP 全字节；文字导出重建 ZIP 容器，因此不声称保留 ZIP 时间戳、extra、目录记录或注释。改变的正文用 stored 方法，避免新增重复文字产生当前资源预算无法重新读取的高压缩比。所有导出必须在原预算下完整重新导入，并合并相邻同格式 run 后核对语义。未知部件原字节及原正文属性未重新生成。

计划持有自己的字节和文档版本；随后继续编辑不会改变已有计划。生成计划不写文件、不替换 backing、不清除 dirty、不标记保存成功。原 unchanged_package_bytes 接口仍在任何已提交编辑后拒绝，包括全撤销；完整导出计划独立验证后可以安全给出全撤销时的原包。这两个接口的证据强度不同。

没有新外部依赖、没有 Cargo.lock 变化，也没有修改 model／engine／OPC／字体／场景／PDF 公共 API。只更新协调者报告和公共文档；W01–W04 报告未修改。公开规范来源及条款见 [PROVENANCE](../../joyword/PROVENANCE.md)。

## 验证

| 完整命令 | 实际结果 |
| --- | --- |
| cargo test -p joyword-shell --test docx_export --locked | 新增 17 项通过；包含 100 轮确定性 Unicode 编辑、导出及间歇 undo／redo |
| JOYWORD_REQUIRE_TEST_FONT=1 JOYWORD_TEST_FONT='/System/Library/Fonts/Supplemental/Songti.ttc:0' cargo test --locked | 全默认成员 160 项通过，0 failed／ignored；实际执行字体整形／PDF 路径 |
| cargo test -p joyword-shell -p joyword-docx --release --locked | 64 项通过，0 failed／ignored；优化构建亦通过 |
| cargo clippy --workspace --all-targets --features dev-runtime-shaders --locked -- -D warnings | passed；已有 block 0.1.6／proc-macro-error2 2.0.1 future-incompat 提示仍在 |
| cargo fmt --all -- --check | passed |
| cargo build -p joyword --features dev-runtime-shaders --locked | passed；未创建窗口，仅开发着色器构建 |
| python3 scripts/check_docs.py | passed：82 个需求、58 个任务、9 个登记用例；不等于 Office 或 GUI 验证 |
| python3 -m unittest discover -s scripts/tests -v | 9 passed |
| git diff --check | passed |

独立读取：运行 `cargo run -p joyword-shell --example docx_text_export --locked -- tests/fixtures/docx/DOCX-MIN-001.docx '导出验证🙂 &<> ' > target/c02-text-export.docx`，再运行 `/usr/bin/textutil -convert txt -stdout target/c02-text-export.docx`；macOS textutil 实际读出完整 `导出验证🙂 &<> 合作协议：甲方与乙方于 2026 年签署。`。这是系统转换器的文本读取证据，不是 Word/WPS 或视觉排版验收。示例仅向 stdout 写二进制，文件路径由 shell 的一次性测试重定向管理，不是产品保存入口。

测试覆盖：五个语料精确复制；未知二进制部件、未知段落／run 属性及属性 XML 不丢失；正文中的 XML 特殊字符、emoji ZWJ、组合字符、首尾空格；tab、CDATA 与同格式 token 合并；空段落与清空段落；计划版本竞争、重复导出、撤销／重做；格式／结构／源引用／全局元数据变化拒绝；独立保护与未知锁的模型绕过拒绝；签名包修改拒绝；宏／模板类型；部件字节、总字节和 XML 事件预算；外部关系保留且未访问；失败保持会话和原包。外链 fixture 原有受锁定超链接，因此保留其关系后使用独立可编辑正文验证，未削弱锁规则。

字体使用既有本地 Songti.ttc face 0 与仓库 OFL 拉丁 fixture，没有提交系统字体。当前证据限本机；新源码的远程 CI 结果另行记录，不能沿用 C01 的 CI 通过结论。

## 后续优先级与未覆盖

1. T103：扩展结构／字符／段落格式导出前继续维持保守拒绝；文件系统安全写入、Office 锁文件、外部修改、链接／权限／扩展属性／来源标记保留和故障注入尚待实现。当前不是完整保存功能。
2. T103：独立 fuzz 目标与夜间执行仍未建立，既有变异冒烟不能冒充覆盖引导 fuzz。
3. T105／T102：完整断点、中文规则、分页与命中后，接入画布编辑和 IME；T100/T101 不重复开发。
4. M0：正式 GPUI 来源、GL／CPU 退路、字体身份、IME／DPI 和信创真机门槛仍待环境验证。未控制桌面；需要 GUI 验证时先与用户约定时段。
5. Word LTSC 2024／固定 WPS 环境和完整人工验收未执行，不能将内部往返或 textutil 读回当作完整互操作证明。

## 远程 CI 进度（源码 4dd3870）

[运行 38548311954](https://github.com/willmove/joyoffice/actions/runs/38548311954) 绑定完整源码 SHA 4dd3870a609509ec225a7f85cb8509fbb602dfae。协调者检查时 7/8 作业 success：macOS、Linux x86_64／ARM64 应用构建，ARM64／LoongArch64 QEMU 无界面测试、依赖与统一 PDF；Windows 应用构建仍 in_progress，整体不能提前记作通过。这份状态不证明后续 C03 源码已通过。
