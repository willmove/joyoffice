# Craft 派生代码阶段 1 第二批验收

2026-11-24 在 Windows x64 上使用 Rust 1.98.1 验证。继续使用阶段 0 冻结的 `14cbb29616d7c0ababbf40a8f6c0218b996e7ae8` 原始二进制和锁文件。本批另存候选二进制、PowerPoint 加密文件、JoyDeck 旧档案、Excel 重算工作簿、语言包发布索引及验收输出。原冻结目录及散列仍可用于复跑。[机器可读结果](verification.json) · [冻结基线](../20261124-craft-pilots/README.md) · [剩余工作量](../../CRAFT-REPLACEMENT-PLAN.md)

| 范围 | 已切换的生产路径 | 兼容性证据 |
| --- | --- | --- |
| PowerPoint 16 加密 PPTX | 在 Microsoft PowerPoint 16.0.20625.20108 中将原普通样本另存为需密码打开的测试文件。 | 文件具有 CFB 容器签名及 `EncryptionInfo`、`EncryptedPackage` 流名。冻结版和候选版 CLI 均退出 1 并给出同一提示。两版桌面启动日志均记录加密拒绝且进程继续运行。源文件散列一致。 |
| JoyWord | HTTP 下载采用单项有界进度通知及可取消的响应体读取。库存按原格式读取，损坏或缺失时按已有语言包目录重建。超出解析预算的原库存通过同目录移动留档。 | 固定发布索引的 75 个正式包均经本机 HTTP 镜像下载，比对 569 个常规文件，其中 276 个许可文件。37 个由冻结版先安装，38 个由候选版先安装，75 个全部换版重装再卸载。比对拒绝联网、HTTP 跳转、404、篡改、超长、三种库存恢复、安装与移除的 8 个中断状态，以及 4 个并发安装请求。真实连接的停滞取消测试通过。 |
| JoyDeck | v1 只读桥移除了 6 个随旧模型导入的几何源文件及其 `kurbo` 依赖边。桥保留 183 个 v1 预设名和参数默认值，输出 DrawingML 交给现行几何路径。 | 4 份旧档案共 36 页，包含 183 种预设、370 个合成对象。两代模型及对象 ID 一致，两种 PPTX 导出的幻灯片 XML 和关系逐项一致，合成档案 13 页 PNG 散列一致。4 个非法预设输入的错误一致且未生成输出。旧档案附件逐字节一致，v2 跨版本重开可改为椭圆并撤销回原三角形及调整值。 |
| JoySheet | 以公开统计公式实现概率密度、累积与尾部概率、分位和相关检验。普通概率与尾部使用稳定的坐标及有界求解。 | 394 条用例覆盖 72 个注册名及别名。372 条与 Excel 16 一致，修正了旧版 44 条差异中的 22 条。其他 22 条维持冻结版行为。新旧写入与重开共四种组合均按读取端版本重算，工作簿公式和原文件散列通过检查。 |

语言包实测另从正式 HTTPS 地址刷新索引并安装荷兰语包，校验词典、断词、许可及安装记录。Windows 桌面与 CLI 同步复核合成包导致的 14→21→14 页变化，卸载后恢复 14 页。JoyWord 桌面使用独立测试副本，将 PE 栈预留设为 32 MiB，以避开前次记录的 Debug 主线程栈溢出。JoySheet 候选桌面编辑 A1 后有 22 格重算，在 5 个状态中分别比对 34 格，检查保存后的全部 394 个类型缓存，并验证撤销、重做和新进程重开。[JoyWord 安装入口测试](../../../../scripts/joyword/check-installed-language-pack.py) · [JoyDeck 预设兼容表](../../../../crates/joydeck-legacy/src/model/preset/defaults.rs) · [DrawingML 预设和调整值](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.presetgeometry?view=openxml-3.0.1)

## 公式差异

概率用例的类型和错误码精确比较。非零数值采用 `1e-10` 相对误差。`T.INV` 在中位数附近的一个样本采用 `5e-17` 绝对误差，保持对极小尾概率归零的检测。候选版改善了 12 条尾部或近中位数值，以及 10 条反函数入参边界。`BINOM.INV` 和 `CRITBINOM` 的概率或阈值等于 0 或 1 时按 Microsoft 文档与 Excel 实测返回 `#NUM!`。`BETA.INV` 和 `BETAINV` 的概率等于 0 时同样返回 `#NUM!`。[NIST 分布公式](https://www.itl.nist.gov/div898/handbook/eda/section3/eda366.htm) · [DLMF 不完全 Beta 函数](https://dlmf.nist.gov/8.17) · [BINOM.INV](https://support.microsoft.com/en-us/excel/functions/binom-inv-function) · [BETA.INV](https://support.microsoft.com/en-us/excel/functions/beta-inv-function)

剩余 22 条差异主要集中于极小卡方右尾反解、二项分位数等于累计概率的阈值、Beta 与 Gamma 的端点、Weibull 在零点的密度，以及超几何和负二项的退化参数。部分 Microsoft 文档与本次 Excel 16 观测存在差异。例如 `BINOM.INV(1,0.5,0.5)` 按文档的“大于或等于”条件返回 0，本机 Excel 返回 1。此次保留该条件和其他 21 条冻结版行为，逐格记录结果，计入后续兼容性工作。概率公式继续调用共用的 Gamma、误差函数和正态数值内核，其中尚沿用的导入实现列入阶段 1 余项。[逐格对照](verification.json) · [共用数值内核](../../../../crates/joysheet-functions/src/special.rs) · [HYPGEOM.DIST](https://support.microsoft.com/en-us/excel/functions/hypgeom-dist-function) · [NEGBINOM.DIST](https://support.microsoft.com/en-us/excel/functions/negbinom-dist-function)

## 首批回归与复跑

首批 2,324 个拼写探针、9 份演示文件及 428 条 `CONVERT` 公式均已复跑。首批 Windows 拼写快照有 5 条问题词经过系统 GBK 解码。用原冻结可执行文件重新按 UTF-8 解码后，与本批候选版的 813 条问题、定位和建议顺序完全一致。历史快照和旧版二进制保留，脚本对历史快照和 UTF-8 重放分别校验。JoySheet 另复跑既有 8 份工作簿、604 个公式缓存和 32 种两代读写组合。

三产品各自的 headless 工程检查通过。JoyWord 4,147 项通过、2 项忽略，JoyDeck 1,016 项通过，JoySheet 1,482 项通过、1 项忽略。最终 JoyWord 语言包定向测试 23 项通过，包含新增 HTTP 停滞取消用例。格式、生产 Clippy、来源、依赖和文档门禁均通过。

本机阶段 1 归档位于 `backups/craft-stage1-20261124/`。将 PowerPoint 16 样本放到 `powerpoint16-encrypted.pptx`。用 Windows Excel 对 `probability-excel16.xlsx` 强制重新计算并保存。将固定发布索引及其 75 个归档放到 `release/`。测试脚本验证发布索引和归档散列，并在隔离目录中完成下载、安装、迁移与重算。[阶段 1 复跑脚本](../../../../scripts/check_craft_stage1.py)

```powershell
python scripts/check_craft_stage1.py prepare backups/craft-stage1-20261124
python scripts/check_craft_stage1.py compare backups/craft-stage1-20261124 --frozen backups/craft-replacement-20261124 --candidate backups/craft-stage1-20261124/final --output target/craft-stage1-recheck
```

本批桌面应用的验证范围为上述 Windows 场景。其他平台真机及正式数据源独立发布继续列入原计划。

第三批已逐项复核此处留下的 22 个概率差异，并验证数值内核、JoyWord 内置索引和 Word／Deck 图元读取器。[阶段 1 续验](../20261124-craft-stage1-followup/README.md)
