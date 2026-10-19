# 排版行为规则目录

随固定 Word 参考环境的实测逐步建立（QUALITY §4）。每条规则记录：观测环境（Word 版本／build、OS、日期）、样例、观测结果与对应的 JoyWord 侧依据。只有经固定 Word 2024 永久版实测的条目才是规则；推测内容不登记。

## 环境基线

- Word：Word 2024 永久版（Office 家庭版 2024），build 16.0.20430.20182，Windows 11 x86_64。
- 观测方式：COM 自动化只读打开／新建文档，`ExportAsFixedFormat` 导出 PDF（位图字体替代关闭），pdfplumber 抽取逐行文字与行框。产物在 `tests/fixtures/reference/word2024/`。
- 记录日期：2026-10-18。

## 已实测条目

### R1：未显式指定字体的中文默认字体

Word 2024 对不含显式中文字体指定的正文使用**等线（DengXian）**渲染（样例 DOCX-MIN-001/003/004/005 与 DOCX-SAMPLE-001 的参考 PDF 内嵌字体均为等线）。DOCX-MIN-002 的显式样式解析为宋体（中文）＋Times New Roman（西文，常规／加粗）。

### R2：同页框同度量下的断行一致（两处对照）

同文、同字体（等线 DengXian）、同字号（12 pt）、同固定行距（18 pt）、同页框（148×140 mm、页边距 18 mm）下：

- 合同样例（20 段，`document_flow` 场景）：Word 与 JoyWord 同为 2 页 24 行，逐行文字与行位一致（行首／行底偏差 ≤1 pt），见 `breaks/result_contract.txt`。
- 标点压力样例（675 字、含连续 `。：；（）` 的 15 次重复）：Word 与 JoyWord 同为 2 页 27 行，逐行文字与行位一致，标点不出现在行首，见 `breaks/result_stress.txt`。

JoyWord 侧依据：`wrap_mixed`＋Unicode 默认断点＋`adjust_cjk_break` 在该样例集合上与 Word 2024 的断行一致。**注意**：样例均为左对齐固定行距短页框，未覆盖两端对齐余量分配、标点压缩、孤行控制与混合格式；这些待后续实测登记。

### R3：两端对齐的余量分配

Word 2024 两端对齐（Justify）下，纯中文行把余量分配到字间并抵满版心（样例 stress 文本，行尾 x1 ≈ 368.65 pt ≈ 右边界），段末行不拉伸（x1 = 351.02 pt < 版心右界）。产物 `breaks/word_justify.pdf`。JoyWord 对纯中文行调用 `justify_intercharacter`（证据 Inferred：等分余量，段末行与单字行不拉伸）。含拉丁字母或 ASCII 标点的行，以及 `distribute`，仍然拒绝。尚未用 Word 2024 重新导出对照。

### R4：单倍行距的机制（实测推导）

Word 2024 默认单倍行距（LineSpacingRule = single）的实测与机制拆解（12 pt 字号，pdfplumber 行距测量）：

| 字体／字号 | 吸附开启（默认） | 吸附关闭（snapToGrid=0） |
| --- | --- | --- |
| 宋体 12 pt | 15.60 pt（1.30 em） | — |
| 宋体 10.5 pt | 15.60 pt | — |
| 等线 10.5 pt | 15.60 pt | 14.16–14.28 pt |
| 等线 12 pt | 31.20 pt（2.60 em） | 16.20–16.35 pt |
| 微软雅黑 12 pt | 31.20 pt（2.60 em） | — |

机制（与全部数据点一致）：

1. **自然行高 ≈ 1.3 ×（usWinAscent＋usWinDescent 跨度）× 字号**，四个 snap-off 实测点全部吻合：
   - 等线 12：1.3×1.042×12 = 16.25 → 实测 16.20–16.35
   - 等线 10.5：1.3×1.042×10.5 = 14.22 → 实测 14.16–14.28
   - 微软雅黑 12：1.3×1.32×12 = 20.59 → 实测 20.52–20.64
   - 宋体 12：1.3×1.0×12 = 15.6 → 实测 15.6
2. **文档自动创建行网格**：本页框下 Word 写出 `<w:docGrid w:type="lines" w:linePitch="312"/>`（312 twips = 15.6 pt），`snapToGrid` 默认开启，实际行距 = ⌈自然行高 ÷ 网格⌉ × 网格。
3. 吸附侧 7 个数据点全部一致：宋体／仿宋／楷体（win 跨度均 1.0 em）12 pt 与 10.5 pt、等线 10.5 pt 吸附 1 格（15.6 pt）；等线 12 pt（自然 16.2 > 15.6）与微软雅黑 12 pt（自然 20.6 > 15.6）吸附 2 格（31.2 pt）。
4. 社区熟知的“等线／雅黑在 Word 中行距异常大”现象由此解释：自然行高超过 1 格后吸附为 2 格。
5. 边界记录：Word 对本机的 Noto Serif SC 可变字体未解析出字体名，导出落回 Calibri 替代链，该数据点不可用于公式。

1.3 系数与 win 跨度的取法由三个字体族、四个 snap-off 实测点拟合（误差 ≤0.4%），宋体／仿宋／楷体 hhea=typo≠win、等线三套跨度相同、雅黑 hhea=win≠typo，数据点均支持 win 跨度。产物：`breaks/word_single_*.pdf`、`breaks/word_nogrid*.pdf`、保存的 DOCX 中的 sectPr 网格设置。

JoyWord 只在段落为单倍行距（`w:spacing@w:line` = 240 且 `w:lineRule` = auto），且整形字体名称表解析为宋体、仿宋、楷体、等线或微软雅黑时使用上述公式（证据 Inferred）。`snapToGrid` 未显式关闭时吸附到这条实测 312 twips 网格；显式关闭则保留自然行高。黑体不在该拟合集合内，未具名字体和其他行距规则继续用调用方给出的行框。模型尚未保存 `w:docGrid`，因此不读取其他 `linePitch`。尚未用 Word 2024 重新对照。

### R5：孤行控制的默认状态与强制拆分实测

- 本机 Word 2024 新建文档（Normal 模板）的段落 `WidowControl` 读值为 0（关闭）。
- 强制拆分场景（页满后接 2 行或 3 行段落）：无论关闭还是显式对受影响段落设置 `WidowControl=True`（已核实 `w:widowControl` 写入保存的 DOCX），Word 仍按 1+1／1+2 拆分，页底保留单行（孤儿）、页首保留单行（寡行）；固定 18 pt 行距与单倍行距（31.2 pt）两种设置下一致复现。
- 产物：`breaks/word_widow.pdf`、`breaks/word_widow_on.pdf`、`breaks/word_widow2.pdf`、`breaks/word_widow3.pdf`、`breaks/word_widow4.pdf`（含对应 DOCX）。
- 注意：该结果与"孤行控制开启即避免单行"的一般预期不符，可能是本 build 的行为或场景特殊性；建议在 GUI 会话中通过段落对话框切换孤行控制复核。
- JoyWord（2026-10-19，T202 第四切片）：`w:widowControl` 为真时按 ECMA-376 §17.3.1.26 在段内分页处尽量保持至少两行同在页顶或页底（`widow_control_keeps_two_lines_together_at_a_page_break`）；未与 Word 2024 逐页对照，不推翻上述 R5 负结果登记。

### R6：keepNext（与下段同页）实测

强制边界场景（页满后接标题行＋正文行）：对标题段设置 `KeepWithNext=True` 后，Word 把标题与正文一起移到下一页（页 1 保持 8 行填充），`w:keepNext` 写入 XML。与 R5 的孤行控制负结果互为对照——分页引擎确实执行段落标记，R5 的负结果因此更可信。产物：`breaks/word_keepnext.pdf`（含对应 DOCX）。

### R7：标点压缩的分类规则

Word 2024 默认标点压缩（DengXian 12 pt 实测，advance 精确到 0.01 pt）：

| 场景 | Word 实测 advance | JoyWord 对照 |
| --- | --- | --- |
| 全角引号 “ ” | 4.31 pt（≈0.36 em） | 12.0 pt（全宽） |
| 标点＋标点（连排） | 前一个字距 6.0 pt（0.5 em），串尾恢复 12.0 pt | 全部 12.0 pt |
| 标点＋汉字 | 12.0 pt（全宽） | 12.0 pt |

即：压缩按位置生效——连排标点除串尾外全部半宽，孤立标点保持全宽；引号在两类场景下都按约三分之一宽排。与 R2 的断行一致互证：压力样例中无连排标点与引号，孤立标点全宽故断行一致。产物：`breaks/word_punct.pdf`、`breaks/word_punct2.pdf`、`breaks/joyword_punct.pdf` 与逐字 advance 表。JoyWord 仅当名称表为等线，且 `punctuation_advance` 成功并窄于该字形的整形宽度时，才把该宽度写入页面场景（Measured／Scaled／Inferred 沿用函数自身的标记）。全宽结果、未验证配对和其他字体保持整形宽度。行尾无跟随标点按函数约定保持未验证。尚未用 Word 2024 重新对照。

### R8：keepLines 强制拆分实测

强制边界处 2 行段落设置 `KeepTogether=True`（w:keepLines），固定行距与单倍行距两种设置下 Word 仍按 1+1 拆分（R5 的 widowControl 同为负结果；R6 的 keepNext 为正对照）。标记—结果矩阵：keepNext 生效、keepLines 与 widowControl 在本 build 的强制场景下未生效，建议 GUI 会话复核后再实现（T202）。产物：`breaks/word_keeplines.pdf`、`breaks/word_keeplines_single.pdf`。

### R9：悬挂标点观测

样例行首均未出现标点悬挂（行起点 x0 恒等于左边距），边界标点按禁则换行处理；未见悬挂标点行为。本机 Word 默认设置下的悬挂开关行为待专门场景实测。

## 待实测方向

- R4 公式的更多字体族复核（Noto Serif SC 可变字体需先解决 Word 名称解析）。
- 悬挂标点的开关与默认行为（需构造行尾恰为标点的场景）。
