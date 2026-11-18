# JoyOffice 产品标识与图标

三款产品共用 J 形回钩与圆角底板。文档和发布包引用对应产品的 SVG 母版。

| 产品 | 图形 | 主色 | SVG |
| --- | --- | --- | --- |
| <img src="../../assets/brand/joyword.svg" width="24" height="24" alt=""> JoyWord | 段落线 | `#3455D1` | [joyword.svg](../../assets/brand/joyword.svg) |
| <img src="../../assets/brand/joydeck.svg" width="24" height="24" alt=""> JoyDeck | 放映三角 | `#C04A30` | [joydeck.svg](../../assets/brand/joydeck.svg) |
| <img src="../../assets/brand/joysheet.svg" width="24" height="24" alt=""> JoySheet | 四格单元 | `#13765D` | [joysheet.svg](../../assets/brand/joysheet.svg) |

## 构建与分发

| 平台 | 图标接入 |
| --- | --- |
| Windows | Cargo 在桌面 EXE 中嵌入多尺寸 ICO，资源编号为 GPUI 加载的 `1`。JoySheet NSIS 安装器和卸载器也使用该 ICO。 |
| macOS | 正式打包和 `cargo xtask run <产品>` 的开发 `.app` 都由 `CFBundleIconFile` 引用 `Contents/Resources/` 内的 ICNS。裸 Mach-O 文件不提供 bundle 图标，开发时使用工程运行入口。 |
| Linux | 每个 GPUI 窗口从内嵌 ICO 解码 RGBA，向 X11 设置 `_NET_WM_ICON`，直接运行编译产物也有图标。桌面入口使用 `.desktop`，SVG 位于 `share/icons/hicolor/scalable/apps/`；JoySheet deb/AppDir 使用相同布局。Wayland 仍由匹配 `app_id` 的已注册桌面项提供图标。 |

Linux 便携包解压到固定位置后，运行 `python3 install-desktop.py --product joyword`（或 `joydeck`、`joysheet`），把所属 SVG 和启动器注册到 `${XDG_DATA_HOME:-$HOME/.local/share}/`。启动器使用可执行文件的绝对路径，支持中文、空格和特殊字符，不依赖 `PATH`；移动应用后重新运行。JoySheet AppDir 内也有此脚本，deb 由包管理器安装资源。开发构建可在仓库根运行：

```sh
python3 scripts/install_linux_desktop.py --product joyword --binary target/debug/joyword
```

三产品的应用内品牌位置使用内嵌 SVG，所有窗口创建入口复用 `joyoffice_ui::product_window_options`，包括 Deck 示例、编辑器、放映和演示者窗口。资源不依赖启动时的工作目录；CLI 不创建桌面图标。

各包包含所属产品的 `assets/brand/<产品>.svg`。发布检查逐字节核对归档中的 SVG、macOS ICNS 和 Linux 桌面图标，并检查平台图标引用。

2026-11-18 缺失图标的原因、修复和验证边界见[桌面图标验证](validation/20261118-desktop-icons/README.md)。

## 更新图标

修改 SVG 后，在 macOS 运行以下命令，检视 16 px 和 32 px 效果，并一并提交更新后的 SVG、ICO 和 ICNS。脚本默认生成三款产品，传入 `joyword`、`joydeck` 或 `joysheet` 可以只生成一款。

```sh
python3 scripts/generate_product_icons.py
```

常规构建和发布直接使用入库的资源。每次打包都将所选产品的图标计入源码指纹。
