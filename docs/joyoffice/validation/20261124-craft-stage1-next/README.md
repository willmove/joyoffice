# Craft 替换阶段 1 第七批验收

冻结版为 `14cbb296`，本批开发起点为 `3699f57c`。Excel 参考值来自 Windows 上的 Microsoft Excel 16.0.20625.20108 完整重算。对比使用原有公式缓存读回规则，数值绝对容差 2e-10、相对容差 1e-9。以下各表两列日期制使用同一组公式，工作簿属性分别启用 1900 和 1904。

## JoyWord

复核全部 15 个待托管包及 57 条固定来源。保加利亚语通过作者 4.1 的 OpenOffice 包重建，拼写数据转换后与固定包逐字节一致，发布包同时附可编辑数据库、构建材料和 GPLv2 通知。加利西亚语通过 Proxecto Trasno 20.08 的模块重建，全部 232,425 个词条以及词缀规则行均相同。构建与固定产物只差一组词缀块的位置和两组撇号词条的排序。发行仍采用原样的固定归档，保留其 SHA-256、大小、语言标识及库存结构。源码包同时提供两个语种的原始断词授权。[逐包审核](language-pack-review.json) · [重建说明](REBUILD-BG-GL.md) · [JoyOffice 发行及源码](https://github.com/willmove/joyoffice/releases/tag/joyword-language-packs-4)

词形识别按恢复后的词根和有效词缀条件归并候选，依次检查同形词和可交叉的词缀。建议排序中的回退词干按容量限定维护候选集，保留同分候选的替换规则及最后的词典顺序。剩余复合词、多语种词缀与断词边界继续沿计划验证。[词形识别](../../../../crates/joyword-proof/src/affix.rs) · [候选排序](../../../../crates/joyword-proof/src/affix_suggest/ranking.rs)

使用 75 个固定档案运行 1,499 个多语种探针，与上一候选版逐条比对拼写标记范围、词语及建议顺序，结果全部一致。另有四组 48 个合成探针和冻结版英美拼写的 2,324 个探针通过。所有 75 个包均完成与冻结版的交叉安装、库存读回及卸载。两个新托管包分别通过主地址下载、模拟主地址失效后从原地址下载、旧版读回和卸载、错误散列拒绝及损坏缓存恢复。[多语种对照](word-spelling-review.json) · [档案交叉安装](word-pack-regression.json) · [保加利亚语回退](rollback-bg.json) · [加利西亚语回退](rollback-gl.json)

## JoySheet

| 用例组 | 每种日期制公式数 | 1900 修正 / 仍有差异 | 1904 修正 / 仍有差异 |
| --- | ---: | ---: | ---: |
| 冻结 ETS 与金融主表 | 416 | 5 / 63 | 5 / 63 |
| 前次边界诊断 | 243 | 5 / 56 | 8 / 56 |
| ETS 分数步置信区间 | 202 | 75 / 84 | 75 / 84 |
| ETS 初始状态、斜率和系数探针 | 366 | 96 / 189 | 96 / 189 |
| 长首期 30/360 月底扩展 | 360 | 8 / 212 | 20 / 224 |
| 前次 ETS 与票息补充探针 | 261 | 12 / 66 | 12 / 50 |
| GCD／LCM 与查找 | 78 | 5 / 2 | 5 / 2 |
| COMBIN／COMBINA 参数和范围 | 67 | 38 / 0 | 38 / 0 |

以上各组、两种日期制均完成冻结版和候选版的四种 XLSX 写入与读回组合。与前一版相比，上表没有新增 Excel 差异。主表 63 条差异含 61 条 ETS 和 2 条已登记的 RATE／XIRR 求解精度选择。61 条 ETS 差异逐格归档，含 27 条预测、10 条置信区间、2 条自动季节周期和 22 条拟合统计。保留 8,760 的文档季节上限。扩展组之间存在重复，按各组用例分别验收。[逐格结果](sheet-review.json) · [Microsoft FORECAST.ETS](https://support.microsoft.com/en-us/office/forecast-ets-function-15389b8b-677e-4fbd-bd95-21d464333f41)

分数步置信区间按方差插值。Excel 16 在末两个历史样本之间有一段零宽区间，最后样本点及其相邻边界为 `#NUM!`。对残差无法识别参数的严格整点线性数据，报告 Excel 16 实测的平滑系数默认值，季节分支至少要求两个完整周期。另用 174 条不同周期、趋势和噪声强度的序列验证自动季节检出，确认 66 条既有差异。部分含二阶波形的四步样本会被 Excel 判断为两步，单独移动自相关阈值无法稳定重现该行为。带噪声拟合和季节选择仍待收口。

30/360 长首期票息在按固定日付息的分支，对二月最后一日按工作簿日期制计算应计天数与折现天数。前次留存的长首期月底差异已清零，完整月底付息日历和部分邻近日仍需继续核对。[证券计日](../../../../crates/joysheet-functions/src/financial/securities.rs)

数学与查找已切换整数 GCD／LCM 内核和近似查找的二分边界。LCM 在所有合法入参中先判定零。传统 MATCH 和现代二分查找各自按 Excel 16 的实测方向选取相等键。1900 日期制的 230 个重复键和整数边界探针修正 41 条，余下 1 条为保留的 2^53 文档上限。其余公式仍按注册表和来源核对。[数学函数](../../../../crates/joysheet-functions/src/math.rs) · [查找函数](../../../../crates/joysheet-functions/src/lookup.rs) · [Microsoft GCD](https://support.microsoft.com/en-us/excel/functions/gcd-function) · [Microsoft LCM](https://support.microsoft.com/pt-br/excel/functions/lcm-function)

COMBIN 和 COMBINA 使用有界的组合数递推，商可表示时避免中间乘积溢出，并按 Excel 16 实测拒绝达到有符号 32 位最大值的组合总体数量。冻结回归通过条件聚合、矩阵和经典文本、现代文本和日期、工程和折旧四族的八份工作簿，以及 428 条 CONVERT 用例。394 条概率分布用例仍有已复核的 22 条 Excel 差异，候选版符合上一批的验收预期。[概率边界复核](../20261124-craft-stage1-followup/README.md)

## JoyDeck

文件页的「开始 → 最近」「打开 → 最近」使用同一个路径显示函数。正常盘符、UNC 和设备路径按其类别显示。列表路径、搜索、提示、无障碍说明和信息页使用友好路径。打开仍沿用存储记录的 Windows 原路径。Windows 原生窗口实测独立配置中已有的带前缀中文路径条目可显示、打开并在信息页显示完整文件名。[文件页](../../../../crates/joydeck-ui-gpui/src/file_ui.rs) · [共享路径显示](../../../../crates/joyoffice-ui/src/lib.rs)

冻结版差分通过 PowerPoint 16 加密文件、183 种预设的 370 个对象、13 张逐像素相同的渲染图，以及 4 组非法预设拒绝。四份迁移样本均验证原件保留、跨版本读回和 PPTX 部件散列。旧版与新版生成的容器大小可不同，回归脚本检查每个容器的实际字节数，并继续严格比对文档内容及导出结果。[差分结果](deck-regression.json) · [回归脚本](../../../../scripts/check_craft_stage1.py)

## 阶段 1 收尾

Windows 上三个产品的 `cargo xtask check` 均通过，含格式、严格 Clippy 和产品测试。仓库文档、源码门禁和 `git diff --check` 通过。本批原生界面复核覆盖 Windows 的 JoyDeck 文件页。

JoyDeck 加密识别、旧版预设迁移、最近文件路径及共享图元读取器按现有验收范围收口。

- JoyWord 资源剩余 `af`、`de`、`de-at`、`de-ch`、`et`、`hi`、`ko`、`nb`、`nn`、`pt-pt`、`sw`、`ta`、`vi`。逐包核对作者原始许可文字和可编辑来源，能复建固定包后再演练原地址回退。[逐包审核](language-pack-review.json)
- JoyWord 生产校对继续复核复合词规则、多级词缀展开及候选生成。覆盖合字、同形词、带重音词和断词的多语种对照，并在原生审阅验证更正、撤销和重开。[复合词](../../../../crates/joyword-proof/src/affix/compound.rs) · [词形展开](../../../../crates/joyword-proof/src/affix/forms.rs) · [候选生成](../../../../crates/joyword-proof/src/affix_suggest.rs)
- JoySheet 优先收口带噪声 ETS 的初始状态、系数估计与自动周期，再复核月底对齐的准票息段切分。公式注册表继续覆盖标量舍入、指数与三角函数的溢出，以及数组／引用查找返回契约。[公式逐格记录](sheet-review.json) · [更新后的计划](../../CRAFT-REPLACEMENT-PLAN.md)
