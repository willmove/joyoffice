# JoyWord 依赖对齐方案

依赖以本次读取的 JoyOffice manifest、Cargo.lock、工具链、本地 patch 和实际 feature 图为基线。GPUI 与完整核心闭包已按宿主 pins 落地并通过 Windows 检查；依赖解析演练通过，三产品编译及跨平台原生验收仍待后续执行。来源散列见 [baseline.json](baseline.json)。

## GPUI 和工程基线

| 项目 | JoyOffice 当前基线 | 迁移要求 |
| --- | --- | --- |
| 工具链 | rust-toolchain.toml channel 1.98.1 | 精确一致，使用 pwsh.exe 执行 Windows 工程命令 |
| Rust 元数据 | edition 2024、rust-version 1.98、resolver 3 | 保留这三个不同层面的设置，不误写成 edition 2021 |
| GPUI Kit | gpui-kit =0.8.1，registry | 统一经 Kit 使用应用 API |
| GPUI | gpui-pre =0.3.10，Kit 传递依赖 | 不额外加入另一条 gpui Git 或 0.2.x 依赖 |
| 平台包 | gpui-pre-platform =0.3.10，font-kit/wayland/x11 | 对齐宿主，保留需要的平台 feature 转发 |
| 配套组件 | gpui-base、gpui-component、gpui-kit-assets 锁定为 0.8.1 | 使用宿主锁定结果，避免在独立项目浮动到另一补丁版本 |
| 可访问性 | accesskit_windows 0.34.0，本地 path patch | 复用 third-party/accesskit_windows 的修复与许可 |
| 公共 UI | joyoffice-ui path | 采用宿主唯一视觉实现；临时快照有指纹 |
| GPUI 来源 | Zed GPUI snapshot 08cbb88a152c3751a43e853425a70e9f631a5918 | 沿用来源核验脚本和已审偏差，不搬运 Zed 编辑器实现 |
| lint | unsafe_code forbid、相关 Clippy、Wordcraft no-panic | 新生产 crate 保留；引入旧代码时修复可诊断错误路径 |

本次读取的 gpui-kit 锁文件 checksum 为 `25210f6e59ea9ffac3a033dbce9edafe5ff0dd2900f894fd1316f54da2fe2b89`，gpui-pre 为 `df4b5f8d0c278308df2f11e0bfe0212dd77d7848ce1bdc4b9d513334414396f6`。完整字段保存在来源快照，不手写猜测远端版本。

默认使用宿主当前 GPUI 后端，不启用 gpui-fast。以后若需要升级，先在宿主统一评估，不能在 JoyWord 私下形成另一套组件栈。GPUI Kit 的上游说明确认其作为 GPUI、基础层、组件和资源的统一入口；精确版本以本地锁文件为准。[GPUI Kit 官方仓库](https://github.com/longbridge/gpui-kit)

## 实际 shader feature 差异

宿主应用 Cargo.toml 写有可选 `dev-runtime-shaders`，但本次执行：

```powershell
cargo tree --locked --offline -p joyword -e features -i gpui-pre-platform
```

普通构建已显示 `gpui-pre-platform/runtime_shaders ← gpui-kit 0.8.1`。本地缓存的 Kit 0.8.1 manifest 也给桌面平台依赖启用了此 feature。

因此，对齐时要复制宿主实际 feature 行为，并将其列入 JW-003/JW-710 的 shader 验证；不能仅凭应用默认 features 为空，声称发布包已经排除了运行时 shader。不同平台实际执行哪条 shader 路径仍需分别检查。本次不修改宿主，也不擅自升级 Kit 来规避这一差异。

## 通用库与核心冲突

| 依赖 | Wordcraft 当前使用或锁定情况 | JoyOffice 基线 | 处理方向 |
| --- | --- | --- | --- |
| serde / serde_json | 1.x；serde_json 锁定 1.0.154，启用 float_roundtrip | =1.0.228 derive/rc；=1.0.154 | 对齐精确版本，检查 feature 合并对 JSON 键序和数值的影响 |
| chrono | 0.4.45 | =0.4.45，default-features=false，std | 按实际模块显式启用 clock，日期规则保留 |
| thiserror / log | 2 / 0.4 | =2.0.21 / =0.4.34 | 使用宿主 pins |
| image | 锁定 0.25.10，多种图像格式 | =0.25.9，默认 features 关闭 | 优先适配宿主版本，显式列出需要的解码器和预算 |
| kurbo / vello_cpu | 0.13.1 / 0.2.0 | =0.13.1 / =0.2.0 | 版本可对齐，仍比较 feature 与线程设置 |
| HarfRust | 主依赖 0.13，锁定 0.13.3 | 文档栈 =0.8.4 | 在 JW-004 试验 API/整形差异，迁移必要能力而非静默降功能 |
| Skrifa / read-fonts | 主 Skrifa 0.47.0；锁中多条 read-fonts | 文档栈 0.42.1 / 0.39.2 | 与字体、PDF 一起适配，不把两个不同版本的 FontRef 当同类型 |
| PDF | craft-pdf 固定 Git，krilla 0.8.2 来自同一 Git patch | krilla =0.8.2，registry | 同版本号但来源不同；优先对宿主后端适配并验证失败写入与 Unicode |
| ZIP | Wordcraft DOCX 用 zip 3.0.0，锁还含 9.1.0 | root zip =9.0.0；OPC 有自己的 ZIP 层 | 容器优先接宿主 OPC，保留读取预算和 round-trip |
| base64 / sha2 | 主依赖分别 0.23 / 0.11 | root 分别 =0.22.1 / =0.10.9 | 优先按宿主 API 适配；宿主锁中已有 sha2 0.11.1 不代表可随意改根 pin |
| quick-xml | 主依赖 0.41，锁还有 0.42 | =0.41.0 | 共享边界对齐 0.41.0，核对原始 XML 保留 |
| rfd / arboard | egui 平台层使用 | =0.17.2 / =3.7.0 | 仅迁入确需的桌面服务，普通文件对话框优先复用现 JoyWord |
| egui / eframe / craft-ui / craft-window egui / craft-update egui | 原界面和更新代码使用 | 新 Word 不需要 | 不进入新生产桌面图，非 egui 的通用服务另作适配 |

表中“文档栈”指宿主 Word/Deck 对正文的直接依赖，GPUI 自带 UI 文字依赖可有其他版本。目标是统一公共 API 和共享直接依赖，并理解已有合法传递多版本；不通过强制压平整个 Cargo.lock 破坏 GPUI 或其他产品。

R0 的试验必须证明上述适配可以保留已承诺能力；未解决时保留具体失败和任务，不能把“版本号已改一致”当作完成。禁止直接把 Wordcraft 的整个 patch 区块放到宿主根：krilla patch 影响所有使用 registry krilla 的产品，egui/winit forks 也不属于新桌面所需。

若 stock krilla 缺少 Wordcraft fork 的必要行为，先研究窄适配或固定来源的最小隔离后端；不得静默替换宿主公共后端。任何最终共享版本/来源调整都作为有完整三产品回归的单独变更处理。

## 外部源码和数据输入

| 输入 | 本次源码所固定的来源 | 迁移要求 |
| --- | --- | --- |
| craft-proof | craft-libs rev 47c9f62530db72bc5c3831c86653b47b20c640be | 保留精确 rev、许可和校对测试；缺数据时能力明示 |
| craft-font-store / craft-font-catalog | rev 762a06a4c6b48b522b7d4de84a87917ef90265e6 | 分离字体/语言服务与 UI；不得带入 egui 功能 |
| craft-pdf | rev 92d392cff132eaf53e13afff77f38b5c11b73aa0 | 优先完成宿主后端适配；如仍引入则检查完整递归来源 |
| craft-i18n 数据 | craft-i18n.rev 内容 059cbf95c2a3eb95d637ca8584faca1129d17f48 | 重接 CRAFT_I18N_DIR、语言数据与翻译构建输入 |
| craft-fonts | 字体 build.rs 使用 CRAFT_FONTS_DIR 等输入 | 冻结使用包的版本和清单；默认可使用系统字体但不得提交系统字体文件 |
| 现 JoyOffice 第三方和字体语料 | 宿主 licenses、fixtures 和 provenance 记录 | 逐项保留原归属；包内包含所需的许可全文 |

Wordcraft 多个 craft-libs 包来自不同提交，不能仅写“依赖 craft-libs main”。默认/离线开发可以使用已知回退，但发布声称支持的语言和字体需要相应固定数据包与验证。

## 每阶段对齐检查

检查 manifest、锁定版本/source/checksum、patch 文件指纹和实际 feature 图。分别覆盖单产品、headless、无 AI、AI、验证构建及整合时的全 workspace 组合。尤其检查 serde_json/preserve_order、图像格式、crypto 和字体 feature 合并。

实施时先核对宿主 HEAD。宿主发生变化时生成依赖差异报告并更新基线，按回归结果同步；不静默跟随远端 main，不为本次 Word 发布无条件提高整个 workspace 版本。


## R2 已实施的适配（2026-11-04）

- 保持 GPUI Kit 0.8.1 / gpui-pre 0.3.10、AccessKit patch、Skrifa 0.42.1、HarfRust 0.8.4、registry Krilla 0.8.2、image 0.25.9、zip 9.0.0、quick-xml 0.41.0。字体目录 Fontique 固定为宿主 0.7.0，用有界注册适配替代新版接口。
- 采用宿主 OPC 原样快照做 DOCX 容器预检；Wordcraft docx 保留语义和未知 XML。ZIP 9 名称读取的 Result 显式传播，保留损坏/加密格式回归。SHA1 0.11 和 SHA2 0.10 使用各自兼容的 HMAC 0.13/0.12；没有自写密码算法。
- PDF 在 joyword-pdf 内部窄适配 craft-pdf write.rs，使用宿主 stock Krilla，单 crate 显式启用 raster-images。移除未使用的 fork print/PDF-X/可选内容接口；页面绘制失败丢弃不完整 writer，并保留 unbalanced Surface unwind 注入回归。当前不声明打印、PDF-X 或 fork 全部能力。
- GPUI 合并 serde_json/preserve_order 后，格式修订属性列表显式按名称排序，保持内核描述稳定。
- 固定 craft-libs 三个来源 rev 的非 UI 子集，固定 craft-i18n.rev 的 26 个语言/归属输入。proof build 默认使用仓库内语言数据，外部 CRAFT_I18N_DIR 仍可覆盖。vendoring 脚本只写全新 staging，核对 Git blob 和 SHA-256，不覆盖适配代码。语言包的在线下载能力未验收。
- 13 个来源 OFL 字体内嵌；CJK 在本机使用安装字体，未复制系统字体。没有安装 CJK 字体的平台仍需补字体包和原生验证。
- 临时宿主工作区解析 63 个成员、924 个包，Word/Deck/Sheet 共用宿主 UI/OPC/patch；此项是 metadata 解析，不能代替三产品编译。旧 Word 的 agent/corpus/rtf 等调用仍需 R8 映射。

日志、许可和实际边界见 [R2 验证](../../evidence/R2/README.md)。

## R3 增量

保持原 GPUI、字体、PDF、ZIP 和通用库 pins。新增保存依赖完全采用宿主版本：winsafe =0.0.19（kernel）、rustix =1.1.5（std/fs）、xattr =1.6.1（Unix）。winsafe 的安全接口承载宿主 ReplaceFileW/稳定文件身份；产品源码仍 forbid unsafe。

只在 UI 的 dev-dependencies 为同一 registry gpui-pre =0.3.10 开启 test-support，用于实际 EntityInputHandler 集成测试。Kit 聚合 test-support 会解析其可选 Fast 依赖的 proptest 1.12，与宿主 1.11 pin 冲突，因此没有开启该聚合开关，也未修改宿主 proptest。正常桌面仍只通过 Kit 接入同一 GPUI，没有 egui/Fast。测试 feature 与发布 feature 分开记录。

R3 在隔离宿主副本中完成 Word 和 CLI 的 cargo check；63 成员、933 包的三产品解析通过。Deck/Sheet 本轮未重编译，宿主目录保持只读。详见 R3 证据目录。

## R4 增量

字体选择复用同版 Kit 的 SearchableVec/Select，字号、颜色及行距复用 Popover/Button，格式表单复用 InputState/WindowExt Dialog；未新增外部库或升级任何版本。joyword-shell 增加对工作区已有 joyword-fonts 的直接依赖，以查询内核实际字体目录；不使用另一套文档字体数据库。公共 joyoffice-ui、OPC、AccessKit patch 和运行时 features 保持基线。新增复用来源登记在 licenses/joyoffice/SOURCE.json。

R4 样式/表格/图片增量未增加外部依赖、修改版本或 Cargo.lock；复用现有 Kit uniform_list/Input/Dialog/Popover 及 geom 图片探测解码。新包再次通过依赖脚本，宿主副本 63 成员/933 包解析及 Word/CLI cargo check；Deck/Sheet 未重编译，最终替换另行实施。
