# 第二波验证手册（2026-10-23）

日期 2026-10-23。第二波四路切片与集成 PR 已合入 `main`（#89 `6c43799`、#91 `2f183d8`、#90 `4c9e34d`、#92 `aec6ba4`、#93 `2466b05`）。本文给出每项特性的**可执行验证步骤**，并写明哪些能手工验证、哪些只能靠自动测试。

本文只提供验证方法，不改任务状态。四个任务仍是 `in_progress`：切片落地不等于任务完成。

## 先读这一段：哪些能手工验证

| 特性 | 手工可达 | 说明 |
| --- | --- | --- |
| 生产恢复双代（T104） | **可以** | 有完整用户路径：保存、杀进程、重启恢复 |
| 栏分隔线 `sep`（T202） | **可以** | 需要一个带 `w:sep="1"` 的文档，本文给出生成方法 |
| 章节 PAGE 编号（T202） | **可以**（有限） | 需要带 `chapStyle` 的文档；**需要有编号的 Heading 1**，否则看不到前缀 |
| PDF 内部跳转（T204） | **部分** | 导出与读回可查；但 DOCX 书签未导入，真实文档不会产生跳转，见下文 |
| 授权替换提案与只读预览（T402） | **不能** | 只有 `joyword-agent` 库级 API，应用与 CLI 尚无入口 |

## 通用准备

```bash
cd D:/Coding/EditorProjects/joyoffice
export CARGO_TARGET_DIR="$PWD/target"     # 共享 target 会串味，见文末
```

## 1. 生产恢复双代（T104）

**做了什么**：恢复日志从单文件改为两个发布槽 `recovery-a.jwr`／`recovery-b.jwr` 交替写入，每代带 `JWREC002` 头与 CRC32；写入经临时文件、flush、sync、读回校验、原子改名后才发布，发布前一代始终保留。最新槽损坏时回退到另一槽；两槽同代内容不同或都损坏时拒绝，不猜。旧 `JWREC001` 单文件格式只读兼容。保存 N 完成时编辑记录 N+1 仍为 dirty，重启可恢复到 N+1。

### 用例 A：正常保存与恢复（手工）

1. 打开 `tests/fixtures/docx/DOCX-SAMPLE-001.docx`，修改一处文字但**先不保存**。
2. 用任务管理器强制结束 JoyWord 进程。
3. 重新启动 JoyWord，打开同一文件：应提示恢复到强制结束前的状态。
4. 与正式保存的文件对照：正式保存那份不含未保存的修改，恢复出来的那份含。

**期望**：恢复内容是最后完整提交的状态；正式文件不被恢复内容覆盖。

### 用例 B：双代与损坏回退（自动 + 手工可查）

```bash
cargo test -p joyword-shell --locked --lib storage::recovery
```

应通过 16 项。重点看：

- `production_rotates_and_falls_back_from_a_corrupt_latest`：最新槽损坏时回退到另一槽。
- `pending_and_tmp_are_never_candidates_and_equal_generation_conflicts_fail`：临时文件不是候选；同代冲突拒绝。
- `detached_save_n_completion_keeps_n_plus_one_dirty_and_recoverable`：保存 N 完成时 N+1 仍 dirty。
- `legacy_is_read_only_through_failed_and_successful_migration`：旧格式只读，迁移不改写旧文件。
- `encryption_is_refused_on_write_and_on_legacy_or_v2_recovery`：加密包不落明文。
- `a_released_lock_can_be_taken_again_by_the_same_process`：同进程可再次取锁（见第 6 节 macOS 说明）。

**手工查文件**：恢复目录在配置目录下（Windows 为 `%APPDATA%/JoyWord/recovery`，可用 `JOYWORD_RECOVERY_DIR` 指定）。恢复后可看到 `recovery-a.jwr`／`recovery-b.jwr` 两个文件交替更新，以及 `recovery.lock` 锁文件（**不会**被删除，这是设计）。

### 用例 C：macOS 锁（重点，Linux/Windows 复现不了）

```bash
cargo test -p joyword-shell --locked --lib storage::recovery::tests::a_released_lock_can_be_taken_again_by_the_same_process
```

在 macOS 上必须通过。曾经的失败不是用例问题：BSD 与 macOS 的 `flock` 锁属于**文件**而非描述符，关闭描述符不释放进程持有的锁，std 的 `File` 析构只关描述构、不发 `LOCK_UN`，于是同进程下一次取锁被自己的旧锁拒绝（`Kind(WouldBlock)`），而 Linux/Windows 随最后句柄关闭即释放，所以只有 macOS 暴露。修法是显式 `LOCK_UN` 的守卫类型。

**Linux/Windows 上这个用例也通过，但只证明回归被钉住，证明不了平台差异**——差异要看 macOS 作业。

## 2. 栏分隔线 `w:sep`（T202）

**做了什么**：`w:cols/@w:sep`（ECMA-376 §17.6.4）从「整节保留不排版」变为可用：等宽与不等宽两种形态都认，非法值（如 `sep="maybe"`）仍照原样保留。排版在栏间空隙画一条竖直细线，屏幕、PDF 与 CPU 光栅共用同一批页面场景矩形。

### 用例 A：准备样例

仓库样例 `DOCX-FEAT-T202-PAGES.docx` **不含** `w:sep`（只有不等宽栏）。生成一个带分隔线的副本：

```python
python - <<'PY'
import zipfile, os
src = "tests/fixtures/feature/DOCX-FEAT-T202-PAGES.docx"
dst = "target/manual-verify/column-separator.docx"
os.makedirs("target/manual-verify", exist_ok=True)
zin = zipfile.ZipFile(src)
zout = zipfile.ZipFile(dst, "w", zipfile.ZIP_DEFLATED)
for item in zin.infolist():
    data = zin.read(item.filename)
    if item.filename == "word/document.xml":
        doc = data.decode("utf-8").replace(
            '<w:cols w:equalWidth="0" w:num="2">',
            '<w:cols w:equalWidth="0" w:num="2" w:sep="1">', 1)
        data = doc.encode("utf-8")
    zout.writestr(item, data)
zout.close()
print("wrote", dst)
PY
```

### 用例 B：肉眼与导出核对

1. 在 JoyWord 打开 `target/manual-verify/column-separator.docx`：**两栏之间应有一条细竖线**。
2. 导出 PDF，打开后确认竖线也在（同一批场景矩形，屏幕与 PDF 一致）。
3. 缩放页面，竖线应保持在栏间空隙中央。

**期望**：1 pt 黑色细线，位于栏间空隙中点，纵向从正文框顶铺到底。

### 用例 C：自动验证

```bash
cargo test -p joyword-layout --locked --test columns
cargo test -p joyword-docx --locked --test column_separator
cargo test -p joyword-pdf --locked --test column_separator
```

重点：`equal_column_separator_paints_one_centered_rule`、`unequal_column_separator_paints_a_rule_per_gap`、`separator_off_paints_no_rule`、`invalid_separator_value_stays_preserved`、`the_column_separator_rule_is_in_the_pdf_page_scene`。

**已知假定**：ECMA-376 §17.6.4 只记录 `sep` 打开，不定线宽与位置，「1 pt／居中／铺满正文框」是 JoyWord 假定，没有 Word 对照。

## 3. 章节 PAGE 编号（T202）

**做了什么**：`w:pgNumType` 的 `chapStyle`（1..=9 的内置标题级别）与 `chapSep`（hyphen／period／colon／emDash／enDash，hyphen 是 U+2011 非断行连字符）类型化进模型。前缀**只作用于 PAGE 域**，NUMPAGES／SECTIONPAGES 不加。章节号取实际渲染出的编号标签原样，不从正文文字里猜数字。

### 用例 A：准备样例

同样从 `DOCX-FEAT-T202-PAGES.docx` 改 `pgNumType`：

```python
python - <<'PY'
import zipfile, os
src = "tests/fixtures/feature/DOCX-FEAT-T202-PAGES.docx"
dst = "target/manual-verify/chapter-page.docx"
os.makedirs("target/manual-verify", exist_ok=True)
zin = zipfile.ZipFile(src)
zout = zipfile.ZipFile(dst, "w", zipfile.ZIP_DEFLATED)
for item in zin.infolist():
    data = zin.read(item.filename)
    if item.filename == "word/document.xml":
        doc = data.decode("utf-8").replace(
            '<w:pgNumType w:fmt="decimal" w:start="1"/>',
            '<w:pgNumType w:fmt="decimal" w:start="1" w:chapStyle="1" w:chapSep="hyphen"/>', 1)
        data = doc.encode("utf-8")
    zout.writestr(item, data)
zout.close()
print("wrote", dst)
PY
```

### 用例 B：肉眼核对

1. 在 JoyWord 打开 `target/manual-verify/chapter-page.docx`，翻到有页码的页。
2. 页脚页码**应带章节前缀**，形如 `1-1`（hyphen 是 U+2011，显示为短横）。

**重要限制**：样例文档**没有用 Heading 1 样式、也没有编号**，所以**看不到前缀**。这是设计而非缺陷——章节号取自「实际渲染出的编号标签」，没有编号就没有章号。要看到前缀，文档必须同时具备：内置 `heading 1` 样式的段落 **且** 该段落有编号定义。仓库内 `chapter_model()` 测试辅助函数构造了这种文档，但它是测试代码，不是可导入的样例文件。

**因此本章节的可靠验证是自动验证**：

```bash
cargo test -p joyword-layout --locked --test pages chapter_
```

应通过 8 项，重点看：

- `chapter_five_separators_body_header_footer_and_counts`：五种分隔符、正文与页眉页脚同前缀。
- `chapter_heading_nine_multilevel_and_no_guessed_heading_numbers`：Heading1..9、多级标签、不猜正文数字。
- `chapter_page_first_heading_then_predecessor_and_section_level_switch`：选章规则与节级别切换。
- `chapter_revision_hidden_projection_and_warm_incremental_agree`：修订与隐藏投影一致，增量与全量一致。
- `chapter_feedback_cache_changes_when_numbering_or_page_placement_changes`：反馈与缓存收敛。
- `chapter_cross_page_field_paragraph_is_refused_instead_of_stale`：跨页 PAGE 段落明确拒绝。
- `chapter_next_column_separator_and_continuous_sections_keep_page_context`：`nextColumn` 与连续节的页上下文。

### 用例 C：与集成层的组合回归

```bash
cargo test -p joyword --locked --bin joyword chapter_bookmarks
```

`chapter_bookmarks_warm_scene_and_selected_pdf_share_real_page_geometry`：带章节前缀的文档与带书签的文档共用一次布局，且选页导出与全量导出字节相同。

**已知限制（重要）**：选章规则是**已声明的 JoyWord 假定**，不是 Word 一致性主张——本物理页首个匹配标题用于整页，否则取前方最后一个同级标题；Word 按域所在位置取当时生效的章节，两者可能不同。另外不做 ECMA 的上一级回退，只用规范的内置标题样式名（`heading 1`..`heading 9`），本地化标题名（如「标题 1」）不被识别。

## 4. PDF 内部跳转（T204）

**做了什么**：模型书签解析为 PDF 命名目标，锚点内链写成真实 `/S /GoTo` 动作与显式 `/D` 引用。名称精确匹配且区分大小写；空名、控制字符名、重名、脱离锚点、未布局位置、过期布局、非法几何与超预算都拒绝，名称永不被当作 URL。

### 用例 A：导出与读回（自动）

```bash
cargo test -p joyword-pdf --locked --test navigation
```

应通过 14 项。重点看：

- `model_forward_backward_same_page_and_uri_actions_survive_pdf_readback`：前进、后退、同页、跨页与 URI 共存。
- `a_non_ascii_bookmark_name_survives_the_name_tree_readback`：中文书签名读回（曾因按 PDFDocEncoding 解码而乱码，现按 ISO 32000-1 §7.9.6 保留原始八位组）。
- `an_anchor_without_a_bookmark_is_counted_and_leaves_the_rest_usable`：目标不存在的锚点被计数并跳过，**不阻断整份导出**。
- `duplicate_names_never_select_a_first_target_and_case_is_exact`：重名不选第一个，大小写精确。

### 用例 B：应用层导出接线（自动）

```bash
cargo test -p joyword --locked --bin joyword scene_export
```

应通过 6 项。重点看：

- `a_selected_page_range_renumbers_both_ends_and_drops_a_link_it_cannot_keep`：选页导出时，源在选页外则丢弃、目标在选页外则丢弃、两者都在则**两端都重编号**（不会误跳到别的页）。
- `plain_scene_pdf_keeps_the_existing_export_and_rejects_stale_versions_and_ranges`：无书签文档的导出字节与既有路径完全相同。
- `an_anchor_without_a_target_does_not_block_the_export`：真实 DOCX 常见情形不阻断导出。
- `scene_export_refuses_invalid_names_detached_and_duplicate_targets_before_slicing`：空名、控制字符名、重名、脱离锚点四种都拒绝。
- `bookmark_endpoints_require_visible_exact_carets_in_the_installed_view`：书签端点必须在已安装视图里有精确 caret，隐藏文字里的端点被拒。
- `chapter_bookmarks_warm_scene_and_selected_pdf_share_real_page_geometry`：带章节前缀的文档与带书签的文档共用一次布局，且选页导出与全量导出字节相同。

### 用例 C：手工核对导出 PDF（有限）

用仓库的样例导出 PDF，然后检查它没有跳转但没有报错：

```bash
# CLI 的字体参数是 PATH:FACE；--output 必须以 .pdf 结尾且目标不存在
cargo run --locked -p joyword-cli -- convert \
  --font "C:/Windows/Fonts/simsun.ttc:0" --to pdf \
  --output target/manual-verify/t202.pdf \
  target/manual-verify/column-separator.docx
```

（Linux 上把字体换成 CI 用的 `/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc:0`。`render` 子命令输出的是 PNG 页面图，不是 PDF。）

**为什么手工看不到跳转**：DOCX 的书签与其锚点链接分处不同部件，而当前只导入链接、不导入书签。所以真实文档在模型里必然是「有锚点、无对应书签」，这类链接按设计被跳过（计数但不阻断）。**能看到真实 GoTo 跳转，需要 JoyWord 自己产生的书签**，而当前应用层没有创建书签的入口。

### 用例 D：外部阅读器复核（未覆盖）

仓库自带的独立字节解析器（`inspect_navigation`）核对页对象身份、`/GoTo /D`、`/XYZ` 与命名目标树。但**没有外部 PDF 结构读取器复核**——本机没有 pypdf／pikepdf／qpdf／MuPDF，只有 `pdftotext`（只抽文本，不看结构）。若要做外部复核：

```bash
pip install pypdf
python -c "
from pypdf import PdfReader
r = PdfReader('target/manual-verify/t202.pdf')
print('named destinations:', list(r.named_destinations))
for i, p in enumerate(r.pages):
    for a in p.get('/Annots', []) or []:
        o = a.get_object()
        if o.get('/A', {}).get('/S') == '/GoTo':
            print('page', i, '->', o['/A']['/D'])
"
```

**期望**：本仓库现有样例不产生 GoTo 注释（没有书签）；一旦有书签，应看到 `/S /GoTo` 与目标页。

## 5. 授权替换提案与只读预览（T402）

**做了什么**：`joyword-agent` 提供单个非空授权选区的纯文本替换提案：提案私有绑定原授权与 runtime，预览只读。预览每次都重新校验授权、版本、指纹、期限与 generation；独立有界 ID 表防重；内容与说明按 UTF-8 字节预算；诊断不含正文。

### 只能自动验证

```bash
cargo test -p joyword-agent --locked
```

应通过 63 项（7 单元 + 18 契约 + 15 预览 + 22 受限工具 + 1 compile-fail）。重点看 `crates/joyword-agent/tests/replace_preview.rs`（15 项）：

- 提案与预览的绑定校验（授权不符、版本不符、期限过期、runtime 不符都拒绝）。
- 预览不修改原稿。
- 重复提案被拒（同一 ID 不占号两次）。
- 预算边界（超长内容、说明、超长 ID 拒绝）。
- Debug 输出不含正文。

### 手工验证：目前做不到

`joyword-agent` 是库，**应用与 CLI 都没有入口**（`grep preview_replace apps/ crates/joyword-cli/ crates/joyword-shell/` 无命中）。AI 功能默认关闭，且没有真实 Provider 接线。要手工验证，需要先做「应用接线」这一后续切片（T402 的下一步）。

**可以手工确认的是它的「拒绝」侧**：授权不足时公开读取入口被拒——这在第一波 #83 已经接入，见 `crates/joyword-agent/tests/scoped_tools.rs`。

## 6. 跨平台与 CI

四个 PR 与集成 PR 的 CI 各 18 项（push 9 + PR 9）。查最近一次：

```bash
gh run list --branch main --limit 3
gh pr checks 93
```

**本地无法覆盖的五个作业**：macos-15、Windows-2022、ubuntu-24.04、ubuntu-24.04-arm、aarch64 与 loongarch64 QEMU 无头测试、unified-pdf（用 Noto CJK 字体）。本地 Windows 只能覆盖其中一部分。

**两个已知的本地噪声**，不要误判为回归：

1. `scripts/tests/test_make_feature_fixtures.py::test_committed_bytes_and_manifest_hashes_match` 本地必然失败：本机 Python 3.14.7 链接 zlib-ng，级别 6 deflate 字节与 CI 的 stock zlib 不同，5 个 DOCX-FEAT 夹具哈希因此不一致。**解压后成员与 CRC 相同**，干净 main 上同样失败。不要重生成夹具。
2. `cargo test --workspace` 会带入 `apps/joyword`（非默认成员），可能触发同一 target 目录串味。**每个工作树用独立 `CARGO_TARGET_DIR`**，最终集成验证串行跑。

## 一页速查

```bash
# 全部自动验证（本地，约十几分钟）
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all -- --check
python scripts/check_docs.py && python scripts/check_acceptance.py && python scripts/check_gates.py

# 分特性
cargo test -p joyword-shell  --locked --lib storage::recovery   # T104
cargo test -p joyword-layout --locked --test columns --test pages # T202
cargo test -p joyword-docx   --locked --test column_separator     # T202 导入
cargo test -p joyword-pdf     --locked --test navigation          # T204
cargo test -p joyword         --locked --bin joyword scene_export # 集成接线
cargo test -p joyword-agent   --locked                            # T402

# 手工：恢复
#   打开样例 → 改一处不保存 → 强杀进程 → 重启恢复
# 手工：栏分隔线与章节页码样例
#   按第 2、3 节生成 target/manual-verify/*.docx 后打开核对
```

## 仍未覆盖（不算通过）

- 图形界面观感、Word／WPS 打开对照、信创真机、真实断电与磁盘满。
- 外部 PDF 结构读取器复核（T204）。
- DOCX 书签导入，因此真实文档的内部跳转在应用层不可见（T204）。
- T402 的应用接线与真实 Provider。
- 章节页码与 Word 的视觉一致性（T202）。
- 多级编号 restart 与章节标签的交互（T202）。
