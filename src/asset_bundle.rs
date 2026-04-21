use image::ImageFormat;
use resvg::{self, usvg};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;
use tiny_skia::{Pixmap, Transform};

#[derive(Debug, Deserialize, Clone)]
pub struct BufferPointer {
    pub offset: u32,
    pub len: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AtlasSprite {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AtlasMeta {
    pub buffer: Option<BufferPointer>,
    pub width: u32,
    pub height: u32,
    pub sprites: HashMap<String, AtlasSprite>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ImageSize {
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ImageEntry {
    pub kind: String,
    pub storage: String,
    pub atlas: Option<AtlasSprite>,
    pub size: Option<ImageSize>,
    pub buffer: Option<BufferPointer>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FontMeta {
    pub file: String,
    pub buffer: BufferPointer,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LayoutBufferMeta {
    pub buffer: BufferPointer,
}

#[derive(Debug, Deserialize, Clone)]
pub struct BundleMetaCounts {
    pub sprite_raster: u32,
    pub standalone_raster: u32,
    pub svg: u32,
    pub fonts: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct BundleMeta {
    pub root: String,
    pub version: u32,
    pub atlas_width: u32,
    pub max_sprite_dim: u32,
    pub max_sprite_area: u32,
    pub counts: BundleMetaCounts,
}

#[derive(Debug, Deserialize, Clone)]
pub struct BundleIndex {
    pub meta: BundleMeta,
    pub atlas: AtlasMeta,
    pub layout: LayoutBufferMeta,
    pub images: HashMap<String, ImageEntry>,
    pub fonts: HashMap<String, FontMeta>,
    #[serde(default)]
    pub font_list: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LayoutPayload {
    pub card: CardCanvas,
    pub base: BaseLayout,
    pub styles: HashMap<String, StyleDefinition>,
    pub resource_rules: ResourceRules,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CardCanvas {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct NameBase {
    pub x: u32,
    pub width_with_attribute: u32,
    pub width_without_attribute: u32,
    pub height: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Position {
    pub x: u32,
    pub y: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PositionedAsset {
    pub asset: String,
    pub x: u32,
    pub y: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LevelRankBase {
    pub asset: String,
    pub star_width: u32,
    pub y: u32,
    pub gap: u32,
    pub max: u32,
    pub right_lt_13: Option<u32>,
    pub right_ge_13: Option<u32>,
    pub left_lt_13: Option<u32>,
    pub left_ge_13: Option<u32>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SpellTrapAssetRule {
    pub icon_asset_prefix: String,
    pub icon_asset_suffix: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FrameRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ImageFrames {
    pub normal: FrameRect,
    pub pendulum: FrameRect,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MaskFrames {
    pub normal: PositionedAsset,
    pub pendulum: PositionedAsset,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PendulumScaleSide {
    pub astral: Position,
    pub default: Position,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PendulumScaleLayout {
    pub left: PendulumScaleSide,
    pub right: PendulumScaleSide,
}

#[derive(Debug, Deserialize, Clone)]
pub struct BoxRect {
    pub x: u32,
    pub y: Option<u32>,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PackageVariant {
    pub x: Option<u32>,
    pub y: u32,
    pub right: Option<u32>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PackageLayout {
    pub font_family: String,
    pub font_size: u32,
    pub pendulum: PackageVariant,
    pub default: PackageVariant,
    pub link: PackageVariant,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ArrowState {
    pub asset: String,
    pub x: u32,
    pub y: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ArrowPair {
    pub on: ArrowState,
    pub off: ArrowState,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AtkDefLinkText {
    pub x: u32,
    pub y: u32,
    pub font_size: u32,
    pub scale_x: Option<f32>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LanguageAwareText {
    pub astral: AtkDefLinkText,
    pub default: AtkDefLinkText,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AtkDefLinkLayout {
    pub background: Position,
    pub atk: LanguageAwareText,
    pub def: LanguageAwareText,
    pub link: LanguageAwareText,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DescriptionBase {
    pub x: u32,
    pub width: u32,
    pub base_height: u32,
    pub atk_bar_height: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PasswordLayout {
    pub x: u32,
    pub y: u32,
    pub font_family: String,
    pub font_size: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CopyrightLayout {
    pub right: u32,
    pub y: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct BaseLayout {
    pub name: NameBase,
    pub attribute: Position,
    pub level: LevelRankBase,
    pub rank: LevelRankBase,
    pub spell_trap: SpellTrapAssetRule,
    pub image: ImageFrames,
    pub mask: MaskFrames,
    pub pendulum_scale: PendulumScaleLayout,
    pub pendulum_description: BoxRect,
    pub package: PackageLayout,
    pub link_arrows: HashMap<String, ArrowPair>,
    pub effect: BoxRect,
    pub description: DescriptionBase,
    pub atk_def_link: AtkDefLinkLayout,
    pub password: PasswordLayout,
    pub copyright: CopyrightLayout,
    pub laser: Position,
    pub attribute_rare: PositionedAsset,
    pub twentieth: PositionedAsset,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct TextBlock {
    #[serde(rename = "fontFamily")]
    pub font_family: Option<String>,
    pub top: Option<u32>,
    #[serde(rename = "fontSize")]
    pub font_size: Option<u32>,
    #[serde(rename = "rtFontSize")]
    pub rt_font_size: Option<u32>,
    #[serde(rename = "rtTop")]
    pub rt_top: Option<i32>,
    #[serde(rename = "rtFontScaleX")]
    pub rt_font_scale_x: Option<f32>,
    #[serde(rename = "letterSpacing")]
    pub letter_spacing: Option<f32>,
    #[serde(rename = "wordSpacing")]
    pub word_spacing: Option<f32>,
    #[serde(rename = "lineHeight")]
    pub line_height: Option<f32>,
    #[serde(rename = "scaleY")]
    pub scale_y: Option<f32>,
    pub right: Option<u32>,
    #[serde(rename = "textIndent")]
    pub text_indent: Option<f32>,
    #[serde(rename = "minHeight")]
    pub min_height: Option<u32>,
    #[serde(rename = "smallFontSize")]
    pub small_font_size: Option<u32>,
    pub icon: Option<IconMargins>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct IconMargins {
    #[serde(rename = "marginTop")]
    pub margin_top: Option<f32>,
    #[serde(rename = "marginLeft")]
    pub margin_left: Option<f32>,
    #[serde(rename = "marginRight")]
    pub margin_right: Option<f32>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct StyleDefinition {
    #[serde(rename = "fontFamily")]
    pub font_family: String,
    pub name: TextBlock,
    #[serde(rename = "spellTrap")]
    pub spell_trap: TextBlock,
    #[serde(rename = "pendulumDescription")]
    pub pendulum_description: TextBlock,
    pub effect: TextBlock,
    pub description: TextBlock,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ResourceRules {
    pub base_image: String,
    pub card_asset: String,
    pub pendulum_asset: String,
    pub attribute_asset: String,
    pub spell_trap_attribute_asset: String,
    pub rare_asset: String,
    pub copyright_asset: String,
    pub laser_asset: String,
    pub atk_def_asset: HashMap<String, String>,
    pub atk_link_asset: HashMap<String, String>,
}

pub struct AssetBundle {
    pub index: BundleIndex,
    pub layout: LayoutPayload,
    payload: Vec<u8>,
    atlas_pixmap: Option<Pixmap>,
}

impl AssetBundle {
    pub fn load_from_bytes(data: &[u8]) -> Result<Self, String> {
        if data.len() < 12 {
            return Err("Invalid bundle size".into());
        }

        let magic = &data[0..4];
        if magic != b"YGOC" {
            return Err("Invalid magic header".into());
        }

        let json_len = u32::from_le_bytes(data[8..12].try_into().unwrap()) as usize;
        if data.len() < 12 + json_len {
            return Err("Bundle truncated".into());
        }

        let index: BundleIndex = serde_json::from_slice(&data[12..12 + json_len])
            .map_err(|e| format!("Failed to parse bundle index: {e}"))?;
        let payload = data[12 + json_len..].to_vec();

        let layout_ptr = index.layout.buffer.clone();
        let layout_start = layout_ptr.offset as usize;
        let layout_end = layout_start + layout_ptr.len as usize;
        if layout_end > payload.len() {
            return Err("Layout buffer pointer out of bounds".into());
        }
        let layout = serde_json::from_slice(&payload[layout_start..layout_end])
            .map_err(|e| format!("Failed to parse layout payload: {e}"))?;

        let mut bundle = Self {
            index,
            payload,
            layout,
            atlas_pixmap: None,
        };

        if let Some(buffer) = &bundle.index.atlas.buffer {
            let atlas_bytes = bundle.get_bytes(buffer)?;
            bundle.atlas_pixmap = Some(decode_webp(atlas_bytes)?);
        }

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

    pub fn image(&self, name: &str) -> Result<&ImageEntry, String> {
        self.index
            .images
            .get(name)
            .ok_or_else(|| format!("Missing image asset: {name}"))
    }

    pub fn has_image(&self, name: &str) -> bool {
        self.index.images.contains_key(name)
    }

    pub fn decode_raster(&self, name: &str) -> Result<Pixmap, String> {
        let image = self.image(name)?;
        if image.kind != "raster" {
            return Err(format!("Asset is not raster: {name}"));
        }

        match image.storage.as_str() {
            "buffer" => {
                let ptr = image
                    .buffer
                    .as_ref()
                    .ok_or_else(|| format!("Missing buffer pointer for {name}"))?;
                decode_webp(self.get_bytes(ptr)?)
            }
            "atlas" => {
                let sprite = image
                    .atlas
                    .as_ref()
                    .ok_or_else(|| format!("Missing atlas entry for {name}"))?;
                let atlas = self
                    .atlas_pixmap
                    .as_ref()
                    .ok_or_else(|| "Atlas pixmap not initialized".to_string())?;
                let rect = tiny_skia::IntRect::from_xywh(
                    sprite.x as i32,
                    sprite.y as i32,
                    sprite.w,
                    sprite.h,
                )
                .ok_or_else(|| format!("Invalid atlas rect for {name}"))?;
                atlas
                    .clone_rect(rect)
                    .ok_or_else(|| format!("Failed to crop atlas rect for {name}"))
            }
            other => Err(format!("Unsupported raster storage '{other}' for {name}")),
        }
    }

    pub fn draw_image_at(
        &self,
        target: &mut Pixmap,
        name: &str,
        x: f32,
        y: f32,
    ) -> Result<(), String> {
        let entry = self.image(name)?;
        match entry.kind.as_str() {
            "raster" => match entry.storage.as_str() {
                "atlas" => {
                    let sprite = entry
                        .atlas
                        .as_ref()
                        .ok_or_else(|| format!("Missing atlas entry for {name}"))?;
                    let atlas = self
                        .atlas_pixmap
                        .as_ref()
                        .ok_or_else(|| "Atlas pixmap not initialized".to_string())?;
                    let rect = tiny_skia::IntRect::from_xywh(
                        sprite.x as i32,
                        sprite.y as i32,
                        sprite.w,
                        sprite.h,
                    )
                    .ok_or_else(|| format!("Invalid atlas rect for {name}"))?;
                    let cropped = atlas
                        .clone_rect(rect)
                        .ok_or_else(|| format!("Failed to crop atlas rect for {name}"))?;
                    target.draw_pixmap(
                        x as i32,
                        y as i32,
                        cropped.as_ref(),
                        &tiny_skia::PixmapPaint::default(),
                        Transform::default(),
                        None,
                    );
                    Ok(())
                }
                "buffer" => {
                    let pixmap = self.decode_raster(name)?;
                    target.draw_pixmap(
                        x as i32,
                        y as i32,
                        pixmap.as_ref(),
                        &tiny_skia::PixmapPaint::default(),
                        Transform::default(),
                        None,
                    );
                    Ok(())
                }
                other => Err(format!("Unsupported raster storage '{other}' for {name}")),
            },
            "svg" => {
                let pixmap = self.decode_svg(name)?;
                target.draw_pixmap(
                    x as i32,
                    y as i32,
                    pixmap.as_ref(),
                    &tiny_skia::PixmapPaint::default(),
                    Transform::default(),
                    None,
                );
                Ok(())
            }
            other => Err(format!("Unsupported asset kind '{other}' for {name}")),
        }
    }

    pub fn decode_svg(&self, name: &str) -> Result<Pixmap, String> {
        let image = self.image(name)?;
        if image.kind != "svg" {
            return Err(format!("Asset is not svg: {name}"));
        }

        let ptr = image
            .buffer
            .as_ref()
            .ok_or_else(|| format!("Missing buffer pointer for {name}"))?;
        let svg_bytes = self.get_bytes(ptr)?;

        let tree = usvg::Tree::from_data(svg_bytes, &usvg::Options::default())
            .map_err(|e| format!("Failed to parse SVG {name}: {e}"))?;
        let size = tree.size().to_int_size();
        let mut pixmap = Pixmap::new(size.width(), size.height())
            .ok_or_else(|| format!("Failed to allocate SVG pixmap for {name}"))?;
        resvg::render(&tree, Transform::default(), &mut pixmap.as_mut());
        Ok(pixmap)
    }
}

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
        .map_err(|e| format!("Image decode error: {e}"))?;
    let rgba = img.into_rgba8();
    let width = rgba.width();
    let height = rgba.height();

    let mut pixmap = Pixmap::from_vec(
        rgba.into_raw(),
        tiny_skia::IntSize::from_wh(width, height).unwrap(),
    )
    .ok_or_else(|| "Failed to create Pixmap".to_string())?;

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
