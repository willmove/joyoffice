# JoyOffice v0.1.2 Windows 验证

2026-11-09，Windows x64，基线 `d65fcdc4`。窗口使用仓库语料的副本、独立配置与恢复目录。原始语料和现有用户配置分开。

## P0

| 任务 | 本机验证 | 剩余验收 |
| --- | --- | --- |
| JW-806 数据迁移与回退 | 带表格的旧日志在桌面提示并打开恢复副本。最新一代已丢弃的日志未再提示。使用旧配置中的 `ui.locale=en` 和 `autosave.minutes=1`，界面实际显示英文，编辑写入快照。强制结束测试进程后，从「Recover latest」恢复并经原生保存框另存。CLI 读回未保存的标记恰好一次，原件与原始样本的 SHA-256 一致。 | 更广的真实 Office 对象语料和正式安装后的偏好／日志回退。 |
| JW-808 功能与协议 | 与 `00fb0687` 的 Windows 旧 CLI 比较 14 份常规和 5 份字段 DOCX，schema 1 报告一致。3 组 V3 字段快照原样另存，正文失配被拒绝。脚注、公式、OLE、图表和 SmartArt 合成语料经两次保存和旧 CLI 读回，差异限于诊断计数。原生打印预览显示一页中文样本，系统打印框列出 Epson 及 Microsoft Print to PDF。 | 系统打印取消回传、队列提交和纸张输出。打印助手仍在等待时，窗口控制只定位到 JoyWord，取消点击未使助手退出。远程界面操作、真实 Provider HTTP/TLS、系统凭据及桌面接线仍开放。 |
| JW-808 原生 PDF/A | 从「文件」确认归档选项，在 Windows 原生保存框写出 PDF/A-2b。veraPDF 1.30.2 通过 144 条规则、1,578 次检查。pypdf 6.21.2 严格读回一页，中文正文和 `English summary` 存在。 | 已验证本样本的原生归档导出。 |
| JW-812 Windows 便携换版 | 用实际发布的 0.1.1 ZIP 与本机 `cargo xtask package joyword --release` 产出的 0.1.2 ZIP 验证。坏包拒绝，候选目录消失后复原，切换后认证控制和 MCP 读回成功，V3 副本与预期快照逐字节一致，主动回退后旧桌面启动且旧 CLI 读回原件及副本。5 个原始输入和 Windows DOCX 默认关联保持原值。 | 当前 Windows 分发是未签名便携 ZIP。正式安装器、注册关联、卸载及签名仍开放。 |

归档 PDF 的 SHA-256 为 `a5bd71fc50992907f42d37618f673743ff99723cfbc84771a7f85b7284d1f052`。崩溃恢复副本为 `1a1f8ad8264f34eae1ef6d29c14791df54d74e865103466647647df6a23cea9a`，原件为 `a767d121341c12a26721ded803f4401f77ff54bc411383328ccc5fd1db91cd2d`。

另核对 `tests/fixtures/reference/word2024` 的 12 份参考文档。11 份由当前 CLI 另存后，旧 CLI 的正文投影与各自原件一致。旧 CLI 仍可读入全部 12 份原件。带只读保护的 `interop/wordresaved/DOCX-MIN-005.docx` 拒绝另存且没有生成输出。两代 CLI 在该组中的报告差异涉及诊断计数、保留片段计数及兼容标志。64 次畸变输入回归中 21 次可解析、43 次干净拒绝。

在启用 `joyword/agent` 的 Windows 构建上，本地提案默认关闭。测试副本选中文字后，主动启用本地提案，填写替换内容。审阅框出现时正文保持原样，接受后只替换选中文字，单步撤销恢复原文。关闭提案后审阅入口恢复禁用。

已发布 0.1.1 ZIP 的 SHA-256 为 `69f9e4e7b9a388e84ec0feee61b36a6807f3e57ccba99141f848b8e17c42dbb8`。本机 0.1.2 Release ZIP 为 `0c6f9d62032d400ec8be28ffdae465aa5bf96d17c60399dce961f2b93533b28b`。旧发布 CLI 只接受 `inspect -- FILE`，回退脚本改用两代都支持的入口，且核对恢复文档正文与旧 CLI 在换版前读取的预期快照一致。Windows `Get-AuthenticodeSignature` 返回 `NotSigned`。

## 统一版本

`cargo metadata --offline --locked --no-deps --format-version 1` 确认 60 个第一方工作区包均为 `0.1.2`。按名称、版本、来源和校验和比较锁文件，868 项外部依赖记录保持原值。产品 GUI 与 CLI 继承同一版本。三个产品标签各自路由到 `0.1.2`，保留历史标签解析。16 项产品工程脚本测试通过，覆盖工作区版本继承及旧的显式版本解析。

Windows x64 使用 `joydeck/validation,joysheet/automation,joyword/agent` 跑全仓锁定测试，5,964 项通过，3 项既有忽略。以相同功能组合运行生产库和二进制严格 Clippy，通过。仓库格式、文档、边界和导入来源门禁通过。
