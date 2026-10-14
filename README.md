# JoyOffice · JoyWord

JoyOffice 面向跨平台 Office 套件；当前集中开发 Rust + GPUI 文字处理软件 **JoyWord**。JoySheet、JoyDeck 为后续方向。项目采用 MIT 许可证。

**当前是规格基线、M0 工程样例与无界面的文档内核，还不是可编辑或保存 Word 文件的产品。** 窗口已接入真实只读文档会话、统一页面的 CPU 图像、语言切换及缩放；尚无编辑入口，新画布的 GUI 验收待执行。joyword-docx 已在 OPC 预算之上实现声明子集的只读导入与原始包保留（T103 首个切片）；joyword-layout 已有单字体、显式或 Unicode 默认自动断点的横向行拟合，以及显式单字体 profile 下的单节分页、版本快照和命中几何，统一场景可输出多页实验 PDF（T003/T105）。joyword-shell 的命令注册表已驱动窗口入口（T106）；无界面 DOCX 会话已支持现有段落／格式边界内的保守文字导出与往返校验（C02），超出范围明确拒绝，并可将已验证计划安全写为新路径的导出副本（C03）；尚无原件覆盖保存。完整 DOCX 编辑保存、Word 兼容中文规则与分页、IME、修订、批注及 AI 尚未接入；当前切片和人工验收边界见实施记录。

## 规格与任务

- [规格入口](docs/joyword/README.md) · [产品 Spec](docs/joyword/SPEC.md) · [实施 Tasks](docs/joyword/TASKS.md)
- [架构与 ADR](docs/joyword/ARCHITECTURE.md) · [兼容矩阵](docs/joyword/COMPATIBILITY.md) · [质量门槛](docs/joyword/QUALITY.md)
- [原生 Agent 设计](docs/joyword/AI.md) · [公开资料调研](docs/joyword/RESEARCH.md) · [独立实现与来源](docs/joyword/PROVENANCE.md)
- [当前实施与验证记录](docs/joyword/IMPLEMENTATION.md)
- [多 Agent 分工与交付](docs/coordination/README.md) · [W01/W03 集成审查](docs/coordination/reports/WAVE01-REVIEW.md)

## 运行技术预览

工具链固定 Rust 1.98.1，GPUI 固定 0.2.2；提交 Cargo.lock。三端 CI（macOS 含预编译 Metal 构建、Windows、Linux x86_64 与原生 ARM64）已实际运行并通过，另在 QEMU 下运行 ARM64／LoongArch64 无界面测试；真实机器上的人工运行另行验收。GPUI 系统依赖参考 [macOS](https://zed.dev/docs/development/macos)、[Linux](https://zed.dev/docs/development/linux)、[Windows](https://zed.dev/docs/development/windows) 构建说明；JoyWord 不依赖 Zed 编辑器。

Mac 开发环境缺少 Metal 编译工具时，可使用 GPUI 的运行时着色器选项：

```sh
cargo run -p joyword --features dev-runtime-shaders --locked
```

正文需要实际 CJK 字体。自动检查 macOS 宋体 face 6、Linux Noto CJK face 2、Windows 宋体 face 0；也可设置 `JOYWORD_FONT=字体文件绝对路径:face序号`。缺失或无法完整显示样例时返回错误，不使用同名字体替代。系统字体不随仓库分发。

需要本地 .app 包时，先构建，再生成开发包装：

```sh
cargo build -p joyword --features dev-runtime-shaders --locked
python3 scripts/package_macos_dev.py
open target/dev/JoyWord.app
```

此包装未签名、未公证，不是发行安装包。每次修改代码后需重新构建和包装。完整 Xcode／Metal 环境及其他平台使用 `cargo run -p joyword --locked`；正式构建还必须验证不启用 dev-runtime-shaders 的路径。

## 验证

无 GPU 核心与规格检查：

```sh
python3 scripts/check_docs.py
cargo fmt --all -- --check
cargo test --locked
```

依赖许可与安全公告（需要 [cargo-deny](https://github.com/EmbarkStudios/cargo-deny)，策略见 deny.toml）：

```sh
cargo deny --locked --all-features check
```

T003 统一字形链路（同一字体字节整形 → 页面场景 → PDF，再用独立阅读器读回；字体需自备，例如 Linux 的 Noto CJK 或 macOS 的宋体，不随仓库分发）：

```sh
cargo run -p joyword-pdf --example unified_scene -- /usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc 2 target/unified
python3 scripts/verify_unified_pdf.py target/unified
```

C05 文档快照 → 单节分页 → 同场景 PDF（显式预览度量，非 Word 兼容规则；独立读回需要 Poppler）：

```sh
cargo run -p joyword-pdf --example document_flow -- /usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc 2 target/document-flow
python3 scripts/verify_document_pdf.py target/document-flow
```

额外安装 pdfplumber 后，可加 `--glyph-baselines` 独立核对全部字符的绝对基线。

C06 无 GPU 页面光栅与同场景 PDF 对照（Poppler 的字体抗锯齿可能不同；按行墨迹位置与双向笔画覆盖核对）：

```sh
cargo run -p joyword-render --example cpu_scene -- /usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc 2 target/cpu-scene
pdftoppm -r 144 -png target/cpu-scene/reference.pdf target/cpu-scene/pdf
cargo run -p joyword-render --example compare_rasters -- target/cpu-scene/cpu-1.png target/cpu-scene/pdf-1.png
```

T100 文本存储基准：

```sh
cargo run --release -p joyword-model --example text_storage
```

Mac 开发路径静态检查：

```sh
cargo clippy --workspace --all-targets --features dev-runtime-shaders --locked -- -D warnings
```

默认 workspace 成员只含无界面的 crate（含基础库、模型、引擎、DOCX/OPC、排版/PDF/CPU 光栅、语料及外壳逻辑），使核心测试不依赖窗口和 GPU。显式使用 `-p joyword` 构建应用。语料种子与登记规则见 [tests/fixtures](tests/fixtures/README.md)；登记校验不代表格式兼容测试通过。

## 实施原则

从公开规范独立实现文档语义、读写、事务与分页；允许复用通用基础库。每阶段交付可运行版本，所有兼容声明附验收证据。AI 计划默认关闭并支持无 AI 构建；当前尚未引入 AI 模块。后续工作从 GPUI 来源决策（ADR-010）、T003 的字形／IME／统一 PDF 验证开始；文档模型（T100）与事务（T101）已有实现、待人工验收，再进入 M1 中文编辑与 DOCX 保存重开闭环。
