# 页面与页眉页脚

## 使用者

布局功能区的「页面设置」：

| 命令 | 界面文字 |
| --- | --- |
| `page.size` | 纸张大小（A4、Letter） |
| `page.margins` | 页边距（普通、窄） |
| `page.orientation` | 纸张方向（纵向、横向） |
| `page.break` | 分页符 |
| `page.number` | 页码 |
| `page.edit.header` | 编辑页眉 |
| `page.edit.footer` | 编辑页脚 |
| `page.edit.body` | 编辑正文 |

功能区上「页眉」对应 `page.edit.header`。

已支持的子集：

- 改页边距、纸型和方向会写回页面设置，保存后再打开仍在。
- 显式分页符另起一页，光标偏移保留。
- 分节类型：下一页、连续、偶数页、奇数页。省略的分节类型按下一页。
- 等宽栏平分页宽。不等宽栏按每栏自己的宽度放字，分栏符推进到下一栏，最后一栏之后换页。
- 页眉页脚有默认、首页、偶数页。空的故事不再向前面继承；缺省的默认页眉会继承。
- 可以编辑页眉和页脚文字并写回部件。不能在页眉里再插入一个新段落。
- 正文和页眉页脚里的 PAGE、NUMPAGES、SECTIONPAGES 使用排出来的页码。`pgNumType` 可以改格式和起始页。省略格式是十进制；省略起始值则接着上一节，第一节从 1 开始。

## 开发者

页面框在 [pages.rs](../../../crates/joyword-layout/src/pages.rs)，页宽页高按 ECMA-376 §17.6.13。栏宽按 §17.6.3（[document.rs](../../../crates/joyword-model/src/document.rs) 的 `NewspaperColumn`）。不等宽导入要求 `equalWidth` 为假、没有分隔线、2 到 45 个带正宽度的 `w:col`。

排版测试 [pages.rs](../../../crates/joyword-layout/tests/pages.rs)：`explicit_page_breaks_split_pages_and_keep_caret_offsets`、`equal_columns_and_a_column_break_share_one_page`、`unequal_columns_place_glyphs_at_different_x_and_a_break_advances`、`a_body_page_field_uses_the_displayed_number`、`omitted_first_and_even_stories_inherit_until_an_empty_story`。孤行在 [w06_pagination.rs](../../../crates/joyword-layout/tests/w06_pagination.rs) 的 `widow_control_keeps_two_lines_together_at_a_page_break`。写回在 [header_footer_writeback.rs](../../../crates/joyword-docx/tests/header_footer_writeback.rs) 的 `edited_header_and_footer_text_round_trips_and_keeps_relationships`，以及 [w05_structure.rs](../../../crates/joyword-docx/tests/w05_structure.rs) 的 `page_setup_margin_change_round_trips_in_export`。

`pgNumType` 的省略规则写在模型 `PageNumbering` 上。布局不稳定时 `repeated_feedback_is_unstable_and_unpublished`（[cache.rs](../../../crates/joyword-layout/src/cache.rs)）拒绝发布。

## 限制

- 栏分隔线 `sep` 使该节保持保留，不排成不等宽栏。
- `nextColumn` 分节未做。
- 章节页码未做。
- 正文段落里新插入的页码只在原段落没有图片、域等行内对象，且域不带字符格式、不在超链接里时才能保存，写成 `w:fldSimple`。其余情况保存时明确拒绝；复杂域不支持。
- 多栏节里的表拒绝。
- 没有分栏符时，不等宽节的行宽取最窄栏。
- 图形界面上改页边距、看缩放后分页的人工验收未在本环境执行。

## JoyWord 假定，没有 Word 观察

- 不等宽栏里省略的 `w:space` 记为 0。实施记录把它写成导入规则，没有对应的 Word 测量。
- 最后一栏自己的 `space` 留在模型里，排版不在整行栏后面再留这段空隙。
