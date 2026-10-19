# 页面场景金样（S1）

本文件记录 `PageScene` → PDF → CPU 光栅这条链的自动证据。输入是测试里现场生成的原创 DOCX，经 `joyword_docx` 导入，再用 `joyword_layout` 排版。字体是仓库内的 `tests/fixtures/fonts/joyoffice-test-serif.ttf`（仅 A、V、f、i 与空格）。金样因此不依赖系统字体，也不依赖 Word、WPS 或图形界面。

对应任务仍为 `in_progress`。这里只增加自动证据，不改任务状态。验收口径见 [QUALITY.md](../QUALITY.md)：屏幕与 PDF 的单位换算误差不超过 0.01 pt；本文件不声称与固定 Word 版本的几何对照。

测试源码：[golden_scenes.rs](../../../apps/joyword-cli/tests/golden_scenes.rs)。期望文件在 `apps/joyword-cli/tests/golden/`，每个场景三份：`<场景>.json`（场景摘要）、`<场景>.pdf.json`（`inspect_pdf` 报告）、`<场景>.raster.json`（光栅摘要）。

## 如何更新金样

```sh
JOYWORD_BLESS_GOLDEN=1 cargo test -p joyword-cli --test golden_scenes
```

环境变量 `CI` 已设置时，即使带上 `JOYWORD_BLESS_GOLDEN=1` 也不会写文件。CI 只比较，不刷新。不匹配时测试打印最多 40 行字段级差异（路径、缺失键、长度、左右值）。

## 摘要里有什么

场景摘要使用稳定的 pretty JSON（`serde_json` 的对象键有序）。坐标是整数 EMU。每页记录宽高、页边距、页眉页脚距离、字形运行（原点、基线、前进量、字号、颜色、字形 id 与偏移；正文是单个字母时不另存文本）、矩形（推断的种类、颜色、边界）、图片（边界与像素 SHA-256）以及排版注释和诊断。

矩形本身没有种类字段。测试按颜色和几何推断：颜色 `0563c1` 记为链接下划线；又高又窄且完全落在正文左缘以左的记为页边修订条；高度的四倍仍小于宽度的记为描边；其余记为填充。

PDF 摘要来自公开的 `inspect_pdf`：页盒（存成百分之一点）、字体数、带 ToUnicode 的字体数、链接 URI、图像 XObject 数、Producer，以及创建/修改日期是否出现。同一场景连续 `render_export` 两次，字节必须相同。页盒与场景宽高的差在测试里另断言不超过 0.01 pt。字体数大于 0 时，要求每个嵌入字体都带 ToUnicode。

公开的 `PdfReport` 没有文本矩阵或字形原点。`joyword-cli` 也不能额外依赖解压库去读内容流。因此本切片不断言“文本矩阵与场景原点相差不超过 0.01 pt”。页盒的 0.01 pt 比较已经覆盖公开 API 实际给出的几何。

原始 PDF 的 SHA-256 不进入金样。CI 会在 macOS、Windows、x86_64 Linux、ARM64 Linux 以及 QEMU 下的 aarch64 与 loongarch64 上跑同一测试。字体子集和 PDF 对象字节是否跨这些目标完全一致，本切片没有证明。进程内两次渲染字节相同，只说明单次进程内可重复。

CPU 光栅走 `joyword_render::rasterize`，72 dpi、缩放 1。像素 SHA-256 同样不入库，原因与 PDF 字节相同。金样改为 32×32 分块的着墨覆盖率：像素的 r+g+b 小于 750 视为着墨，覆盖率以万分比计（10000 为整块着墨），允许偏差 250。一块里几个抗锯齿像素的漂移可以通过；缺一个约 12 pt 的字形则不行。同一页光栅两次，像素必须逐字节相同。尺寸不同或覆盖率超出容差则失败。

## 场景

| 金样 | 测试 | 来源 | 断言 |
| --- | --- | --- | --- |
| `t202-equal-columns` | `equal_columns_match_the_golden_scene` | T202 等宽栏 | 一页，两个不同的运行原点 |
| `t202-unequal-columns` | `unequal_columns_and_a_column_break_match_the_golden_scene` | T202 不等宽栏与栏分隔 | 一页；第二栏起点比第一栏靠右，间隔大于 900 twip |
| `t202-continuous-even-page` | `continuous_and_even_page_sections_match_the_golden_scene` | T202 连续节与 evenPage | 至少三页；首页同时有 A 与 V；中间一页无运行；末页有 f |
| `t202-headers-page-fields` | `first_even_and_default_headers_match_the_golden_scene` | T202 首页/偶数/默认页眉，PAGE 与 NUMPAGES | 三页；首页只有 A，次页只有 V，第三页只有 f |
| `t203-wrap-none` | `wrap_none_anchor_matches_the_golden_scene` | T203 页面绝对 `wrapNone` | 一张图；字母 A 的原点仍在正文左缘，并与图矩形重叠 |
| `t203-wrap-square` | `wrap_square_text_avoids_the_rectangle` | T203 页面相对 `wrapSquare`（#60） | 没有运行与图矩形重叠，且有运行移到图的右侧或下方 |
| `t201-banded-nested-table` | `banded_table_with_a_nested_table_matches_the_golden_scene` | T201 横向条带与一层嵌套表（#57 的可导入部分） | 两张表；样式含 `band1Horz`；页面上有蓝色 `0000ff` 填充 |
| `t300-revisions-markup` / `final` / `original` | `revision_markup_final_and_original_match_the_golden_scene` | T300 正文与页眉的插入、删除和移动（#58） | 标记视图含 V、f、i，有非黑色运行和页边条；最终视图有 V 且无 f/i；原始视图有 f、i 且无 V |
| `t204-paper-and-link` | `paper_sizes_and_an_https_link_match_the_golden_scene` | T204 纸型与 https 链接 | 两页尺寸不同；首页有 `0563c1` 下划线；PDF 链接恰为 `https://example.invalid/joyword` |
| `t200-numbering-tab` | `numbering_label_and_a_tab_stop_match_the_golden_scene` | T200 编号标签与制表位 | 场景含标签 A、正文 f 与 i，并有制表间隙 |

`sha256_matches_fips_180_4_vectors` 用空字节和 `abc` 核对测试内的 SHA-256（FIPS 180-4）。图片摘要用的是这个实现，不另加依赖。

## 这些金样不能证明的事

- 不与 Word 或 WPS 的分页、栏宽、页眉或修订色对照。
- 不覆盖图形界面、选择、光标、打印对话框或真机。
- 不证明 PDF 内容流里的文本矩阵。公开检查报告没有该项。
- 不证明 PDF 字节或像素在 x86_64、aarch64、loongarch64 之间逐字节相同。
- 页眉金样记录了 `story-exceeds-margin`。样例页把页眉距离设成 200 twip，12 pt 运行比这段距离高。这是小页面上的确定结果，存在场景摘要里，不是对 Word 的符合性结论。
- 页码数字在提交的测试字体里没有字形。页眉运行里对应数字的字形 id 为 0。摘要保存这个 id，测试字体下没有单独的 missing-glyph 诊断。
- 编号加制表位的光栅金样是 `status: refused`、`error: InvalidGeometry`。光栅器要求每个字形的 `x` 等于此前前进量之和；制表间隙把后面的字形推到 1440 twip，因而被拒绝。场景摘要和 PDF 摘要仍然覆盖标签与制表位。这是已知缺口，本切片不改 `crates/**`。光栅开始接受这种行时，金样会失败，需要重新 bless 并改写本节。

## 已知导入缺口

`t201` 的样式同时写了 `band1Horz`（蓝）、`firstCol`（红）、`lastCol`（黄）和 `band1Vert`（绿）。导入后样式只保留 `Band1Horizontal`。`FirstColumn` 与 `Band1Vertical` 不在模型里，页面上也没有对应的红或绿填充。被忽略的测试 `vertical_column_bands_are_imported_from_table_style_pr` 期望三者都被导入。最小输入就是该测试使用的 `banded_table_package`。布局侧在模型直接带有这些条件时可以绘制，缺口在 DOCX 条件样式的读入。本切片不修改 `crates/**`。

## 可选的独立回读

`apps/joyword-cli/examples/feature_scenes.rs` 用调用者提供的字体（CI 为 Noto CJK，面索引 2）把同一批特征写成 PDF。短语是原创中文，便于 `pdftotext -enc UTF-8` 读回。脚本是 `scripts/verify_feature_pdf.py`，单测是 `scripts/tests/test_verify_feature_pdf.py`。它核对页数，以及必须出现和不得出现的短语（最终视图不得含删除文本，原始视图不得含插入文本）。空白页算一页。它同样不证明栏的几何、绕排矩形或像素。

```sh
cargo run --locked -p joyword-cli --example feature_scenes -- /usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc 2 target/feature-scenes
python3 scripts/verify_feature_pdf.py target/feature-scenes
```
