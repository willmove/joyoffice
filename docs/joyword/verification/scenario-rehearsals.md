# 人工验收场景的无界面排练

这些测试走生产路径上的 `Registry`、`Session` 和 `TextInput`，不打开 GPUI 窗口。每一条都是 automated rehearsal; does not replace 人工验收。屏幕外观、输入法候选窗、拖动、真实缩放绘制，以及 Word / WPS 一侧的交换，仍要人在界面上确认。

验收原文见 [TASKS](../TASKS.md)。缩放不改变物理几何的约定见 [QUALITY](../QUALITY.md)。排练实现在 [apps/joyword/src/tests.rs](../../../apps/joyword/src/tests.rs)。

## 已有覆盖

单步命令已有测试。本文件只补「命令、撤销、导出、再导入」这条场景链。

| 场景 | 已有测试 |
| --- | --- |
| T200 查找替换与格式刷 | [find_replace_and_format_painter_go_through_the_command_registry](../../../crates/joyword-shell/tests/find_paint.rs)、[find_width_and_regex_flags_go_through_the_command_registry](../../../crates/joyword-shell/tests/find_paint.rs)、[regex_capture_replacement_goes_through_the_command_registry](../../../crates/joyword-shell/tests/find_paint.rs)、[replace_all_is_one_undo_and_skips_locked_and_preserved](../../../crates/joyword-engine/tests/find_paint.rs)、[regex_replace_expands_groups_and_leaves_unknown_dollars](../../../crates/joyword-engine/tests/find_paint.rs)、[regex_replace_keeps_original_case_and_width](../../../crates/joyword-engine/tests/find_paint.rs) |
| T200 样式与列表 | [style_list_and_apply_go_through_the_command_registry](../../../crates/joyword-shell/tests/m1_edit_save.rs)、[list_apply_level_and_remove_go_through_the_command_registry](../../../crates/joyword-shell/tests/m1_edit_save.rs)、[list_apply_bullet_is_one_undo_and_the_ribbon_sends_it](../../../crates/joyword-shell/tests/m1_edit_save.rs)、[applying_a_paragraph_style_is_one_undoable_transaction](../../../crates/joyword-engine/tests/engine.rs)、[style_assignments_export_and_unknown_styles_are_rejected](../../../crates/joyword-docx/tests/import.rs)、[theme_font_and_colour_resolve_when_the_scheme_has_them](../../../crates/joyword-model/src/style.rs) |
| T201 表格 | [table_commands_go_through_the_command_registry](../../../crates/joyword-shell/tests/tables.rs)、[table_properties_go_through_the_command_registry](../../../crates/joyword-shell/tests/tables.rs)、[column_commands_refuse_a_merged_table](../../../crates/joyword-shell/tests/tables.rs)、[inserting_and_deleting_a_column_is_one_undoable_transaction](../../../crates/joyword-engine/tests/tables.rs)、[merging_cells_is_one_undoable_transaction](../../../crates/joyword-engine/tests/tables.rs)、[splitting_a_cell_is_one_undoable_transaction](../../../crates/joyword-engine/tests/tables.rs)、[setting_table_properties_is_one_undoable_transaction](../../../crates/joyword-engine/tests/tables.rs)、[column_and_row_edits_refuse_merged_structure](../../../crates/joyword-engine/tests/tables.rs) |
| T202 页面 | [page_setup_and_break_go_through_the_command_registry](../../../crates/joyword-shell/tests/m1_edit_save.rs)、[inserting_a_page_number_is_one_undoable_transaction](../../../crates/joyword-shell/tests/m1_edit_save.rs)、[page_setup_change_exports_through_the_docx_plan](../../../crates/joyword-shell/tests/m1_edit_save.rs)、[a_body_page_field_uses_the_displayed_number](../../../crates/joyword-layout/tests/pages.rs)、[field_codes_replace_body_page_fields_and_match_the_cache](../../../crates/joyword-layout/tests/pages.rs)、[formatting_marks_do_not_change_line_breaks](../../../crates/joyword-layout/tests/view_paint.rs)、[zoom_changes_projection_without_changing_page_or_hit_geometry](../../../crates/joyword-layout/tests/document.rs)、[screen_zoom_does_not_mutate_physical_geometry](../../../crates/joyword-foundation/src/lib.rs) |
| T209 撤销粒度 | [keystrokes_are_one_undo_step_restoring_the_starting_caret](../../../crates/joyword-shell/tests/undo_groups.rs)、[enter_paste_commands_and_ime_commits_are_separate_steps](../../../crates/joyword-shell/tests/undo_groups.rs)、[deletion_coalesces_separately_from_typing](../../../crates/joyword-shell/tests/undo_groups.rs)、[docx_backends_seal_groups_too](../../../crates/joyword-shell/tests/undo_groups.rs) |
| T300 修订 | [review_commands_use_the_registry_and_the_author_preference](../../../crates/joyword-shell/tests/review.rs)、[review_commands_accept_a_paired_move_and_a_settings_patch_keeps_it](../../../crates/joyword-shell/tests/review.rs)、[text_revisions_round_trip_and_tracking_is_a_settings_patch](../../../crates/joyword-docx/tests/import.rs)、[header_and_footer_markup_paints_and_projections_omit_it](../../../crates/joyword-layout/tests/header_revisions.rs)、[accepting_or_rejecting_a_header_revision_is_one_undo](../../../crates/joyword-layout/tests/header_revisions.rs) |

应用里原有的查找栏测试 `find_bar_checkboxes_set_width_and_regex_on_every_find_command` 与 `find_bar_width_hits_fullwidth_and_regex_rejects_invalid_patterns` 仍保留。

## T200

人工验收：「制作多级标题与列表，格式刷、查找替换和撤销。」

测试：`t200_headings_lists_painter_and_regex_undo_round_trip`。

排练步骤：

1. 打开五段虚构正文。样式按 `w:name` 提供 heading 1 / heading 2，编号定义 numId 1 的 0、1 两级。
2. `style.list` 能看见两个标题样式。对第一段 `style.apply` 为 Title1，第二段为 Title2。
3. 第三段加粗，`format.copy` 后 `format.paint` 到第四段。
4. 第三、四段 `list.apply`（num 1），第四段 `list.level` 为 1。
5. `find.replace_all` 对 `(A+)(b+)` 先不开 `width`，全角「Ａ」不命中；再开 `width` 与 `regex`，替换为 `$2-$$-$1`，得到 `bbb-$-ＡＡ fee`。捕获文本保持原稿宽度。
6. 一直撤销到打开时的段落标记，再重做回编辑后状态。
7. `file.save_copy` 后 `file.open`，样式、编号、加粗和替换结果相同。

不能无界面排练的部分：标题在页面上的字号与间距、列表符号的绘制、格式刷的光标形态、查找栏勾选框的外观。

五分钟界面抽查：

1. 打开含两级标题和两级列表的文档，看标题层级和列表缩进。
2. 把一段加粗，用格式刷刷到下一段，确认只有目标文字变粗。
3. 查找替换勾选正则和宽度折合，模式 `(A+)(b+)`，替换 `$2-$$-$1`，全角字母应留在结果里，`$$` 变成一个 `$`。
4. 多次撤销，直到文字回到打开时的样子。

## T201

人工验收：「制作合同表格，插行、合并、拆分、撤销、保存重开。」

测试：`t201_contract_table_edits_undo_and_reimport`。

排练步骤：

1. 打开一段「Northwind supply note」。`table.insert` 2 行 2 列，撤销后没有表格，重做后恢复。
2. 撤销会把光标映射回表格后面的段落。每条需要单元格的命令之前，排练把光标放回第一个单元格。
3. `table.row.insert`、`table.column.insert`、`table.row.delete` 各做一次并撤销。删除那一步撤销后留在三行，不再重做。
4. `table.properties`（`commit=apply`，宽度 `dxa:4800`，居中，虚线边框，宽度 8）后撤销再重做。
5. `table.merge.down`、`table.split`、`table.merge.right` 各做一次并撤销再重做。向右合并后首格 `gridSpan` 为 2。
6. 指纹含列数、表宽、对齐、跨度、纵向合并和上边框。另存再打开后指纹相同，正文句子仍在。

不能无界面排练的部分：表格在页面上的列宽、虚线观感、合并后的选区高亮。嵌套表、跨页和重复标题行不在这条链里。

五分钟界面抽查：

1. 插入 2×2 表格，插一行、插一列、删一行，每步撤销看网格还原。
2. 把表宽设为固定值、居中、虚线边框。
3. 向下合并、拆分、向右合并，每步撤销。
4. 另存为新文件再打开，行列、合并和边框与保存前一致。

## T202

人工验收：「改变页边距与方向，检查页码、页眉及缩放后分页。」

测试：`t202_margins_orientation_page_numbers_and_zoom`。

排练步骤：

1. 正文是 60 段只含 `A` 和空格的行（测试字体只有 A、V、f、i 和空格）。页眉故事是「V」加一个 `PAGE` 域，节属性含完整页尺寸和页边距。
2. 打开后页眉故事存在，页数至少 2，页眉排版出的页码是 1 到页数。
3. `page.margins` 把上下边距改为 4000 twip，页数增加；撤销恢复，重做保留。
4. `page.orientation` 为 landscape，宽高对调，页数再增加；撤销恢复纵向，重做保留。
5. `page.number` 插入正文页码。首页正文排版含数字 1，页眉页码仍是 1 到页数，页数不变。撤销后正文里的 1 消失，重做后回来，`export_plan` 在保留正文页码时成功。
6. `ViewOptions.formatting_marks` 不改变页数，也不改变各行所在页；`LayoutKey.view` 不同。`ScreenScale` 0.5、1、2 只缩放页面投影：一倍约是半倍的两倍，两倍约是一倍的两倍，页数不变。
7. 另存再打开：方向为横向，上边距 4000 twip，页眉故事还在，页数与页眉页码与保存前一致。第一段正文开头恰有一个没有缓存值的 `PAGE` 域，其后是原来的文字，首页正文排版以 1 开头。

不能无界面排练的部分：标尺上的边距拖动、纸张在屏幕上的旋转动画、缩放滑块的绘制、页码的字体与位置。没有显示器，不能看真实像素。

五分钟界面抽查：

1. 打开多页文档，记下页数和页眉页码。
2. 加大上下边距，页数应增加；改为横向，页面变宽，页数再变。
3. 在正文插入页码，当前页出现对应数字。撤销后该数字消失。
4. 缩放到 50%、100%、200%。分页位置不变，只有屏幕上的大小变。
5. 另存再打开。边距、方向、页眉页码和正文里插入的页码都仍在。

## T209

人工验收：「连续打字、IME 提交、拖动与粘贴混合操作后逐步撤销，每步粒度符合预期。」

测试：`t209_keyboard_paste_command_and_ime_undo_one_step_at_a_time`。

排练步骤：

1. 打开「Note」，光标在末尾。键盘输入 `a`、`b` 合成一步，正文为 `Noteab`，撤销深度 1。
2. 两次退格合成另一步，回到 `Note`。
3. `InputSource::Paste` 插入 `P`，`InputSource::Command` 插入 `C`，各一步。
4. `mark`「fa」再 `unmark` 是一次 IME 提交。`mark`「ii」期间 `replace`「i」是另一次提交。
5. 正文 `NotePCfai`，深度 6。逐步撤销得到 `NotePCfa`、`NotePC`、`NoteP`、`Note`、`Noteab`、`Note`。
6. 全部重做后另存再打开，正文仍是 `NotePCfai`。

不能无界面排练的部分：拖动未实现；自动更正未实现。输入法候选窗、下划线预编辑和按键动画也看不到。

五分钟界面抽查：

1. 连续打几个字母，一次撤销应整段消失，而不是一字一字撤。
2. 粘贴一段，再通过命令插入文字，这两步各撤一次。
3. 用系统输入法打一个词并上屏，撤销只去掉这次上屏。组合过程中的替换再单独成一步。
4. 连续退格一次撤销。不要在界面上寻找拖动文本或自动更正，这两项还没有。

## T300

人工验收：「在 Word 与 JoyWord 交换文字修订，逐项和批量接受／拒绝。」

测试：`t300_tracked_edits_views_and_revision_round_trip`。只排练 JoyWord 一侧。

排练步骤：

1. 正文带一对移动标记（`A` 为 moveFrom，`V` 为 moveTo）和普通文字 `i`。页眉是 `i`。包内有空的 `word/settings.xml`（成对结束标签），这样导出才能补上 `w:trackRevisions`。
2. `review.track` 打开。正文末尾键入 `f`，再在偏移 2 向前删除。页眉同样键入 `f` 并删除开头。正文回到 `AVif`。
3. 正文修订种类含 Insert、Delete、MoveFrom、MoveTo。页眉含 Insert 和 Delete。移动标记仍是 4 个。
4. `review.view.markup`、`review.view.final`、`review.view.original` 切换视图。标记视图排版含 A、V、i、f；最终视图含 V 和 f，不含 A；原始视图含 A 和 i，不含 V 或 f。
5. 在插入上 `review.accept` 并撤销；在删除上 `review.reject` 并撤销。`review.accept_all` 清空修订和移动标记，撤销后标记回来。`review.reject_all` 清空，撤销后种类与页眉、正文的操作后状态一致。
6. 导出含 `w:ins`、`w:del`、`w:moveFrom`、`w:moveTo`。另存再打开后种类、4 个移动标记和正文 `AVif` 相同。

不能无界面排练的部分：修订的颜色和下划线、批注气泡、Word 或 WPS 打开该文件后的接受／拒绝。本环境没有 Word / WPS。

五分钟界面抽查：

1. 打开修订，在正文和页眉各打一个字、删一个字，看标记。
2. 切换标记、最终、原始，文字取舍与上面三种投影一致。
3. 接受一处、拒绝一处，再全部接受、全部拒绝，每步都能撤销回来。
4. 保存。JoyWord 重开后修订还在。用 Word 或 WPS 打开并逐项接受／拒绝，这一步要另做，本排练没有覆盖。

## F1 已修复（Fixed in F1）

T202 原有一条已知导出限制：对已有正文段落执行 `page.number` 后，`export_plan` 拒绝，原因是 `paragraph formatting, source or inline objects changed`。排练当时先确认这条拒绝，再撤销正文页码才保存；五分钟抽查第 5 步也提醒正文页码不会写回。现在原段落没有行内对象、新增的只是无格式的页码域时，导出把域写成 `w:fldSimple`（` PAGE `、` NUMPAGES ` 或 ` SECTIONPAGES `，有缓存值时附结果 run），其余段落属性与文字仍按原有检查。带格式的页码域或超链接里的页码域仍明确拒绝。排练改为保留正文页码保存，并在再导入后断言它。说明见 [verify-wave-fixes.md](verify-wave-fixes.md)。
