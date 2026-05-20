//! CLI for rendering Yu-Gi-Oh! card images from a CDB database and an art
//! directory.
//!
//! # Usage
//!
//! ```text
//! render [OPTIONS] --bundle <PATH> --cdb <PATH> --art-dir <DIR> --out-dir <DIR>
//! render [OPTIONS] --bundle <PATH> --cdb <PATH> --art-dir <DIR> --id <CODE>  --out <FILE>
//! ```
//!
//! ## Common options
//!
//! | flag | description |
//! |------|-------------|
//! | `--bundle <PATH>` | Path to `yugioh_bundle.bin` asset bundle |
//! | `--cdb <PATH>` | Path to a YGOPro `.cdb` card database |
//! | `--art-dir <DIR>` | Directory containing `<code>.jpg` / `<code>.png` art images |
//! | `--out-dir <DIR>` | **Batch mode**: output directory (renders all cards in cdb) |
//! | `--id <CODE>` | **Single mode**: numeric card code to render |
//! | `--out <FILE>` | **Single mode**: output PNG path |
//! | `--lang <LANG>` | Language hint (`sc`, `tc`, `jp`, `en`, …). Default: `sc` |
//! | `--scale <F>` | Output scale factor. Default: `1.0` |
//! | `--effect-mask <PATH>` | Optional black/white mask; black protects areas from effects |
//! | `--jobs <N>` | Parallel workers for batch mode. Default: logical CPU count |
//!
//! Art images are looked up as `<art-dir>/<code>.jpg` then `<art-dir>/<code>.png`.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

#[cfg(feature = "onnx-mask")]
use ygo_card_renderer_rs::mask_generator::{MaskGenerationOptions, MaskGenerator};
use ygo_card_renderer_rs::{
    CardKind, RenderOptions, RenderRequest, Renderer,
    asset_bundle::init_global_bundle_from_file,
    model::{EffectMask, YgoCardMeta},
};
use ygopro_cdb_encode_rs::YgoProCdb;

#[cfg(feature = "onnx-mask")]
type SharedMaskGenerator = Arc<Mutex<MaskGenerator>>;
#[cfg(not(feature = "onnx-mask"))]
type SharedMaskGenerator = ();

// ── CLI argument parsing (no external crate needed) ──────────────────────────

#[derive(Debug, Clone)]
struct Args {
    bundle: PathBuf,
    cdb: PathBuf,
    art_dir: PathBuf,
    /// Batch mode: output directory
    out_dir: Option<PathBuf>,
    /// Single mode: card code
    id: Option<u32>,
    /// Single mode: output file
    out: Option<PathBuf>,
    lang: String,
    scale: f32,
    effect_mask: Option<PathBuf>,
    effect_mask_dir: Option<PathBuf>,
    auto_mask_model: Option<PathBuf>,
    auto_mask_metadata: Option<PathBuf>,
    mask_cache_dir: Option<PathBuf>,
    mask_threshold: Option<f32>,
    mask_dilate: Option<u32>,
    overwrite_mask: bool,
    format_text: bool,
    jobs: usize,
}

fn parse_args() -> Result<Args, String> {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let mut bundle: Option<PathBuf> = None;
    let mut cdb: Option<PathBuf> = None;
    let mut art_dir: Option<PathBuf> = None;
    let mut out_dir: Option<PathBuf> = None;
    let mut id: Option<u32> = None;
    let mut out: Option<PathBuf> = None;
    let mut lang = "sc".to_string();
    let mut scale = 1.0f32;
    let mut effect_mask: Option<PathBuf> = None;
    let mut effect_mask_dir: Option<PathBuf> = None;
    let mut auto_mask_model: Option<PathBuf> = None;
    let mut auto_mask_metadata: Option<PathBuf> = None;
    let mut mask_cache_dir: Option<PathBuf> = None;
    let mut mask_threshold: Option<f32> = None;
    let mut mask_dilate: Option<u32> = None;
    let mut overwrite_mask = false;
    let mut format_text = false;
    let mut jobs: usize = num_cpus();

    let mut i = 0usize;
    while i < raw.len() {
        match raw[i].as_str() {
            "--bundle" => {
                i += 1;
                bundle = Some(PathBuf::from(next(&raw, i, "--bundle")?));
            }
            "--cdb" => {
                i += 1;
                cdb = Some(PathBuf::from(next(&raw, i, "--cdb")?));
            }
            "--art-dir" => {
                i += 1;
                art_dir = Some(PathBuf::from(next(&raw, i, "--art-dir")?));
            }
            "--out-dir" => {
                i += 1;
                out_dir = Some(PathBuf::from(next(&raw, i, "--out-dir")?));
            }
            "--id" => {
                i += 1;
                let v = next(&raw, i, "--id")?;
                id = Some(
                    v.parse::<u32>()
                        .map_err(|e| format!("--id: invalid number: {e}"))?,
                );
            }
            "--out" => {
                i += 1;
                out = Some(PathBuf::from(next(&raw, i, "--out")?));
            }
            "--lang" => {
                i += 1;
                lang = next(&raw, i, "--lang")?.to_string();
            }
            "--scale" => {
                i += 1;
                let v = next(&raw, i, "--scale")?;
                scale = v
                    .parse::<f32>()
                    .map_err(|e| format!("--scale: invalid float: {e}"))?;
            }
            "--effect-mask" => {
                i += 1;
                effect_mask = Some(PathBuf::from(next(&raw, i, "--effect-mask")?));
            }
            "--effect-mask-dir" => {
                i += 1;
                effect_mask_dir = Some(PathBuf::from(next(&raw, i, "--effect-mask-dir")?));
            }
            "--auto-mask-model" => {
                i += 1;
                auto_mask_model = Some(PathBuf::from(next(&raw, i, "--auto-mask-model")?));
            }
            "--auto-mask-metadata" => {
                i += 1;
                auto_mask_metadata = Some(PathBuf::from(next(&raw, i, "--auto-mask-metadata")?));
            }
            "--mask-cache-dir" => {
                i += 1;
                mask_cache_dir = Some(PathBuf::from(next(&raw, i, "--mask-cache-dir")?));
            }
            "--mask-threshold" => {
                i += 1;
                let v = next(&raw, i, "--mask-threshold")?;
                mask_threshold = Some(
                    v.parse::<f32>()
                        .map_err(|e| format!("--mask-threshold: invalid float: {e}"))?,
                );
            }
            "--mask-dilate" => {
                i += 1;
                let v = next(&raw, i, "--mask-dilate")?;
                mask_dilate = Some(
                    v.parse::<u32>()
                        .map_err(|e| format!("--mask-dilate: invalid integer: {e}"))?,
                );
            }
            "--format" => format_text = true,
            "--overwrite-mask" => overwrite_mask = true,
            "--jobs" => {
                i += 1;
                let v = next(&raw, i, "--jobs")?;
                jobs = v
                    .parse::<usize>()
                    .map_err(|e| format!("--jobs: invalid integer: {e}"))?;
                if jobs == 0 {
                    return Err("--jobs must be greater than 0".to_string());
                }
            }
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
        i += 1;
    }

    Ok(Args {
        bundle: bundle.ok_or("--bundle is required")?,
        cdb: cdb.ok_or("--cdb is required")?,
        art_dir: art_dir.ok_or("--art-dir is required")?,
        out_dir,
        id,
        out,
        lang,
        scale,
        effect_mask,
        effect_mask_dir,
        auto_mask_model,
        auto_mask_metadata,
        mask_cache_dir,
        mask_threshold,
        mask_dilate,
        overwrite_mask,
        format_text,
        jobs,
    })
}

fn next<'a>(args: &'a [String], i: usize, flag: &str) -> Result<&'a str, String> {
    args.get(i)
        .map(|s| s.as_str())
        .ok_or_else(|| format!("{flag} requires a value"))
}

fn num_cpus() -> usize {
    // Stable way without an external crate: read from env or fall back to 4.
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}

fn print_help() {
    eprintln!(
        r#"ygo-card-renderer — render card images from CDB + art directory

USAGE
  render --bundle <PATH> --cdb <PATH> --art-dir <DIR> --out-dir <DIR> [OPTIONS]
  render --bundle <PATH> --cdb <PATH> --art-dir <DIR> --id <CODE> --out <FILE> [OPTIONS]

OPTIONS
  --bundle <PATH>   yugioh_bundle.bin asset bundle  (required)
  --cdb    <PATH>   YGOPro .cdb card database       (required)
  --art-dir <DIR>   directory with <code>.jpg/.png  (required)
  --out-dir <DIR>   batch output directory
  --id     <CODE>   single card code
  --out    <FILE>   single output PNG path
  --lang   <LANG>   language: sc|tc|jp|en  [default: sc]
  --scale  <F>      output scale factor    [default: 1.0]
  --effect-mask <PATH>
                   black protects pixels from visual/rare effects; white allows them
  --effect-mask-dir <DIR>
                   lookup per-card masks as <DIR>/<code>.png
  --auto-mask-model <ONNX>
                   generate missing masks with TinyMaskNet; requires onnx-mask feature
  --auto-mask-metadata <JSON>
                   optional metadata JSON; defaults to model path with .json extension
  --mask-cache-dir <DIR>
                   where auto-generated masks are written; defaults to --effect-mask-dir
  --mask-threshold <F>
                   override model subject threshold
  --mask-dilate <PX>
                   override subject dilation in model pixels
  --overwrite-mask
                    regenerate masks even when the cache file already exists
  --format           enable text formatting: title compress + compact description
  --jobs   <N>      parallel workers       [default: CPU count]
  --help            show this message
"#
    );
}

// ── Art lookup ───────────────────────────────────────────────────────────────

fn find_art(art_dir: &Path, code: u32) -> Option<PathBuf> {
    for ext in ["jpg", "png", "webp"] {
        let p = art_dir.join(format!("{code}.{ext}"));
        if p.exists() {
            return Some(p);
        }
    }
    None
}

fn find_effect_mask(mask_dir: &Path, code: u32) -> Option<PathBuf> {
    let path = mask_dir.join(format!("{code}.png"));
    path.exists().then_some(path)
}

fn auto_mask_output_dir(args: &Args) -> Option<&Path> {
    args.mask_cache_dir
        .as_deref()
        .or(args.effect_mask_dir.as_deref())
}

fn auto_mask_is_enabled(args: &Args) -> bool {
    args.auto_mask_model.is_some() && args.effect_mask.is_none()
}

fn create_mask_generator(args: &Args) -> Result<Option<SharedMaskGenerator>, String> {
    #[cfg(not(feature = "onnx-mask"))]
    let _ = (
        &args.auto_mask_metadata,
        args.mask_threshold,
        args.mask_dilate,
        args.overwrite_mask,
    );
    if !auto_mask_is_enabled(args) {
        return Ok(None);
    }
    if auto_mask_output_dir(args).is_none() {
        return Err(
            "--auto-mask-model requires --effect-mask-dir or --mask-cache-dir for generated masks"
                .to_string(),
        );
    }
    create_mask_generator_impl(args).map(Some)
}

#[cfg(feature = "onnx-mask")]
fn create_mask_generator_impl(args: &Args) -> Result<SharedMaskGenerator, String> {
    let model = args
        .auto_mask_model
        .as_deref()
        .ok_or("--auto-mask-model is required")?;
    let generator = MaskGenerator::from_model_path(model, args.auto_mask_metadata.as_deref())
        .map_err(|e| format!("load auto mask model {:?}: {e}", model))?;
    Ok(Arc::new(Mutex::new(generator)))
}

#[cfg(not(feature = "onnx-mask"))]
fn create_mask_generator_impl(_args: &Args) -> Result<SharedMaskGenerator, String> {
    Err("--auto-mask-model requires rebuilding with --features onnx-mask".to_string())
}

fn resolve_effect_mask_for_card(
    card_code: u32,
    art_image: Option<&Path>,
    args: &Args,
    auto_mask_generator: Option<&SharedMaskGenerator>,
) -> Result<Option<PathBuf>, String> {
    if let Some(mask) = &args.effect_mask {
        return Ok(Some(mask.clone()));
    }

    if let Some(mask_dir) = &args.effect_mask_dir {
        if let Some(mask) = find_effect_mask(mask_dir, card_code) {
            return Ok(Some(mask));
        }
    }

    if !auto_mask_is_enabled(args) {
        return Ok(None);
    }

    generate_auto_mask_for_card(card_code, art_image, args, auto_mask_generator)
}

#[cfg(feature = "onnx-mask")]
fn generate_auto_mask_for_card(
    card_code: u32,
    art_image: Option<&Path>,
    args: &Args,
    auto_mask_generator: Option<&SharedMaskGenerator>,
) -> Result<Option<PathBuf>, String> {
    let Some(art_image) = art_image else {
        eprintln!("warning: auto mask skipped for {card_code}: no art image found");
        return Ok(None);
    };
    let mask_dir = auto_mask_output_dir(args).ok_or(
        "--auto-mask-model requires --effect-mask-dir or --mask-cache-dir for generated masks",
    )?;
    let out_path = mask_dir.join(format!("{card_code}.png"));
    if out_path.exists() && !args.overwrite_mask {
        return Ok(Some(out_path));
    }
    let generator = auto_mask_generator.ok_or("auto mask generator is not initialized")?;
    let options = MaskGenerationOptions {
        threshold: args.mask_threshold,
        subject_dilation: args.mask_dilate,
    };
    let mut generator = generator
        .lock()
        .map_err(|_| "auto mask generator mutex poisoned".to_string())?;
    generator
        .generate_mask_file(art_image, &out_path, &options)
        .map_err(|e| format!("generate auto mask for {card_code}: {e}"))?;
    Ok(Some(out_path))
}

#[cfg(not(feature = "onnx-mask"))]
fn generate_auto_mask_for_card(
    _card_code: u32,
    _art_image: Option<&Path>,
    _args: &Args,
    _auto_mask_generator: Option<&SharedMaskGenerator>,
) -> Result<Option<PathBuf>, String> {
    Err("--auto-mask-model requires rebuilding with --features onnx-mask".to_string())
}

// ── Render helpers ────────────────────────────────────────────────────────────

fn make_request(
    card: YgoCardMeta,
    lang: &str,
    scale: f32,
    art_dir: &Path,
    effect_mask: Option<&Path>,
    format_text: bool,
) -> RenderRequest {
    let art_image = find_art(art_dir, card.entry.code);
    RenderRequest {
        kind: CardKind::Yugioh,
        options: RenderOptions {
            language: Some(lang.to_string()),
            scale,
            art_image,
            effect_mask: effect_mask.map(|path| EffectMask {
                path: path.to_path_buf(),
                x: None,
                y: None,
            }),
            title_width_compress: format_text,
            format_text,
            ..RenderOptions::default()
        },
        card,
    }
}

fn render_one(
    renderer: &Renderer,
    card: &YgoCardMeta,
    lang: &str,
    scale: f32,
    art_dir: &Path,
    effect_mask: Option<&Path>,
    out_path: &Path,
    format_text: bool,
) -> Result<(), String> {
    let request = make_request(card.clone(), lang, scale, art_dir, effect_mask, format_text);
    let png = renderer
        .render_png(&request)
        .map_err(|e| format!("render error for {}: {e}", card.entry.code))?;
    fs::write(out_path, &png).map_err(|e| format!("write error {:?}: {e}", out_path))?;
    Ok(())
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {e}");
            eprintln!("Run with --help for usage.");
            std::process::exit(1);
        }
    };

    // Load asset bundle via mmap so startup does not copy the whole payload.
    init_global_bundle_from_file(&args.bundle)
        .unwrap_or_else(|e| fatal(&format!("init bundle: {e}")));

    // Load CDB
    let cdb = YgoProCdb::from_path(&args.cdb)
        .unwrap_or_else(|e| fatal(&format!("cannot open cdb {:?}: {e}", args.cdb)));
    let cards = cdb
        .find_all()
        .unwrap_or_else(|e| fatal(&format!("cannot read cards from cdb: {e}")));

    let renderer = Renderer::new();
    let auto_mask_generator = create_mask_generator(&args).unwrap_or_else(|e| fatal(&e));
    if auto_mask_is_enabled(&args) {
        if let Some(mask_dir) = auto_mask_output_dir(&args) {
            fs::create_dir_all(mask_dir).unwrap_or_else(|e| {
                fatal(&format!("cannot create mask cache dir {:?}: {e}", mask_dir))
            });
        }
    }

    if let Some(code) = args.id {
        // ── Single mode ──────────────────────────────────────────────────────
        let entry = cards
            .into_iter()
            .find(|c| c.code == code)
            .unwrap_or_else(|| fatal(&format!("card with code {code} not found in cdb")));

        let out_path = args
            .out
            .clone()
            .unwrap_or_else(|| PathBuf::from(format!("{code}.png")));

        let card: YgoCardMeta = entry.into();
        let art_image = find_art(&args.art_dir, code);
        let effect_mask = resolve_effect_mask_for_card(
            code,
            art_image.as_deref(),
            &args,
            auto_mask_generator.as_ref(),
        )
        .unwrap_or_else(|e| fatal(&e));
        render_one(
            &renderer,
            &card,
            &args.lang,
            args.scale,
            &args.art_dir,
            effect_mask.as_deref(),
            &out_path,
            args.format_text,
        )
        .unwrap_or_else(|e| fatal(&e));

        println!("→ {:?}", out_path);
    } else {
        // ── Batch mode ───────────────────────────────────────────────────────
        let out_dir = args
            .out_dir
            .clone()
            .unwrap_or_else(|| fatal("either --out-dir (batch) or --id + --out (single) required"));

        fs::create_dir_all(&out_dir)
            .unwrap_or_else(|e| fatal(&format!("cannot create out-dir {:?}: {e}", out_dir)));

        let total = cards.len();
        if total == 0 {
            println!("No cards found in CDB — nothing to render.");
            return;
        }
        println!("Rendering {total} cards with {} workers…", args.jobs);

        let errors: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let done = Arc::new(std::sync::atomic::AtomicUsize::new(0));

        // Split cards into chunks for parallel rendering.
        // We use simple scoped threads (stable, no rayon needed).
        let cards: Arc<Vec<_>> = Arc::new(
            cards
                .into_iter()
                .map(|e| -> YgoCardMeta { e.into() })
                .collect(),
        );
        let lang = Arc::new(args.lang.clone());
        let art_dir = Arc::new(args.art_dir.clone());
        let out_dir = Arc::new(out_dir.clone());
        let shared_args = Arc::new(args.clone());

        let chunk_size = (total + args.jobs - 1) / args.jobs;

        std::thread::scope(|s| {
            for chunk in cards.chunks(chunk_size) {
                let renderer = &renderer;
                let errors = Arc::clone(&errors);
                let done = Arc::clone(&done);
                let lang = Arc::clone(&lang);
                let art_dir = Arc::clone(&art_dir);
                let out_dir = Arc::clone(&out_dir);
                let args = Arc::clone(&shared_args);
                let auto_mask_generator = auto_mask_generator.clone();
                // SAFETY: chunk lives for 'scope which is shorter than cards
                let chunk: &[YgoCardMeta] = chunk;

                s.spawn(move || {
                    for card in chunk {
                        let code = card.entry.code;
                        let out_path = out_dir.join(format!("{code}.png"));
                        let art_image = find_art(&art_dir, code);
                        let effect_mask = match resolve_effect_mask_for_card(
                            code,
                            art_image.as_deref(),
                            &args,
                            auto_mask_generator.as_ref(),
                        ) {
                            Ok(mask) => mask,
                            Err(e) => {
                                errors.lock().unwrap().push(e);
                                let n = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                                if n % 100 == 0 || n == total {
                                    eprintln!("  {n}/{total}");
                                }
                                continue;
                            }
                        };
                        if let Err(e) = render_one(
                            renderer,
                            card,
                            &lang,
                            args.scale,
                            &art_dir,
                            effect_mask.as_deref(),
                            &out_path,
                            args.format_text,
                        ) {
                            errors.lock().unwrap().push(e);
                        }
                        let n = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                        if n % 100 == 0 || n == total {
                            eprintln!("  {n}/{total}");
                        }
                    }
                });
            }
        });

        let errs = errors.lock().unwrap();
        if errs.is_empty() {
            println!("Done. {total} cards → {:?}", out_dir);
        } else {
            eprintln!("{} error(s):", errs.len());
            for e in errs.iter() {
                eprintln!("  {e}");
            }
            println!(
                "Done with {} error(s). {total} cards → {:?}",
                errs.len(),
                out_dir
            );
        }
    }
}

fn fatal(msg: &str) -> ! {
    eprintln!("fatal: {msg}");
    std::process::exit(1);
}
