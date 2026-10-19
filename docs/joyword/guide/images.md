# 图片与绕排

## 使用者

插入功能区的「插图」：`edit.insert_image`，界面文字是「插入图片」。也可以把本地图片文件拖进窗口。

已支持的子集：

- 行内图片跟在同一行的文字后面。两次插入同一份字节会共用一个资源。插入是一步撤销。
- 页面绝对、位于文字前面的 `wrapNone`：文字不避开矩形。
- 页面相对、位于文字前面的 `wrapSquare`：正文行留在矩形外面。距离读 wrap 上的 distT／distB／distL／distR，没有再读锚点上的同名属性。
- 页面相对、位于文字前面的 `wrapTight` 和 `wrapThrough`：按锚点范围近似为方形绕排，正文行留在矩形外面。轮廓多边形 `wp:wrapPolygon` 不跟随，文字不贴近图形，也不穿过空隙。原绕排 XML 在保存时照原样写回。
- `wrapTopAndBottom`、衬于文字下方，以及不是相对页面的位置，整段保持原字节并锁定，不假装已经绕排。
- 页眉和页脚里的行内图片画在各自的带上。
- 远程链接、SVG、EMF、WMF 和 OLE 在取字节之前就拒绝。不会按 URL 去下载。
- 过大的图片在使用前拒绝。被拒绝的粘贴或拖放不改文档；同一批拖放里，拒绝的文件跳过，其余仍按顺序插入。

## 开发者

导入在 [images.rs](../../../crates/joyword-docx/src/images.rs)。`square_distances` 在属性和锚点都没有距离时返回 0。位置要求 `relativeFrom` 为 `page`，或者 `simplePos`。非零 `effectExtent` 拒绝。

排版 [images.rs](../../../crates/joyword-layout/tests/images.rs)：`text_after_an_inline_image_stays_on_the_same_line`、`a_wrap_square_keeps_text_outside_the_rectangle_and_wrap_none_does_not`、`a_page_anchor_is_a_positioned_rectangle_shared_with_the_cpu`、`a_header_and_footer_image_paint_on_their_bands`。导入测试 [images.rs](../../../crates/joyword-docx/tests/images.rs)：`a_wrap_square_page_anchor_imports_and_other_wraps_stay_preserved`、`a_tight_or_through_wrap_is_approximated_as_a_square_and_its_markup_survives_an_edit`、`a_remote_link_is_not_fetched_and_an_oversize_header_is_kept`、`an_anchor_refuses_svg_emf_wmf_and_ole_without_fetching`。PDF 与 CPU 共用矩形：`a_wrap_square_rectangle_is_shared_by_pdf_and_the_cpu`（[images.rs](../../../crates/joyword-render/tests/images.rs)）。

代码注释把这条绕排标成 JoyWord 假设。行为登记里的 `IMAGE-WRAP-SQUARE` 与 `IMAGE-WRAP-TIGHT-APPROX` 同样没有 Word 观察。

## 限制

- 真正的紧密型和穿越型轮廓绕排未做，`wrapTight`／`wrapThrough` 只是方形近似。
- 两侧同时排字未做。`wrapText` 为 left 或 right 时只用那一侧；省略、`bothSides` 或 `largest` 记为较宽一侧。
- 锚点段之前已经放下的行不改。
- 表格单元格里的文字不绕这张图。页眉页脚里的方形不参与正文绕排。
- 负偏移、`wp:align`、非页面的 `relativeFrom` 拒绝。
- 平台专有的富剪贴板图片格式未做。Word／WPS／浏览器双向粘贴未在本环境执行。
- 带图片的页眉页脚段落不另画修订装饰。

## JoyWord 假定，没有 Word 观察

- 省略的绕排距离是 0 EMU。这不是 Word 在省略距离时使用的 0.125 英寸。实施记录写明来源是假设。
