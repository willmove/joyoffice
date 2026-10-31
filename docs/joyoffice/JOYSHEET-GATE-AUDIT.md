# JoySheet 接入门禁审阅

2026-10-31。本次沿用原有产品实现并按确切文件登记副作用；模型、公式、计算、图表、XLSX 和 PDF 内存实现没有文件系统豁免。根工程工具不依赖产品运行时。

## 生产错误与测试

原核心已有禁止 unsafe、unwrap/expect/panic/todo/unimplemented 的 Clippy 约束。内联测试体原样移入对应 `tests/` 文件，模块名和测试语义保留，具体去向见[测试迁移清单](validation/joysheet-test-module-migration.json)。测试夹具加密 helper 继续由原 `cfg(any(test, feature = "test-support"))` 保护。普通应用构建不启用 test-support。

公式解析器的自定义 `expect(Tok)` 更名为 `require(Tok)`，继续返回 ParseError，避免与会 panic 的标准 expect 混淆。工程示例中的失败路径改为返回 Result。生产 panic 基线没有批量追加导入代码。

zip 9 的包名解码错误向上传递为 ZIP 错误，超大成员的原始名称仅作诊断显示；名称失败不能静默隐藏成员。新增非法 UTF-8 名称拒绝、CP437/UTF-8 标志差异与两个旧名称不合并的回归。CP437 依照公开映射独立实现，XLSX 读取与 CLI 共用同一解码函数。ODS 实体匹配显式使用字节切片，避免新依赖带入的 AsRef 实现改变类型推断。

## 文件及进程职责

| 文件 | 已审阅职责与边界 |
| --- | --- |
| apps/joysheet-cli/src/main.rs | 用户显式指定的脚本、工作簿和参数文件；维持原命令及保存错误传播 |
| apps/joysheet-cli/src/check.rs | 用户指定语料目录的枚举和检查；保持原遍历预算 |
| crates/joysheet-app/src/control.rs | 仅显式启用的 loopback 服务和用户凭据；8 连接、64 UI 请求及原请求长度预算 |
| crates/joysheet-app/src/recovery.rs | 专用容量 1 队列；256 MiB 恢复记录预算、修订标识、关闭标记和自身临时文件 |
| crates/joysheet-app/src/smoke.rs | 显式工程验证输出目录，普通会话不会自行运行 |
| crates/joysheet-engine/src/io.rs | 原生文件适配；脚本沙箱、仅普通文件、实际读取 2 GiB 上限、保存失败及临时文件所有权 |
| crates/joysheet-engine/src/cmd/spelling.rs | 原有可选本地拼写字典；维持缺字典回退行为 |
| crates/joysheet-mcp/src/control_auth.rs | 原兼容凭据目录；Unix 私有权限、独占创建及凭据错误 |
| crates/joysheet-platform/src/lib.rs | 产品专属配置、用户文件选择与剪贴板；不改变已有配置标识 |
| crates/joysheet-platform/src/printing.rs | 自身随机打印目录、预览页与累计内存预算、拥有的 helper 进程；不删除用户目录 |
| crates/joysheet-protocol/src/lib.rs | 读取指定端口的本地凭据，维持 toolkit-free 协议 |
| crates/joysheet-ui-gpui/src/control.rs | automation 下等待匹配帧后输出用户指定截图；沿用并发预算 |
| crates/joysheet-ui-gpui/src/printing.rs | 清理平台适配器创建的打印作业目录；修订/代次校验和晚到结果失效 |
| tools/joysheet/src/main.rs | 显式命令清单、引擎 smoke 和几何基准；输出进入 Sheet 目录 |

这些登记不会把工程迁移视为上述功能的新增人工验收。原实现的产品限制与未覆盖项继续保留在 Sheet 状态和逐任务矩阵中。
