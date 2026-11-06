# JD-807/808 补充许可文件完整性检查

2026-11-06，基于 `main@aac15f2f9078fa5a0ff784575d9f296fd3a35c9c`，Linux x86_64。本批未构建或发布安装包，也未获取、补写缺失的许可正文。

## 已核实漏洞与修复

`scripts/joydeck/sbom.py` 原先复制补充许可文件而不核对 `SOURCES.json` 中已有的 SHA-256。回归先修改一份有来源哈希记录的测试许可文件，确认普通开发模式和 `require_license_texts=True` 模式都未拒绝。

现在凡补充目录带有 `SOURCES.json`，生成器会检查清单结构、唯一且目录内的文件名、SHA-256、记录文件存在性、大小与实际字节，拒绝未登记文件及符号链接。复制后再次检查目标字节，防止在检查和复制之间改变内容却仍输出验证成功的 SBOM。该检查在开发和严格模式均执行；普通缺少许可正文仍按原有模式报告或拒绝。

没有来源清单的历史补充目录保留既有收集方式，不因此宣称已验证来源。哈希核对只证明文件与记录一致，不能证明记录真实、许可适用或法律合规；SPDX 的 `licenseConcluded` 仍为 `NOASSERTION`。本批没有改变“找到文件名即识别正文”的既有收集规则，也没有把开发包改为正式签名发布。

## 当前核查结果

仓库内 48 份 JoyDeck 补充来源清单的登记文件哈希一致，无额外未登记文件。按当前锁文件补齐本地 Cargo 缓存后，针对 `joydeck` 和 `joydeck-cli` 的 `x86_64-unknown-linux-gnu` 普通／构建依赖生成 SBOM：**559 包，9 项缺少可识别许可正文**，详见 [linux-inventory.json](linux-inventory.json)。这是当前源树的依赖盘点，不是对已有二进制包的重新验签。

缺口为 `accesskit_atspi_common@0.19.1`、`accesskit_unix@0.22.1`、`harfrust@0.5.2`、`mac@0.1.1`、`pathfinder_geometry@0.5.1`、`seahash@4.1.0`、`spirv@0.4.0+sdk-1.4.341.0`、`xim-ctext@0.3.0`、`xim-parser@0.2.2`。严格模式拒绝并且没有生成 SBOM 输出文件。

Windows 最近一次包记录仍报告 458 包、缺 `mac@0.1.1` 和 `seahash@4.1.0`，见 [上游 P2 验证](../../../joyoffice/validation/20261106-p2/README.md)。本批没有重建该 Windows 包或复核其所有载荷，不把历史记录当作当前跨平台验收。

## 验证与剩余工作

专项测试覆盖篡改文件、两种生成模式、合法字节原样复制、异常 JSON／记录、重复／越界文件名、缺失或未登记文件、符号链接、复制期间字节变化，以及当前全部补充来源清单。共享生成器的 JoySheet 打包测试同步运行。

缺失正文仍需逐版本核实可追溯来源；正式发布签名、公证、实际更新源和设备验证保持开放。本批仅本地修改，未推送或创建 PR。

结果：6 项新增专项测试通过；`python3 -m unittest discover -s scripts/tests` 运行 **158 项，跳过 2 项，无失败**；`python3 -m unittest discover -s scripts/joysheet/tests -p test_packaging.py` **7 项通过**。文档检查和 `git diff --check` 通过。实际 Linux SBOM 生成与严格拒绝已运行，未更改 Cargo.lock、依赖版本或任何许可正文。

## 精确版本正文补充（后续批次）

在同一主干基线上继续核查原 Linux 9 项缺口。为 `accesskit_atspi_common@0.19.1`、`accesskit_unix@0.22.1`、`harfrust@0.5.2`、`spirv@0.4.0+sdk-1.4.341.0`、`xim-ctext@0.3.0`、`xim-parser@0.2.2` 补入上游原始许可文件。AccessKit 保留 MIT 和 Apache 两份，其余保留原项目正文。未增写版权归属。

每项均先将本地下载的 `.crate` 字节 SHA-256 与 Cargo.lock 核对，再从包内 `.cargo_vcs_info.json` 获取精确提交；使用该提交的上游 Cargo.toml 核对版本（harfrust 继承同提交的 workspace 版本），并下载同提交的仓库根许可。文件逐字节保留，固定 URL 和哈希记入对应 `SOURCES.json`。证据见 [版本、包与文件来源](supplement-provenance.json)。现有完整性测试覆盖新增目录。记录一致不等于独立证明上游授权或法律合规。

仍未补写的三项：

- `mac@0.1.1`：发布包没有 VCS 记录；上游 [0.1.1 标签提交](https://github.com/reem/rust-mac/tree/66afc663b68a65633ea165c742b5a9c6734581c2) 的 Cargo.toml 版本匹配，但完整文件树没有许可正文，README 只有 `MIT/Apache-2.0, like rust itself` 声明。不能将该声明改名充作正文或自拟版权。
- `pathfinder_geometry@0.5.1`：包记录提交 `a5e98fac00f433fe2ff4b2135383d82491b8bfc5`；声明仓库 `servo/pathfinder` 的固定提交原始文件返回 404，Git 读取该对象返回 `not our ref`。未用当前分支或其他版本替代；仍需找到可验证的精确来源。
- `seahash@4.1.0`：包记录提交 `94b632aeac099031c373599313d5b5f0acbbaec0`；声明来源 `https://gitlab.redox-os.org/redox-os/seahash` 的 Git 和 raw 请求均被环境网络入口以 403 拒绝。尝试同组织 GitHub 路径返回仓库不存在，未据此认定任何第三方镜像可信。

严格验证范围是实际所选依赖图的 `sbom.generate(..., require_license_texts=True)` 打包许可阶段。JoyDeck 当前 `package-native.py` 仍采用缺口报告模式；本批没有更改该开发包政策，也没有构建安装包、签名或运行桌面。严格门禁不通过时，不会宣称完整严格打包成功。


### 平台差异与复验

同一 Cargo.lock、默认 features、`joydeck` / `joydeck-cli` normal/build 图；在 Linux 上选择目标平台进行依赖盘点，没有交叉编译或真实设备验证。详见 [平台结果与直接依赖来源](platform-followup.json)。

| 目标 | 本次结果 | 严格许可阶段 |
| --- | --- | --- |
| Linux x86_64 | 559 包，原 9 项均在图内；补充后剩 mac、pathfinder_geometry、seahash | 拒绝，未产生严格 SBOM |
| Windows x86_64 | 452 包，原 9 项中仅 mac、seahash 在图内，仍缺这 2 项 | 拒绝，未产生严格 SBOM |
| macOS ARM64 | cargo tree 中原缺口仅 mac、pathfinder_geometry、seahash；完整 SBOM 生成在 core_detect 身份歧义处拒绝 | 未到许可检查阶段，不报告完整缺口数 |

Linux 特有的这批缺口来自 AccessKit Unix、cosmic-text/harfrust、Naga/SPIR-V 和 zed-xim；pathfinder_geometry 经 zed-font-kit 引入，在 macOS 图也存在。mac（宏库，名称并非 macOS 平台限制）经 HTML/XML 依赖引入，seahash 经 gpui-pre 引入。Windows 本次 452 包是当前默认特性的源图盘点，不能与历史 458 包记录等同或据此声称历史安装包已修复。

本批复验：6 项完整性专项通过（覆盖现在全部 54 份来源清单）；脚本全量 158 项、跳过 2 项、无失败；共享 JoySheet 打包回归 7 项通过。Cargo.lock、依赖版本和已有整合分支均未更改。新增 macOS 身份歧义是后续可修复工程事项；三项缺失正文、完整严格打包、签名与设备验收仍开放。


## 跨目标构建依赖身份修复

后续确认此前 `core_detect` “歧义”实际是 metadata 身份缺失。Cargo.lock 只有 registry `core_detect@1.0.0` 一条；Linux x86_64 主机为 macOS ARM64 目标运行的构建依赖链选中了它（经 serde-saphyr → encoding_rs_io → encoding_rs）。encoding_rs 将 core_detect 声明为 x86/x86_64 依赖，`cargo tree --target aarch64-apple-darwin --edges normal,build` 包含主机构建图，而 `cargo metadata --filter-platform aarch64-apple-darwin` 排除了该身份。不能从目标过滤的 metadata 直接推断完整构建依赖目录。

共享 SPDX 生成器和 JoyDeck 原生打包的辅助清单现在读取不按目标过滤的 metadata 身份目录，继续仅按目标 `cargo tree` 的 normal/build 图选择包与边。没有把整个 workspace 或其他平台的包写入 SBOM；未知身份、真正同名同版本多来源歧义仍拒绝，缺失身份另报 Missing。离线使用需要先缓存完整锁定 metadata 所需包，可用 `cargo fetch --locked` 准备；本次已下载锁定依赖，未修改锁文件或版本。

新增 3 项测试先在旧代码复现失败，随后通过：跨目标保留主机构建依赖并排除未选包、原生打包清单同样保留、缺失与多来源歧义继续拒绝。全量脚本 **161 项、跳过 2 项、无失败**；共享 JoySheet 打包 **7 项通过**。

[实际三目标复验](cross-target-fixed.json)：Linux 559 包/3 项缺口、Windows 452 包/2 项缺口保持不变；macOS ARM64 在本 Linux 主机上得到 463 包，缺 `block@0.1.6`、`mac@0.1.1`、`malloc_buf@0.0.6`、`pathfinder_geometry@0.5.1`、`seahash@4.1.0`。原生打包辅助清单也实际返回 463 包并保留 core_detect。三个目标的严格许可阶段均拒绝、未输出严格 SBOM。没有跳过许可正文检查，没有补写这五项来源。

这些是当前 Linux 主机的跨目标源依赖图验证，主机构建依赖会随构建主机变化，不等同于 macOS 真机构建清单、安装包签名或设备验收。本次未构建或发布安装包；许可来源限制及发布 PR 等待状态保持不变。

## 剩余来源复核与下一发布验证项

继续核查的五项仍未补入正文；[发布包校验和与文件树证据](unresolved-source-audit.json) 为每份本地 `.crate` 重新核对 Cargo.lock，记录原始发布文件清单。没有将许可证标识、README 声明、第三方项目许可清单或标准模板当作本依赖的原文。

- `block@0.1.6`：上游 [0.1.6 固定提交](https://github.com/SSheldon/rust-block/tree/47178790cfc9d4a8b092051d8b413b78bd31254a) 可读取，Cargo.toml 版本匹配且声明 MIT；完整文件树无许可正文，源文件也未找到许可授权全文。
- `malloc_buf@0.0.6`：上游 [0.0.6 固定提交](https://github.com/SSheldon/malloc_buf/tree/a7811e5f4c6f9685dd57ad9bfe34e9bf6b0ba4f9) 可读取，版本匹配且声明 MIT；标签只有 `.gitignore`、Cargo.toml 和 src/lib.rs，无许可正文。
- `mac@0.1.1`：再次核对已取得的精确标签及锁定发布包。仍只有许可标识/声明，未找到可补入的正文。
- `pathfinder_geometry@0.5.1`：搜索可见当前上游 Cargo.toml 仍标 0.5.1，但当前分支版本字符串相同不足以证明字节对应包内不可获取的提交；未用当前许可替代该版本来源证据。
- `seahash@4.1.0`：公开 registry/docs 索引能确认版本和声明，但未提供本次可验证的精确提交许可全文。原托管站拒绝访问保持为阻碍；未重试被拒绝的站点、代理该请求或采用未验证镜像。docs.rs 的精确版本源码页也不可通过当前浏览工具访问，未转用其他工具绕行该拒绝。

后续解决需要上游或其他可验证的权威发布渠道提供与精确版本绑定的正文；本次未联系维护者或向任何外部服务发送消息，也未得出不存在许可或不合规的法律结论。

### 下一项：当前发布矩阵门禁的归档成员安全

审查当前 `scripts/verify_release_assets.py`，其已检查平台矩阵、SHA-256、构建信息、二进制架构和执行权限，但缺少归档成员路径及类型检查。使用现有测试 fixture 构造四平台 JoyDeck 矩阵，在 Linux ZIP 分别加入 `../escape` 条目与指向包外的符号链接，再重算真实 sidecar，完整 `verify_assets` 均接受。见 [复现结果](release-gate-assessment.json)。仅临时测试文件，未解压、未执行、未发布，也不能据此宣称已有真实包包含此类内容。

此缺口可以在无桌面环境修复：在内容校验前拒绝越界/绝对/平台歧义路径、ZIP 符号链接与特殊文件、TAR 符号/硬链接与特殊文件；覆盖三产品 ZIP/TAR 以及合法包回归。该任务涉及共享发布门禁，应保留当前源依赖与许可严格检查。本批仅评估并复现，生产门禁尚未修改。

本批验证：现有发布矩阵测试 5 项通过，许可完整性测试 6 项通过；危险归档两项探针均被当前门禁接受（这是已复现缺口，不能计作安全通过）。文档检查通过。许可缺口数量不变：Linux 3、Windows 2、Linux 主机所选 macOS 图 5；完整严格发布仍未通过。
