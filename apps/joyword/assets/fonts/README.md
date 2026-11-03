# Word 单色 emoji 备用字体

`NotoEmoji-Regular.ttf` 是 Google `googlefonts/noto-emoji` 提交 `9a5261d871451f9b5183c93483cbd68ed916b1e9` 的原始未修改字体，文件位于上游 `fonts/NotoEmoji-Regular.ttf`。许可全文为应用根目录 `LICENSE-NotoEmoji.txt`，精确 SHA-256 与固定下载地址登记在仓库 `licenses/fonts/sources.json`。该字体不是宿主系统字体。字体内版权为 Copyright 2013 Google Inc. All Rights Reserved.，版本为 1.05。

桌面排版在主字体缺字时尝试整个 shaping cluster；最终页面场景按真实字体拆分，屏幕与 PDF 使用同一字形与坐标。备用字体嵌入可执行文件，包携带 OFL 许可，运行不联网、不安装系统字体。

该字体为旧版可缩放单色轮廓，用于补齐已报告的 😀 等覆盖；不承诺全部新 Unicode emoji 或彩色显示。仍缺字时保留原文字并报告诊断，不用空白替代。
