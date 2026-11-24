# Craft 派生代码阶段 1 第三批验收

本批于 2026-11-24 在 Windows x64 上验证，继续使用阶段 0 的 `14cbb296` 冻结二进制，并以阶段 1 上一批候选版校验增量。Excel 参考工作簿由 Microsoft Excel 16.0.20625.20108 强制重算并保存。全部证据使用合成语料和既有发布数据。[验证明细](verification.json) · [上一批验收](../20261124-craft-stage1/README.md) · [剩余工作量](../../CRAFT-REPLACEMENT-PLAN.md)

## Excel 16 逐项复核

原有 22 条差异全部复现。对每条都校验了冻结版、上一批候选版、本批候选版和 Excel 16 的值类型及结果。依照公开参数契约或可独立计算的数学结果，本批继续保留下面的冻结版行为。[卡方右尾反解](https://support.microsoft.com/en-us/excel/functions/chisq-inv-rt-function) · [二项分位数](https://support.microsoft.com/en-us/excel/functions/binom-inv-function) · [Gamma 密度](https://support.microsoft.com/en-us/excel/functions/gamma-dist-function) · [Beta 密度](https://support.microsoft.com/en-us/excel/functions/beta-dist-function) · [Beta 反解](https://support.microsoft.com/en-us/excel/functions/beta-inv-function) · [Weibull 密度](https://support.microsoft.com/en-us/excel/functions/weibull-dist-function) · [超几何分布](https://support.microsoft.com/en-us/excel/functions/hypgeom-dist-function) · [旧超几何分布](https://support.microsoft.com/en-us/excel/functions/hypgeomdist-function) · [负二项分布](https://support.microsoft.com/en-us/excel/functions/negbinom-dist-function)

| 原工作簿 | 公式 | JoySheet | Excel 16 | 保留依据 |
| --- | --- | --- | --- | --- |
| G109 | `CHISQ.INV.RT(1E-40,5)` | `197.44468613194988` | `#NUM!` | 有限反解经半整数或整数上不完全 Gamma 递推独立校验。 |
| G114 | `CHIINV(1E-40,5)` | `197.44468613194988` | `#NUM!` | 与 CHISQ.INV.RT 为相同右尾反解，独立回代通过。 |
| G192 | `BINOM.INV(1,0.5,0.5)` | `0` | `1` | 按公开定义取第一个累积概率大于或等于阈值的整数。 |
| G200 | `CRITBINOM(1,0.5,0.5)` | `0` | `1` | 旧别名沿用公开定义的含等号阈值。 |
| G249 | `GAMMA.DIST(0,1,2,FALSE)` | `0.5` | `#NUM!` | 形状参数为 1 时零点右极限和指数密度均为 1/scale。 |
| G256 | `GAMMADIST(0,1,2,FALSE)` | `0.5` | `#NUM!` | 旧别名保留指数密度的零点值。 |
| G283 | `BETA.DIST(0,1,2,FALSE)` | `2` | `#NUM!` | 端点形状参数为 1 时，有限的单侧密度为另一形状参数除以区间宽度。 |
| G284 | `BETA.DIST(1,2,1,FALSE)` | `2` | `#NUM!` | 端点形状参数为 1 时，有限的单侧密度为另一形状参数除以区间宽度。 |
| G292 | `BETA.INV(1,1,2)` | `1` | `#NUM!` | 公开参数范围允许概率 1，分位数为区间上界。 |
| G298 | `BETAINV(1,1,2)` | `1` | `#NUM!` | 旧别名保留概率 1 对应区间上界。 |
| G318 | `WEIBULL.DIST(0,1,5,FALSE)` | `0.2` | `0` | 形状为 1 时密度是 1/scale，小于 1 时发散并报告数字错误。 |
| G319 | `WEIBULL.DIST(0,0.5,5,FALSE)` | `#NUM!` | `0` | 形状为 1 时密度是 1/scale，小于 1 时发散并报告数字错误。 |
| G327 | `WEIBULL(0,1,5,FALSE)` | `0.2` | `0` | 旧别名保留相同的零点密度及非有限值契约。 |
| G328 | `WEIBULL(0,0.5,5,FALSE)` | `#NUM!` | `0` | 旧别名保留相同的零点密度及非有限值契约。 |
| G337 | `HYPGEOM.DIST(3,9,10,10,TRUE)` | `#NUM!` | `0` | 公开契约拒绝样本数或总体成功数为零，以及超出支持集的样本成功数。 |
| G339 | `HYPGEOM.DIST(2,4,0,20,TRUE)` | `#NUM!` | `1` | 公开契约拒绝样本数或总体成功数为零，以及超出支持集的样本成功数。 |
| G340 | `HYPGEOM.DIST(0,0,3,20,TRUE)` | `#NUM!` | `1` | 公开契约拒绝样本数或总体成功数为零，以及超出支持集的样本成功数。 |
| G345 | `HYPGEOMDIST(2,4,0,20)` | `#NUM!` | `0` | 旧别名继续拒绝总体成功数为零及超出支持集的输入。 |
| G348 | `NEGBINOM.DIST(0,1,0,FALSE)` | `0` | `#NUM!` | 公开参数范围包含 0 和 1，使用成功概率端点的退化分布。 |
| G349 | `NEGBINOM.DIST(0,3,1,FALSE)` | `1` | `#NUM!` | 公开参数范围包含 0 和 1，使用成功概率端点的退化分布。 |
| G350 | `NEGBINOM.DIST(0,3,1,TRUE)` | `1` | `#NUM!` | 公开参数范围包含 0 和 1，使用成功概率端点的退化分布。 |
| G351 | `NEGBINOM.DIST(5,3,0,TRUE)` | `0` | `#NUM!` | 公开参数范围包含 0 和 1，使用成功概率端点的退化分布。 |

另有 95 条相邻用例。50 条和 Excel 16 一致，另 45 条覆盖这些分布的相邻端点或参数组合，均沿用上一批的结果。补验发现 `BINOM.INV(3,0.5,0.5)` 和 `CRITBINOM(3,0.5,0.5)` 在上一批受计算舍入影响返回 2，本批依照对称二项分布的下中位数修正为 1，与 Excel 16 和阶段 0 一致。22 个卡方右尾探针已用整数或半整数 Gamma 递推独立回代，涵盖至 `1e-300` 的尾概率。工作簿按类型和错误码精确比较，非零数值采用 `1e-10` 相对容差，独立回代采用 `3e-12` 相对容差。[用例生成和回归](../../../../scripts/check_craft_boundaries.py)

## 生产路径和回归

| 范围 | 实现 | 证据 |
| --- | --- | --- |
| JoySheet 共用数值内核 | Gamma 使用递推、反射和 Stirling 修正。不完全 Gamma／Beta 使用缩放连分式及对数尾部。正态反解使用带括区间的 Newton 迭代。工程误差函数和 ETS 置信区间继续调用共用内核。 | 394 条概率公式重算仍有 372 条与 Excel 16 相符，四种跨版本读写通过。以 Python 标准数学库及独立概率求和核对了 174 条内核用例。另复跑 8 份既有工作簿、604 个公式缓存和 32 种读写组合。候选版桌面修改 A1 后验证了 22 个联动格、5 个编辑状态、撤销、重做、394 个保存缓存和新进程重开。 |
| JoyWord 语言包发布索引 | `joyword-language-packs-1` 随程序交付，授权后刷新直接读取内置字节。新旧缓存分开存放，校验通过的旧缓存允许现有用户继续安装。 | 可重复生成固定散列的索引，核对 75 个档案、569 个普通文件、276 个许可文件与 25 项默认数据源和说明。CLI 验证未授权拒绝、旧缓存升级、缓存损坏恢复和旧版回退，并经原地址安装验证荷兰语档案。旧版和候选版全量镜像安装、换版重装及卸载回归通过。 |
| Word／Deck 图元读取器 | 按 MS-WMF／MS-EMF 重写有界字段和点列视图，保留字节序、浮点位、零长度与越界语义。 | Word 和 Deck 各打开 WMF、EMF 文档。4 张页面 PNG 和 4 张 PDF 光栅，与同机冻结版的 RGB SHA-256 全部一致。专项还覆盖未对齐字段、极值、截断和伪造长度。 |

语言包索引的生成、校验和本地发布产物已完成。75 个档案仍由原地址提供并按固定 SHA-256 校验，独立档案托管及发行列入剩余工作。JoyWord 桌面验证使用 32 MiB 栈预留的独立测试副本。当前桌面验收覆盖 Windows。[内置索引](../../../../crates/joyword-engine/data/language-pack-index.json) · [逐文件来源和许可清单](language-pack-manifest.json) · [MS-WMF](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-wmf/) · [MS-EMF](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-emf/) · [DLMF 数值公式](https://dlmf.nist.gov/)

三产品 headless 工程检查通过。JoyWord 4,149 项通过、2 项忽略，JoyDeck 1,017 项通过，JoySheet 1,483 项通过、1 项忽略。共用读取器、两个图元 crate 和公式 crate 的定向测试共 344 项通过。首批 2,324 个拼写探针、9 份演示文件和 428 条 `CONVERT` 回归也通过。

## 复跑

本机证据目录为 `backups/craft-stage1-followup-20261124/`，冻结版仍位于 `backups/craft-replacement-20261124/14cbb296/`。以下命令使用已经由 Excel 16 重算的参考工作簿与准备好的候选二进制，输出目录应为新路径。

```powershell
$run = 'backups/craft-stage1-followup-20261124'
$frozen = 'backups/craft-replacement-20261124/14cbb296'
$prior = 'backups/craft-stage1-20261124/final'
python scripts/check_craft_boundaries.py target/stage1-boundaries --source "$run/boundaries.xlsx" --excel "$run/boundaries-excel16.xlsx" --frozen $frozen --prior $prior --candidate "$run/candidate"
python scripts/check_craft_boundaries.py target/stage1-kernels --kernels --candidate "$run/candidate"
python scripts/joyword/stage-language-pack-index.py backups/craft-stage1-20261124/release target/stage1-word-index --check
python scripts/verify_shared_metafile.py "$run/candidate" target/stage1-metafile --baseline "$run/metafile-frozen/pixels.json"
```
