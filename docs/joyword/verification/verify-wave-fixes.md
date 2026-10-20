# 验证波次缺口修复（F1）

日期 2026-10-19。S1、S3、S5 三份验证记录里各有已知缺口：[页面场景金样](golden-scenes.md)、[特性样例](feature-fixtures.md)、[场景排练](scenario-rehearsals.md)。F1 修复其中五条，每条都先用测试重现，再改实现。相关任务状态不在本文改动，T200–T203、T210、T300 仍按 [TASKS](../TASKS.md) 记录。本文只有自动证据，没有图形界面操作，也没有 Word／WPS 对照，不能当作人工验收。

实现依据是 ECMA-376 Part 1 公开条文，没有搬运第三方实现。没有新增依赖。

## 修复

| 缺口 | 根因 | 修复 | 回归测试 |
| --- | --- | --- | --- |
| 表格样式 `firstCol`、`lastCol`、`band1Vert` 导入时丢失 | `w:tblStylePr` 的类型（§17.7.6、§17.18.89）只映射了行条件和横向条带 | 增加 `firstCol`、`lastCol`、`band1Vert`、`band2Vert` 到模型条件的映射；样式部件仍不改写 | `column_and_band_table_style_conditions_import_and_survive_an_edit`（[tables.rs](../../../crates/joyword-docx/tests/tables.rs)）；`vertical_column_bands_are_imported_from_table_style_pr` 不再忽略 |
| 编号标签后接制表符时 CPU 光栅返回 `InvalidGeometry`；T200 样例 `render` 拒绝 | 列表排版把正文第一段放在左缩进而不是标签后的制表位，正文与标签重叠，首行缩进也落错；光栅器要求每个字形原点等于此前前进量之和 | 正文从标签后的位置开始；光栅器允许笔位向前跳过制表间隙或点线引导符的余量，仍拒绝向后回退 | `a_tab_suffix_without_hanging_indent_starts_the_body_at_the_next_stop`、`a_first_line_indent_moves_only_the_first_line`（[document.rs](../../../crates/joyword-layout/tests/document.rs)）；`a_tab_gap_may_skip_the_pen_forward_but_not_back`（[raster.rs](../../../crates/joyword-render/tests/raster.rs)） |
| 已有正文段落 `page.number` 后导出拒绝 | 段落改写只接受文字与格式变化，新增行内对象一律拒绝 | 原段落没有行内对象、新增的都是无格式页码域时，按原有段落检查去掉域后比较，再把域写成 `w:fldSimple`（§17.16.19） | `a_page_field_inserted_into_a_plain_paragraph_exports_and_round_trips`（[import.rs](../../../crates/joyword-docx/tests/import.rs)）；排练 `t202_margins_orientation_page_numbers_and_zoom` |
| 保留的 `wrapTight` 让排版拒绝、编辑导出返回 `ExportError::Restricted` | 只有 `wrapNone`／`wrapSquare` 进入模型，其余绕排整段保留并锁定 | `wrapTight`／`wrapThrough`（§20.4.2）按锚点范围近似为方形绕排，`wp:wrapPolygon` 不跟随；drawing 不登记为片段，导出写回原绕排 XML | `a_tight_or_through_wrap_is_approximated_as_a_square_and_its_markup_survives_an_edit`（[images.rs](../../../crates/joyword-docx/tests/images.rs)）；`feature_fixtures_import_round_trip_and_keep_unknown_parts` |
| T300 的 `text` 输出含删除和移出文字 | 修订导入后成为带标记的 run，CLI 投影仍逐字输出 | 投影跳过最终视图里不可见的 run（删除与 moveFrom），包括其中的行内对象 | `imported_deleted_and_moved_out_runs_stay_out_of_the_text`（[commands.rs](../../../apps/joyword-cli/tests/commands.rs)）；`feature_fixture_cli_projections_match_goldens` |

## 选择说明

- 投影一项可以改输出，也可以改元数据。CLI 帮助、[CLI 指南](../guide/cli.md) 与实现记录的 T210 段都写明不含已删除修订和 moveFrom，元数据也这样声明，所以改的是输出。
- `wrapTight` 只是近似。文字留在矩形之外，不沿图形轮廓贴近，也不穿过 `wrapThrough` 的空隙。这不是已实现的紧密型或穿越型环绕。`wrapTopAndBottom`、衬于文字下方和非页面相对位置仍保留原字节并锁定。
- 页码域只覆盖插入到无行内对象段落的无格式域。带字符格式、位于超链接内，或段落原来已有图片、域等对象时，导出仍明确拒绝。

## 黄金文件

重新 bless 的文件：`apps/joyword-cli/tests/golden/t200-numbering-tab.json`、`t200-numbering-tab.raster.json`（由 `refused` 变为 `ok`）、`t201-banded-nested-table.json`、`t201-banded-nested-table.raster.json`，以及 `apps/joyword-cli/tests/fixture_golden/DOCX-FEAT-T203-IMAGES.inspect.json`、`DOCX-FEAT-T300-REVIEW.inspect.json`、`DOCX-FEAT-T300-REVIEW.txt`。全部为 LF 行尾。

## 仍未覆盖

- 真正的轮廓绕排。
- 带格式页码域和已有行内对象段落里的新增页码域。
- 图形界面观感、Word／WPS 打开结果、信创真机。
- T203 四条图片关系另报 `not-imported` 的诊断（资源已导入）。

用户指南、行为登记、样例清单、任务与实现记录、验收清单里 `wrapTight` 保留锁定的旧描述，以及五条缺口的 open 状态，已在 #67 合入后的文档收尾里更新。
