# 特性样例无界面证据

日期 2026-10-20。样例由 [make_feature_fixtures.py](../../../scripts/make_feature_fixtures.py) 用 Python 标准库确定性生成，登记在 [manifest.json](../../../tests/fixtures/manifest.json)，说明见 [fixtures README](../../../tests/fixtures/README.md)。`feature/` 不由 `joyword-corpus` 生成。

本文只记录自动测试看到的导入、往返、投影和排版结果。任务 T200–T203、T300 仍为 `in_progress`。没有 Word／WPS 打开记录，没有图形界面操作，不能当作人工验收。

生成与核对：

```text
python3 scripts/make_feature_fixtures.py --check
python3 -m unittest discover -s scripts/tests
cargo test --locked -p joyword-cli --test fixture_roundtrip
```

测试文件是 [fixture_roundtrip.rs](../../../apps/joyword-cli/tests/fixture_roundtrip.rs)。`inspect` 与 `text` 的标准输出锁定在 `apps/joyword-cli/tests/fixture_golden/`，例如 [DOCX-FEAT-T200-NUM.inspect.json](../../../apps/joyword-cli/tests/fixture_golden/DOCX-FEAT-T200-NUM.inspect.json) 与 [DOCX-FEAT-T200-NUM.txt](../../../apps/joyword-cli/tests/fixture_golden/DOCX-FEAT-T200-NUM.txt)。重新生成黄金文件时设置 `JOYWORD_BLESS_GOLDEN=1`。

五个包都带未引用部件 `word/custom/note.xml`。未编辑导出走原包复制，字节与提交文件相同。可编辑样例在正文段「可编辑句。」末尾插入「补」，再导出、再导入；修订后的语义转储一致，其余正文段文字不变，`note.xml` 的未压缩字节不变。正则 `$1` 替换、插行、合并、拆分、撤销、接受和拒绝都需要引擎命令，本测试没有调用。

## 样例结果

| ID | 特性 | 子集结果 | 测试 | 仍未验证 |
| --- | --- | --- | --- | --- |
| [DOCX-FEAT-T200-NUM](../../../tests/fixtures/feature/DOCX-FEAT-T200-NUM.docx) | STYLE-002／003／004、EDIT-006：Body→Clause→Article 的 `basedOn`，多级编号 `lvlRestart` 与 `startOverride`，左对齐点线制表位，正文 `Order-014` | 导入诊断为空。样式链为 Article、Clause、Body。编号标签为 `3`、`3.一`、`4`、`4.一`。制表位 2880 twip、左对齐、点线。未编辑包字节相同；文本修补往返保留该编辑 | `feature_fixtures_import_round_trip_and_keep_unknown_parts`、`feature_fixture_cli_projections_match_goldens`、`feature_fixture_render_and_convert_follow_the_subset` | 引擎正则 `$1` 未执行。`render` 与 `convert` 在 Noto Sans CJK face 2 下均为 1 页、`omitted_blocks` 为 0。无 Word／WPS 对照 |
| [DOCX-FEAT-T201-TABLE](../../../tests/fixtures/feature/DOCX-FEAT-T201-TABLE.docx) | LAYOUT-005：合同表 `gridSpan`／`vMerge`、边框、`clear` 底纹、`tblLook` 首行／首列／末列与条纹、一层嵌套表 | 导入诊断为空，整表类型化。未编辑包字节相同；文本修补往返成功。`render` 与 `convert` 均为 1 页、`omitted_blocks` 为 0 | 同上三项 | 插行、合并、拆分、撤销未执行。无图形界面制作合同的记录。无 Word／WPS 对照 |
| [DOCX-FEAT-T202-PAGES](../../../tests/fixtures/feature/DOCX-FEAT-T202-PAGES.docx) | LAYOUT-004／012／013：`nextPage` 后接 `continuous`，不等宽栏与分栏符，`titlePg` 与奇偶页眉，PAGE／NUMPAGES，`keepNext`／`widowControl`／`keepLines` | 导入诊断为空。第二节没有页眉引用。页眉域为 PAGE 与 NUMPAGES。未编辑包字节相同；文本修补往返成功。`render` 与 `convert` 均为 2 页、`omitted_blocks` 为 0 | 同上三项 | 本测试不断言链接后的页眉故事标识，也不断言孤行／同页的行几何。无 Word／WPS 对照 |
| [DOCX-FEAT-T203-IMAGES](../../../tests/fixtures/feature/DOCX-FEAT-T203-IMAGES.docx) | LAYOUT-006：行内 PNG、相对页面的 `wrapNone`、相对页面的 `wrapSquare`，以及原 XML 必须保留的 `wrapTight` | 四张图都成为图片资源。`wrapTight` 按锚点范围近似为方形绕排（不沿轮廓多边形，不算已实现 wrapTight），drawing 原 XML 不改写。四条图片关系报 `not-imported`（关系遍历不把图片算作已消费关系，资源仍然导入）。未编辑包字节相同；文本修补往返成功，`wrapTight` 字节保留。`render` 与 `convert` 均为 1 页、`omitted_blocks` 为 0 | 同上三项 | 真正的轮廓绕排未实现。清理、资源去重、样式编号映射、单次撤销和剪切失败原子性未在本样例执行。无 Word／WPS 对照 |
| [DOCX-FEAT-T300-REVIEW](../../../tests/fixtures/feature/DOCX-FEAT-T300-REVIEW.docx) | REVIEW-001／003／004：正文插入／删除、成对 `moveFrom`／`moveTo`（名称 `move-a`），页眉插入／删除，`trackRevisions` | 导入诊断为空。正文修订种类为插入、删除、移出、移入；页眉为插入和删除。未编辑包字节相同；文本修补往返后这些修订仍在。`render` 与 `convert` 均为 1 页、`omitted_blocks` 为 0 | 同上三项 | 接受、拒绝和对应撤销未执行。无 Word／WPS 对照 |

`render`／`convert` 使用本机 `/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc` 的第 2 个 face（与 `JOYWORD_TEST_FONT` 相同）。找不到该字体时，视觉测试直接返回，不把缺字体写成失败。

## F1 已修复（Fixed in F1）

下面三条原来写在上表的「仍未验证」或结果里，F1 修复后黄金文件已重新 bless。说明见 [verify-wave-fixes.md](verify-wave-fixes.md)。

- `DOCX-FEAT-T200-NUM` 的 `render` 原来拒绝：`page raster refused: invalid content or budget`。列表正文现在从标签后的制表位开始，光栅器允许笔位向前跳过制表间隙和点线引导符留下的余量，`render` 写出 1 页。
- `DOCX-FEAT-T203-IMAGES` 原来因 `wrapTight` 锁定段落：诊断有 `image:rId4:Unsupported` 与 `preserved:w:drawing:Unsupported`，文本修补导出为 `ExportError::Restricted`，`render` 与 `convert` 都拒绝 `document layout refused: unsupported content or budget`。现在 `wrapTight`／`wrapThrough` 按锚点范围近似为 `wrapSquare`，原绕排 XML 仍按原字节写回，未编辑与编辑后保存都成功。这只是近似，轮廓绕排仍未实现。
- `DOCX-FEAT-T300-REVIEW` 的投影元数据声明排除删除和移出，stdout 原来仍含「删去的旧句」和「旧址」。T210 契约（CLI 帮助与实现记录）同样规定排除，所以改的是输出，不是元数据。现在 `text` 与 `inspect` 的文字为「新增一句虚构说明」「新址」「可编辑句。」三段。

## 图形界面与 Word／WPS 抽查

有图形界面或对照软件时，用下面的步骤看这些包，不要改预期来迁就观察结果：

1. 打开 `tests/fixtures/feature/` 下对应 docx。确认样式链、编号从 3 起、制表点线、合同表合并与嵌套、分节与页眉、四张图里 `wrapTight` 仍在、修订标记仍在。
2. 在每个样例中只改「可编辑句。」，保存后再打开，其余段落和 `word/custom/note.xml` 应仍在。`DOCX-FEAT-T203-IMAGES` 的 `wrapTight` 在 JoyWord 里按方形绕排显示，保存后在 Word／WPS 中应仍是紧密型环绕。
3. 用固定 Word 与 WPS 打印或导出 PDF 后，再用 `scripts/compare_pdf_lines.py` 对 JoyWord `convert` 的 PDF 做行对照。对照档案建立前，清单里的 `reference_profile.status` 保持 `unverified`，用例状态保持 `planned`。

本次没有执行上述步骤。
