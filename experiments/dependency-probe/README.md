# JoyDeck 依赖兼容实验

独立实验 workspace，不进入桌面应用或 CLI 依赖。精确固定 JoyOffice 的 harfrust 0.8.4、skrifa 0.42.1、官方 crates.io Krilla 0.8.2、ZIP 9.0.0，以及新增 Vello CPU/common 0.2.0。

传入自行提供的字体及 face index：

```sh
cargo run --locked --manifest-path experiments/dependency-probe/Cargo.toml -- FONT_FILE FACE_INDEX
```

实验读取同一字体 face、整形中英文、验证 Krilla 接受字体并生成 PDF、执行 ZIP 往返、核对 CPU 矩形像素。失败返回非零。系统字体不随仓库分发。本实验不等于 GPUI 字体身份映射、完整幻灯片/PDF 保真或性能验收。
