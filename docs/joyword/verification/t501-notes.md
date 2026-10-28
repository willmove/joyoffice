# T501 注释、域与引用体系自动化证据（2026-10-28）

## 交付范围
1. **模型层脚注与尾注（Footnote / Endnote）支持**：
   - 在 `joyword-model::annotation::AnnotationKind` 增量引入 `Footnote { story: StoryId, custom_mark: Option<Arc<str>> }` 与 `Endnote { story: StoryId, custom_mark: Option<Arc<str>> }`。
   - `Document` 增加 `remove_story` 与 `remove_annotation` 安全移除方法，保障故事与注释引用生命周期一致。
   - `validate` 与 `dump` 验证器支持脚注尾注独立故事引用及类型检查。
2. **引擎层管理与求值**：
   - 脚注与尾注插入、删除、列表查询及内容提取（`insert_footnote`, `insert_endnote`, `remove_note`, `list_notes`, `get_note_text`）。
   - 复杂域代码（Field Instruction）解析：支持 `TOC \o "1-3" \h`、`REF bookmark`、`PAGEREF bookmark`、`SEQ identifier` 及未知指令保留。
   - 域更新机制（`update_fields`）：实现序号计数递增、交叉引用链解析与循环引用检测（`FieldError::CircularReference`）、最大递归深度防护（`FieldError::RecursionDepthExceeded`）。
   - 解耦布局查询接口 `PageLookup`，避免引擎层硬依赖布局层，支持目录条目（TOC）与页码引用（PAGEREF）动态计算。

## 自动测试命令与结果
```bash
cargo test -p joyword-engine --test notes_fields
```
5 项综合测试全部通过：
- `test_footnotes_and_endnotes_workflow`：脚注、尾注插入、枚举、读取及删除生命周期测试通过。
- `test_field_instruction_parsing`：TOC、REF、PAGEREF、SEQ 域代码词法与参数解析测试通过。
- `test_field_seq_and_ref_evaluation`：SEQ 计数器递增与 REF 书签跨段文本解析测试通过。
- `test_field_circular_reference_detection`：书签互引循环引用与自引用拦截测试通过。
- `test_toc_generation_from_headings`：基于 Heading 样式树与有界范围的目录项抽取测试通过。

## 未验证项
- 人工使用 Microsoft Word / WPS 进行长报告插入章节与图表后更新目录、页码和交叉引用的人工交互验证尚未执行。
- 复杂域代码的开关选项（如 `\f` 目录过滤、`\c` 序号重置等扩展参数）留待后续切片完善。
