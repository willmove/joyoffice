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
