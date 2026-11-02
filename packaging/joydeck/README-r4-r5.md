# JoyDeck 0.5.0 · R4/R5 Windows 开发包

解压到可写目录，运行 `joydeck.exe`，或把 PPTX / `.joydeck` / `.deckcraft` 路径作为参数。默认采用 GPUI 场景，可在标题栏切换“CPU 对照”。名称继续使用 JoyDeck。

## 本次可体验的切片

- 普通路径、实色填充、基本描边和精确字形轮廓交给 GPUI；图片、图表、表格、组合、渐变与效果使用有界对象位图。可查看回退原因。母版/版式背景独立缓存。
- 移动时直接更新对象位置，复用几何和纹理；松开提交一个撤销步骤。缩略图只使变动页面失效。场景和 GPU 资源分别有 48 / 64 MiB 缓存预算，支持 LRU、取消及过时结果丢弃。
- “设计”提供现有版式、母版背景、页面背景、母版图形显隐、主题强调色/拉丁字体、页面尺寸和文稿标题/作者。页面尺寸修改不自动缩放对象。
- “图片”支持本地位图插入/替换、裁剪、透明度、亮度和对比度。导入规范化为 PNG；16 MiB 输入、8192 边长、16 MP、64 MiB 解码/文稿媒体预算。
- “图表”支持常见分类图表标题、单元格数值、柱形/折线/饼图类型切换。修改后的公开 PPTX 同时包含图表缓存与新生成的只含数据的 XLSX 工作簿；原有未修改部件保留。带扩展、独立样式或外部依赖的图表修改会拒绝公开保存，可另存 `.joydeck`。
- 属性侧栏显示真实对象和继承信息，并提供基本形状、实色/线性渐变填充。窄窗口自动收起属性侧栏；操作集中到标签工具区。保留 R2/R3 原位输入、表格、剪贴板、撤销重做和恢复。

## 真实样本与 CLI

`samples/r4-r5/independent-complex.pptx` 是独立 python-pptx 生成的原创公开 OOXML，含真实母版图形、主题、图片、图表/内嵌工作簿、旋转渐变、组合及合并表格。

```powershell
.\joydeck.exe ".\samples\r4-r5\independent-complex.pptx"
.\joydeck-cli.exe content ".\samples\r4-r5\independent-complex.pptx"
.\joydeck-cli.exe edit ".\samples\r4-r5\independent-complex.pptx" --script ".\samples\r4-r5\edit.json" --output ".\edited.pptx"
.\joydeck-cli.exe import-picture ".\samples\r4-r5\independent-complex.pptx" ".\samples\r4-r5\shapes.png" --output ".\picture.pptx"
.\joydeck-cli.exe scene-bench ".\samples\r4-r5\independent-complex.pptx" --page 2 --iterations 100 --output ".\scene-benchmark"
```

输出路径必须是新文件/目录，CLI 不覆盖已有文件。`content` 输出完整模型元数据，媒体数据不作为字节数组输出。`scene-bench` 输出 CPU 图、场景参考图、固定阈值像素检查、字体字节 SHA-256/face 及 p50/p95/p99。

桌面“渲染诊断 → 运行桌面对照”在同一真实窗口各测试 60 帧，临时平移同一对象，输出实际准备/画布提交的样本与百分位到 stderr。可用 PowerShell 7 重定向运行日志。这些测量是准备和提交耗时；屏幕呈现、GPU 时间戳、输入到光子延迟尚未测量。不能由这些数据宣称实际 FPS 或整体加速倍数。

## 边界与依赖

这是 R4/R5 可运行纵向切片，完整阶段仍在推进：原生组/图片管线、全部路径/描边/文字效果、完整字体热更新、全部图表/工作簿公式与样式、SmartArt 编辑、母版对象编辑、跨文稿复制、页面排序/节、审阅和完整 329 命令 UI 尚未收口。背景依赖等不能安全拆分的场景会提示切回 CPU。公开保存保留原 ZIP 部件；未知结构不静默删除。

依赖与 JoyOffice 基线一致：Rust 1.98.1、GPUI Kit 0.8.1、GPUI pre 0.3.10；无 egui/gpui-fast。复用来源和变更见 `licenses/*/source-map.json` / `file-map.json`，精确依赖见 `sbom.json`，源码指纹见 `source.json`，包内校验见 `SHA256SUMS`。系统字体仅在本机发现，不随包分发。开发包未签名；Linux/macOS 本轮未实机验收。
