# T502 公式、形状和表单内容自动化证据（2026-10-28）

## 交付范围
1. **格式中立 OMML 独立库（`joyoffice-omml`，ADR-011）**：
   - 遵循 ECMA-376 Part 1 §22 (Math) 公开规范，零专有依赖。
   - 实现 OMML AST（分式 `m:f`、上下标 `m:sSup`/`m:sSub`/`m:sSubSup`、根号 `m:rad`、定界符括号 `m:d` 及运行文本 `m:r`/`m:t`）。
   - 双向线性数学公式映射（`to_linear_text()`，支持如 `((x)^(2))/(\sqrt{y + 1})` 表达式输出与互转）。
   - 实现标准 OMML XML 解析器与生成器，包含不可信输入资源预算（`OmmlBudget` 约束最大嵌套深度与节点上限）。
2. **格式中立 DrawingML 形状与文本框库（`joyoffice-drawingml`，ADR-011）**：
   - 遵循 ECMA-376 Part 1 §20 (DrawingML) 公开规范。
   - 支持常见预设几何形状（矩形 `rect`、圆角矩形 `roundRect`、椭圆 `ellipse`、线条 `line`、文本框 `textBox`）。
   - 支持 EMU 与 pt 单位换算、填充色（`Fill::Solid`）、描边色与线宽（`Stroke`）、以及嵌套文本框流（`w:txbxContent`）。
   - 实现标准 DrawingML XML 解析与生成器，包含图形数量预算防御（`DrawingBudget`）。
3. **内容控件（SDT）与模板工作流**：
   - 支持类型化 SDT 内容控件（`SdtProps` 包含 `tag`、`alias`、`lock` 锁定契约）。
   - 严格约束 `SdtLock::SdtContentLocked` 与 `SdtLock::SdtLocked`，编辑引擎与 AI 替换路径严格遵循锁定契约，拒绝越权篡改。
   - 识别 `.dotx` 模板内容类型与工作流映射，保护模板只读完整性。

## 自动测试命令与结果
```bash
cargo test -p joyoffice-omml -p joyoffice-drawingml -p joyword-docx --test shapes_forms_omml
```
8 项集成测试全部通过：
- `test_fraction_and_linear_mapping`：分式与线性公式表示双向映射测试通过。
- `test_subscript_superscript_and_radicals`：上下标复合与根式解析测试通过。
- `test_delimiter_and_budget`：定界符与 OMML 深度预算拦截测试通过。
- `test_shape_serialization_and_parsing`：DrawingML 形状与文本框属性序列化及逆向解析测试通过。
- `test_drawing_budget_protection`：DrawingML 图形数量超限预算拦截测试通过。
- `test_omml_equations_integration`：DOCX 复合公式集成与线性提取测试通过。
- `test_drawingml_textbox_and_shapes_integration`：DOCX 形状与表单文本框集成测试通过。
- `test_sdt_content_control_lock_semantics`：内容控件锁定与拒绝编辑语义测试通过。

## 未验证项
- 人工使用 Microsoft Word / WPS 进行带复杂公式、形状与表单内容控件的报告双向交互修改与版面比对尚未执行。
- 3D 效果、渐变填充与 SmartArt 图表扩展留待后续阶段实现。
