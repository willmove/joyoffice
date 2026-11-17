# JoyDeck @VERSION@ · @PACKAGE_LABEL@

解压后在 Windows 运行 joydeck.exe，在 macOS 打开 JoyDeck.app。产品在 JoyOffice 工作区构建，具体依赖版本见包内 Cargo.lock，工具链见 rust-toolchain.toml。

本包的 JoyDeck 标识位于 `assets/brand/joydeck.svg`。

Linux 可直接运行解压后的 joydeck。如需桌面菜单入口，将 joydeck 放入 PATH，并将 share/ 中的内容合入 XDG 数据目录。

默认新建空白稿；Ctrl+O 打开，Ctrl+S 保存，Ctrl+Shift+S 另存为 .pptx 或 .joydeck。支持基础原位文字、字体/段落、图片/形状/表格、对象剪贴板、备注、撤销重做、浏览/选择窗格、F5 放映。旧 v1 .joydeck 需显式迁移并保留原件；完整 UI、命令和长尾格式/媒体/动画尚有缺项。

joydeck-cli 提供 inspect、render、edit、migrate、exchange、export、control、mcp 等入口；无参数显示完整列表。控制 envelope 名称沿用 control-1，但命令 schema 为 2，先查询 engine.commands，不可把旧 schema 1 脚本当作自动兼容。CLI/MCP 是独立 headless 文稿会话，不能修改已打开的桌面窗口。

包内 samples 为可重现样例；SOURCE-HASHES.json / source.json、BUILD-INFO.json、SBOM.spdx.json、来源台账、许可与 SHA256SUMS 用于追溯。辅助程序只处理显式本机输入，不包含系统字体。输出默认使用新路径，原件请另存保留。

macOS 12 或以上可用包内 joydeck-cli 执行 `export INPUT.pptx --format mp4 --output NEW.mp4 --seconds 2 --fps 24`。同目录的 joydeck-video-macos 使用 AVFoundation 编码静态幻灯片，MP4 不包含音频、备注、动画或嵌入媒体。

本包未作发行者签名。平台验证的范围与进展见源码仓库 docs/joydeck/STATUS.md。
