# 验收清单

2026-10-20。本目录把 [TASKS](../TASKS.md) 里 T103、T200–T205、T300–T301 的自动验证、人工验收、产物和错误路径拆成原子准则，并指向 `main` `a875adc` 上真实存在的测试（2026-10-22 波次新增的行除外，见下）。任务状态仍以 TASKS 为准。这些任务都还是 `in_progress`：自动测试和人工验收没有同时齐备，本目录不把任何一条标成任务完成。

2026-10-20 验证波次收口：S2、S5、S1、S3 合入 main `f0ccc659` 之后，各清单补了无界面排练、页面场景金样和特性样例的行。这些行指向 [verification](../verification/README.md) 下的四份证据。验证文档报告的已知缺口在对应行里写成 open，等分支 fix/verify-wave-gaps 的修复 PR 合并后再改。

2026-10-20 缺口修复：该分支已作为 [#67](https://github.com/willmove/joyoffice/pull/67) 合入 main `caf84147`，对应行改为已修复，见 [验证波次缺口修复](../verification/verify-wave-fixes.md)。T202 的正文页码只限无格式域，T203 的 wrapTight 只是方形近似，两条限制写在行里。T203 图片关系另报 not-imported 已由 [#69](https://github.com/willmove/joyoffice/pull/69) 修复。任务状态不变；`scripts/check_acceptance.py` 现在在 CI 作业 dependencies 里运行。

2026-10-22 六分支并行波次：新增 [T301](T301.md) 清单（正文段落的 `w:pPrChange`／`w:rPrChange`，表格结构修订明确不实现），[T200](T200.md) 补编号部件追加式写回与级别制表位写回的行，[T202](T202.md) 把栏分隔线从 not-implemented 改为 partial、把 `nextColumn` 写进基础分节行，并更正一处过时的测试名引用。这一波新增的行最初指向集成分支 `chore/integration-wave-20261022` 上的测试；2026-10-23 核验时，六个实现 PR 与 #86 门禁加固已全部进入 `main` `85a24de`，其主干 CI 九项全绿。`check_acceptance.py` 仍校验当前工作树里的测试名和文件，收尾记录另作复验。全部是自动证据，没有任务改为 `done`，也没有任何一条人工验收被关闭。

`scripts/check_acceptance.py` 读取本目录的 `T*.md`。状态必须是下表之一；证据列里用反引号标出的、含下划线的 Rust 标识必须是 `crates/` 或 `apps/` 里的 `fn` 名。本地链接必须指向本分支已有的文件。每份清单必须原样引用该任务的自动验证和人工验收句子，并带「GUI 抽查脚本」。

## 状态

| 标记 | 含义 |
| --- | --- |
| automated | 命名测试覆盖该原子准则，并在 CI 作业 build 或 unified-pdf 中运行 |
| partial | 有测试或已记录的会话，但只覆盖准则的一部分，或只用合成样例 |
| not-started | 没有实现，也没有测试 |
| needs-gui | 需要图形界面上的人工操作；本环境没有做 |
| needs-word-wps | 需要固定 Word 或 WPS 对照 |
| needs-hardware | 需要打印机、信创真机或其他本环境没有的硬件 |
| not-implemented | 代码里没有这条行为 |

## 索引

| 任务 | 清单 |
| --- | --- |
| T103 DOCX 最小语义与保留读写 | [T103](T103.md) |
| T200 样式、编号与编辑便利 | [T200](T200.md) |
| T201 基础表格编辑 | [T201](T201.md) |
| T202 页面与页眉页脚 | [T202](T202.md) |
| T203 图片与富文本剪贴板 | [T203](T203.md) |
| T204 统一 PDF 与平台打印 | [T204](T204.md) |
| T205 功能区、标尺与状态栏 | [T205](T205.md) |
| T300 文字与移动修订 | [T300](T300.md) |
| T301 格式与表格结构修订 | [T301](T301.md) |

功能怎么用、代码从哪里进入，见 [功能指南](../guide/README.md)。
