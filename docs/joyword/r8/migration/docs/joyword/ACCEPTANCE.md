# JoyWord 构建和验收

每个阶段需要可复现的构建和实际运行证据。当前已提供 R1 Windows 外壳的工程入口。Debug 构建与检查已执行，Release 和其他桌面平台尚未验证。当前检查见 [STATUS.md](STATUS.md)。

## 产品工程入口

R1 的 JW-103 复用宿主工具并提供：

```powershell
cargo xtask check joyword --headless
cargo xtask check joyword
cargo xtask build joyword
cargo xtask run joyword
cargo xtask package joyword --release
```

同时保留直接定位问题的命令：

```powershell
cargo check --locked -p joyword
cargo build --locked -p joyword
cargo run --locked -p joyword
cargo test --locked -p joyword-model -p joyword-geom -p joyword-metafile -p joyword-shell
cargo fmt --all -- --check
```

R2 起增加与改动相关的字体、格式、render/pdf、shell、CLI 测试。Rust 修改按宿主要求运行适当的 Clippy；不能仅执行应用入口的零项测试当成内核通过。headless 检查保证不依赖 GPUI 或 GPU。

Windows 使用 PowerShell 7 的 `pwsh.exe`。工程工具处理所需 CMake/SDK/编译器环境，路径不写死当前机器；缺少工具返回明确错误。macOS/Linux 使用等价产品命令。

## 阶段退出标准

| 阶段 | 最小桌面演示 | 必要自动检查 |
| --- | --- | --- |
| R0 | 复现来源应用的已知主线，保存构建事实 | 固定来源/依赖/数据，命令并集和冲突试验 |
| R1 | 从开发包启动窗口，切换功能区、调整尺寸并退出 | 三端编译、品牌组件、窗口和焦点冒烟 |
| R2 | 打开真实 DOCX，滚动/缩放/翻页并导出 PDF | 核心闭包、场景一致性、文字/结构读回、宿主依赖演练 |
| R3 | 中文组合输入、选择、撤销、保存、关闭重开和恢复 | typing、恶意参数、IME、竞态、安全保存和恢复故障回归 |
| R4 | 编辑带样式、编号、表格、图片和页眉页脚的报告 | 两来源对应命令/格式语料、DOCX/PDF 独立读回 |
| R5 | 引用、审阅、合并、多窗口、格式和自动化主线 | 功能并集逐项对照，协议、授权、只读和保护测试 |
| R6 | 同机长文档输入、滚动和缩放前后比较 | 固定负载、呈现延迟、帧时间、RSS/显存与图形保真 |
| R7 | 三端原生窗口、IME、读屏和平台流程 | 三端构建、打包、签名条件及已具备环境的设备检查 |
| R8 | 从宿主包运行新 Word，同时操作 Deck/Sheet | 三产品与联合 feature 回归、迁移、回退和旧功能核对 |

普通阶段优先保证 Windows 有可实际测试的包，并从 R1 开始三端编译检查。某平台尚无真机时，允许继续其他切片并提供对应构建产物，但该平台原生验收仍保持未验证；不能因此宣称 R7 的三端原生验收已通过。遵循宿主的信创例外：相关真机补验不阻塞开发、阶段依赖或发布，构建与自动测试仍需执行。

## 每个开发包的证据

包目录 `dist/joyword/Rn/` 至少包含可执行文件及必需资源、版本/源码指纹和文件 SHA-256。独立的 `evidence/Rn/` 保存构建命令/退出码/工具链/target、自动测试摘要、实际包启动记录、操作步骤、截图、输入/输出样例散列和已知限制。

验收必须从最终包启动一次，在仓库外含中文和空格的目录进行；不能只看 cargo run。截图必须来自真实应用，静态示意图不能代替编辑、保存或性能验证。

## 数据和行为不变量

- UI、快捷键、CLI、MCP 都经同一命令内核；拒绝操作不改变文档、撤销历史或已保存状态。
- 布局只读；同一文档/字体/兼容配置下，屏幕和 PDF 共用分页及定位字形。
- IME 预编辑和取消不污染正式撤销；一次提交不重复输入；每个文本 API 对位置单位有明确转换。
- 后台结果带会话与版本身份；取消、重复、过期和关闭后的结果不能重新安装文档或覆盖新页面。
- 导入/图像/字体/ZIP/XML/命令参数均有预算与边界检查；生产代码无 panic。
- 未知 XML/关系/对象按两来源契约保留或显式只读/拒绝，不静默丢弃；未实现命令不能返回空壳成功。
- 保存失败、外部修改、目录更换、锁、磁盘满、取消或迟到结果不损坏原件，不误清除更新的 dirty。
- AI 关闭/无 AI 构建不影响普通编辑；关闭后旧提案或流式结果不能提交。

每个新命令至少覆盖成功、拒绝、撤销及对应持久化路径。迁移 UI 的小样式调整不额外写镜像实现的测试，保留实际交互与结果验证。

## 性能预算

沿用宿主 QUALITY 中的初始目标，作为新 JoyWord 的待验证目标，不能当作现有测量结果：

| 指标 | 待验证目标 |
| --- | --- |
| 冷启动至空白文档可输入 | p95 ≤ 1.5 s |
| 100 页常规输入至实际呈现 | p95 ≤ 50 ms |
| 60 Hz 普通滚动帧时间 | p95 ≤ 16.7 ms |
| 100 页首屏可交互 | p95 ≤ 2 s |
| 1000 页首屏可交互 | p95 ≤ 5 s，全文处理可取消 |
| 空白/100 页文档稳定 RSS | 分别 ≤ 150/350 MiB，另列峰值和 GPU/共享内存 |
| 单架构安装体积 | ≤ 150 MiB，另列可选字体/语言/模型资源 |
| 空闲且 AI 关闭 | 无持续重排、页面上传或 Provider 请求 |

同一设备、同一电源状态、同一字体字节、同一语料、相同窗口/DPI 和 Release 配置比较来源 egui 与新 GPUI。至少 30 次启动样本，输入/滚动至少 1000 事件，保留 p50/p95/p99/最大值。首先在 R2 建立真实基线，再按瓶颈决定原生绘制和缓存投入。

输入到命令返回、输入到布局完成、页面光栅完成都不能代替“输入到实际呈现”。首屏可交互、全文导入、全文分页分别计时。高 DPI 页面清晰度、字体身份、显示质量和内存同时核对。

## 宿主整合检查

在 R2/R3/R4 的隔离演练中尽早核对宿主依赖和 Word 构建。R8 采用实施时最新宿主规则，至少覆盖：

```powershell
cargo xtask check joyword
cargo xtask check joydeck
cargo xtask check joysheet
cargo clippy --workspace --locked --all-targets --features joydeck/validation,joysheet/automation -- -D warnings
cargo test --workspace --locked --features joydeck/validation,joysheet/automation
cargo test --locked -p joyword-shell --features ai --test ai_toggle
python scripts/check_docs.py
python scripts/check_gates.py
```

依赖和来源检查复用宿主 `cargo deny --locked --all-features check`、GPUI provenance 及各产品脚本。测试名/feature 随新实现需要调整时，要保留同等行为验证并记录映射，不能删除失败项目来获得通过。

构建资格、自动测试、人工原生验收、正式签名/分发资格各自记录。开发包可供测试，不自动等于正式可发布包。

## 旧功能替换和回退

JW-002 的功能矩阵包含来源、命令/参数、UI 入口、结果、自动验证、人工验证、目标阶段及差距任务。完整替换验收按两来源的必保留项核对；没有接入的功能不能因为“上游 engine 有代码”就标可用。

整合前备份旧 Word 源码和可运行包；偏好、恢复与文件关联有独立迁移版本。DOCX 原件和旧恢复日志保留。至少演练一次“升级失败 → 启动旧包 → 找回原文件/恢复数据”，并区分代码回退和数据格式回退。

