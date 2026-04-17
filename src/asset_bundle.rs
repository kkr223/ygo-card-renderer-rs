use image::ImageFormat;
use serde::Deserialize;
use std::collections::HashMap;
use tiny_skia::{Pixmap, Transform};

#[derive(Debug, Deserialize, Clone)]
pub struct BufferPointer {
    pub offset: u32,
    pub len: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SpriteInfo {
    pub atlas: [u32; 4],  // u, v, w, h
    pub layout: [f32; 2], // x, y (some old coordinates could be negative, float is safer)
}

#[derive(Debug, Deserialize, Clone)]
pub struct AtlasMeta {
    pub buffer: BufferPointer,
    pub sprites: HashMap<String, SpriteInfo>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FrameMeta {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub buffer: BufferPointer,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FramesMeta {
    pub normal: HashMap<String, FrameMeta>,
    pub pendulum: HashMap<String, FrameMeta>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FontMeta {
    pub buffer: BufferPointer,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PositionDetail {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MetaSection {
    pub version: String,
    pub language: String,
    pub star_level_pos: Option<HashMap<String, PositionDetail>>,
    pub star_rank_pos: Option<HashMap<String, PositionDetail>>,
}

/// 从 PSD 文字图层提取的单个区域布局参数。
/// 所有坐标单位与 PSD 画布像素一致（卡片基准尺寸 1394×2031）。
#[derive(Debug, Deserialize, Clone, Default)]
pub struct TextLayoutEntry {
    /// 文字框左边界 x（对应 layout.rs 的 *_x 参数）
    pub x: Option<f32>,
    /// 文字框顶部 y（对应 layout.rs 的 *_top 参数）
    pub y: Option<f32>,
    /// 文字框宽度（对应 layout.rs 的 body_max_width / title_max_width_* 参数）
    pub width: Option<f32>,
    /// 文字框高度
    pub height: Option<f32>,
    /// PSD 中字号（PostScript pt）。注意 PSD 72pt = 屏幕 96px，需乘以 96/72 = 4/3。
    pub font_size_pt: Option<f32>,
    /// PSD Tracking（千分之一字宽的字间距，正=加宽，负=收紧）
    pub tracking: Option<i32>,
    /// PSD Leading（行距 pt，绝对值）
    pub leading_pt: Option<f32>,
    /// Leading / FontSize，即行高倍数（对应 layout.rs 的 *_line_height 参数）
    pub line_height_ratio: Option<f32>,
}

/// assets.json 中 text_layout 字段的完整结构。
/// key 与 normalize_assets.py 中写入的一致：
///   "card_name" / "effect_text" / "description_text" / "type_line"
#[derive(Debug, Deserialize, Clone, Default)]
pub struct TextLayout {
    pub card_name: Option<TextLayoutEntry>,
    pub effect_text: Option<TextLayoutEntry>,
    pub description_text: Option<TextLayoutEntry>,
    pub type_line: Option<TextLayoutEntry>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AssetsIndex {
    pub meta: MetaSection,
    pub atlas: AtlasMeta,
    pub frames: FramesMeta,
    pub rare: HashMap<String, FrameMeta>,
    pub fonts: HashMap<String, FontMeta>,
    #[serde(default)]
    pub text_layout: TextLayout,
}

pub struct AssetBundle {
    pub index: AssetsIndex,
    payload: Vec<u8>,
    atlas_pixmap: Option<Pixmap>,
}

impl AssetBundle {
    pub fn load_from_bytes(data: &[u8]) -> Result<Self, String> {
        if data.len() < 12 {
            return Err("Invalid bundle size".into());
        }

        let magic = &data[0..4];
        if magic != b"YGOA" {
            return Err("Invalid magic header".into());
        }

        // let version = u32::from_le_bytes(data[4..8].try_into().unwrap());
        let json_len = u32::from_le_bytes(data[8..12].try_into().unwrap()) as usize;

        if data.len() < 12 + json_len {
            return Err("Bundle truncated".into());
        }

        let json_bytes = &data[12..12 + json_len];
        let index: AssetsIndex = serde_json::from_slice(json_bytes)
            .map_err(|e| format!("Failed to parse index JSON: {}", e))?;

        let payload_start = 12 + json_len;
        let payload = data[payload_start..].to_vec();

        let mut bundle = Self {
            index,
            payload,
            atlas_pixmap: None,
        };

        // Pre-decode atlas since it's frequently used
        let atlas_bytes = bundle.get_bytes(&bundle.index.atlas.buffer)?;
        bundle.atlas_pixmap = Some(decode_webp(atlas_bytes)?);

        Ok(bundle)
    }

    pub fn get_bytes(&self, ptr: &BufferPointer) -> Result<&[u8], String> {
        let start = ptr.offset as usize;
        let end = start + ptr.len as usize;
        if end > self.payload.len() {
            return Err("Buffer pointer out of bounds".into());
        }
        Ok(&self.payload[start..end])
    }

    pub fn decode_frame(&self, ptr: &BufferPointer) -> Result<Pixmap, String> {
        let bytes = self.get_bytes(ptr)?;
        decode_webp(bytes)
    }

    pub fn draw_sprite(&self, target: &mut Pixmap, sprite_name: &str, dx: f32, dy: f32) {
        if let Some(atlas) = &self.atlas_pixmap {
            if let Some(info) = self.index.atlas.sprites.get(sprite_name) {
                let [u, v, w, h] = info.atlas;
                let rect = tiny_skia::IntRect::from_xywh(u as i32, v as i32, w, h);
                if let Some(rect) = rect {
                    if let Some(cropped) = atlas.clone_rect(rect) {
                        target.draw_pixmap(
                            (info.layout[0] + dx) as i32,
                            (info.layout[1] + dy) as i32,
                            cropped.as_ref(),
                            &tiny_skia::PixmapPaint::default(),
                            Transform::default(),
                            None,
                        );
                    }
                }
            }
        }
    }

    pub fn draw_sprite_at(&self, target: &mut Pixmap, sprite_name: &str, x: f32, y: f32) {
        if let Some(atlas) = &self.atlas_pixmap {
            if let Some(info) = self.index.atlas.sprites.get(sprite_name) {
                let [u, v, w, h] = info.atlas;
                let rect = tiny_skia::IntRect::from_xywh(u as i32, v as i32, w, h);
                if let Some(rect) = rect {
                    if let Some(cropped) = atlas.clone_rect(rect) {
                        target.draw_pixmap(
                            x as i32,
                            y as i32,
                            cropped.as_ref(),
                            &tiny_skia::PixmapPaint::default(),
                            Transform::default(),
                            None,
                        );
                    }
                }
            }
        }
    }
}

// Global singleton for convenience, but you can also pass it around.
use std::sync::OnceLock;

static BUNDLE: OnceLock<AssetBundle> = OnceLock::new();

pub fn init_global_bundle(data: &[u8]) -> Result<(), String> {
    let bundle = AssetBundle::load_from_bytes(data)?;
    BUNDLE
        .set(bundle)
        .map_err(|_| "Bundle already initialized".to_string())?;
    Ok(())
}

pub fn get_bundle() -> &'static AssetBundle {
    BUNDLE.get().expect("AssetBundle not initialized")
}

pub fn decode_webp(bytes: &[u8]) -> Result<Pixmap, String> {
    let img = image::load_from_memory_with_format(bytes, ImageFormat::WebP)
        .map_err(|e| format!("Image decode error: {}", e))?;
    let rgba = img.into_rgba8();
    let width = rgba.width();
    let height = rgba.height();

    // Image crate returns unpremultiplied RGBA. tiny_skia expects premultiplied RGBA.
    // However, tiny_skia::Pixmap::from_vec parses as-is. We need to handle premultiplication if required,
    // but WebP lossless alpha is generally okay. We'll use tiny_skia's premultiply logic to be safe.
    let mut pixmap = Pixmap::from_vec(
        rgba.into_raw(),
        tiny_skia::IntSize::from_wh(width, height).unwrap(),
    )
    .ok_or_else(|| "Failed to create Pixmap".to_string())?;

    // Premultiply alpha manually just in case
    // tiny_skia::Pixmap doesn't have an in-place premultiply public fn, but its from_vec assumes
    // it's already premultiplied. Actually image crate RGBA8 is NOT premultiplied.
    let pixels = pixmap.pixels_mut();
    for pixel in pixels.iter_mut() {
        let color = pixel.demultiply();
        *pixel = tiny_skia::PremultipliedColorU8::from_rgba(
            color.red(),
            color.green(),
            color.blue(),
            color.alpha(),
        )
        .unwrap_or(tiny_skia::PremultipliedColorU8::TRANSPARENT);
    }

    Ok(pixmap)
}
