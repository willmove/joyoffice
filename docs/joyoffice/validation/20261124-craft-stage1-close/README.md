# Craft 派生代码阶段 1 第五批验收

本批在 Windows x64 使用冻结版 `14cbb296`、上一批 `fecec53c` 和本批候选 CLI。参考工作簿由 Microsoft Excel `16.0.20625.20108` 重建依赖并全量重算。两种日期制单独保存，再按原误差门槛比较数值类型、错误码和结果。[验收证据](verification.json) · [第四批记录](../20261124-craft-stage1-release/README.md) · [剩余工作量](../../CRAFT-REPLACEMENT-PLAN.md)

## JoyWord 发行与回退

逐项核对 34 个候选包的 132 个固定来源地址。新发布的 18 个归档沿用原始字节、版本、长度和 SHA-256。公开发行附有源文件、许可与作者通知，全部 21 个发行资产经匿名下载核验。JoyOffice 累计托管 59 个语言包，现有 75 项索引保留原版包系列标识。 [JoyOffice 发布](https://github.com/willmove/joyoffice/releases/tag/joyword-language-packs-2) · [源码与通知包](https://github.com/willmove/joyoffice/releases/download/joyword-language-packs-2/sources-and-notices.zip) · [逐包决定](language-pack-review.json)

两种加泰罗尼亚语的词干和词缀与 Softcatala v3.0.8 的生成结果逐字节一致，随发源码包含可编辑词根和生成工具。希伯来语词典与 Debian `myspell-he` 1.4-3.1 的两个文件逐字节一致，随发 Hspell 1.4 和匹配的 Debian 构建材料。威尔士语、印度尼西亚语和瑞典语等 LGPLv3 包补齐 GPLv3 全文。 [Softcatala 固定源码](https://github.com/Softcatala/catalan-dict-tools/tree/d1fc142c482699f487c2ca78b9ae9ba8646f908f) · [Debian Hspell 源包](https://packages.debian.org/sid/source/hspell) · [GNU LGPLv3](https://www.gnu.org/licenses/lgpl-3.0.html)

16 个包继续从原发行方获取。德语三种变体已经取得 igerman98 20161207，Frami 对应补充词表及生成转换仍待核实。韩语 0.7.92 的词缀匹配，LibreOffice 词表与 0.7.92、0.7.93 的公开产物均有差异。泰米尔语已找到的 Apache 扩展与固定包字节及许可声明不同。挪威语两个包尚缺合并词表的对应生成材料及部分来源的分发授权。其余包的缺项列在逐包决定中。 [逐包决定](language-pack-review.json)

使用加泰罗尼亚语和希伯来语验证公开主地址、主地址 404 后回退、双源散列不符时拒绝安装，以及冻结版读取并卸载候选版安装。继续验证旧索引缓存、损坏缓存恢复、联网授权和源文档字节。全 75 个归档已通过镜像重装与跨版本库存验收。 [回退脚本](../../../../scripts/joyword/check-language-pack-index.py)

## JoySheet 逐条复核

76 个位置逐项重算并记录三代 JoySheet、Excel 的结果和每一条的处置依据。比较继续使用 `1e-9` 相对容差、`2e-10` 绝对容差，值类型与错误码精确比较。 [逐条结果](sheet-review.json)

| 日期制 | 冻结版一致 | 上一批一致 | 本批一致 | 消除差异 | 新增差异 | 双版本重开 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 1900 | 315 / 416 | 340 / 416 | 348 / 416 | 8 | 0 | 4 / 4 |
| 1904 | 316 / 416 | 340 / 416 | 348 / 416 | 8 | 0 | 4 / 4 |

- `ODDFPRICE` 和 `ODDFYIELD` 的四条差异全部消除。实际／360 和实际／365 下，首个不规则片段及应计利息按实际天数计算，后续完整准票息期按一整期计入首笔票息。用零票息、零收益率和带息价格分开校验赎回折现、应计与首期现金流。两种日期制的 120 条证券诊断各新增 24 条达到容差。剩余控制差异涉及发行日恰等于准票息边界和少量 30/360 月底计日，保留在后续工作量中。 [Microsoft ODDFPRICE](https://support.microsoft.com/en-us/excel/functions/oddfprice-function)
- `FORECAST.ETS` 现可查询样本时间轴内的一步拟合值，并在采样点间插值。样本起点以前仍返回 `#NUM!`。主工作簿中四条样本内预测达到容差，另五条从错误值变成数值，仍有拟合误差。两个日期制的 105 条 ETS 预测诊断各新增 25 条达到容差。Excel 16 的样本内行为和 Microsoft 文档存在差异，本批采用实测的样本内查询行为，保持公开的 8,760 季节参数上限。 [Microsoft FORECAST.ETS](https://support.microsoft.com/en-us/excel/functions/forecast-ets-function) · [ETS 状态空间模型](https://otexts.com/fpp3/exponential-smoothing.html)
- 两种日期制各余 68 条原差异，其中 66 条为 ETS。自动季节、平滑参数、一步残差及置信区间继续逐项保留对照。周期诊断确认 B 序列的真实重复间隔为 4，C 去趋势后滞后 5 的自相关结构可以复现。`RATE` 的精确根为 0，`XIRR` 的候选值与 65 位十进制复算根相差约 `2.87e-18`，这两条保留候选求解结果。 [逐条结果](sheet-review.json) · [Microsoft XIRR](https://support.microsoft.com/en-us/excel/functions/xirr-function)

合计额外重算 486 条诊断公式，新达到 Excel 容差 98 条。两种日期制此前达到容差的公式全部通过。复跑 394 条概率公式、117 条边界、174 条共用内核探针、8 份既有工作簿的 604 个公式缓存及 32 种双版本读写组合。JoySheet headless 工程检查通过 1,483 项，1 项既有忽略。 [验收证据](verification.json)

## 复跑

以下工作簿及二进制位于本机 `backups/`。主对照工作簿与诊断工作簿使用相同的 Excel 16 版本重算。旧索引与旧归档保持各自的散列。

```powershell
$run = 'backups/craft-stage1-close-20261124'
$previous = 'backups/craft-stage1-release-20261124'
$frozen = 'backups/craft-replacement-20261124/14cbb296'
$oldPacks = 'backups/craft-stage1-20261124/release'
python scripts/check_craft_stage1_differences.py workbook "$previous/finance-ets.xlsx" target/ets-and-coupons.xlsx
python scripts/check_craft_finance_ets.py target/stage1-close-1900 --source "$previous/finance-ets.xlsx" --excel "$run/finance-replay-excel16.xlsx" --frozen $frozen --prior "$previous/sheet-accepted" --candidate "$run/candidate"
python scripts/check_craft_finance_ets.py target/stage1-close-1904 --source "$previous/finance-ets-1904.xlsx" --excel "$run/finance-replay-1904-excel16.xlsx" --frozen $frozen --prior "$previous/sheet-accepted" --candidate "$run/candidate"
python scripts/joyword/check-language-pack-index.py "$frozen/joyword-cli.exe" "$run/candidate/joyword-cli.exe" "$oldPacks/index.json" target/word-stage1-close --locale he --expected-hosted 59 --expected-tag joyword-language-packs-2
```
