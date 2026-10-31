# JoyOffice 产品与模块边界

## 仓库与版本

主产品在同一个 Cargo workspace 内使用根 Cargo.lock、Rust 1.98.1、resolver 3 和工程门禁。`apps/joyword`、`apps/joyword-cli`、`apps/joydeck`、`apps/joydeck-cli` 是独立构建目标。应用 manifest 是各产品版本真值；CLI 跟随所属产品，内部库和其它产品不被强制一起升版。现有独立实验沿其受控工作区规则运行，影响其 path 依赖的变更须同步验证。

## 依赖方向

- `joyword-*` 和 `joydeck-*` 拥有各自产品语义；产品应用之间没有依赖。
- `joyoffice-*` 是格式或平台中立的公共实现，不能反向依赖产品模型、应用、GUI 或产品服务。
- Deck model 只含数据、序列化与不变量；engine 执行内存编辑；formats 在字节与模型间转换；scene 只读解析布局与场景；export 生成输出内容；platform 隔离文件、系统与 helper 进程；shell 管理会话、持久化和自动化。具体可用依赖由统一 LAYERS/DEV_LAYERS 检查。
- GPUI 仅进入桌面 UI。CLI 和核心测试必须独立构建，并检查其依赖闭包不包含 GPUI。
- 文件/进程副作用须有明确归属与逐文件登记。不能因拆包降低输入预算、未知部件保留或失败拒绝契约。

## 公共能力的抽取

已有 OPC、DrawingML、OMML、CFB 等格式中立模块优先复用。将 Deck 现有实现替换成 OPC 前，先比较预算、关系、未知成员保留与写回契约，再增加能验证行为的往返用例。没有第二个实际调用方的 UI/字体/几何设计不为共享先建空壳。Word 流式分页、Deck 固定画布、未来 Sheet 单元格模型分别维护。

## 构建、测试和交付

`cargo build -p <应用> --locked` 只构建选择的应用及其依赖。产品测试入口选择产品自己的所有库/入口与相关公共库，不把应用入口测试当成全产品测试。共享锁文件和 feature 配置修改，除分别测试外还要联合检查，防止 feature 合并引起行为差异。

发布标签使用 `joyword-v*`、`joydeck-v*`，`v*` 仅兼容历史 Word 流程；Sheet 实现后采用 `joysheet-v*`。产品包只包含所属二进制、helper、资源与实际依赖的许可/SBOM。主仓库锁文件代表共同依赖决议，不代表所有产品必须一起发布。发布前仍需各产品自己的质量与签名门槛，本次迁移不等于正式发布资格。
