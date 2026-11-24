# Craft 派生代码阶段 1 第四批验收

在 Windows x64 上使用阶段 0 冻结版 `14cbb296`、阶段 1 上一批 `ccf067b2` 和本批候选版。Excel 参考由 Microsoft Excel `16.0.20625.20108` 强制重算并保存。用例及版本交叉结果见[机器可读证据](verification.json)。[剩余工作量](../../CRAFT-REPLACEMENT-PLAN.md)

## JoyWord 语言包

逐包核对了 75 个归档及 339 条来源记录，覆盖归档大小、SHA-256、授权声明、通知文件和归档内数据的源文件散列。核对以发布索引和上游固定提交 `20467b63558f04fe075e83aaea501bd10f7fa684` 为准。41 个具备本批发行证据的归档保持原字节和通知，已独立发布。匿名下载核验通过 41 个归档、1 份索引和 1 份逐包许可清单。[固定发行规范](https://github.com/storytold/craft-i18n/blob/20467b63558f04fe075e83aaea501bd10f7fa684/docs/adr/0004-third-party-language-data.md) · [JoyOffice 独立发布](https://github.com/willmove/joyoffice/releases/tag/joyword-language-packs-1) · [逐包许可清单](https://github.com/willmove/joyoffice/releases/download/joyword-language-packs-1/publication-audit.json)

候选版内置的 75 项索引保持归档版本、长度和散列。41 项优先使用 JoyOffice 地址，主地址失败或校验不通过后尝试原地址。34 项继续使用原发行地址。待补材料包括德语三种变体的 igerman98／frami 首选源文件、希伯来语生成输入、韩语和挪威语生成文件的配套源码，以及泰米尔语词典文件的许可证明。其他待发行项目的许可选择和通知记录列在逐包清单。[内置索引](../../../../crates/joyword-engine/data/language-pack-index.json) · [发行核对脚本](../../../../scripts/joyword/stage-hosted-language-packs.py)

CLI 对照覆盖无网络授权拒绝、新旧索引缓存读取、旧版回退、缓存损坏恢复，以及从新地址实际安装荷兰语包、主地址 404 后从原地址安装、两处内容均不符合索引散列时拒绝写入库存。镜像回归使用全部 75 个原始归档，覆盖 75 次跨版本重装、8 种库存中断状态、4 次并发安装和最终卸载。Windows 桌面测试使用 32 MiB 栈预留的候选副本，验证从公开地址安装后的断词分页与 CLI 一致，卸载后恢复起始结果。文档字节和既有网络设置保持一致。[回退脚本](../../../../scripts/joyword/check-language-pack-index.py) · [全量归档回归](../../../../scripts/check_craft_stage1.py) · [桌面复验](../../../../scripts/joyword/check-installed-language-pack.py)

## JoySheet 公式

- ETS 四个入口共用新的加性状态递推、季节初值和有界拟合实现。保留工作表参数、时间轴预算和 Forecast Sheet 入口。现有时间轴适配及统计量包装仍在生产路径中。Excel 的自动季节识别、优化和置信区间差异留有逐项证据。[ETS 模型](https://otexts.com/fpp3/ets.html) · [Excel FORECAST.ETS](https://support.microsoft.com/en-us/excel/functions/forecast-ets-function)
- 证券定价在票息日历上计算折现现金流、应计利息、久期及收益率反解。复核了五种日计数基础、单票息、跨闰日和非整期票息。两种固定日计数基础下的长首期票息仍需专项复核。[Excel 证券函数](https://support.microsoft.com/en-us/excel/financial-functions-reference) · [Excel ODDFPRICE](https://support.microsoft.com/en-au/excel/functions/oddfprice-function)
- 年金按首末现金流、支付时点和期间增长率估值。`RATE` 用对数增长坐标解平衡式。`FV`、`PV` 和 `PDURATION` 保留 Excel 近零输入的舍入行为，PMT 沿用此前经 Excel 对照的实现。已有现金流函数回归覆盖 `NPV`、`XNPV`、`IRR`、`XIRR` 和 `MIRR`。[Excel RATE](https://support.microsoft.com/en-us/excel/functions/rate-function)

每种日期制使用 416 条公式、52 个函数名称。值类型和错误码精确比较，数值使用相对容差 `1e-9`、绝对容差 `2e-10`。

| Excel 日期制 | 冻结版一致 | 候选版一致 | 消除差异 | 新增差异 | 双版本重开 |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1900 | 315 / 416 | 340 / 416 | 25 | 0 | 4 / 4 |
| 1904 | 316 / 416 | 340 / 416 | 24 | 0 | 4 / 4 |

每种日期制仍有 76 条相同位置的既有差异，包含 70 条 ETS、4 条长首期证券和各 1 条 `RATE`、`XIRR`。`RATE` 探针的年金平衡式精确根为零，候选值在机器舍入量级，Excel 缓存为约 `-4.34e-10`。`XIRR` 的候选结果沿用上一批。另用 120 格复核欧洲 30/360 在两种日期制中的月底计算，覆盖票息日前、票息日和闰日，候选版与 Excel 结果全部一致。

394 条概率分布公式、117 条概率边界用例及 174 条数值内核探针复跑通过，沿用上一批已登记的概率兼容差异。既有 8 份工作簿的 604 个公式缓存及 32 种双版本读写组合通过。阶段 0 的 2,324 个拼写探针、9 份演示文件和 428 条 `CONVERT` 公式通过。JoyWord 和 JoySheet 的 headless 工程检查通过。完整 XLSX 重算结果保存在本机证据目录。

## 复跑

```powershell
$run = 'backups/craft-stage1-release-20261124'
$frozen = 'backups/craft-replacement-20261124/14cbb296'
$prior = 'backups/craft-stage1-followup-20261124/candidate'
python scripts/check_craft_finance_ets.py target/stage1-finance --source "$run/finance-ets.xlsx" --excel "$run/finance-ets-excel16.xlsx" --frozen $frozen --prior $prior --candidate "$run/sheet-accepted"
python scripts/check_craft_finance_ets.py target/stage1-finance-1904 --source "$run/finance-ets-1904.xlsx" --excel "$run/finance-ets-1904-excel16.xlsx" --frozen $frozen --prior $prior --candidate "$run/sheet-accepted"
python scripts/joyword/check-language-pack-index.py "$frozen/joyword-cli.exe" "$run/verified-build/joyword-cli.exe" backups/craft-stage1-20261124/release/index.json target/stage1-word-hosting
```
