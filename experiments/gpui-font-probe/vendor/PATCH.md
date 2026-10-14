# Vendored gpui 0.2.2 与最小补丁说明

## 来源与完整性

`vendor/gpui-0.2.2/` 是 crates.io 包 `gpui 0.2.2` 的源码副本，取自本机
cargo registry 缓存（与 crates.io 发布的 `.crate` 内容一致）。根
`Cargo.lock` 记录该版本的 registry 校验和：

```
gpui 0.2.2
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "979b45cfa6ec723b6f42330915a1b3769b930d02b2d505f9697f8ca602bee707"
```

副本完整性核对方式（在本目录）：

```sh
find . -type f -not -path './.git/*' -exec shasum -a 256 {} \; | sort -k2 | shasum -a 256
```

入库前已删除与本实验无关且体积较大的 `examples/`、`docs/`、`tests/`
目录；`src/`、`build.rs`、`resources/`、`LICENSE-APACHE`、
`Cargo.toml(.orig)` 未做其他改动。许可证为 Apache-2.0，原样保留。

## 补丁（唯一改动）

`src/text_system.rs` 中：

```diff
-pub struct GlyphId(pub(crate) u32);
+pub struct GlyphId(pub u32);
```

原因：0.2.2 的 `Window::paint_glyph(origin, font_id, glyph_id: GlyphId, …)`
本身已是公开接口，但 `GlyphId` 的内部字段是 `pub(crate)`，仓库外部的
调用方无法从 HarfRust 得到的字形 id 构造 `GlyphId`，因此 0.2.2 原样
无法绘制外部整形结果（ADR-010 待决问题）。该补丁只放开字段可见性，
不改变任何实现逻辑。

## 生效范围

补丁通过本实验 workspace 的 `[patch.crates-io]`（见
`experiments/gpui-font-probe/Cargo.toml`）只对本实验生效；根
workspace 与正式应用的依赖不受影响。若 ADR-010 采纳该路线，正式接入
应以同样的最小 diff 维护一个内部 fork 或推动上游发布。
