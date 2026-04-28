# ygo-card-renderer-rs

Rust 核心库，用于渲染游戏王（Yu-Gi-Oh!）自定义卡片图像。从 YGOPro `.cdb` 数据库或手写卡片数据出发，生成高质量 PNG 卡片图像，支持多种罕贵效果、多语言排版和扩展显示元数据。

---

## 功能特性

- **完整卡片渲染**：普通/效果/融合/同调/超量/连接/摆钟/速攻决斗怪兽，魔法/陷阱卡
- **罕贵效果叠加**：SR · UR · UTR · GR · HR · SeR · GSeR · PSeR · DT 共 10 种，逐像素合成
- **多语言排版**：`sc`（简体）、`tc`（繁体）、`jp`（日文，含振假名）、`en`（英文）
- **文字自适应**：标题/效果文本自动缩放、压缩，多行描述自动换行
- **卡片名渐变/描边**：任意颜色、水平或垂直渐变
- **Out-frame 卡片**：插图延伸到卡框外，支持自定义前景/后景图层
- **RenderDocument 中间层**：JSON 可序列化的渲染指令树，可在渲染前任意编辑节点
- **CLI 工具**：直接从 CDB + 中间图目录批量生图

---

## 目录结构

```
ygo-card-renderer-rs/
├── src/
│   ├── lib.rs              # 公共 API 导出
│   ├── bin/render.rs       # CLI 入口
│   ├── renderer.rs         # Renderer 核心
│   ├── document.rs         # RenderDocument / RenderOp 中间层
│   ├── model.rs            # 数据模型（RenderRequest、YgoCardMeta 等）
│   ├── asset_bundle.rs     # 资源包加载
│   ├── card_logic.rs       # 卡片类型判断、布局计算
│   ├── layout.rs           # 语言/类型相关排版参数
│   ├── rare_effect.rs      # 罕贵效果合成
│   ├── ruby.rs             # 振假名标记解析
│   ├── text/               # 文字测量、绘制、多行排版
│   └── ...
├── tests/render.rs         # 集成测试
├── benches/render.rs       # Criterion 性能基准
├── scripts/
│   ├── render_single_card.ps1   # 单张卡片渲染脚本
│   └── render_tuning.ps1        # 排版参数微调脚本
└── resources/
    └── yugioh_bundle.bin   # 预编译资源包（图集、字体、布局配置）
```

---

## 依赖

| crate | 用途 |
|-------|------|
| `tiny-skia` | 2D 像素合成与变换 |
| `cosmic-text` | 跨平台文字渲染（含 RTL/CJK） |
| `resvg` | SVG 光栅化（部分资源） |
| `image` | WebP/PNG 解码与编码 |
| `serde` / `serde_json` | RenderDocument JSON 序列化 |
| `ygopro-cdb-encode-rs` | YGOPro CDB 读取（工作区本地 crate） |
| `ygo-woff2` | WOFF2 字体解压（工作区本地 crate） |

---

## 快速上手

### 前提

1. 工作区目录中存在 `ygopro-cdb-encode-rs` 和 `ygo-woff2` 两个本地 crate。
2. 准备好 `resources/yugioh_bundle.bin` 资源包。
3. 准备好一个 YGOPro 格式的 `.cdb` 数据库文件。

### 编译

```bash
cargo build --release
```

### 作为库使用

```rust
use std::path::PathBuf;
use ygo_card_renderer_rs::{
    CardKind, RenderOptions, RenderRequest, Renderer,
    asset_bundle::init_global_bundle,
    model::YgoCardMeta,
};
use ygopro_cdb_encode_rs::YgoProCdb;

// 1. 加载资源包（全局初始化一次）
let bundle = std::fs::read("resources/yugioh_bundle.bin").unwrap();
init_global_bundle(&bundle).unwrap();

// 2. 从 CDB 读取卡片数据
let cdb = YgoProCdb::from_path("cards.cdb").unwrap();
let entry = cdb.find_by_code(46986414).unwrap().unwrap();

// 3. 构造渲染请求
let request = RenderRequest {
    kind: CardKind::Yugioh,
    card: YgoCardMeta::from_entry(entry),
    options: RenderOptions {
        language: Some("sc".to_string()),
        art_image: Some(PathBuf::from("art/46986414.jpg")),
        ..RenderOptions::default()
    },
};

// 4. 渲染为 PNG
let renderer = Renderer::new();
let png_bytes = renderer.render_png(&request).unwrap();
std::fs::write("output.png", png_bytes).unwrap();
```

---

## CLI 工具

编译后的可执行文件为 `render`（`cargo build --bin render`）。

### 单张模式

```bash
render \
  --bundle resources/yugioh_bundle.bin \
  --cdb cards.cdb \
  --art-dir /path/to/art \
  --id 46986414 \
  --out output.png \
  --lang sc
```

### 批量模式

读取 CDB 中的全部卡片，并发渲染，输出到目录：

```bash
render \
  --bundle resources/yugioh_bundle.bin \
  --cdb cards.cdb \
  --art-dir /path/to/art \
  --out-dir ./export \
  --lang sc \
  --jobs 8
```

输出文件命名规则：`<code>.png`，如 `46986414.png`。

### 完整参数

| 参数 | 说明 | 默认值 |
|------|------|--------|
| `--bundle <PATH>` | `yugioh_bundle.bin` 路径 | 必填 |
| `--cdb <PATH>` | YGOPro `.cdb` 数据库路径 | 必填 |
| `--art-dir <DIR>` | 中间图目录，查找 `<code>.jpg/.png/.webp` | 必填 |
| `--out-dir <DIR>` | 批量输出目录 | 与 `--id` 二选一 |
| `--id <CODE>` | 单张卡片数字编码 | 与 `--out-dir` 二选一 |
| `--out <FILE>` | 单张输出 PNG 路径 | `<code>.png` |
| `--lang <LANG>` | 语言：`sc` `tc` `jp` `en` | `sc` |
| `--scale <F>` | 输出缩放倍数 | `1.0` |
| `--jobs <N>` | 批量模式并行线程数 | 逻辑 CPU 数 |

> 中间图目录中找不到对应编码的图片时，卡片图框渲染为空白，不报错。

---

## PowerShell 辅助脚本

### 单张渲染

```powershell
.\scripts\render_single_card.ps1 `
    -CardCode 46986414 `
    -Language sc `
    -ArtImage "D:\art\46986414.jpg"
```

### 排版微调

编辑 `scripts/render_tuning.ps1` 内的环境变量后直接运行：

```powershell
pwsh -ExecutionPolicy Bypass -File .\scripts\render_tuning.ps1
```

支持微调名称、效果、描述的字号、行距、字间距、X/Y 坐标等几十个排版参数，输出到 `export/` 目录供对比。

---

## 核心 API

### `Renderer`

```rust
impl Renderer {
    pub fn new() -> Self;

    /// 一步渲染为 PNG 字节。
    pub fn render_png(&self, request: &RenderRequest) -> Result<Vec<u8>, RenderError>;

    /// 构建可编辑的中间层文档。
    pub fn build_document(&self, request: &RenderRequest) -> RenderDocument;

    /// 渲染已（可能经过编辑的）中间层文档。
    pub fn render_document(&self, document: &RenderDocument) -> Result<Vec<u8>, RenderError>;
}
```

### `RenderRequest`

```rust
pub struct RenderRequest {
    pub kind: CardKind,       // Yugioh | RushDuel
    pub card: YgoCardMeta,    // CDB 数据 + 显示元数据
    pub options: RenderOptions,
}
```

### `RenderOptions`（常用字段）

```rust
pub struct RenderOptions {
    pub language: Option<String>,                        // "sc" | "tc" | "jp" | "en"
    pub art_image: Option<PathBuf>,                      // 卡片插图路径
    pub foreground_image: Option<PositionedRenderImage>, // 前景叠图
    pub scale: f32,                                      // 输出缩放，默认 1.0
    pub text_colors: TextColorOverrides,                 // 各文字通道颜色覆盖
    pub layout_overrides: LayoutOverrides,               // 精细排版参数覆盖
    // ...
}
```

### `YgoCardMeta`（显示元数据，扩展自 `CardDataEntry`）

| 字段 | 类型 | 说明 |
|------|------|------|
| `rare` | `Option<RareType>` | 罕贵效果叠加（SR/UR/UTR/GR/HR/SeR…） |
| `name_color` | `NameColor` | 卡片名颜色（Auto/Dark/Light/Custom） |
| `name_gradient` | `Option<TextGradient>` | 卡片名渐变 |
| `package` | `Option<String>` | 卡包编号 |
| `copyright` | `Option<String>` | 版权文字 |
| `laser` | `Option<String>` | 激光全息标识 |
| `twentieth` / `twenty_fifth` | `bool` | 周年纪念标记 |
| `out_frame` | `bool` | Out-frame 模式 |
| `scale` | `Option<f32>` | 卡片级别缩放覆盖 |

### `RenderDocument`（中间层）

`build_document` 返回一个包含 `Vec<RenderNode>` 的 JSON 可序列化结构。每个节点有 `id`、`z`（层叠顺序）、`visible` 和一个 `RenderOp`。可在渲染前任意修改：

```rust
let mut doc = renderer.build_document(&request);

// 隐藏标题
doc.nodes.iter_mut()
    .find(|n| n.id == "title")
    .map(|n| n.visible = false);

// 自定义效果
doc.nodes.push(RenderNode {
    id: "my_effect".to_string(),
    z: 100,
    visible: true,
    op: RenderOp::VisualEffect {
        target: EffectTarget::Art,
        effect: EffectStyle::RainbowFoil { opacity: 0.8 },
    },
});

let png = renderer.render_document(&doc).unwrap();
```

### 罕贵效果（`RareType`）

| 值 | 效果 |
|----|------|
| `Sr` | 超级罕贵：Art 彩虹箔 |
| `Ur` | 究极罕贵：Art 彩虹箔 + 属性/星级全息 + 标题渐变 |
| `Utr` | 终极罕贵：Art 浮雕刻纹 + 卡面磨砂箔 + 同心圆刻纹 |
| `Gr` | 黄金罕贵：卡框/图框金洗 + 标题金色渐变 |
| `Hr` | 全息罕贵：全卡全息 |
| `Ser` | 秘密罕贵：Art/属性/星级秘密织纹 + 标题渐变 |
| `Gser` | 黄金秘密罕贵：同 SeR + 金色调 |
| `Pser` | 棱镜秘密罕贵：Art 棱镜秘密织纹 |
| `PserPrint` | 棱镜秘密罕贵（印刷版） |
| `Dt` | 决斗终端平行罕贵：全卡点阵网 |

---

## 测试

```bash
# 单元 + 集成测试（需要 resources/yugioh_bundle.bin）
cargo test

# 从 CDB 渲染单张卡片（需要 cards.cdb，输出到 export/）
YGO_ART_DIR=/path/to/art cargo test render_single_card_from_cdb -- --nocapture

# 渲染所有罕贵效果预览
YGO_ART_DIR=/path/to/art cargo test render_rare_effects -- --nocapture

# 性能基准
YGO_CDB=cards.cdb YGO_ART_DIR=/path/to/art cargo bench
```

---

## 许可证

MIT
