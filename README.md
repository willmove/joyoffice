# JoyOffice · JoyWord

JoyOffice 面向跨平台 Office 套件；当前集中开发 Rust + GPUI 文字处理软件 **JoyWord**。JoySheet、JoyDeck 为后续方向。项目采用 MIT 许可证。

**当前是 M0／M1 技术预览，不是 1.0 产品，M1 发布门槛也尚未通过。** M1 十二项里七项已经 `done`，门槛任务 T107 仍是 `planned`：前置 T109（打开与保存的并发安全）和 T110（原件保存的元数据与确认路径）还没有同时具备自动证据和所要求的人工验收。M2 十二项里九项 `in_progress`、两项 `done`（T210、T211），门槛 T206 仍 `planned`；样式、编号、表格、页面、图片、PDF 与中文排版已有大量自动测试，所在任务仍因人工验收未关闭而保持进行中。M3 只有 T300（文字与移动修订）`in_progress`。M4 四项均未开始。2026-10-21 合入的 [#71](https://github.com/willmove/joyoffice/pull/71) 把 DOCX 健壮性实验改为绑定检出的 `crates/` 树 id，[#70](https://github.com/willmove/joyoffice/pull/70) 把银河麒麟修复会话的机器侧证据落进仓库。真实 DOCX 会话、中文文本编辑、基础格式、文本剪贴板、另存与重开、原件保存的元数据观测与显式确认路径、Windows 原件原子替换、统一页面场景的 PDF／CPU 光栅与无头输出、可见页缓存、打开／保存并发控制和恢复日志已有实现及相应自动测试。逐项复选框、阶段统计和完成证据见 [TASKS 进度总览](docs/joyword/TASKS.md#进度总览)。

当前支持边界与剩余门槛：

- DOCX 仅支持已声明子集；原始包、未知部件与无法安全映射的内容按契约保留或拒绝，超出支持边界的编辑不作假成功。
- 中文编辑—格式—剪贴板—另存—重开已有自动闭环测试；Mac GUI／IME 与固定 Word／WPS 读回已有记录，Windows 原生 GUI 原件保存确认、WPS 参考 PDF 和信创真机仍待补验。信创真机不阻塞开发、阶段验收或发布，但仍明确标为未验证。
- M2 的样式、编号、表格、页面、图片、查找替换、PDF 与打印到文件、功能区已有进行中的自动覆盖，任务状态仍是 `in_progress`。M3 的文字与移动修订（T300）进行中。M4 及以后尚未开始。批注、比较合并、AI 和 1.0 门槛仍属后续任务。富文本剪贴板只覆盖已声明的安全子集。

详细实现与验证记录见 [IMPLEMENTATION](docs/joyword/IMPLEMENTATION.md)。

## 规格与任务

- [规格入口](docs/joyword/README.md) · [产品 Spec](docs/joyword/SPEC.md) · [实施 Tasks](docs/joyword/TASKS.md)
- [架构与 ADR](docs/joyword/ARCHITECTURE.md) · [兼容矩阵](docs/joyword/COMPATIBILITY.md) · [质量门槛](docs/joyword/QUALITY.md)
- [原生 Agent 设计](docs/joyword/AI.md) · [公开资料调研](docs/joyword/RESEARCH.md) · [独立实现与来源](docs/joyword/PROVENANCE.md)
- [当前实施与验证记录](docs/joyword/IMPLEMENTATION.md)
- [验收清单](docs/joyword/acceptance/README.md) · [功能指南](docs/joyword/guide/README.md)
- [开发约定](AGENTS.md) · [历史协作归档（已停用）](docs/coordination/README.md)

## 运行技术预览

工具链固定 Rust 1.98.1，GPUI 固定 crates.io gpui-pre 0.3.9（Zed 主干 93d7456 的快照，CI 逐文件核对其来源，见 ADR-010）；提交 Cargo.lock。三端 CI（macOS 含预编译 Metal 构建、Windows、Linux x86_64 与原生 ARM64）已实际运行并通过，另在 QEMU 下运行 ARM64／LoongArch64 无界面测试；真实机器上的人工运行另行验收。GPUI 系统依赖参考 [macOS](https://zed.dev/docs/development/macos)、[Linux](https://zed.dev/docs/development/linux)、[Windows](https://zed.dev/docs/development/windows) 构建说明；JoyWord 不依赖 Zed 编辑器。

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

Linux 虚拟机若没有可用的 Vulkan 硬件，JoyWord 会在没有 `/dev/dri/renderD*` 时自动寻找 SwiftShader／lavapipe ICD，或在实际窗口打开失败后带软件 ICD 重启一次；显式设置的 `VK_ICD_FILENAMES`／`VK_DRIVER_FILES` 优先，也可用 `JOYWORD_SOFTWARE_VULKAN_ICD` 指定 JSON。找不到软件 ICD 时仍返回原来的窗口诊断，不假称成功。

## 无头检查与页面输出

`joyword-cli` 提供只读 `inspect`／`text`，以及统一 PageScene 的 `render` PNG 和 `convert --to pdf`。Windows 示例使用本机宋体；输出目标已存在时拒绝覆盖，重复验证请换用新路径：

```sh
cargo run -p joyword-cli --locked -- inspect tests/fixtures/docx/DOCX-SAMPLE-001.docx
cargo run -p joyword-cli --locked -- render tests/fixtures/docx/DOCX-SAMPLE-001.docx --font C:/Windows/Fonts/simsun.ttc:0 --output target/cli-png --page 1
cargo run -p joyword-cli --locked -- convert tests/fixtures/docx/DOCX-SAMPLE-001.docx --to pdf --font C:/Windows/Fonts/simsun.ttc:0 --output target/cli-output.pdf
```

应用内 Ctrl／Cmd+S 会先列出原件的元数据风险，默认选择另存副本；只有显式确认后才替换原件。保存期间继续输入的内容仍标为未保存。

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

从公开规范独立实现文档语义、读写、事务与分页；允许复用通用基础库。每阶段交付可运行版本，所有兼容声明附验收证据。AI 计划默认关闭并支持无 AI 构建；当前尚未引入 AI 模块。M2 的样式、编号、表格、图片等已经在进行并有自动测试。仍须关闭 M0／M1 剩余验收，尤其是 T107 所依赖的 T109／T110，以及固定 Word／WPS 对照和恢复与原件保存的人工场景。M4 及以后尚未开始。信创真机留待有条件时补验，不阻塞上述推进与发布。逐项依据见 [TASKS](docs/joyword/TASKS.md) 与 [IMPLEMENTATION](docs/joyword/IMPLEMENTATION.md)。
