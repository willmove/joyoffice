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
| Linux | 每个 GPUI 窗口从内嵌 ICO 解码 RGBA，向 X11 设置 `_NET_WM_ICON`。任务栏可能优先使用匹配的 `.desktop`，即使从终端直接运行编译产物也是如此。用户安装器和 JoySheet deb 的启动器直接引用安装后的 SVG 绝对路径，避免主题名称查询失败掩盖窗口图标。AppDir 保留可移位布局，使用注册脚本生成绝对引用。Wayland 由匹配 `app_id` 的已注册桌面项提供图标。 |

Linux 便携包解压到固定位置后，运行 `python3 install-desktop.py --product joyword`（或 `joydeck`、`joysheet`），把所属 SVG 和启动器注册到 `${XDG_DATA_HOME:-$HOME/.local/share}/`。启动器使用可执行文件和图标的绝对路径，支持中文、空格和特殊字符，不依赖 `PATH`；可执行文件保留符号链接的调用名，支持按名称分派的包装脚本。移动应用后重新运行。JoySheet AppDir 内也有此脚本，deb 由包管理器安装资源。开发构建可在仓库根运行：

```sh
python3 scripts/install_linux_desktop.py --product joyword --binary target/debug/joyword
```

已有自定义启动器（例如 glibc 兼容包装器）只需修复图标时，运行 `python3 scripts/install_linux_desktop.py --product joyword --refresh-icon`，另两产品替换产品名；它保留现有 Exec、文件关联、本地化名称和桌面动作。脚本在 DDE 应用管理器已运行时通知其重读注册项，无需重启桌面。关闭并重新打开已有产品窗口，使任务栏重新读取引用。

三产品的应用内品牌位置使用内嵌 SVG，所有窗口创建入口复用 `joyoffice_ui::product_window_options`，包括 Deck 示例、编辑器、放映和演示者窗口。资源不依赖启动时的工作目录；CLI 不创建桌面图标。

各包包含所属产品的 `assets/brand/<产品>.svg`。发布检查逐字节核对归档中的 SVG、macOS ICNS 和 Linux 桌面图标，并检查平台图标引用。

2026-11-18 缺失图标的原因、修复和验证边界见[桌面图标验证](validation/20261118-desktop-icons/README.md)。
同日用户复测发现的已注册启动器覆盖窗口图标问题、前轮验证遗漏及补充修复见 [UOS 任务栏复验](validation/20261118-uos-registered-icons/README.md)。

## 更新图标

修改 SVG 后，在 macOS 运行以下命令，检视 16 px 和 32 px 效果，并一并提交更新后的 SVG、ICO 和 ICNS。脚本默认生成三款产品，传入 `joyword`、`joydeck` 或 `joysheet` 可以只生成一款。

```sh
python3 scripts/generate_product_icons.py
```

常规构建和发布直接使用入库的资源。每次打包都将所选产品的图标计入源码指纹。
