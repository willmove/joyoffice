# Craft 派生代码阶段 1 第六批验收

本批继续使用冻结版 `14cbb296`，以上一批 `2fa5a586` 检查新增差异。运行环境为 Windows x64，工作簿参考值由 Excel `16.0.20625.20108` 重算。阶段 1 仍有待交付项，分产品剩余估算为 12–22 人周。[替换计划](../../CRAFT-REPLACEMENT-PLAN.md) · [验收数据](verification.json)

## JoyWord 校对与语言包

- 已把运行时的断词模式匹配从打包键哈希查找切换为基于 Liang 权重规则的字母前缀树。五套内置模式表与单独的重叠、例外和 Unicode 边界用例核对逐字间隙权重。五种语言的窄页文档启用自动断词后均有栅格变化，候选与冻结版的栅格和手动断词响应一致。按模式树实际分配核算已装语言包的内存，超出解码预算的模式表报错。[断词匹配](../../../../crates/joyword-proof/src/patterns.rs) · [Liang 论文](https://www.gtoal.com/scrabble/liang/liang-thesis.pdf)
- 新增独立托管爱尔兰语包。GaelSpell 5.1 源码重建的词典和词缀与作者产物逐字节一致，固定包词缀仅少了注释末尾六个空格。四件公开发行资产下载后的 SHA-256 与本地发布件一致，公开索引与应用内置索引逐字节一致。验证了主地址安装、主地址 404 后回退、双源散列错误时拒绝安装，以及冻结版读取并卸载候选版安装。[作者发行](https://github.com/kscanne/gaelspell/releases/tag/v5.1) · [JoyOffice 发行](https://github.com/willmove/joyoffice/releases/tag/joyword-language-packs-3) · [逐包决定](language-pack-review.json)
- 共 60／75 个包独立托管，15 个继续走原地址。越南语的 OpenOffice 1.0.45 扩展与固定包的词缀、词典和英文许可通知逐字节一致。已取得 GNU Aspell 的早期源词表，转为 Unicode 后仍需核对字词增删、声调位置及生成记录。词形识别、复合词和跨语种建议排序仍有 Craft 派生路径。[逐包决定](language-pack-review.json) · [OpenOffice 1.0.45](https://extensions.openoffice.org/en/projectrelease/vietnamese-spellchecker-1045.html) · [GNU Aspell 越南语源包](https://ftp.gnu.org/gnu/aspell/dict/vi/)

## JoySheet 工作簿

两种日期制分别核对公式值类型和错误码。财务与 ETS 沿用相对误差 `1e-9` 和绝对误差 `2e-10`。每份工作簿均用冻结版与候选版交叉保存和读回四种组合。

| 样本 | Excel 1900 一致 | Excel 1904 一致 | 较上一批新增达到容差 |
| --- | ---: | ---: | ---: |
| 416 条主财务／ETS | 348 | 348 | 0／0 |
| 243 条已有诊断 | 182 | 179 | 12／12 |
| 261 条新增长首期和置信区间诊断 | 183 | 199 | 49／43 |

- `ODDFPRICE` 和 `ODDFYIELD` 在实际首期按指定日计基础等于规则付息期时返回 `#NUM!`。相同日期在不同日计基础下可以分别落入规则和不规则分支。个别 30/360 月底样本仍与 Excel 有差异，尤其是两个日期制下的历史二月月底行为。[Microsoft ODDFPRICE](https://support.microsoft.com/en-us/excel/functions/oddfprice-function)
- `FORECAST.ETS.CONFINT` 在样本末点返回 `#NUM!`，与两种日期制的诊断一致。样本末点前后的分段小数位置、自动季节选择、拟合参数和残差置信区间仍需复核。主工作簿每种日期制还剩 66 条 ETS 差异，以及有独立数值依据的 `RATE`、`XIRR` 两条保留结果。[Microsoft FORECAST.ETS.CONFINT](https://support.microsoft.com/en-us/excel/functions/forecast-ets-confint-function) · [第五批逐条结果](../20261124-craft-stage1-close/sheet-review.json)
- `CONVERT` 针对当前目录及前缀、大小写和错误用例核对 273 条公式，273 条与冻结版一致，263 条满足 Excel 容差。10 条差异对应三个转换常数 `eV`、`u` 和 `mmHg`。保留冻结版常数以维持已有工作簿重算结果。电子伏特和原子质量单位采用 2018 CODATA 值，毫米汞柱系数四舍五入至 NIST 表的精度后为 133.3224。[Microsoft CONVERT](https://support.microsoft.com/en-us/excel/functions/convert-function) · [NIST 2018 CODATA](https://physics.nist.gov/cuu/pdf/nonsi_2018.pdf) · [NIST 换算表](https://www.nist.gov/system/files/documents/2025/12/23/2026_NIST_HB133_AppendixE-20251222.pdf)

其余工作簿重跑覆盖 394 条概率公式、117 条概率边界、174 条独立数值内核探针和 8 份工作簿的 604 个既有公式缓存。阶段 0 短回归覆盖 2,324 个拼写探针、9 个演示文件和 428 条换算公式。Deck 专项另复跑 183 种预设的 370 个对象和 PowerPoint 16 的加密文件。[回归证据](verification.json)

## 桌面应用

- JoyWord 页面位图的尺寸按设备像素向上取整，旧图像控件按页面的逻辑尺寸再次适配，边缘取整后可能让 GPU 对相邻像素做线性采样。候选版对满分辨率的位图按设备像素原尺寸绘制，页面和编辑叠层沿用同一布局坐标。超出页面预算的位图按原路径适配。在 Windows 隔离配置用默认纸张和 A4 DOCX 验证 100% 显示，并用控制端复核 A4 的 125% 缩放。A4 六页尺寸、布局指纹和正文与冻结版相同。分数 DPI 的对齐由单元测试验证。[桌面页面绘制](../../../../crates/joyword-ui-gpui/src/document_view.rs)
- JoyDeck 和 JoySheet 正常桌面编辑窗口均可从「文件 → 关于」查看当前版本。Windows 实测两个对话框均显示 `0.2.0`，版本来自构建时的应用版本。JoySheet 的入口和对话框支持中英文界面。[JoyDeck 关于](../../../../crates/joydeck-ui-gpui/src/file_ui.rs) · [JoySheet 关于](../../../../crates/joysheet-ui-gpui/src/ribbon_widgets.rs)

## 工程检查

三个产品的 `cargo xtask check` 均通过格式、Clippy 和测试。JoyWord 通过 4,184 项，忽略 2 项。JoyDeck 通过 1,035 项。JoySheet 通过 1,519 项，忽略 1 项。新增用例和全工作区格式、文档及来源约束检查均通过。原始日志和工作簿保存在本机 `backups/craft-stage1-remaining-20261124`。[验收数据](verification.json)

## 后续推进

1. JoyWord 用已装语言包构建逐语种的词缀、复合词和排名对照，按调用链替换剩余校对逻辑。并行补齐 15 个原地址包的对应生成材料和作者通知。
2. JoySheet 用可解析序列定位 ETS 的自动季节与拟合误差，用两种日期制继续收紧置信区间和 30/360 月底计日。按函数登记推进剩余数学和查找分支。
3. JoyDeck 及公共图元读取器的阶段 1 项目已验收。下阶段从文稿对象兼容与编辑模型推进。
