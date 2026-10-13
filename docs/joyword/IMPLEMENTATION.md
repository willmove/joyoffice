# JoyWord 当前实施与验证记录

记录日期：2026-10-13。这是源码工作区的实施状态，不能替代 SPEC 的目标或宣称所有路线图已经完成。文档基线为 v0.2，工程版本为 0.0.1。

## 已落地的内容

- 八份专题规格、目录入口、本记录；需求与任务数量以 check_docs.py 输出为准（v0.2：82 个需求、58 个任务），每项有输入、输出、依赖、错误路径和验收要求。
- Rust workspace：joyword-foundation 物理单位与屏幕投影；joyword GPUI 工程窗口。
- 精确 Rust 工具链、GPUI 包版本、Cargo.lock；三端 CI 配置。
- 中文／英文独立资源；静态 A4 页面、语言切换和 50%–150% 缩放。
- 原创 UTF-8 种子、散列清单，以及需求覆盖／依赖／本地链接／语料／语言资源检查器。
- 未签名 macOS 开发包装脚本。没有修改系统工具链或安装 Xcode。

尚无文档模型、编辑事务、文件格式、排版内核、IME 编辑、PDF、修订、批注、剪贴板或 AI 实现。工程页面由 GPUI 直接显示样例文字，不是未来统一排版场景，也不构成 Word 保真证明。

## 当前环境

| 项目 | 实际情况 |
| --- | --- |
| 本机 | Apple M4，24 GiB，macOS 27.0.1，arm64 |
| Rust／Cargo | 1.98.1／1.98.1；工具链文件精确固定 |
| GPUI | crates.io 0.2.2（最后官方版本，已停更），长期来源待 ADR-010 选定；锁文件约 708 个外部包全部经由 GPUI 引入，含 MPL-2.0、Unicode-3.0 等需登记的许可 |
| Xcode／Metal | 只有 Command Line Tools；`xcrun --find metal` 失败 |
| 开发构建 | dev-runtime-shaders 使用 GPUI 官方运行时着色器选项，已编译成功 |
| 对照软件 | 发现已安装 WPS、Pages；本次未做 Office 运行或往返测试 |
| Windows／Linux | 工作流已配置；未在真实 runner 或机器上执行 |

不以未测量的最新 GPUI 网站能力反推 0.2.2 已满足所有要求。当前直接依赖只有 GPUI 和本地基础库；HarfRust、Fontique、Skrifa、ICU4X、Ropey、krilla 仍是选型，尚未接入自研内核。

## 验证记录

| 项目 | 结果与范围 |
| --- | --- |
| 开发构建 | `cargo build -p joyword --features dev-runtime-shaders --locked` 成功 |
| 真实窗口 | 验证中；系统辅助功能／屏幕录制授权尚未可用，不能确认界面交互 |
| 文档／语料／资源检查 | 2026-10-13 `python3 scripts/check_docs.py` 通过：82 个需求、58 个任务，阶段一致，M0–M7 均在 T704 前置闭包内 |
| 基础库测试 | `cargo test -p joyword-foundation --locked` 通过 3 项 |
| rustfmt／clippy | `cargo fmt --all -- --check` 通过；`cargo clippy -p joyword-foundation --all-targets --locked -- -D warnings` 无警告；含 GPUI 的 workspace clippy 尚未执行 |
| 标准 Metal 构建 | 环境前置不满足；不能记录通过 |
| 三端 CI、Office 往返、性能 | 未执行，不能计为通过 |

构建出现上游 block 0.1.6 与 proc-macro-error2 2.0.1 的 future-incompat 提示；目前不阻断固定工具链的开发构建，但依赖升级前需处理。依赖图含 GPUI 间接带入的 rustybuzz；独立排版的 HarfRust 选型不代表已经消除全部传递依赖。

## 未完成的 M0 门槛与下一步

1. T001：依据 T003、T005 的实测按 ADR-010 选定 GPUI 来源（须支持 GL 或 CPU 渲染）；安装完整 Xcode 并单独下载 Metal 工具链后验证普通构建；加入 cargo-deny、按 SHA 固定 Actions，实际执行 Windows／Linux／macOS CI 并保留运行证据。
2. T002：实际检查中英文窗口、切换、缩放和退出；不把编译或进程存在算成交互通过。
3. T003：整形—屏幕—PDF 使用同一字体实例与定位字形，验证中文 IME、组合取消／提交／撤销及 DPI。GPUI 0.2.2 的 GlyphId 不可在外部构造（上游主干已公开），按字节加载字体也不返回 FontId；外部整形结果接入取决于 ADR-010 的来源选择或最小补丁，禁止用系统同名字体替代来宣称同一字形。
4. T004：在种子登记上实现规范文件生成、语义／几何比较器和固定参考环境（Word 为 Office LTSC 2024，并对照 WPS）；当前种子全部为 planned。
5. T005：准备统信 UOS 与银河麒麟（x86_64、ARM64、LoongArch64）测试机或镜像，核对 LoongArch ABI，验证 GL／CPU 渲染、Fcitx 中文输入和系统字体。
6. T100 只依赖 T000，可与 M0 并行；T101–T107 按依赖推进，在 M1 门槛 T107 通过前不提供真实用户文档的编辑保存承诺。

M0 整体尚未通过，M1–M9 未实施。后续完成任务时须更新本记录、TASKS 状态与对应测试报告。
