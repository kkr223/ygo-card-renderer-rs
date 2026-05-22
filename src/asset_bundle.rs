use image::{ImageFormat, ImageReader, Limits};
use memmap2::Mmap;
use resvg::{self, usvg};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::Cursor;
use std::path::Path;
use std::sync::{Arc, OnceLock};
use tiny_skia::{Pixmap, Transform};

pub use crate::bundle_layout::*;

const MAGIC: &[u8; 4] = b"YGOC";
const SUPPORTED_VERSION: u32 = 1;
const HEADER_LEN: usize = 12;
const MAX_JSON_LEN: usize = 16 * 1024 * 1024;
const MAX_BUNDLE_LEN: usize = 512 * 1024 * 1024;
const MAX_DECODED_PIXELS: u64 = 4096 * 4096;

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

// Layout types moved to bundle_layout.rs
// Style types moved to bundle_layout.rs

pub struct AssetBundle {
    pub index: BundleIndex,
    pub layout: LayoutPayload,
    storage: BundleStorage,
    payload_offset: usize,
    atlas_pixmap: Option<Pixmap>,
    image_cache: HashMap<String, OnceLock<Result<Arc<Pixmap>, String>>>,
}

enum BundleStorage {
    Bytes(Vec<u8>),
    Mmap(Mmap),
}

impl BundleStorage {
    fn as_slice(&self) -> &[u8] {
        match self {
            Self::Bytes(bytes) => bytes.as_slice(),
            Self::Mmap(mmap) => mmap.as_ref(),
        }
    }
}

impl AssetBundle {
    pub fn load_from_bytes(data: &[u8]) -> Result<Self, String> {
        Self::load_from_vec(data.to_vec())
    }

    pub fn load_from_vec(data: Vec<u8>) -> Result<Self, String> {
        Self::load_from_storage(BundleStorage::Bytes(data))
    }

    pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        let file =
            File::open(path).map_err(|e| format!("Failed to open bundle {:?}: {e}", path))?;
        let file_len = file
            .metadata()
            .map_err(|e| format!("Failed to stat bundle {:?}: {e}", path))?
            .len();
        if file_len > MAX_BUNDLE_LEN as u64 {
            return Err(format!("Bundle too large: {file_len} bytes"));
        }
        // SAFETY: callers must keep the bundle file immutable while the process uses it.
        // The CLI treats bundle files as read-only build artifacts.
        let mmap = unsafe { Mmap::map(&file) }
            .map_err(|e| format!("Failed to mmap bundle {:?}: {e}", path))?;
        Self::load_from_storage(BundleStorage::Mmap(mmap))
    }

    fn load_from_storage(storage: BundleStorage) -> Result<Self, String> {
        let data = storage.as_slice();
        if data.len() < HEADER_LEN {
            return Err("Invalid bundle size".into());
        }
        if data.len() > MAX_BUNDLE_LEN {
            return Err(format!("Bundle too large: {} bytes", data.len()));
        }

        let magic = &data[0..4];
        if magic != MAGIC {
            return Err("Invalid magic header".into());
        }

        let version = read_u32_le(data, 4, "bundle version")?;
        if version != SUPPORTED_VERSION {
            return Err(format!(
                "Unsupported bundle version {version}; expected {SUPPORTED_VERSION}"
            ));
        }

        let json_len = read_u32_le(data, 8, "bundle index length")? as usize;
        if json_len > MAX_JSON_LEN {
            return Err(format!("Bundle index too large: {json_len} bytes"));
        }
        let payload_offset = HEADER_LEN
            .checked_add(json_len)
            .ok_or_else(|| "Bundle index length overflow".to_string())?;
        if data.len() < payload_offset {
            return Err("Bundle truncated".into());
        }

        let index: BundleIndex = serde_json::from_slice(&data[HEADER_LEN..payload_offset])
            .map_err(|e| format!("Failed to parse bundle index: {e}"))?;
        let payload_len = data.len() - payload_offset;

        let layout_ptr = &index.layout.buffer;
        validate_buffer(layout_ptr, payload_len, "layout")?;
        if let Some(buffer) = &index.atlas.buffer {
            validate_buffer(buffer, payload_len, "atlas")?;
        }
        for (name, image) in &index.images {
            validate_image_entry(name, image, &index.atlas, payload_len)?;
        }
        for (name, font) in &index.fonts {
            validate_buffer(&font.buffer, payload_len, name)?;
        }

        let layout_start = payload_offset + layout_ptr.offset as usize;
        let layout_end = checked_end(layout_start, layout_ptr.len as usize, data.len(), "layout")?;
        let layout = serde_json::from_slice(&data[layout_start..layout_end])
            .map_err(|e| format!("Failed to parse layout payload: {e}"))?;
        let image_cache = index
            .images
            .keys()
            .map(|name| (name.clone(), OnceLock::new()))
            .collect();

        let mut bundle = Self {
            index,
            storage,
            payload_offset,
            layout,
            atlas_pixmap: None,
            image_cache,
        };

        if let Some(buffer) = &bundle.index.atlas.buffer {
            let atlas_bytes = bundle.get_bytes(buffer)?;
            bundle.atlas_pixmap = Some(decode_webp(atlas_bytes)?);
        }

        Ok(bundle)
    }

    pub fn get_bytes(&self, ptr: &BufferPointer) -> Result<&[u8], String> {
        let data = self.storage.as_slice();
        let start = self
            .payload_offset
            .checked_add(ptr.offset as usize)
            .ok_or_else(|| "Buffer pointer overflow".to_string())?;
        let end = checked_end(start, ptr.len as usize, data.len(), "buffer")?;
        Ok(&data[start..end])
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
        self.decoded_image(name)
            .map(|pixmap| pixmap.as_ref().clone())
    }

    fn decode_raster_uncached(&self, name: &str) -> Result<Pixmap, String> {
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
        let pixmap = self.decoded_image(name)?;
        target.draw_pixmap(
            x as i32,
            y as i32,
            pixmap.as_ref().as_ref(),
            &tiny_skia::PixmapPaint::default(),
            Transform::default(),
            None,
        );
        Ok(())
    }

    pub(crate) fn decoded_image_for_render(&self, name: &str) -> Result<Arc<Pixmap>, String> {
        self.decoded_image(name)
    }

    fn decoded_image(&self, name: &str) -> Result<Arc<Pixmap>, String> {
        let cache = self
            .image_cache
            .get(name)
            .ok_or_else(|| format!("Missing image asset: {name}"))?;
        cache
            .get_or_init(|| self.decode_image_uncached(name).map(Arc::new))
            .clone()
    }

    fn decode_image_uncached(&self, name: &str) -> Result<Pixmap, String> {
        let entry = self.image(name)?;
        let pixmap = match entry.kind.as_str() {
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
                    atlas
                        .clone_rect(rect)
                        .ok_or_else(|| format!("Failed to crop atlas rect for {name}"))?
                }
                "buffer" => self.decode_raster_uncached(name)?,
                other => return Err(format!("Unsupported raster storage '{other}' for {name}")),
            },
            "svg" => self.decode_svg(name)?,
            other => return Err(format!("Unsupported asset kind '{other}' for {name}")),
        };
        Ok(pixmap)
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
        validate_decode_size(size.width(), size.height())?;
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

pub fn init_global_bundle_from_file(path: impl AsRef<Path>) -> Result<(), String> {
    let bundle = AssetBundle::load_from_file(path)?;
    BUNDLE
        .set(bundle)
        .map_err(|_| "Bundle already initialized".to_string())?;
    Ok(())
}

pub fn try_get_bundle() -> Result<&'static AssetBundle, String> {
    BUNDLE
        .get()
        .ok_or_else(|| "AssetBundle not initialized".to_string())
}

pub fn get_bundle() -> &'static AssetBundle {
    try_get_bundle().expect("AssetBundle not initialized")
}

pub fn decode_webp(bytes: &[u8]) -> Result<Pixmap, String> {
    let (declared_width, declared_height) =
        ImageReader::with_format(Cursor::new(bytes), ImageFormat::WebP)
            .into_dimensions()
            .map_err(|e| format!("Image dimension read error: {e}"))?;
    validate_decode_size(declared_width, declared_height)?;

    let mut reader = ImageReader::with_format(Cursor::new(bytes), ImageFormat::WebP);
    let mut limits = Limits::default();
    limits.max_image_width = Some(declared_width);
    limits.max_image_height = Some(declared_height);
    limits.max_alloc = Some(MAX_DECODED_PIXELS * 4);
    reader.limits(limits);
    let img = reader
        .decode()
        .map_err(|e| format!("Image decode error: {e}"))?;
    let rgba = img.into_rgba8();
    let width = rgba.width();
    let height = rgba.height();
    validate_decode_size(width, height)?;

    let size = tiny_skia::IntSize::from_wh(width, height)
        .ok_or_else(|| format!("Invalid decoded image size: {width}x{height}"))?;
    let mut pixmap = Pixmap::from_vec(rgba.into_raw(), size)
        .ok_or_else(|| "Failed to create Pixmap".to_string())?;

    // `image` delivers straight-alpha RGBA; tiny_skia's Pixmap expects premultiplied alpha.
    // Opaque pixels (α = 255) are already correct and need no adjustment.
    for pixel in pixmap.pixels_mut() {
        let a = pixel.alpha();
        if a == 255 {
            continue;
        }
        if a == 0 {
            *pixel = tiny_skia::PremultipliedColorU8::TRANSPARENT;
            continue;
        }
        let r = (pixel.red() as u16 * a as u16 / 255) as u8;
        let g = (pixel.green() as u16 * a as u16 / 255) as u8;
        let b = (pixel.blue() as u16 * a as u16 / 255) as u8;
        *pixel = tiny_skia::PremultipliedColorU8::from_rgba(r, g, b, a)
            .unwrap_or(tiny_skia::PremultipliedColorU8::TRANSPARENT);
    }

    Ok(pixmap)
}

fn checked_end(start: usize, len: usize, total: usize, label: &str) -> Result<usize, String> {
    let end = start
        .checked_add(len)
        .ok_or_else(|| format!("{label} buffer pointer overflow"))?;
    if end > total {
        return Err(format!("{label} buffer pointer out of bounds"));
    }
    Ok(end)
}

fn read_u32_le(data: &[u8], offset: usize, label: &str) -> Result<u32, String> {
    let end = checked_end(offset, 4, data.len(), label)?;
    let bytes: [u8; 4] = data[offset..end]
        .try_into()
        .map_err(|_| format!("Invalid {label}"))?;
    Ok(u32::from_le_bytes(bytes))
}

fn validate_buffer(ptr: &BufferPointer, payload_len: usize, label: &str) -> Result<(), String> {
    checked_end(ptr.offset as usize, ptr.len as usize, payload_len, label).map(|_| ())
}

fn validate_image_entry(
    name: &str,
    image: &ImageEntry,
    atlas: &AtlasMeta,
    payload_len: usize,
) -> Result<(), String> {
    match (image.kind.as_str(), image.storage.as_str()) {
        ("raster", "buffer") => {
            let buffer = image
                .buffer
                .as_ref()
                .ok_or_else(|| format!("Missing buffer pointer for {name}"))?;
            validate_buffer(buffer, payload_len, name)?;
            if let Some(size) = &image.size {
                validate_decode_size(size.w, size.h)?;
            }
        }
        ("raster", "atlas") => {
            if atlas.buffer.is_none() {
                return Err(format!("Missing atlas buffer for {name}"));
            }
            let sprite = image
                .atlas
                .as_ref()
                .ok_or_else(|| format!("Missing atlas entry for {name}"))?;
            validate_atlas_sprite(sprite, atlas.width, atlas.height, name)?;
        }
        ("svg", "buffer") => {
            let buffer = image
                .buffer
                .as_ref()
                .ok_or_else(|| format!("Missing SVG buffer pointer for {name}"))?;
            validate_buffer(buffer, payload_len, name)?;
        }
        (kind, storage) => {
            return Err(format!(
                "Unsupported image entry kind/storage for {name}: {kind}/{storage}"
            ));
        }
    }
    Ok(())
}

fn validate_atlas_sprite(
    sprite: &AtlasSprite,
    atlas_width: u32,
    atlas_height: u32,
    label: &str,
) -> Result<(), String> {
    let x2 = sprite
        .x
        .checked_add(sprite.w)
        .ok_or_else(|| format!("Atlas sprite overflow for {label}"))?;
    let y2 = sprite
        .y
        .checked_add(sprite.h)
        .ok_or_else(|| format!("Atlas sprite overflow for {label}"))?;
    if x2 > atlas_width || y2 > atlas_height {
        return Err(format!("Atlas sprite out of bounds for {label}"));
    }
    validate_decode_size(sprite.w, sprite.h)
}

fn validate_decode_size(width: u32, height: u32) -> Result<(), String> {
    let pixels = width as u64 * height as u64;
    if pixels == 0 || pixels > MAX_DECODED_PIXELS {
        return Err(format!(
            "Decoded image size out of bounds: {width}x{height}"
        ));
    }
    Ok(())
}
