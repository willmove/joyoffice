# JoyOffice · JoyWord

JoyOffice 面向跨平台 Office 套件；当前集中开发 Rust + GPUI 文字处理软件 **JoyWord**。JoySheet、JoyDeck 为后续方向。项目采用 MIT 许可证。

**当前是规格基线、M0 工程样例与无界面的文档模型和编辑引擎，还不是可编辑或保存 Word 文件的产品。** 窗口展示原创中英文文字、A4 物理页面投影、语言切换及缩放；joyword-model 提供类型化文档结构、结构共享快照和不变量校验（T100），joyword-engine 提供原子编辑事务、位置映射与撤销（T101），二者尚未接入窗口；joyoffice-opc 以资源预算读取不可信的 OPC 包（T103 起步）；joyword-shell 的命令注册表已驱动窗口的菜单、快捷键与按钮（T106）。尚未实现 DOCX、排版、修订、批注、PDF 或 AI。

## 规格与任务

- [规格入口](docs/joyword/README.md) · [产品 Spec](docs/joyword/SPEC.md) · [实施 Tasks](docs/joyword/TASKS.md)
- [架构与 ADR](docs/joyword/ARCHITECTURE.md) · [兼容矩阵](docs/joyword/COMPATIBILITY.md) · [质量门槛](docs/joyword/QUALITY.md)
- [原生 Agent 设计](docs/joyword/AI.md) · [公开资料调研](docs/joyword/RESEARCH.md) · [独立实现与来源](docs/joyword/PROVENANCE.md)
- [当前实施与验证记录](docs/joyword/IMPLEMENTATION.md)

## 运行 M0 样例

工具链固定 Rust 1.98.1，GPUI 固定 0.2.2；提交 Cargo.lock。三端 CI（macOS 含预编译 Metal 构建、Windows、Linux x86_64 与原生 ARM64）已实际运行并通过，另在 QEMU 下运行 ARM64／LoongArch64 无界面测试；真实机器上的人工运行另行验收。GPUI 系统依赖参考 [macOS](https://zed.dev/docs/development/macos)、[Linux](https://zed.dev/docs/development/linux)、[Windows](https://zed.dev/docs/development/windows) 构建说明；JoyWord 不依赖 Zed 编辑器。

Mac 开发环境缺少 Metal 编译工具时，可使用 GPUI 的运行时着色器选项：

```sh
cargo run -p joyword --features dev-runtime-shaders --locked
```

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

T100 文本存储基准：

```sh
cargo run --release -p joyword-model --example text_storage
```

Mac 开发路径静态检查：

```sh
cargo clippy --workspace --all-targets --features dev-runtime-shaders --locked -- -D warnings
```

默认 workspace 成员只含无界面的 crate（基础库、模型、引擎、OPC、外壳逻辑），使核心测试不依赖窗口和 GPU。显式使用 `-p joyword` 构建应用。语料种子与登记规则见 [tests/fixtures](tests/fixtures/README.md)；登记校验不代表格式兼容测试通过。

## 实施原则

从公开规范独立实现文档语义、读写、事务与分页；允许复用通用基础库。每阶段交付可运行版本，所有兼容声明附验收证据。AI 计划默认关闭并支持无 AI 构建；当前尚未引入 AI 模块。后续工作从 GPUI 来源决策（ADR-010）、T003 的字形／IME／统一 PDF 验证开始；文档模型（T100）与事务（T101）已有实现、待人工验收，再进入 M1 中文编辑与 DOCX 保存重开闭环。
