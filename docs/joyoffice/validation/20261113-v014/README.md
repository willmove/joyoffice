# v0.1.4 三产品发布验证

日期：2026-11-13，Windows x86_64，Rust 1.98.1。本轮从 main `73771d2e48c97896916c2d0d4c96be3b6ae2b665` 开始，用户授权本机验证通过后发布 JoyWord、JoyDeck 和 JoySheet。

三产品及 61 个第一方组件统一升级到 0.1.4，首轮版本升级未改变外部依赖。下述本机首验后，远端依赖门禁发现 rustix 公告，发布前补丁与复验另记于下节。JoyWord 多倍行距分页回归改用既有固定字体测试工具及足够长的正文，核对确切 Source Sans 3 字节并保留末行文本、下一行文本及下边距断言，避免系统默认字体改变测试输入。首轮旧测试复现失败，修复后单项与完整检查均通过；未修改生产分页逻辑。

## 自动检查

| 命令 | 结果 |
| --- | --- |
| `cargo xtask check joyword` | fmt、严格 Clippy、3,618 项通过，0 失败，2 项既有忽略 |
| `cargo xtask check joydeck` | 来源/依赖/文档门禁、fmt、严格 Clippy、982 项通过，0 失败 |
| `cargo xtask check joysheet` | fmt、严格 Clippy、1,420 项通过，0 失败，1 项既有忽略 |
| `python -m unittest discover -s scripts/tests` | 187 项运行，7 项环境相关跳过，无失败 |
| `python -m unittest discover -s tools/xtask` | 16 项通过 |
| `python scripts/check_gates.py` | 1,519 个 Rust 文件通过 |
| `python scripts/check_docs.py` | 文档、历史任务、样例哈希和工作流 action pins 通过 |
| 三产品 `cargo xtask package PRODUCT --release` | 普通生产 feature、Release 桌面与 CLI 构建及打包通过；Deck 使用 `--release-layout` |
| 单平台发布资产验证 | 三份 ZIP 的 sidecar、内容、版本、PE 架构、BUILD-INFO/SBOM 通过；Sheet Setup EXE 头与 SHA-256 通过 |

[检查与包哈希](checks.json)记录最终日志 SHA-256、本地归档、安装器和截图摘要。完整日志保留在本机 `target/release-v014-checks/`。初轮 Word 构建期间增加测试 import，打包器正确拒绝变化中的源码；源文件稳定后重新打包并通过哈希一致性检查。

## 本机窗口验证

使用 Computer Use 的 sky、Windows SendInput 和真实桌面截图，未将合成输入称为物理键盘认证。普通构建先做功能区/编辑冒烟，再从 Release ZIP 解压到带中文和空格的独立目录启动桌面和 CLI。

- **Word**：混合中英文、图片与表格文档显示；双击开始标签收起；中英文输入及保存副本。Release 包重开后再次输入 `PortableRelease014`，按现有 ACL 确认流程保存临时副本，包内 CLI 读回标记。明暗主题切换不改变正文颜色。包内 CLI 另通过混合 DOCX 编辑/保存/OOXML/PNG/PDF、旧 merge/exchange/PDF-A 接口、EPUB 和页面设置回归；其中 PDF 结构未启用独立 PDF 库检查。证据：[重开](gui/word-release-reopened.png)、[保存](gui/word-release-saved.png)、[暗色](gui/word-release-dark.png)。
- **Deck**：20 页 PPTX 编辑模式下键 1→2；F5 放映、下键翻页、清晰中文文字和 Escape 退出。Release 包再验同一放映流程；输入中英文备注并另存 `.joydeck`，包内 CLI 独立读回 20 页及备注。证据：[原生保存](gui/deck-release-native-saved.png)、[放映第一页](gui/deck-release-show-start.png)、[第二页](gui/deck-release-show-down.png)。
- **Sheet**：销售仪表盘表格、图表和格式正常显示，双击开始标签收起后仍可输入公式。普通构建保存 `SUM(10,20,30)`，独立 XLSX 读回缓存 60。Release 包从中文路径重开，再修改为 `SUM(1,2,3)` 并保存，独立 XML 读回公式与缓存 6。证据：[重开](gui/sheet-release-reopened.png)、[保存](gui/sheet-release-saved.png)。
- **Sheet 安装器**：NSIS 3.11 ZIP 按仓库固定 SHA-256 校验后使用；`/S /TEST` 隔离安装到本机 target，安装的桌面 EXE 与包内哈希一致，安装目录的 CLI 可重开工作簿。卸载后产品 EXE 移除，额外 sentinel 文件保留。此模式不修改注册表或快捷方式，未将其计入完整系统集成验收。

## 已知边界

没有可复用备注部件的导入 PPTX，新增备注后保留写回会明确拒绝。本轮核对拒绝后源文件 SHA-256 不变，另存原生格式并读回备注成功；该行为已写入 Deck v0.1.4 发布说明。未将其声称为任意 PPTX 备注新增写回。Deck SBOM 仍报告 `mac@0.1.1`、`seahash@4.1.0` 两项精确正文缺口，未给出合规验收结论。

其它系统和信创真机、物理键盘输入延迟、系统 IME 候选、真实读屏、打印设备和物理多屏/4K 不在本轮本机验收范围。CI 原生构建或 QEMU 不替代对应人工验收。预览包沿用未签名/未公证状态，产品既有功能差距见各自 STATUS 和交接。

三个产品发布均由当前 main 的六平台工作流执行；远端 CI、公开 Release 与资产摘要以 GitHub 实时结果为准。


## 发布前安全补丁

首轮 main `87ed9891e4a85b5c87432a920521a892157cc521` 的依赖作业因 [RUSTSEC-2026-0486](https://rustsec.org/advisories/RUSTSEC-2026-0486.html) 失败，尚未创建产品发布标签。按[上游修复](https://github.com/bytecodealliance/rustix/commit/8e6aaa6713d2bb64b1fdd8c472e3a32a10a76d70)将根工作区及 Deck Linux 直接 pin 从 1.1.5 更新为精确 1.1.6；锁文件只更新 rustix 及配套的 xattr 两个包，未添加公告忽略或升级无关依赖。修复零长 xattr 缓冲区被错误标记初始化的问题。兼容性核对发现 xattr 1.6.1 的超过 4 KiB 读取回退仍依赖旧查询语义，因此配套固定到 [xattr 1.7.0 源码](https://github.com/Stebalien/xattr/blob/c0d3de0d4229e496584c4beb5882cbb1739f0632/src/sys/linux_macos.rs)的新 `*_query` 调用。新增 macOS 8 KiB 属性观察与恢复回归，由远端 macOS CI 执行；本机 Windows 不声称运行了该平台测试。原有重命名/事件/预算契约保持。

补丁后的三产品完整检查再次通过，分别为 Word 3,618 / Deck 982 / Sheet 1,420 项，无失败，既有忽略仍为 3 项。三产品普通 Release 包重新构建、打包并通过归档内容/版本/架构/sidecar 校验；新哈希在 checks.json 的 `local_packages` 中，首轮哈希单列保留。

从新包再验 Windows GUI 重开：[Word](gui/word-security-reopened.png)、[Deck](gui/deck-security-reopened.png)、[Sheet](gui/sheet-security-reopened.png)。Word 新包重复通过混合文档编辑/保存/渲染、旧 CLI 与 EPUB 格式脚本；Deck 新包 CLI 读回 20 页和中英文备注；Sheet 新包及隔离安装目录的 CLI 均读回公式值 6。新版 Sheet 安装器重复通过隔离安装、桌面 EXE 哈希核对和保留额外文件的卸载。新增 macOS 8 KiB 属性回归的执行结果以最终提交的 macOS CI 为准，本机未执行。
