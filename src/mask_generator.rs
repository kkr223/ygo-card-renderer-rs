use std::{fs, path::Path};

use image::{GrayImage, ImageBuffer, Luma, RgbImage, imageops::FilterType};
use ort::{inputs, session::Session, value::Tensor};
use serde::Deserialize;
use thiserror::Error;

const DEFAULT_INPUT_NAME: &str = "image";
const DEFAULT_OUTPUT_NAME: &str = "subject_logits";
const DEFAULT_THRESHOLD: f32 = 0.42;
const DEFAULT_DILATION: u32 = 1;
const RGB_CHANNELS: usize = 3;

#[derive(Debug, Clone, Deserialize)]
pub struct MaskModelMetadata {
    pub input_size: u32,
    #[serde(default)]
    pub input: MaskModelInputMetadata,
    #[serde(default)]
    pub output: MaskModelOutputMetadata,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MaskModelInputMetadata {
    #[serde(default)]
    pub layout: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub resize: Option<String>,
    #[serde(default = "default_mean")]
    pub mean: Vec<f32>,
    #[serde(default = "default_std")]
    pub std: Vec<f32>,
}

impl Default for MaskModelInputMetadata {
    fn default() -> Self {
        Self {
            layout: None,
            color: None,
            resize: None,
            mean: default_mean(),
            std: default_std(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct MaskModelOutputMetadata {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default = "default_threshold")]
    pub recommended_threshold: f32,
    #[serde(default = "default_dilation")]
    pub recommended_subject_dilation_px_at_model_size: u32,
}

impl Default for MaskModelOutputMetadata {
    fn default() -> Self {
        Self {
            name: None,
            recommended_threshold: DEFAULT_THRESHOLD,
            recommended_subject_dilation_px_at_model_size: DEFAULT_DILATION,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct MaskGenerationOptions {
    pub threshold: Option<f32>,
    pub subject_dilation: Option<u32>,
}

#[derive(Debug, Clone, Copy)]
struct LetterboxMeta {
    orig_w: u32,
    orig_h: u32,
    new_w: u32,
    new_h: u32,
    pad_left: u32,
    pad_top: u32,
}

#[derive(Debug, Error)]
pub enum MaskGenerationError {
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("image error: {0}")]
    Image(#[from] image::ImageError),
    #[error("metadata json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("onnx runtime error: {0}")]
    Ort(#[from] ort::Error),
    #[error("invalid mask model metadata: {0}")]
    InvalidMetadata(String),
    #[error("model output error: {0}")]
    Output(String),
}

pub struct MaskGenerator {
    session: Session,
    metadata: MaskModelMetadata,
    input_name: String,
    output_name: String,
}

impl MaskGenerator {
    pub fn from_model_path(
        model_path: impl AsRef<Path>,
        metadata_path: Option<&Path>,
    ) -> Result<Self, MaskGenerationError> {
        let model_path = model_path.as_ref();
        let metadata_path = metadata_path
            .map(Path::to_path_buf)
            .unwrap_or_else(|| model_path.with_extension("json"));
        let metadata_text = fs::read_to_string(&metadata_path)?;
        let metadata: MaskModelMetadata = serde_json::from_str(&metadata_text)?;
        validate_metadata(&metadata)?;

        let output_name = metadata
            .output
            .name
            .clone()
            .unwrap_or_else(|| DEFAULT_OUTPUT_NAME.to_string());
        let session = Session::builder()?.commit_from_file(model_path)?;

        Ok(Self {
            session,
            metadata,
            input_name: DEFAULT_INPUT_NAME.to_string(),
            output_name,
        })
    }

    pub fn metadata(&self) -> &MaskModelMetadata {
        &self.metadata
    }

    pub fn generate_mask_image(
        &mut self,
        art_path: impl AsRef<Path>,
        options: &MaskGenerationOptions,
    ) -> Result<GrayImage, MaskGenerationError> {
        let art = image::open(art_path)?.to_rgb8();
        let (input, letterbox) = preprocess_art(&art, &self.metadata)?;
        let size = self.metadata.input_size as usize;
        let input =
            Tensor::from_array(([1usize, RGB_CHANNELS, size, size], input.into_boxed_slice()))?;
        let outputs = self
            .session
            .run(inputs![self.input_name.as_str() => input])?;
        let output = outputs.get(self.output_name.as_str()).ok_or_else(|| {
            MaskGenerationError::Output(format!("missing output {}", self.output_name))
        })?;
        let (_shape, logits) = output.try_extract_tensor::<f32>()?;
        let logits: Vec<f32> = logits.iter().copied().collect();
        if logits.len() != size * size {
            return Err(MaskGenerationError::Output(format!(
                "expected {} logits, got {}",
                size * size,
                logits.len()
            )));
        }

        let threshold = options
            .threshold
            .unwrap_or(self.metadata.output.recommended_threshold)
            .clamp(0.0, 1.0);
        let dilation = options.subject_dilation.unwrap_or(
            self.metadata
                .output
                .recommended_subject_dilation_px_at_model_size,
        );
        let mut subject = logits_to_subject_mask(&logits, threshold);
        if dilation > 0 {
            subject = dilate_subject_mask(&subject, size, dilation as usize);
        }
        Ok(subject_to_foil_mask(&subject, size, letterbox))
    }

    pub fn generate_mask_file(
        &mut self,
        art_path: impl AsRef<Path>,
        out_path: impl AsRef<Path>,
        options: &MaskGenerationOptions,
    ) -> Result<(), MaskGenerationError> {
        let out_path = out_path.as_ref();
        if let Some(parent) = out_path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        let mask = self.generate_mask_image(art_path, options)?;
        mask.save(out_path)?;
        Ok(())
    }
}

fn validate_metadata(metadata: &MaskModelMetadata) -> Result<(), MaskGenerationError> {
    if metadata.input_size == 0 {
        return Err(MaskGenerationError::InvalidMetadata(
            "input_size must be greater than 0".to_string(),
        ));
    }
    if metadata.input.mean.len() != RGB_CHANNELS || metadata.input.std.len() != RGB_CHANNELS {
        return Err(MaskGenerationError::InvalidMetadata(
            "input mean/std must have 3 RGB values".to_string(),
        ));
    }
    if metadata
        .input
        .std
        .iter()
        .any(|v| *v == 0.0 || !v.is_finite())
    {
        return Err(MaskGenerationError::InvalidMetadata(
            "input std values must be finite non-zero numbers".to_string(),
        ));
    }
    validate_optional_metadata_value("input.layout", metadata.input.layout.as_deref(), "NCHW")?;
    validate_optional_metadata_value("input.color", metadata.input.color.as_deref(), "RGB")?;
    validate_optional_metadata_value(
        "input.resize",
        metadata.input.resize.as_deref(),
        "letterbox_longest_side_pad_black",
    )?;
    Ok(())
}

fn validate_optional_metadata_value(
    name: &str,
    value: Option<&str>,
    expected: &str,
) -> Result<(), MaskGenerationError> {
    let Some(value) = value else {
        return Ok(());
    };
    if value != expected {
        return Err(MaskGenerationError::InvalidMetadata(format!(
            "{name} must be {expected}, got {value}"
        )));
    }
    Ok(())
}

fn preprocess_art(
    art: &RgbImage,
    metadata: &MaskModelMetadata,
) -> Result<(Vec<f32>, LetterboxMeta), MaskGenerationError> {
    let size = metadata.input_size;
    let (orig_w, orig_h) = art.dimensions();
    if orig_w == 0 || orig_h == 0 {
        return Err(MaskGenerationError::Image(image::ImageError::Limits(
            image::error::LimitError::from_kind(image::error::LimitErrorKind::DimensionError),
        )));
    }

    let scale = size as f32 / orig_w.max(orig_h) as f32;
    let new_w = ((orig_w as f32 * scale).round() as u32).max(1);
    let new_h = ((orig_h as f32 * scale).round() as u32).max(1);
    let pad_left = (size - new_w) / 2;
    let pad_top = (size - new_h) / 2;

    let resized = image::imageops::resize(art, new_w, new_h, FilterType::CatmullRom);
    let mut canvas = ImageBuffer::from_pixel(size, size, image::Rgb([0, 0, 0]));
    image::imageops::overlay(&mut canvas, &resized, pad_left as i64, pad_top as i64);

    let size_usize = size as usize;
    let plane = size_usize * size_usize;
    let mut input = vec![0.0f32; RGB_CHANNELS * plane];
    for y in 0..size_usize {
        for x in 0..size_usize {
            let pixel = canvas.get_pixel(x as u32, y as u32).0;
            let idx = y * size_usize + x;
            for channel in 0..RGB_CHANNELS {
                let value = pixel[channel] as f32 / 255.0;
                input[channel * plane + idx] =
                    (value - metadata.input.mean[channel]) / metadata.input.std[channel];
            }
        }
    }

    Ok((
        input,
        LetterboxMeta {
            orig_w,
            orig_h,
            new_w,
            new_h,
            pad_left,
            pad_top,
        },
    ))
}

fn logits_to_subject_mask(logits: &[f32], threshold: f32) -> Vec<bool> {
    logits
        .iter()
        .map(|logit| sigmoid(*logit) >= threshold)
        .collect()
}

fn sigmoid(x: f32) -> f32 {
    if x >= 0.0 {
        1.0 / (1.0 + (-x).exp())
    } else {
        let exp = x.exp();
        exp / (1.0 + exp)
    }
}

fn dilate_subject_mask(subject: &[bool], size: usize, radius: usize) -> Vec<bool> {
    let mut dilated = subject.to_vec();
    for y in 0..size {
        for x in 0..size {
            if !subject[y * size + x] {
                continue;
            }
            let x0 = x.saturating_sub(radius);
            let y0 = y.saturating_sub(radius);
            let x1 = (x + radius).min(size - 1);
            let y1 = (y + radius).min(size - 1);
            for yy in y0..=y1 {
                for xx in x0..=x1 {
                    dilated[yy * size + xx] = true;
                }
            }
        }
    }
    dilated
}

fn subject_to_foil_mask(subject: &[bool], size: usize, meta: LetterboxMeta) -> GrayImage {
    let mut out = GrayImage::new(meta.orig_w, meta.orig_h);
    for y in 0..meta.orig_h {
        let src_y = meta.pad_top + nearest_source_coord(y, meta.orig_h, meta.new_h);
        for x in 0..meta.orig_w {
            let src_x = meta.pad_left + nearest_source_coord(x, meta.orig_w, meta.new_w);
            let idx = src_y as usize * size + src_x as usize;
            let value = if subject[idx] { 0 } else { 255 };
            out.put_pixel(x, y, Luma([value]));
        }
    }
    out
}

fn nearest_source_coord(dst: u32, dst_len: u32, src_len: u32) -> u32 {
    (((dst as f32 + 0.5) * src_len as f32 / dst_len as f32).floor() as u32).min(src_len - 1)
}

fn default_mean() -> Vec<f32> {
    vec![0.485, 0.456, 0.406]
}

fn default_std() -> Vec<f32> {
    vec![0.229, 0.224, 0.225]
}

fn default_threshold() -> f32 {
    DEFAULT_THRESHOLD
}

fn default_dilation() -> u32 {
    DEFAULT_DILATION
}
