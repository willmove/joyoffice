# 键盘编辑与格式快捷键

## 使用者

快捷键与主流文字处理软件一致。Windows 与 Linux 用 Ctrl，macOS 用下表右栏：

| 操作 | Windows / Linux | macOS | 命令 |
| --- | --- | --- | --- |
| 按词左移／右移 | Ctrl+← ／ Ctrl+→ | Option+← ／ Option+→ | `edit.word_left`／`edit.word_right` |
| 到段首／下一段段首 | Ctrl+↑ ／ Ctrl+↓ | Option+↑ ／ Option+↓ | `edit.paragraph_up`／`edit.paragraph_down` |
| 到文首／文末 | Ctrl+Home ／ Ctrl+End | Cmd+↑ ／ Cmd+↓ | `edit.document_start`／`edit.document_end` |
| 上翻一屏／下翻一屏 | PgUp ／ PgDn | PgUp ／ PgDn | `edit.page_up`／`edit.page_down` |
| 删除前一个词／后一个词 | Ctrl+Backspace ／ Ctrl+Delete | Option+Backspace ／ Option+Delete | `edit.delete_word_backward`／`edit.delete_word_forward` |
| 剪切 | Ctrl+X | Cmd+X | `edit.cut` |
| 加粗／倾斜／下划线 | Ctrl+B ／ Ctrl+I ／ Ctrl+U | Cmd+B ／ Cmd+I ／ Cmd+U | `edit.bold`／`edit.italic`／`edit.underline` |
| 左对齐／居中／右对齐／两端对齐 | Ctrl+L ／ Ctrl+E ／ Ctrl+R ／ Ctrl+J | Cmd+L ／ Cmd+E ／ Cmd+R ／ Cmd+J | `edit.align_left`／`edit.align_center`／`edit.align_right`／`edit.justify` |
| 分页符 | Ctrl+Enter | Cmd+Enter | `page.break` |
| 替换 | Ctrl+H | Control+H | `find.replace` |
| 字体对话框 | Ctrl+D | Cmd+D | `format.font` |

移动类快捷键加 Shift 即扩展选区，例如 Ctrl+Shift+→、Shift+PgDn。Linux 上 Ctrl+E 仍是导出 PDF，居中没有快捷键，从功能区进。

- 按词移动停在下一个词首、标点和段落标记处，词后的空白随词跳过。中日韩表意字按 UAX #29 逐字停靠，与双击选词一致。字素簇不会被拆开。
- 有选区时不按 Shift 移动，先折叠到移动方向一侧再移动。
- Ctrl+↑ 先回到本段段首，已在段首时到上一段段首。
- PgUp／PgDn 按已排版的行累计约一屏高度，视图同步滚动同样距离，插入点在屏幕上的位置大致不变。
- 删除一个词是一步撤销。在段首或段尾删除词会与相邻段合并，与删除一个字符相同。
- Ctrl+B／I／U 在选区内只要有一处没有该格式就整体加上，全部已有时整体取消。是否「已有」按样式解析后的效果判断，因此标题这类样式本身加粗的文字也能来回切换。插入点处按这些键不改动文档。
- 剪切把选区放进剪贴板再删除，删除是单独一步撤销。没有选区时剪切与复制都不覆盖剪贴板。
- 只读文档里，删除词、剪切、加粗／倾斜／下划线和四种对齐都被禁用；移动和复制照常可用。

开始功能区提供「剪切」「下划线」「左对齐」「右对齐」「两端对齐」（窄窗口时部分收进「更多」菜单），布局功能区提供「编辑页眉」「编辑页脚」「编辑正文」。加粗、倾斜、下划线、四种对齐、编辑标记／隐藏文字／域代码、修订跟踪和三种修订视图按钮在当前状态生效时显示为按下。2026-10-24 外壳刷新后，Linux 不再有单独的顶部编辑栏，这些命令都在开始功能区里。

窗口标题显示「文件名 - JoyWord」，有未保存修改时文件名前加 `*`。关闭时的未保存提示写出文档名。

## 开发者

命令和快捷键注册在 [command.rs](../../../crates/joyword-shell/src/command.rs) 的 `Registry::builtin`。居中用 `Shortcut::windows_only`，所以 Linux 不绑定 Ctrl+E。中英文标签在 [zh-CN.lang](../../../crates/joyword-shell/locales/zh-CN.lang) 和 [en.lang](../../../crates/joyword-shell/locales/en.lang)。

编辑逻辑在 [input.rs](../../../crates/joyword-shell/src/input.rs) 的 `TextInput`：`move_by`（`CaretUnit::Word`／`Paragraph`／`Document`）、`delete_word`、`toggle_runs` 和 `shows`。`toggle_runs` 只在样式效果与目标不同的部分写直接格式，其余清除直接格式，所以结果不依赖直接格式与样式的叠加方式。无头宿主 [host.rs](../../../crates/joyword-shell/src/host.rs) 的 `Office` 执行切换、对齐预设和剪切。

窗口接线在 [main.rs](../../../apps/joyword/src/main.rs) 的 `key_bindings` 和动作监听。PgUp／PgDn 走 [input_ui.rs](../../../apps/joyword/src/input_ui.rs) 的 `move_visual` 和 `page_line`。功能区按下状态在 [chrome_ui.rs](../../../apps/joyword/src/chrome_ui.rs) 的 `ribbon_pressed`。

测试：

- [word_navigation.rs](../../../crates/joyword-shell/tests/word_navigation.rs)：`word_moves_stop_at_word_starts_punctuation_and_paragraph_ends`、`word_moves_treat_leading_spaces_and_empty_paragraphs_as_stops`、`word_moves_keep_clusters_whole_and_step_through_ideographs`、`extending_by_words_keeps_the_anchor_and_a_plain_move_collapses_first`、`paragraph_and_document_moves_follow_word_processor_keys`、`word_deletion_removes_to_the_word_stop_as_one_undo_step`、`word_deletion_at_a_paragraph_edge_joins_paragraphs`、`read_only_documents_allow_word_moves_but_refuse_word_deletion`、`format_toggles_turn_on_for_mixed_selections_and_off_when_all_show_it`、`format_toggles_round_trip_on_text_whose_style_already_shows_it`、`format_toggles_span_paragraphs_and_respect_read_only`。
- [m1_edit_save.rs](../../../crates/joyword-shell/tests/m1_edit_save.rs)：`cut_toggles_and_alignment_presets_go_through_the_command_registry`、`protected_documents_refuse_edits_and_a_failed_save_keeps_bytes`。
- 应用内：`word_processor_chords_reach_their_commands`、`window_title_names_the_document_and_marks_unsaved_changes`、`registry_commands_all_have_actions_and_bindings`（[main.rs](../../../apps/joyword/src/main.rs)），`page_keys_move_about_one_screen_and_stop_at_the_ends`、`every_word_unit_command_maps_to_one_caret_move`（[input_ui.rs](../../../apps/joyword/src/input_ui.rs)）。

## 限制

- 以上都是自动测试证据。真实窗口里的按键、IME 组合期间的行为和 PgUp／PgDn 滚动手感没有人工验收；麒麟、统信真机未验证。
- PgUp／PgDn 的一屏高度按行高累计，不计页间空隙，跨页时移动距离会略小于一屏。
- 对齐按钮的按下状态只看插入点所在段落；选区跨多个对齐不同的段落时仍按插入点段落显示。

## JoyWord 假定，没有 Word 观察

- 词停靠、段落移动、删除词和格式切换的规则登记在 [行为登记表](../behavior_registry.json)（`EDIT-WORD-MOVE`、`EDIT-PARAGRAPH-DOCUMENT-MOVE`、`EDIT-WORD-DELETE`、`FORMAT-TOGGLE-MIXED`、`FORMAT-TOGGLE-STYLE`、`CLIP-CUT`），来源都是 hypothesis。
- 中日韩文本按 UAX #29 逐字停靠，没有使用词典分词。
- 对齐快捷键总是设置该对齐，再按一次不切换回左对齐。
- 标题里的 `*` 是本程序的未保存标记；macOS 另有系统的已编辑标记。
