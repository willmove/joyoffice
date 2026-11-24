# Craft 派生代码首批替换验收

2026-11-24 在 Windows 11 x64 上以 Rust 1.98.1 验证。基线源码为 `14cbb29616d7c0ababbf40a8f6c0218b996e7ae8`。第一次修改产品 Rust 之前，从该源码构建并冻结六个 Debug 二进制、`Cargo.lock`、Arial 字体散列与 face 0、27 份输入及旧版输出。数据使用独立工作簿、拼写样本和演示文件。精确散列及代际差分见机器可读记录。[验证记录](verification.json) · [计划及剩余人周](../../CRAFT-REPLACEMENT-PLAN.md)

| 试点 | 生产入口与替换 | 首批兼容性结果 |
| --- | --- | --- |
| JoyWord | `joyword-proof` 按词形建立地区索引，按 en-US、en-CA、en-GB、en-AU 查询建议、接受规则和词典对应形。词表数据沿用现有归属。 | 桌面与 CLI 共用校对逻辑。通过 CLI 的 `review.issues` 对 16 份 DOCX、2,324 个探针和 813 条问题精确对照。接受结果、UTF-8 区间、问题词和有序建议一致。 |
| JoyDeck | PPTX 打开入口中的加密识别按 CFB 目录和 FAT 扫描实现，复用 `joyoffice-cfb` 的有界扇区及名称解码。继续对无法读取目录的历史样本使用原有标记诊断。 | 9 份 PPTX／CFB 的退出码及诊断一致。覆盖 WPS 生成的密码保护包和普通 PPTX、512／4,096 字节目录扇区、普通 OLE、数据区碰巧含标记、截断及非 CFB 输入。 |
| JoySheet | 公式注册仍走 `CONVERT`。新实现包括大小写敏感的符号、十进制和二进制前缀、面积和体积幂次、绝对温标及错误传播。 | 428 个公式结果与旧版一致。旧写旧读、旧写新读、新写旧读、新写新读均为零差异。Windows 原生窗口修改 A1 后复核 2 个联动格，检查 5 个状态中的 17 格及保存后的 428 个类型缓存，撤销、重做、保存和新进程重开均通过。 |

JoyWord 的英美变体表继续保留 `craft-i18n` 来源。JoyDeck 保留旧探针的来源指纹，另记载本批适配与新文件散列。JoySheet 的历史单位符号、倍率和精确重算行为继续用于工作簿互读。[拼写数据](../../../../licenses/craft-i18n/SOURCE.json) · [Deck 来源映射](../../../../licenses/deckcraft/file-map.json) · [Sheet 历史输入](../../../joysheet/upstream-source.json)

Microsoft Excel 16.0.20625 实际全量重算后，416 个结果在相对误差 `2e-12` 内或类型和值精确一致。余下 12 个单元格涉及 `u`、`parsec`／`pc`、`mmHg` 和 `eV`／`ev` 的换算常数，在旧版和候选版中相同。为保持已保存工作簿的重算行为，本批保留这些结果。另一套系统 COM 自注册指向 WPS，单独保存其对照；其中 237 个结果与 JoySheet 不同，也已存在于基线。[Excel CONVERT](https://support.microsoft.com/en-us/excel/functions/convert-function) · [MS-CFB 目录](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-cfb/a94d7445-c4be-49cd-b6b9-2f4abc663817) · [MS-OFFCRYPTO 流](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-offcrypto/b60c8b35-2db2-4409-8710-59d88a793f83)

## Windows 桌面补验

- JoyWord 的两版审阅窗口都将 `colour` 标为问题，并按 `color, cooler, colder, colon, colons, colors` 显示建议。候选版更正、撤销、重做，确认保存到测试副本，独立取出 DOCX 正文，再用新进程重开均通过。冻结版原始 Debug 程序出现主线程栈溢出。用 MSVC `editbin` 给两版的独立测试副本设定相同的 32 MiB PE 栈预留后复验，原冻结二进制的散列保留。
- JoyDeck 两版从桌面启动参数打开普通 PPTX，启动日志均确认读取到一页。同样打开 WPS 加密包，两版启动日志均记录加密拒绝，窗口进程继续运行，源文件散列一致。PowerPoint 16 实际生成的样本已在下一批补验。[阶段 1 验收](../20261124-craft-stage1/README.md)
- JoySheet 的候选桌面程序与最终构建逐字节一致。另用两版 CLI 复跑条件聚合、矩阵及经典文本、现代文本及日历、工程及折旧这四批已有语料，共 8 份工作簿、604 个公式缓存和 32 种文件读写组合。

## 工程检查

三个 `cargo xtask check <产品> --headless` 均完成 fmt、严格 Clippy 和测试。JoyWord 单线程复跑 4,147 通过、2 项忽略，JoyDeck 1,016 通过，JoySheet 1,482 通过、1 项忽略。首轮并行检查中的 Word 百万字节单词分页耗时 16.96 秒，超过 15 秒上限，同一测试在单线程复跑通过。三个试点 crate 加共享 CFB 的定向检查共 714 项通过，与产品级检查有重叠。

来源检查通过 292 条指纹，工程门禁通过。锁文件仅新增 `joydeck-pptx` 到现有 `joyoffice-cfb` 的依赖边。用归档目录复跑三产品差分再次通过。

## 复跑

本机归档位于 `backups/craft-replacement-20261124/`，运行日志位于 `target/craft-replacement/`。`14cbb296/` 保存六个原始二进制及锁文件，`final/` 保存六个候选二进制，`baseline.json` 含原始问题和公式预期结果。`corpus/` 中的 `plain.pptx`、`office-encrypted.pptx` 和 `convert-excel.xlsx` 由注册的 WPS 自动化生成。`convert-excel16.xlsx` 由明确启动的 Microsoft Excel 16 补测。以 `prepare` 生成原始 DOCX 和公式工作簿，以 `freeze` 从旧版捕获结果。对现有归档，直接运行差分脚本并指定一个新的输出目录。[复跑脚本](../../../../scripts/check_craft_pilots.py)

```powershell
python scripts/check_craft_pilots.py compare backups/craft-replacement-20261124 --candidate backups/craft-replacement-20261124/final --output target/craft-pilots-recheck --excel-reference backups/craft-replacement-20261124/corpus/convert-excel16.xlsx
```

本批的自动和桌面验证结果仅适用于表中场景。复杂分页、动画及文件对象的逐调用归因和专项语料仍列在后续工作量中。
