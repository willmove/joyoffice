# Attribution

The `core`, `numfmt`, `formula`, `functions`, `model`, `calc`, `xlsx`, `chart`,
`pdf`, `engine`, and `mcp` crates and the CLI originate from GridCraft 0.12.1,
https://github.com/storytold/gridcraft, licensed MIT OR Apache-2.0.
Original notices are retained in LICENSE-MIT, LICENSE-APACHE and NOTICE.
The exact imported core sources are recorded in docs/joysheet/upstream-source.json.

The GPUI Kit dependency is Apache-2.0; its default icon assets are supplied by
that dependency. Joysheet's identity and interface are implemented in code.
ArtCraft trademarks and branding assets are not included.

Sheet icon shapes use the Lucide catalog bundled with GPUI Kit Assets 0.9.1 (Lucide 1.43.0). ISC/MIT notices are preserved in docs/joysheet/LICENSE-LUCIDE. Legacy icon names are mapped where an equivalent is available.

The [JoySheet app logo](../../assets/brand/joysheet.svg) is original JoyOffice artwork. The SVG master and its ICO and ICNS exports are distributed under the JoyOffice repository license.

Redistributed Windows runtime DLLs originate from the Visual Studio C++ redistributable directory and retain Microsoft signatures/copyright; redistribution is subject to the Visual Studio runtime redistribution terms. The NSIS installer uses the zlib compressor; its original COPYING notice is in the binary package licenses/NSIS-COPYING.txt. Dependency license declarations, source repository links and available original license texts are collected into dependency-licenses/ and SBOM.spdx.json by cargo xtask package joysheet for the target host.

Windows W1–W4 的条件格式矢量图标和几何绘制由本项目实现；插入图标使用 GPUI Kit 随包 Lucide 资产，许可随依赖清单分发。旋转文字/形状文字从目标机器已安装字体生成运行时轮廓，不复制或分发系统字体文件。新增 winsafe 0.0.19 使用其发布包附带的 MIT 许可证正文。

ZIP part-name compatibility decoding is independently implemented from PKWARE APPNOTE section 4.4.4 (language encoding flag) and the standard Microsoft CP437-to-Unicode mapping published by Unicode: https://www.unicode.org/Public/MAPPINGS/VENDORS/MICSFT/PC/CP437.TXT (table 2.00). Mapping values are standard character assignments; no editor implementation is used.

JoySheet's probability distributions use density and cumulative formulas from the NIST Engineering Statistics Handbook and the incomplete beta and gamma relationships in DLMF chapter 8. Worksheet argument and error behavior is checked against the Microsoft function reference, a frozen JoySheet build and Microsoft Excel 16. The original import records remain available for provenance. [Stage 1 evidence](../joyoffice/validation/20261124-craft-stage1/README.md)

The shared special-function kernels use the Gamma recurrence, reflection and Stirling expansion, scaled continued-fraction convergents for the incomplete functions, and a safeguarded normal-tail inverse. The original import hashes remain in the upstream ledger. [DLMF Gamma](https://dlmf.nist.gov/5.11) · [DLMF continued fractions](https://dlmf.nist.gov/3.10) · [DLMF incomplete gamma and beta](https://dlmf.nist.gov/8) · [DLMF error functions](https://dlmf.nist.gov/7)
