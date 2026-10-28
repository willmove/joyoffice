# T504 中文高级版式与公文自动化证据（2026-10-28）

## 交付范围
1. **中文高级内联排版对象模型（LAYOUT-011）**：
   - 在 `joyword-model` 中扩展内联对象体系：
     - 拼音指南 `Ruby`（`base`、`text`、`align: RubyAlign`、`font_size_hps`、`raise_hps`）。
     - 双行合一 `CombineCharacters`（`text`、`brackets: CombineBrackets`）。
     - 着重号 `Emphasis`（`text`、`mark: EmphasisMark::Dot/Comma/Circle/UnderDot`）。
     - 带圈字符与组合形状 `EnclosedCharacter`（`text`、`shape: EncloseShape::Circle/Square/Triangle/Diamond`）。
   - 适配 `canonical_dump` 文本化序列化表示，确保无歧义调试与往返比对。
2. **GB/T 9704-2012 公文版式规格与校验断言（LAYOUT-011）**：
   - 严格遵循国家标准 GB/T 9704-2012《党政机关公文格式》§5.2.1 与 §5.2.2：
     - 标准 A4 纸张：210 mm × 297 mm（11,906 × 16,838 twips）。
     - 版心尺寸与四方边距：版心 156 mm × 225 mm（8,844 × 12,756 twips）；天头（上白）37 mm（2,098 twips）、地脚（下白）35 mm（1,984 twips）、订口（左白）28 mm（1,587 twips）、切口（右白）26 mm（1,474 twips）。
     - 行字排版网格：一行 28 字（字间距网格约 316 twips），一页 22 行（行距网格约 580 twips，28.9 pt），网格类型 `DocGridKind::LinesAndChars`。
   - 实现 `verify_gbt9704_section` 规则检查器，能够精确检出边距不合规、网格缺失或行距偏离。
   - 实现 `create_gbt9704_document_sample` 生成包含发文字号、居中标题、首行缩进（2字/632 twips）、拼音指南、着重号、带圈字符、双行合一及落款的完整合规公文样本文档。
3. **稿纸（Manuscript Paper）网格规格**：
   - 实现标准 20×20（400 字）与 20×25（500 字）稿纸网格参数配置与构建契约。
4. **双向文字（BiDi RTL）与复杂文字保真与受限诊断（PLATFORM-004、LAYOUT-014）**：
   - 实现 `scan_complex_scripts` 与字符集分类器，对阿拉伯语、希伯来语等双向文字和天城文/泰文等复杂文字进行范围识别。
   - 保真保留原始字符与编码序列，同时输出排版受限诊断提示，防止未经验证的复杂排版产生伪对齐。

## 自动测试命令与结果
```bash
cargo test -p joyword-layout --test cjk_gbt9704
```
5 项自动化测试全部通过：
- `test_gbt9704_spec_and_verification`：GB/T 9704 版面几何规格验证及边距/网格违规拦截测试通过。
- `test_cjk_inline_structures_and_dump`：拼音指南、双行合一、着重号、带圈字符构建与 dump 表现测试通过。
- `test_manuscript_paper_specs`：400 字与 500 字稿纸网格参数测试通过。
- `test_complex_scripts_and_bidi_diagnostics`：中英文正常流不报警、阿拉伯语/希伯来语及天城文双向与复杂脚本精确定位与受限提示测试通过。
- `test_gbt9704_document_sample_layout`：完整公文样本文档经过分页排版引擎成功生成第一页版面并保持标准 A4 几何尺寸。

## 未验证项
- 在真实打印机上输出 GB/T 9704 公文样张使用钢卷尺人工核对 37mm/35mm/28mm/26mm 留白尚未执行。
- 在 Word 2024 / WPS Office 最新桌面客户端中与真机公文模板进行点对点视图叠层比对尚未执行。
