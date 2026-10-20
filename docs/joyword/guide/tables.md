# 表格

## 使用者

插入功能区的「表格」组：

| 命令 | 界面文字 |
| --- | --- |
| `table.insert` | 插入表格 |
| `table.row.insert` | 插入行 |
| `table.row.delete` | 删除行 |
| `table.column.insert` | 插入列 |
| `table.column.delete` | 删除列 |
| `table.select` | 选择单元格 |
| `table.merge.right` | 向右合并 |
| `table.merge.down` | 向下合并 |
| `table.split` | 拆分单元格 |
| `table.properties` | 表格属性 |

已支持的子集：

- 网格必须盖满。短行、零跨度、孤立的纵向合并会被拒绝，模型保持原样。
- 插入或删除一行、一列各是一步撤销。合并结构上的行列编辑会拒绝。
- 向右或向下合并、拆分单元格，各是一步，撤销能回到合并前。
- 选择停在一个单元格里。跨单元格删除会拒绝。
- 单元格里可以再放一层表。第二层仍整表不透明并锁定，旁边的正文段落也不能改到把这张表丢掉。
- 边框子集是 nil、single、double、dotted、dashed、thick。直接格式盖过表格样式。
- 样式可以画首行、末行、横向条纹，以及首列、末列和纵向条纹。这些条件都从样式部件的 `w:tblStylePr` 读入；样式部件保存时原样写回。
- 取消表格属性对话框不会开启事务。

## 开发者

模型在 [block.rs](../../../crates/joyword-model/src/block.rs)：`w:tblLook` 按 ECMA-376 §17.4.56，宽度百分比按 §17.18.90，`w:tblLayout` 按 §17.4.52，表对齐按 §17.4.28，`w:vAlign` 按 §17.4.83，行高按 §17.4.80。单元格边距按 §17.4.43。省略的 `w:vMerge` 取值按 §17.4.84 为 continue（[table.rs](../../../crates/joyword-docx/src/table.rs)）。嵌套表的单元格内容按 §17.4.64。

条件格式的叠放顺序在 [style.rs](../../../crates/joyword-model/src/style.rs) 与 [style_paint.rs](../../../crates/joyword-layout/src/style_paint.rs)，按 ECMA-376 §17.7.6：整表，然后纵向条纹，再横向条纹，再列，再行。代码注释写明不采用 MS-OE376 里 Word 的偏差顺序。

引擎测试 [tables.rs](../../../crates/joyword-engine/tests/tables.rs)：`grid_invariants_reject_a_short_row_and_an_orphan_merge`、`merging_cells_is_one_undoable_transaction`、`splitting_a_cell_is_one_undoable_transaction`、`editing_outer_or_inner_cell_text_is_one_undoable_transaction`。导入 [tables.rs](../../../crates/joyword-docx/tests/tables.rs)：`a_one_level_nested_table_is_typed_and_round_trips`、`unknown_or_deeper_nesting_stays_opaque_and_locks`。排版 [tables.rs](../../../crates/joyword-layout/tests/tables.rs)：`table_styles_paint_first_and_last_columns_and_vertical_bands`、`a_nested_table_places_glyphs_inside_the_parent_cell`、`a_table_nested_two_levels_deep_is_refused`。

## 限制

- 不能拖列宽。
- 第二层及更深的嵌套表保持不透明。
- 四角单元格、主题色底纹未做。
- 多栏节里的表以 `table in newspaper columns` 拒绝。
- 合同表格的人工插行、合并、保存重开未在本环境执行。

## JoyWord 假定，没有 Word 观察

- 省略的 `w:tblLook`：首行、末行和横向条纹开，纵向条纹关，首列和末列关。元素在而属性省略则为假。实施记录写明这是假定。
