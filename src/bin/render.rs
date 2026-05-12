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

use ygo_card_renderer_rs::{
    CardKind, RenderOptions, RenderRequest, Renderer,
    asset_bundle::init_global_bundle_from_file,
    model::{EffectMask, YgoCardMeta},
};
use ygopro_cdb_encode_rs::YgoProCdb;

// ── CLI argument parsing (no external crate needed) ──────────────────────────

#[derive(Debug)]
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

// ── Render helpers ────────────────────────────────────────────────────────────

fn make_request(
    card: YgoCardMeta,
    lang: &str,
    scale: f32,
    art_dir: &Path,
    effect_mask: Option<&Path>,
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
) -> Result<(), String> {
    let request = make_request(card.clone(), lang, scale, art_dir, effect_mask);
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

    if let Some(code) = args.id {
        // ── Single mode ──────────────────────────────────────────────────────
        let entry = cards
            .into_iter()
            .find(|c| c.code == code)
            .unwrap_or_else(|| fatal(&format!("card with code {code} not found in cdb")));

        let out_path = args
            .out
            .unwrap_or_else(|| PathBuf::from(format!("{code}.png")));

        let card: YgoCardMeta = entry.into();
        render_one(
            &renderer,
            &card,
            &args.lang,
            args.scale,
            &args.art_dir,
            args.effect_mask.as_deref(),
            &out_path,
        )
        .unwrap_or_else(|e| fatal(&e));

        println!("→ {:?}", out_path);
    } else {
        // ── Batch mode ───────────────────────────────────────────────────────
        let out_dir = args
            .out_dir
            .unwrap_or_else(|| fatal("either --out-dir (batch) or --id + --out (single) required"));

        fs::create_dir_all(&out_dir)
            .unwrap_or_else(|e| fatal(&format!("cannot create out-dir {:?}: {e}", out_dir)));

        let total = cards.len();
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
        let effect_mask = Arc::new(args.effect_mask.clone());
        let out_dir = Arc::new(out_dir.clone());

        let chunk_size = (total + args.jobs - 1) / args.jobs;

        std::thread::scope(|s| {
            for chunk in cards.chunks(chunk_size) {
                let renderer = &renderer;
                let errors = Arc::clone(&errors);
                let done = Arc::clone(&done);
                let lang = Arc::clone(&lang);
                let art_dir = Arc::clone(&art_dir);
                let effect_mask = Arc::clone(&effect_mask);
                let out_dir = Arc::clone(&out_dir);
                // SAFETY: chunk lives for 'scope which is shorter than cards
                let chunk: &[YgoCardMeta] = chunk;

                s.spawn(move || {
                    for card in chunk {
                        let code = card.entry.code;
                        let out_path = out_dir.join(format!("{code}.png"));
                        if let Err(e) = render_one(
                            renderer,
                            card,
                            &lang,
                            args.scale,
                            &art_dir,
                            effect_mask.as_ref().as_deref(),
                            &out_path,
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
