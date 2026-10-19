# 查找与替换

## 使用者

开始功能区的「查找」组，或命令：

| 命令 | 界面文字 |
| --- | --- |
| `find.next` | 查找下一个 |
| `find.previous` | 查找上一个 |
| `find.replace` | 替换 |
| `find.replace_all` | 全部替换 |

查找栏有「区分大小写」和「全字匹配」。另外两个勾选的本地化键是 `find.width`、`find.regex`；语言包里还没有这两条，应用回退显示「全半角」和「正则」（[main.rs](../../../apps/joyword/src/main.rs)）。

- 区分大小写：关闭时按字素折叠大小写。ß 不扩成 SS。
- 全字匹配：命中两端都要落在字素词边界上。
- 全半角：打开后，比较前把全角形式折成半角。汉字不折。
- 正则：查询按线性时间正则编译。编译失败时命令拒绝，文档不变。
- 全部替换是一步。撤销一次恢复全部命中。
- 锁定的内容控件和含保留标记的段落只计数、不修改。
- 一次查找最多列出 256 处。超出后状态栏说明结果已截断。
- 查询最长 1024 个标量，替换最长 4096 个标量。

正则替换里：

- `$0` 是整段命中。
- `$1` 到 `$n` 是第 n 个捕获组。`$10` 是第 10 组，不是 `$1` 后面跟一个 0。
- `$$` 写成一个 `$`。
- 模式里没有的组号原样留下，例如两段捕获上的 `$9` 仍是 `$9`。
- 插入的是原文，不是折过宽度的比较串，并且不再扫描插入文字里的 `$`。

不勾选正则时，`$1` 和 `$$` 都按字面写入。

## 开发者

入口在 [find.rs](../../../crates/joyword-engine/src/find.rs) 的 `FindQuery`。`fold_width` 与 `regex` 经 `EditRequest::Find`／`Replace` 进引擎。命令参数 `width` 和 `regex` 在 [command.rs](../../../crates/joyword-shell/src/command.rs)。查找栏把勾选写进每一次查找命令，测试是 `find_bar_checkboxes_set_width_and_regex_on_every_find_command`（[tests.rs](../../../apps/joyword/src/tests.rs)）。

引擎测试在 [find_paint.rs](../../../crates/joyword-engine/tests/find_paint.rs)：`fullwidth_forms_fold_to_halfwidth`、`regex_find_matches_patterns`、`regex_replace_expands_groups_and_leaves_unknown_dollars`、`regex_replace_skips_locked_and_preserved_and_undoes_once`、`replace_all_is_one_undo_and_skips_locked_and_preserved`。壳层注册表测试是 `find_replace_and_format_painter_go_through_the_command_registry`（[find_paint.rs](../../../crates/joyword-shell/tests/find_paint.rs)）。

正则实现用工作区锁里已有的 `regex` crate，线性时间、无回溯。代码注释没有把这套 `$n` 语法对应到 ECMA-376 的某一条。

## 限制

- 替换不能跨修订边界；跨修订或只读范围会拒绝。
- 命中不能穿过行内对象，也不能拆开一个字素簇。空匹配跳过。
- 查找栏的全半角和正则勾选没有单独的本地化译文。
- 没有与 Word 查找选项逐项对照的观察记录。

## JoyWord 假定，没有 Word 观察

- 全半角折叠表：U+FF01..=U+FF5E 减去 0xFEE0，U+3000 折成 U+0020。模块注释写明这不是 Word 观察。
- 正则替换的 `$0`、`$1`..`$n` 和 `$$` 是本引擎的语法，不是 Word 通配符里的 `\1`。
