# ygo-card-renderer-rs

Rust 游戏王（Yu-Gi-Oh!）卡片渲染核心库与 CLI。项目可以从 YGOPro `.cdb` 数据库和卡图目录生成 PNG 卡片图像，支持多语言排版、多种罕贵效果、可编辑的 `RenderDocument` 中间层，以及纯 Rust 资源打包。

---

## 功能特性

- **完整卡片类型**：普通/效果/融合/同调/超量/连接/摆钟怪兽，魔法/陷阱卡，Rush Duel 布局。
- **罕贵效果**：SR、UR、UTR、GR、HR、SeR、GSeR、PSeR、PSeR Print、SCR、ESR、NPR、UPR、SEPR、DT——全部程序化生成（rainbow foil、dot grid、holographic、secret weave、optical SER/SCR、diamond foil、gold wash、frosted foil、engrave/relief、bright border 等）。
- **神经网络 mask**：可用 TinyMaskNet ONNX 模型为卡图生成特效保护 mask（需 `onnx-mask` feature）。
- **多语言排版**：`sc`、`tc`、`jp`、`kr`、`en`、`astral`、`custom1`、`custom2`。
- **文本自适应**：标题/类型/效果/描述自动测量、缩放、压缩和换行；日文支持振假名（ruby/furigana）。
- **丰富的文本样式**：每个文本通道可单独配置纯色、水平/垂直渐变、阴影颜色和渐变（`TextPaint`/`TextGradient`/`TextColorOverrides`）。
- **Out-frame 与扩展显示**：支持卡图外溢、前景图层、周年标记（20th/25th）、激光标识、卡包编号、版权行等。
- **卡图控制**：支持 `ImageFit`（stretch/cover/contain）、`ImageAlign`、`ImageCrop`、缩放和偏移。
- **`RenderDocument` 中间层**：可先构建 JSON 可序列化的渲染指令树，再编辑节点后渲染。
- **纯 Rust 资源打包**：`build_bundle` CLI 可打包图片、SVG、字体、布局元数据，不再需要 Python。
- **特效 mask 生成 CLI**：`generate_mask` CLI 可从单张/目录卡图批量生成保护 mask（需 `onnx-mask`）。
- **运行时优化**：bundle 支持 mmap 载入；图片按需解码缓存；字体按 family 懒加载。

---

## 目录结构

```text
ygo-card-renderer-rs/
├── src/
│   ├── asset_bundle.rs         # bundle 读取、mmap、资源解码/cache
│   ├── bundle_layout.rs        # 从 bundle layout payload 构建 LayoutStyle
│   ├── card_logic.rs           # 卡型、框图、属性、文本规则
│   ├── facts.rs                # CardFacts：一次性计算的卡片类型事实
│   ├── document.rs             # RenderDocument / RenderOp 中间层编排
│   ├── document/
│   │   ├── layers/             # 按层构建：frame、text、footer
│   │   ├── paint.rs            # 文本 paint / 颜色解析
│   │   └── rare.rs             # 稀有度 preset → 效果节点展开
│   ├── layout.rs               # LayoutStyle / LayoutOverrides 合并
│   ├── model.rs                # RenderRequest、YgoCardMeta、RenderOptions 等数据模型
│   ├── pixel_ops.rs            # 像素混合/颜色/hash 工具
│   ├── ruby.rs                 # ruby markup 解析器
│   ├── mask_generator.rs       # ONNX mask 生成核心（feature gated）
│   ├── renderer/               # 渲染执行层
│   │   ├── mod.rs              # Renderer、节点分发、PNG 编码
│   │   ├── draw_card.rs        # 外部图片加载与绘制
│   │   ├── color.rs            # 颜色解析、文本 brush、渐变
│   │   ├── effect_areas.rs     # 特效区域、保护 mask、像素恢复
│   │   └── visual_effects.rs   # gold wash、frosted foil、engrave/relief
│   ├── text/                   # 文本测量、绘制、字体懒加载
│   ├── rare_effect/            # 稀有度像素算法
│   │   ├── rainbow_foil.rs     # 彩虹渐变膜
│   │   ├── dot_grid.rs         # 点阵彩虹膜
│   │   ├── holographic.rs      # 全息效果
│   │   ├── secret.rs           # Secret Rare 纹理
│   │   ├── optical.rs          # SER/SCR 光学模型
│   │   ├── diamond_foil.rs     # 钻石膜
│   │   ├── bright_border.rs    # 亮边效果
│   │   └── math.rs             # smoothstep、hash、noise 等工具
│   └── bin/
│       ├── render.rs           # 从 CDB + 卡图目录渲染 PNG
│       ├── build_bundle.rs     # 纯 Rust 资源打包 CLI
│       └── generate_mask.rs    # ONNX 特效保护 mask 生成 CLI
├── assets/yugioh/
│   ├── image/                  # WebP/SVG 源图资源 + filelist.csv
│   └── font/                   # 字体资源 + filelist.csv
├── resources/
│   └── yugioh_bundle.bin       # 打包后的资源包（通常不提交）
├── model/                      # ONNX mask 模型与元数据
│   ├── ygo-mask-medium-640.onnx
│   └── ygo-mask-medium-640.json
├── tests/render.rs             # 集成测试
├── benches/render.rs           # Criterion benchmark
└── scripts/                    # 辅助/调参脚本；打包优先使用 Rust CLI
```

---

## 依赖与前提

- Rust 2024 edition。
- 工作区同级目录需要存在本地 crate：`../ygo-woff2`、`../ygopro-cdb-encode-rs`。
- 渲染 CLI 需要：`resources/yugioh_bundle.bin`、YGOPro `.cdb`、卡图目录。
- 卡图目录按 `<code>.jpg` → `<code>.png` → `<code>.webp` 优先级查找。

主要依赖：`tiny-skia`、`image`、`resvg/usvg`、`cosmic-text/fontdb`、`memmap2`、`serde/serde_json`、`ygo-woff2`、`ygopro-cdb-encode-rs`。可选 `onnx-mask` feature 会启用 `ort` 运行 ONNX mask 模型。

---

## 快速开始

### 1. 构建资源包

推荐使用纯 Rust CLI：

```bash
cargo run --bin build_bundle
```

默认读取 `assets/yugioh/`，写出 `resources/yugioh_bundle.bin`。小 WebP 会合并进 atlas，大 WebP 直接打入 payload，SVG 会在构建期用 `resvg` 栅格化为 lossless WebP，字体直接打入 payload。

自定义参数：

```bash
cargo run --bin build_bundle -- \
  --root assets/yugioh \
  --out resources/yugioh_bundle.bin \
  --atlas-width 2048 \
  --max-sprite-dim 320 \
  --max-sprite-area 100000
```

| 参数 | 说明 | 默认值 |
|---|---|---|
| `--root <DIR>` | 资源根目录，内部需有 `image/` 和 `font/` | `<repo>/assets/yugioh` |
| `--out <FILE>` | 输出 bundle 路径 | `<repo>/resources/yugioh_bundle.bin` |
| `--atlas-width <N>` | 小图 atlas 宽度 | `2048` |
| `--max-sprite-dim <N>` | 进入 atlas 的最大宽/高 | `320` |
| `--max-sprite-area <N>` | 进入 atlas 的最大像素面积 | `100000` |

### 2. 编译

```bash
cargo build --release
```

### 3. 渲染单张卡

```bash
cargo run --bin render -- \
  --bundle resources/yugioh_bundle.bin \
  --cdb cards.cdb \
  --art-dir /path/to/art \
  --id 46986414 \
  --out output.png \
  --lang sc
```

### 4. 批量渲染

```bash
cargo run --bin render -- \
  --bundle resources/yugioh_bundle.bin \
  --cdb cards.cdb \
  --art-dir /path/to/art \
  --out-dir ./export \
  --lang sc \
  --jobs 8
```

输出文件名为 `<code>.png`。

### 5. 生成特效保护 mask（可选）

仓库内置 TinyMaskNet 模型（`model/` 目录）：

```text
model/ygo-mask-medium-640.onnx
model/ygo-mask-medium-640.json
```

单张生成：

```bash
cargo run --features onnx-mask --bin generate_mask -- \
  --model model/ygo-mask-medium-640.onnx \
  --art /path/to/art/65741786.jpg \
  --out export/masks/65741786.png
```

批量生成：

```bash
cargo run --features onnx-mask --bin generate_mask -- \
  --model model/ygo-mask-medium-640.onnx \
  --art-dir /path/to/art \
  --out-dir export/masks
```

mask 语义：黑色保护主体不覆特效，白色允许特效。默认阈值和主体膨胀来自同名 `.json` 元数据，可用 `--threshold` / `--dilate` 覆盖。

`onnx-mask` feature 会通过 `ort` 使用 ONNX Runtime；首次构建/部署时需确保对应运行时库可用。

---

## 资源打包与 filelist

`build_bundle` 支持资源名与真实文件路径分离。每个目录会按以下顺序查找 filelist：

```text
filelist.json
filelist.csv
filelist.tsv
filelist
```

- `assets/yugioh/image/filelist.*`：图片资源映射，path 相对 `image/`。
- `assets/yugioh/font/filelist.*`：字体资源映射，path 相对 `font/`。
- 如果没有 filelist，则回退到扫描当前目录下的 `*.webp` / `*.svg` / 字体文件。

CSV 示例：

```csv
name,path
card-normal.webp,frames/card-normal.webp
copyright-en-black.svg,copyright/en-black.svg
ygo-sc,ygo-sc.woff2
```

JSON 示例：

```json
[
  { "name": "card-normal.webp", "path": "frames/card-normal.webp" },
  ["copyright-en-black.svg", "copyright/en-black.svg"]
]
```

规则：

- `name` 是 bundle index 里的资源名，也是渲染代码查找的 key。
- `path` 是真实文件相对路径。
- image 支持 `.webp`、`.svg`；font 支持 `.woff2`、`.woff`、`.ttf`、`.otf`。
- filelist 顺序就是打包顺序；无 filelist 时按路径排序。
- 重复资源名、缺失文件、不支持扩展名会直接报错。

---

## render CLI 参数

```text
render --bundle <PATH> --cdb <PATH> --art-dir <DIR> --out-dir <DIR> [OPTIONS]
render --bundle <PATH> --cdb <PATH> --art-dir <DIR> --id <CODE> --out <FILE> [OPTIONS]
```

| 参数 | 说明 | 默认值 |
|---|---|---|
| `--bundle <PATH>` | `yugioh_bundle.bin` 路径 | 必填 |
| `--cdb <PATH>` | YGOPro `.cdb` 数据库 | 必填 |
| `--art-dir <DIR>` | 卡图目录，查找 `<code>.jpg/.png/.webp` | 必填 |
| `--out-dir <DIR>` | 批量输出目录 | 与 `--id` 二选一 |
| `--id <CODE>` | 单张卡片 code | 与 `--out-dir` 二选一 |
| `--out <FILE>` | 单张输出文件 | `<code>.png` |
| `--lang <LANG>` | `sc`、`tc`、`jp`、`kr`、`en` 等 | `sc` |
| `--scale <F>` | 输出缩放倍率 | `1.0` |
| `--effect-mask <PATH>` | 黑白特效遮罩：黑色保护不覆特效，白色允许特效 | 无 |
| `--effect-mask-dir <DIR>` | 按 `<dir>/<code>.png` 查找每张卡的特效遮罩 | 无 |
| `--auto-mask-model <ONNX>` | 缺失 mask 时自动生成；需 `--features onnx-mask` | 无 |
| `--auto-mask-metadata <JSON>` | 自动 mask 元数据；默认使用模型同名 `.json` | 无 |
| `--mask-cache-dir <DIR>` | 自动生成 mask 的写入目录；默认使用 `--effect-mask-dir` | 无 |
| `--mask-threshold <F>` | 覆盖自动 mask 主体阈值 | 元数据推荐值 |
| `--mask-dilate <PX>` | 覆盖自动 mask 主体膨胀像素 | 元数据推荐值 |
| `--overwrite-mask` | 自动 mask cache 已存在时也重新生成；不会覆盖 `--effect-mask-dir` 已命中的 mask | false |
| `--jobs <N>` | 批量渲染线程数 | 逻辑 CPU 数 |

如果 `art-dir` 中找不到对应卡图，卡图区域会留空，不会中断渲染。
`--effect-mask` 可使用完整卡片尺寸遮罩，或与卡图区域同尺寸的遮罩；未指定坐标时后者会自动贴到卡图区域。

mask 优先级：`--effect-mask` 最高，指定后不会自动生成；批量渲染时 `--effect-mask-dir` 先查已有 `{code}.png`，若同时指定 `--auto-mask-model`，只有缺失的卡才会自动生成并写入 cache。若某张卡没有 art，自动 mask 会输出 warning 并跳过该卡的 mask，渲染继续。

示例：批量渲染时自动补齐缺失 mask：

```bash
cargo run --features onnx-mask --bin render -- \
  --bundle resources/yugioh_bundle.bin \
  --cdb cards.cdb \
  --art-dir /path/to/art \
  --out-dir ./export \
  --effect-mask-dir export/masks \
  --auto-mask-model model/ygo-mask-medium-640.onnx
```

---

## 作为库使用

```rust
use std::path::PathBuf;
use ygo_card_renderer_rs::{
    CardKind, RenderOptions, RenderRequest, Renderer,
    asset_bundle::init_global_bundle_from_file,
    model::YgoCardMeta,
};
use ygopro_cdb_encode_rs::YgoProCdb;

// 全局初始化资源包。文件路径初始化会使用 mmap。
init_global_bundle_from_file("resources/yugioh_bundle.bin").unwrap();

let cdb = YgoProCdb::from_path("cards.cdb").unwrap();
let entry = cdb
    .find_all()
    .unwrap()
    .into_iter()
    .find(|card| card.code == 46986414)
    .unwrap();

let mut card = YgoCardMeta::from_entry(entry);
card.rare = Some(ygo_card_renderer_rs::model::RareType::Ser);
card.package = Some("SD25-JP001".to_string());
card.copyright = Some("©スタジオ・ダイス／集英社・テレビ東京・KONAMI".to_string());

let request = RenderRequest {
    kind: CardKind::Yugioh,
    card,
    options: RenderOptions {
        language: Some("sc".to_string()),
        art_image: Some(PathBuf::from("art/46986414.jpg")),
        scale: 1.0,
        ..RenderOptions::default()
    },
};

let renderer = Renderer::new();
let png = renderer.render_png(&request).unwrap();
std::fs::write("output.png", png).unwrap();
```

如果调用方已经把 bundle bytes 嵌入或读入内存，也可以使用 `asset_bundle::init_global_bundle(&bytes)`。

---

## 核心 API

```rust
impl Renderer {
    pub fn new() -> Self;
    pub fn render_png(&self, request: &RenderRequest) -> Result<Vec<u8>, RenderError>;
    pub fn build_document(&self, request: &RenderRequest) -> RenderDocument;
    pub fn render_document(&self, document: &RenderDocument) -> Result<Vec<u8>, RenderError>;
}

pub struct RenderRequest {
    pub kind: CardKind,
    pub card: YgoCardMeta,
    pub options: RenderOptions,
}
```

`Renderer::build_document` 会返回可编辑的中间层。常见用途：隐藏节点、修改 z-index、插入额外视觉效果、序列化为 JSON 后交给上层编辑器。

```rust
let mut doc = renderer.build_document(&request);

if let Some(title) = doc.nodes.iter_mut().find(|node| node.id == "title") {
    title.visible = false;
}

let png = renderer.render_document(&doc).unwrap();
```

---

## 关键类型概览

### RareType（罕贵度）

```rust
pub enum RareType {
    Sr,       // Super Rare
    Hr,       // Holographic Rare
    Gr,       // Gold Rare
    Ur,       // Ultra Rare
    Utr,      // Ultimate Rare
    Ser,      // Secret Rare
    Gser,     // Gold Secret Rare
    Pser,     // Prismatic Secret Rare
    PserPrint,// Prismatic Secret Rare (print)
    Scr,      // Secret Collector's Rare
    Esr,      // Extra Secret Rare
    Npr,      // Normal Parallel Rare
    Upr,      // Ultimate Parallel Rare
    Sepr,     // Secret Extra Parallel Rare
    Dt,       // Duel Terminal parallel rare
}
```

### YgoCardMeta（卡片元数据）

除 CDB 基础字段外，`YgoCardMeta` 支持：

| 字段 | 说明 |
|---|---|
| `rare: Option<RareType>` | 罕贵度 |
| `name_color: NameColor` | 标题颜色（Auto/Dark/Light/Custom） |
| `name_gradient: Option<TextGradient>` | 标题渐变 |
| `name_shadow_color` / `name_shadow_gradient` | 标题阴影 |
| `package: Option<String>` | 卡包编号 |
| `copyright: Option<String>` | 版权行 |
| `laser: Option<String>` | 激光标识 |
| `twentieth` / `twenty_fifth` | 20th/25th 周年标记 |
| `out_frame: bool` | 是否启用 out-frame |
| `out_frame_image` | out-frame 前景图 |
| `out_frame_effect_enabled` / `out_frame_effect_box` | out-frame 效果框 |
| `out_frame_name_block_enabled` | out-frame 名称块 |
| `monster_type: Option<String>` | 自定义怪兽种族/类型行 |
| `scale: Option<f32>` | 单卡输出缩放 |

### RenderOptions（渲染选项）

| 字段 | 说明 |
|---|---|
| `language` | 语言 |
| `art_image` / `art_fit` / `art_align` / `art_crop` | 卡图及其布局控制 |
| `art_scale` / `art_offset_x` / `art_offset_y` | 卡图缩放与偏移 |
| `effect_mask` | 特效保护 mask |
| `foreground_image` | 前景叠加图 |
| `scale` | 整体输出缩放 |
| `text_colors: TextColorOverrides` | 各文本通道的颜色/渐变/阴影覆盖 |
| `font` | 自定义字体 |
| `align` / `description_align` | 文本对齐 |
| `description_zoom` | 描述文本缩放 |
| `description_weight` | 描述文本字重 |
| `description_first_line_compress` | 描述首行压缩 |
| `title_width_compress` | 标题宽度压缩 |
| `layout_overrides: LayoutOverrides` | 精确布局参数覆盖 |

---

## 资源包运行时行为

- CLI 通过 `init_global_bundle_from_file` mmap 资源包，避免启动时复制完整 payload。
- 初始化时校验 magic、version、buffer offset/len、atlas rect 等。
- atlas 在初始化时解码，单个资源图片按需解码并通过 per-asset `OnceLock` 缓存。
- 字体不会全部预载；文本引擎按 family 首次使用时从 bundle 解码 WOFF2/TTF 并加载。
- 构建期 SVG 已经栅格化为 lossless WebP；读取端仍保留 SVG 解码兼容路径。

---

## 测试与验证

```bash
cargo check
cargo check --bin build_bundle
cargo run --bin build_bundle -- --out resources/yugioh_bundle.bin
cargo test

YGO_ART_DIR=/path/to/art cargo test render_single_card_from_cdb -- --nocapture
YGO_ART_DIR=/path/to/art cargo test render_rare_effects -- --nocapture

YGO_BUNDLE=resources/yugioh_bundle.bin \
YGO_CDB=cards.cdb \
YGO_ART_DIR=/path/to/art \
cargo bench
```

Windows/MSVC 目标运行 `cargo test` 需要可用的 Visual Studio C++ Build Tools（`link.exe`）。

---

## 辅助脚本

`scripts/` 中保留一些调参/调测工具：

- `scripts/tune_ser.py`：SeR 效果调参预览，需要 Python + Pillow + NumPy。
- `scripts/render_single_card.ps1`：PowerShell 单张渲染辅助（设置 `YGO_*` 环境变量后调用测试）。
- `scripts/render_tuning.ps1`：PowerShell 排版调参辅助。
- `scripts/build_bundle.py`：旧 Python 打包脚本；新流程优先使用 `cargo run --bin build_bundle`。

---

## 许可证

MIT
