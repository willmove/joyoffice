# JoyOffice 产品与模块边界

## 仓库与版本

主产品在同一个 Cargo workspace 内使用根 Cargo.lock、Rust 1.98.1、resolver 3 和工程门禁。`apps/joyword`、`apps/joyword-cli`、`apps/joydeck`、`apps/joydeck-cli`、`apps/joysheet`、`apps/joysheet-cli` 是独立构建目标。第一方组件和应用清单默认继承根工作区的统一版本，GUI 和 CLI 随各自产品打包。现有独立实验沿其受控工作区规则运行，影响其 path 依赖的变更须同步验证。

## 依赖方向

- `joyword-*`、`joydeck-*` 和 `joysheet-*` 拥有各自产品语义；产品应用之间没有依赖。
- `joyoffice-*` 是格式或平台中立的公共实现，不能反向依赖产品模型、应用、GUI 或产品服务。
- Deck R9 使用 `joydeck-model/engine` 的单一可编辑模型；`format/pptx` 在字节与模型间转换；`geom/color/text/anim/metafile/render/pdf` 是纯计算/呈现；`fonts` 的系统字体发现是逐文件登记的只读边界；`shell` 管理文稿、存储、平台 helper、输出与控制；`ui-gpui` 负责 GPUI 窗口；`legacy` 是旧 v1 格式的私有只读 DTO/OOXML 转换桥。旧 `formats/scene/export/platform` crate 已移除。具体依赖由统一 LAYERS/DEV_LAYERS 检查。
- GPUI 仅进入桌面 UI。CLI 和核心测试必须独立构建，并检查其依赖闭包不包含 GPUI。
- 文件/进程副作用须有明确归属与逐文件登记。不能因拆包降低输入预算、未知部件保留或失败拒绝契约。

## 公共能力的抽取

三产品的阶段顺序、首轮任务、验收数值与回退条件见[重构与自研替换路线图](REFACTOR-ROADMAP.md)。

界面主题由无 GUI、无文件访问的 `joyoffice-theme` 定义；Word shell、Deck shell、Sheet platform 在各自已有偏好存储中保存其稳定 ID。`joyoffice-ui` 依赖该目录并把 Palette 投影到 GPUI Kit 和 Base tokens，三产品 UI 共用预览和主题应用函数。该层不修改文档模型，CLI 依赖闭包仍不引入 GPUI。[主题目录与兼容规则](THEMES.md)

Word 和 Deck 共用 WMF／EMF 的有界读取与系统字体枚举。三产品都使用公共 OPC 读取，产品决定部件名、对象预算、未知 XML 保留及写回。Word 和 Sheet 共用 CFB 扇区／目录读取与 OMML 上下标，三产品都消费 DrawingML。Word／Deck 复用自定义几何公式和椭圆弧，Deck／Sheet 复用颜色处理。[公共库与各桌面、CLI 的调用关系](REFACTOR-ROADMAP.md#已在生产入口使用的公共代码)

Word 的 `joyword-proof` 与 `joyword-proof-rules` 现均为第一方工作区组件：`joyword-proof-rules` 从原文 UTF-8 范围产生标点、空白和句首大小写建议，零第三方依赖；`joyword-proof` 负责多语拼写、词缀形态、建议排序、断词与语言包读取，语言包存储与事务安装归属 `joyword-engine/src/packs/`，桌面与 CLI 不再依赖外部 `craft-libs` crate。桌面审阅、CLI 及 DOCX 修正仍通过同一编辑会话。Deck 和 Sheet 使用各自的文本编辑入口。[规则实现](../../crates/joyword-proof-rules/src/lib.rs) · [语言适配](../../crates/joyword-proof/src/editorial.rs)

公共 UI 持有呈现组件，产品适配层传入命令、焦点、可访问名称和保存状态。Word 的流式分页、Deck 的固定画布与 Sheet 的单写者计算引擎分别维护。每次抽取列出至少两个实际调用产品、各自的正常入口和可独立恢复的旧路径。共享组件的调用矩阵、首批候选与验收条件见[桌面应用共享复用](REFACTOR-ROADMAP.md#桌面应用共享复用)。

## 构建、测试和交付

`cargo build -p <应用> --locked` 只构建选择的应用及其依赖。产品测试入口选择产品自己的所有库/入口与相关公共库，不把应用入口测试当成全产品测试。共享锁文件和 feature 配置修改，除分别测试外还要联合检查，防止 feature 合并引起行为差异。

发布标签使用 `joyword-v*`、`joydeck-v*`，`v*` 仅兼容历史 Word 流程；Sheet 采用 `joysheet-v*`。产品包只包含所属二进制、helper、资源与实际依赖的许可/SBOM。主仓库锁文件代表共同依赖决议，不代表所有产品必须一起发布。发布前仍需各产品自己的质量与签名门槛，本次迁移不等于正式发布资格。


## Sheet 适配边界

Sheet 的 Session 由单引擎 worker 持有；UI 使用不可变快照和有界队列。公式、计算、模型、图表、XLSX 和 PDF 不依赖 GUI/平台对话框。既有引擎文件适配与拼写字典职责逐文件登记；本轮保留其宿主文件 API 和沙箱语义。`joysheet-ui-gpui` 和 `joysheet-platform` 为显式 UI/平台适配器，headless 产品选择排除二者。专用 `joysheet-tools` 支持 smoke/清单/基准，根工程工具保持无产品运行时依赖。来源、范围和验收见[Sheet 整合](JOYSHEET-INTEGRATION.md)。
