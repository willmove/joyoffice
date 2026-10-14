# 第一轮集成审查（2026-10-14）

总协调者：本 Codex 聊天。集成分支 codex/wave01-integration。W01/W03 的根 workspace 源码校验点为 2d4093b885eaf5928929f972ca25c48bd330082e。后续 W02 作为独立实验接收，复验及协调者修正见[W02 审查](W02-REVIEW.md)，不混用两个 workspace 的测试计数。

## 收到的交付

| 任务 | 提交 | 实际执行者 | 审查结论 |
| --- | --- | --- | --- |
| W01 | 1be58b28ac63c5c4249147cf5a36f48bb30592c9 | Claude Desktop Code | 只读导入切片接收；两处 XML 输入校验遗漏已在集成分支修正 |
| W03 | 4487380ec2b7cad2f7395326b6dd70df0fb77130 | Codex Desktop（以交接报告为准） | 有界、单字体、显式断点的 LTR 行拟合切片接收 |

两目录提交时工作区干净，分支均已推送；相对 eda121b 的路径符合任务卡。独立集成分支以保留历史的 merge 接入，两项没有合并冲突。未修改执行者原工作目录。W02 随后交付，已单独审查并作为实验接入。

## 发现及修正

1. W01 的 mc:MustUnderstand/mc:Ignorable 值使用未声明前缀时，prefixed_list 的过滤会擦掉无效 token，从而绕过应有拒绝。新增回归先在原交付上失败，修正为在读属性时验证本元素及祖先命名空间绑定，拒绝未绑定或指向 MCE 自身的前缀。空列表依然允许。
2. W01 只校验数字字符引用的 XML Char 范围，没有校验文字、CDATA 和属性中的字面控制字符。回归在 U+0001 上失败；修正后这三条路径拒绝非法字符。两个新测试均有明确的修复前失败记录。

修正不变更公开 API，不引入新依赖。代码提交为 2d4093b；后续仅修正文内规范节号注释。

来源：[XML 1.0 第五版 §2.2](https://www.w3.org/TR/xml/#charsets)、[ECMA-376 官方 Part 3 下载](https://ecma-international.org/wp-content/uploads/ECMA-376-3_5th_edition_december_2015.zip)。已从官方压缩包读取规范原文，核对 §7.2 与 §7.4 的作用域绑定及不允许 MCE 自身的规定；不将整份规范转载入仓库。

## 集成复验

执行目录为本聊天的 agent-coordination/joyoffice worktree。字体为系统 Songti.ttc face 0 和 Avenir Next.ttc face 0，完整散列见 W03 交接与 W04 环境证据。

| 命令 | 实测结果 |
| --- | --- |
| JOYWORD_REQUIRE_TEST_FONT=1 JOYWORD_TEST_FONT='/System/Library/Fonts/Supplemental/Songti.ttc:0' JOYWORD_LINE_LATIN_FONT='/System/Library/Fonts/Avenir Next.ttc:0' cargo test --locked -- --nocapture | 135 tests passed，0 failed；新增字体集成 8 次实际执行，SKIPPED 标记 0；已有字体/PDF 回归同样被强制要求字体 |
| cargo clippy -p joyword-docx -p joyword-layout -p joyword-pdf --all-targets --locked -- -D warnings | passed |
| cargo fmt --all -- --check | passed（新增测试初次格式检查提示已格式化修正） |
| python3 scripts/check_docs.py | passed：82 个需求、58 项任务、9 个登记用例 |
| python3 -m unittest discover -s scripts/tests -v | 9 passed |
| git diff --check | passed |

本机中间日志保存在 target/coordination-review；新增回归修复前后日志随 W04 证据归档。未借用 W01/W03 自报结果代替上述复验。

## 接口接入条件和下一步

后续 Windows CI 发现系统 Segoe UI 不具备测试需要的 fi 连字；已改为固定 OFL 最小字体并保留断言，详见[Windows 测试输入修正](CI-WINDOWS-FONT.md)。本机 135 项测试已复验；最新远程提交的 CI 仍须独立确认。

- W01 的 Editability.locked 尚未接入 engine；在局部锁接入前，未来打开流程必须把全篇只读或任何 locked 非空都映射为 engine 的全篇只读。当前尚未添加应用打开/保存，因此不新建不安全编辑入口。
- PackageBacking 仅证明未编辑原包及片段可保留；不支持编辑后写回、ExportPlan 或原子保存，不提前释放保存任务。
- W03 需要调用者给定合法断点，不生成 UAX #14/中文禁则；默认 64 KiB 段落等硬预算不能作为最终产品容量/性能指标。
- W02 结果到达后先审查候选 GPUI/FontKey 一致性，再决定正式来源与屏幕接入。保持现有 shape/PageScene 公共 API。
- 第一轮 W01/W03 暂停接新工作，等待统一基线和下一轮任务；T103/T105 仍为 in_progress，M0/M1 未整体通过。
