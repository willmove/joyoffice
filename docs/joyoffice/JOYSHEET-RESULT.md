# JoySheet 整合结果

2026-10-31。任务分支 `codex/integrate-joysheet` 从 `d47f036` 开始，执行用户批准的整合方案；交付通过非草稿 PR，由用户审核合并。原 JoySheet 目录和原主仓库工作区保留，未在本任务中切换主工作区或发布正式版本。

## 实现

- 两个应用、15 个产品库及一个 CPU 工程工具接入根 workspace；产品包名为 `joysheet-*`，应用版本保持 0.1.0。
- 共用锁文件、工具链和根门禁，保留产品独立构建、功能开关、版本和 `joysheet-v*` 发布路由。CLI/核心没有 GPUI 或其他产品运行时依赖。
- Windows 便携/安装器/源码包、macOS app/DMG、Linux AppDir/deb 与四平台 CI 路由已实现；普通版与 automation 验证版分别构建。
- 原版权、双许可证、来源清单、测试和历史证据保留。开发及运行使用新目录，不依赖原独立源码目录或本机备份。
- OPC/DrawingML、字体和其他跨产品服务的进一步抽取留待后续任务。

## 来源及恢复

[来源复核](validation/joysheet-source-audit-20261031.json)确认 703 个源码文件的路径集合及哈希未改变，首次导入提交 `ffc076df151d54f67b896a44a8b1d1f4935146a7` 保留原始字节；`7fedd54becb684dfeed8ba5e21274f15fe240c31` 单独处理包名更名。原独立仓库没有提交历史，因此没有构造不存在的 Git 历史。

原源码 ZIP、30,610 个历史产物/日志及完整索引保存在任务检出的 `backups/joysheet-migration-20261031/`，归档大小与 SHA256 已记录；原目录 `D:\Coding\EditorProjects\joysheet` 保留。[路径映射](validation/joysheet-migration-map.json)记录最终文件去向和测试拆分。归档不作为日常构建输入。

## 本机自动验证

| 范围 | 结果 |
| --- | --- |
| 原独立产品基线 | 1,390 通过、1 忽略；fmt、严格 Clippy 通过 |
| workspace 默认配置 | 2,788 通过、1 忽略；全部 target 严格 Clippy 通过 |
| workspace 联合验证功能 | 2,788 通过、1 忽略；Sheet automation + Deck validation，全部 target 严格 Clippy 通过 |
| 解压独立源码包 | 仓库外中文/空格路径下执行产品 check；fmt、严格 Clippy、1,395 测试通过、1 忽略 |
| XLSX 独立 crate | 268 通过；包含名称编码、保留、预算、加密和往返回归 |
| 工程脚本 | 根脚本 145、产品路由 13、Sheet 分发 4、Deck 工程 11 项通过 |
| 根门禁/依赖 | 分层、文件副作用、生产 panic、文档检查及 cargo-deny advisories/bans/licenses/sources 通过 |
| 可移植核心/工具 | WASM check、355 条命令清单、M8 引擎往返、稀疏几何基准通过 |
| 新旧文件对照 | 17 条命令持久化 JSON 一致；两代 CLI 交叉读取两代 XLSX 的值/公式通过 |
| 最终解压 Windows 验证包 | 52 项串联通过；含 GPU 读回、实际 UIA 和自身打印取消边界 |
| 普通 Windows 包 | 实际启动、创建/编辑工作簿通过；automation 截图默认关闭；版本资源为 0.1.0 |
| 隔离安装/卸载 | 安装后原生运行通过；卸载保留额外用户文件；TEST 模式不改快捷方式/注册表 |

原始命令输出留存于本机备份目录，运行摘要与日志哈希见[验证清单](validation/joysheet-tests-20261031.json)；历史报告与新运行分开记录。原版本和最终版本的 [Windows 串联](validation/joysheet-native-20261031.json)、[文件对照](validation/joysheet-core-parity-20261031.json)、[安装/卸载](validation/joysheet-installer-20261031.json)及[安装后运行](validation/joysheet-installed-native-20261031.json)单列。

Sheet 开发 profile 保持原 opt-level=1；不放宽递归预算。zip 接入根 9.0.0，避免原 3.0.0 的三项安全公告；没有新增公告忽略。名称按元数据编码标志解码，避免 UTF-8 猜测合并旧 CP437 名称。根原有注册表版本及校验和全部保留，见[分发复核](validation/joysheet-packages-20261031.json)。

## 性能对照

本轮使用旧 M8-preview6 与最终解压验证包，顺序运行同类脚本，采样期间没有并发 Rust 构建。六组 10k/100k 自定义尺寸、密集公式和动态数组，各 12 次编辑；真实单元格结果、公式求值数量和样本完整性逐次断言。另运行 CSV/聚合及复杂公式场景。

| 场景/规模 | 原版 CPU 总响应 P95 | 新包 CPU 总响应 P95 | 原版/新包引擎 P95 |
| --- | ---: | ---: | ---: |
| 自定义尺寸 10,000 | 18.92 ms | 19.19 ms | 0.14 / 0.14 ms |
| 密集公式 10,000 | 28.69 ms | 27.65 ms | 12.43 / 10.20 ms |
| 动态数组 10,000 | 20.74 ms | 18.94 ms | 2.08 / 2.63 ms |
| 自定义尺寸 100,000 | 24.31 ms | 22.71 ms | 0.13 / 0.09 ms |
| 密集公式 100,000 | 135.72 ms | 130.93 ms | 117.97 / 114.69 ms |
| 动态数组 100,000 | 35.19 ms | 36.45 ms | 17.52 / 15.20 ms |

本轮全部结果和求值数量断言通过，计时存在小幅波动：100k 动态数组 CPU 总响应 P95 增加约 1.26 ms；10k 动态数组引擎 P95 为 2.08→2.63 ms，总响应则为 20.74→18.94 ms。没有隐去波动或声称逐项提速。这些是单机少样本 CPU 提交/首帧数据，不代表 GPU 呈现、物理输入延迟或长期上界。[对照汇总](validation/joysheet-performance-comparison-20261031.json)、[旧原始压力样本](validation/joysheet-stress-old-20261031.json)、[新原始压力样本](validation/joysheet-stress-new-20261031.json)及 CSV/复杂公式原始报告均保留。

## 分发与 CI

本机产物位于 `dist/joysheet/`，含普通版和验证版的便携 ZIP、NSIS 安装器与源码 ZIP。[分发复核](validation/joysheet-packages-20261031.json)记录完整文件名、SHA256、二进制、857 项源码输入及载荷校验。两个源码 ZIP 的 859 个文件内容一致。

普通版实际依赖 431 项、验证版 451 项，均附许可正文，未包含 Word/Deck 产品代码。源码包独立验证通过；普通/验证特性分别写入构建信息。包不含正式发行者签名，本轮没有创建 Git 标签、正式 release 或更新 feed。

新增 CI 覆盖四平台产品构建/分发、headless/WASM、独立源码解析及 workspace 默认/联合特性。实际远端结果以 PR 检查页为准；合并前须同步最新 main、CI 全绿且可合并，由用户完成合并。

## 保留的验收项

本机实测平台为 Windows x64。macOS/Linux 的实际 GUI、正式签名/公证、真实系统 IME、多显示器/系统 DPI、读屏语音、物理打印设备与 GPU 呈现性能仍未验证。CI 构建或合成输入不代替这些验收；信创真机按仓库约定后补，不阻塞工程整合。本次不将原 M8 或产品功能矩阵整体改为完成。
