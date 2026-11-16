# GPUI Kit 0.9.1 / GPUI Pre 0.3.12 升级

2026-11-16。用户授权统一升级三产品的公共桌面依赖及必要源码适配。起点为 `main` 的 `1c189c62f7eae382e4f095b49670090d1891280e`，任务分支为 `feat/gpui-kit-0.9.1`。开始时工作区干净，本地与远端 main 一致。依赖与源码适配、本机自动验证和三产品桌面冒烟已完成；全量测试仍有六项升级前已存在的失败，未标为全绿。详细命令、日志指纹、原生报告和截图见[验证记录](validation/20261116-gpui-kit-091/README.md)。

## 依赖与实现

| 层级 | 原版本 | 当前版本 |
| --- | --- | --- |
| GPUI Kit / Base / Component / Component Macros / Assets | 0.8.1 | 0.9.1 |
| GPUI Pre 核心及平台、宏、辅助快照 | 0.3.10 | 0.3.12 |
| 新快照引入的 bench-metrics | 无 | 0.3.12 |
| 独立版本的 gpui-pre-reqwest | 0.12.15 | 0.12.15 |
| AccessKit Windows 本地生命周期补丁 | 0.34.0 | 0.34.0，继续保留 |

根 manifest、Sheet 的 Base / automation readback 和 Word 的测试核心保持精确对齐。默认 Component / Assets 和 GPUI Pre 后端保持原有选择。没有启用 GPUI Fast；产品统一版本仍为 0.1.4。

锁文件仅替换 GPUI 版本线并增加其需要的包：新 ICU segmenter 固定为上游声明的 2.3.0 基线，保留原有 ICU 2.3 系列、tinystr、yoke 和 zerovec 版本；不随新增依赖改用 ICU 2.4。新增 bench-metrics 及其基准工具依赖按实际 feature 闭包区分，不能把锁文件中的所有包都计为普通发行运行依赖。旧 Metal 包随 Apple 快照依赖调整移出根锁。

Sheet 图表测量适配器随 `Window::text_system()` 改用 `Rc<WindowTextSystem>`。引擎测量回调继续通过独立 `Mutex<WindowTextSystem>` 保持现有 `Send + Sync` 契约；没有放宽线程约束或引入 unsafe。公共主题原先使用 `Theme::update` 修改字段，未依赖新版禁止的主题结构体字面量。Deck 的诊断版本同步更新。

依赖门禁检查整个 GPUI Pre 快照家族，包括非当前宿主平台和新辅助包，拒绝旧版本、重复组件和未审阅来源；独立的 reqwest fork 继续按既有精确来源规则检查。历史迁移、实验锁、旧发行包及其验收记录保留原义。

## 来源与许可审查

上游依据为 [v0.9.0 兼容说明](https://github.com/longbridge/gpui-kit/releases/tag/v0.9.0)、[v0.9.1 发布说明](https://github.com/longbridge/gpui-kit/releases/tag/v0.9.1) 和对应 crates.io 发布归档。

当前快照指向 Zed `d2a4fb6ded7c470be25d1541281da3882495b65b`。先运行未带偏差登记的逐文件检查，再审阅全部差异，更新 `scripts/gpui_pre_reviewed_deviations.json`、CI 固定提交及该登记文件的宿主适配指纹。没有放宽 provenance 校验。

- 22 个 GPUI Pre 包：17 个与上游逐文件相同，4 个快照包有 12 处经内容固定的发布适配，另一个 reqwest fork 与独立参照归档相同。
- `actions!` 使用 `$crate::Action`；宏入口重写到 Kit 门面，新增重写模块与前次实现相比仅改变快照注释。
- Apple 构建脚本从 `vendor/gpui` 读取生成 Metal 头文件需要的源码；五份副本与本次固定 Zed 源文件一致。
- WGPU 的两处 `DeviceDescriptor.default_queue` 删除及 `has_srgb_suffix` → `is_srgb` 是发布依赖 wgpu 30.0.1 的适配；本次 path alpha blending 与所固定上游一致。
- `gpui-pre-reqwest 0.12.15` 继续逐文件对照 `zed-reqwest 0.12.15-zed`。它是带 Zed 网络行为修改的 fork，不能称为未经修改的官方 reqwest。

版本化许可证目录更新到实际依赖版本，保留 Apache-2.0 及 Lucide ISC/MIT 正文。Lucide 资产仍为 1.43.0。AccessKit 补丁及原始来源清单不变。新增依赖的实际产品闭包、许可和公告检查按验证记录列出。

## 验证与边界

| 检查 | 本轮结果 |
| --- | --- |
| 三产品桌面及可选功能联合编译 | 通过，含 Word Agent、Deck validation、Sheet automation |
| 三产品 GUI + CLI 构建 | 通过；Word/Deck 的 dev-runtime-shaders 组合另行 check 通过 |
| 全仓 fmt、联合功能生产代码严格 Clippy | 通过；Deck/Sheet 产品检查另含 all-targets Clippy |
| Deck / Sheet 完整产品检查 | 988 / 1,439 项通过；Sheet 一项既有计时测试忽略 |
| Word 完整产品检查 | 1,933 项通过后遇到一项引擎导航失败；后续模块由联合测试继续覆盖 |
| 联合 workspace 测试，不因失败提前停止 | 6,057 通过、6 失败、3 既有忽略；三个 UI crate 的 69 项全部通过 |
| 升级前 main 对照 | 同一提交的隔离工作树复现全部六项失败；原 Kit 0.8.1 / Pre 0.3.10 |
| Python 回归 | 233 项，含两项平台跳过；新增混合快照、重复组件及未审来源反例 |
| 来源、版本/无界面隔离、工程/文档门禁 | 通过；22 个 Pre 包、291 条导入指纹；三产品 CLI 闭包无 GPUI |
| cargo-deny 0.20.2 | advisories / bans / licenses / sources 通过，无新增忽略；既有 async-executor 1.14.0 yanked 警告保留 |
| UOS X11 实际窗口 | Word 5、Deck 5、Sheet 13 组通过；系统合成键盘与生产 GPUI 合成事件分别记录 |

六项失败分别为 Word 引擎的连续视图 Page Up，以及 layout 的符号项目符号行高、Word 2013 两端对齐、浮动图片重排、旧表单字段绘制及长脚注分页断言。升级前隔离工作树使用原锁文件复跑，失败名称及断言一致；没有修改这些模块、断言或忽略列表。它们作为现有基线问题继续开放，不归因为本次升级，也不隐去失败。

桌面检查覆盖中文/空格路径、实际查找输入框、图表、撤销重做、格式表单、明暗主题及 DOCX/PPTX/XLSX 保存重开。Word 保存临时副本时触发现有元数据保护确认，显式确认后独立 OOXML 读回通过；原测试语料未变。Sheet 的 Linux 截图来自 X11 窗口像素，未冒充 GPUI GPU readback。原始日志位于本机 `target/gpui-kit-091/`，提交的验证记录保存摘要、复跑脚本和选定证据。

宿主为 UOS Desktop 25 Professional x86_64、X11；当前终端位于玲珑 Deepin 25 运行环境。使用已安装的宿主 Git / GCC / CMake 和本机临时 sysroot 包装恢复工具路径，包装不作为仓库构建依赖。Rust 为 1.98.1。

本机窗口验证为 125% 显示缩放下的限定场景，不代表完整 DPI 矩阵或物理呈现延迟。Windows/macOS 原生、系统中文输入法、真实读屏、多显示器、物理打印、三平台安装包及远端 CI 本轮未验证；AccessKit Windows 补丁只保留并核对来源，没有冒充本轮 Windows UIA 实测。信创真机按有条件时补验，不阻塞本次开发。尚未推送或发布；历史整阶段验收状态保持原义。
