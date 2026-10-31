# M1 独立只读查看器实施规格

版本 1.0 · 2026-10-29。实施 FR-APP-02、FR-DOC-02、FR-SLD-05 的读取/浏览子集、FR-IO-02。完整 M1 仍受 11/12 的验收约束。

## 原生 v1

ZIP 中 `mimetype` 精确为 `application/vnd.joydeck.presentation+zip`。`manifest.json` 为 `{"format":"joydeck","schemaMajor":1,"schemaMinor":0,"minReader":1,"requiredFeatures":[],"document":"document.json"}`。

`document.json`：title、width/height（点）、slides。每页有唯一 id、title、background（#RRGGBB）和 objects。对象有 kind（rect/ellipse/line/text/placeholder）、x/y/width/height、fill、text、fontSize；默认颜色 #202b3a、字号 24。几何与字号需有限；页大小 1–4096 点、字号 1–512 点。文字按换行分行预览，自动换行与富文本尚未保证。未知必需特性、major、对象类型拒绝；未知可选文档字段不承诺重写。只读不丢失源文件；写入在 M2。

## PPTX 读取子集

依据 OPC 根关系找到 presentation，再按 sldIdLst 的 rId 顺序定位页面，读取 sldSz（12700 EMU/点）。首个实现支持 Transitional PresentationML/DrawingML 的直接坐标、实色矩形/椭圆/直线、段落纯文本。复杂形状、图片、组、表格、图表、继承版式、主题、旋转及效果不承诺保真；能定位的对象给占位和报告。Strict、宏、加密格式明确拒绝。外部关系不读取网络。

## 资源边界

M1 有意使用比通用目标更小的预算：文件 64 MiB、解压总计 128 MiB、单成员 8 MiB、4096 个 ZIP 成员、500 页、总对象 50000/单页 10000。拒绝重复、绝对、逃逸、反斜线、编码歧义路径和符号链接。XML 禁 DTD、最多 100000 节点/128 层。后台读取分块检查取消；先验证后发布；失败不替换当前文档。

## 桌面行为与渲染

默认启动查看器；保留 `--diagnostics`。文件选择器、路径输入与 CLI 路径共用读取服务。加载有阶段状态及取消；结果带请求序号，关闭窗口/取消/再次打开均使迟到结果失效。

后台将独立模型转换为受控 SVG，再用通用 resvg 栅格化；主线程只接收 BGRA 图像。系统字体在后台一次加载、复用，不随包分发。明确提示字体替代与简化排版。页面和缩略图共享模型，缓存有上限；翻页和缩放结果带文档请求/页码/尺寸键。

缩略图列表可滚动和点击，上一/下一、首尾页键盘导航；适应窗口随可用区域变化，手动缩放 25–400%，大页面可滚动。状态栏显示只读、页码、比例、载入信息。报告显示告警代码、页号、部件和原因。无保存/内容编辑入口。

每切片运行 build 和真实桌面启动；最终验证 native/PPTX、翻页缩放、取消/失败保留、窗口尺寸、损坏/超限、迟到结果及 M0 回归。性能以实际测量记录，完整 AT-02/15 和外部语料未测部分保持待验收。
