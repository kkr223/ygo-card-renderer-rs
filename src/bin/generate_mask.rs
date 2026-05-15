//! CLI for generating effect protection masks from art images using the YGO
//! subject-mask ONNX model.

#[cfg(feature = "onnx-mask")]
mod app {
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    use ygo_card_renderer_rs::mask_generator::{MaskGenerationOptions, MaskGenerator};

    #[derive(Debug)]
    struct Args {
        model: PathBuf,
        metadata: Option<PathBuf>,
        art: Option<PathBuf>,
        out: Option<PathBuf>,
        art_dir: Option<PathBuf>,
        out_dir: Option<PathBuf>,
        threshold: Option<f32>,
        dilate: Option<u32>,
        overwrite: bool,
    }

    pub fn main() {
        let args = match parse_args() {
            Ok(args) => args,
            Err(e) => {
                eprintln!("error: {e}");
                eprintln!("Run with --help for usage.");
                std::process::exit(1);
            }
        };

        let options = MaskGenerationOptions {
            threshold: args.threshold,
            subject_dilation: args.dilate,
        };

        let mut generator = MaskGenerator::from_model_path(&args.model, args.metadata.as_deref())
            .unwrap_or_else(|e| fatal(&format!("load mask model: {e}")));

        if let Some(art) = args.art.as_deref() {
            let out = args
                .out
                .as_deref()
                .unwrap_or_else(|| fatal("--out is required when --art is used"));
            if out.exists() && !args.overwrite {
                fatal(&format!(
                    "output already exists: {:?} (pass --overwrite to replace it)",
                    out
                ));
            }
            generator
                .generate_mask_file(art, out, &options)
                .unwrap_or_else(|e| fatal(&format!("generate mask for {:?}: {e}", art)));
            println!("→ {:?}", out);
            return;
        }

        let art_dir = args
            .art_dir
            .as_deref()
            .unwrap_or_else(|| fatal("either --art or --art-dir is required"));
        let out_dir = args
            .out_dir
            .as_deref()
            .unwrap_or_else(|| fatal("--out-dir is required when --art-dir is used"));
        fs::create_dir_all(out_dir)
            .unwrap_or_else(|e| fatal(&format!("cannot create out-dir {:?}: {e}", out_dir)));

        let images = collect_art_images(art_dir)
            .unwrap_or_else(|e| fatal(&format!("cannot scan art-dir {:?}: {e}", art_dir)));
        if images.is_empty() {
            println!("No art images found in {:?}.", art_dir);
            return;
        }

        let mut generated = 0usize;
        let mut skipped = 0usize;
        let mut failed = 0usize;
        for (index, art) in images.iter().enumerate() {
            let Some(stem) = art.file_stem().and_then(|s| s.to_str()) else {
                skipped += 1;
                eprintln!("warning: skipped path with invalid file stem: {:?}", art);
                continue;
            };
            let out = out_dir.join(format!("{stem}.png"));
            if out.exists() && !args.overwrite {
                skipped += 1;
                continue;
            }
            match generator.generate_mask_file(art, &out, &options) {
                Ok(()) => generated += 1,
                Err(e) => {
                    failed += 1;
                    eprintln!("warning: failed to generate mask for {:?}: {e}", art);
                }
            }
            let n = index + 1;
            if n % 100 == 0 || n == images.len() {
                eprintln!("  {n}/{}", images.len());
            }
        }

        println!(
            "Done. generated={generated}, skipped={skipped}, failed={failed} → {:?}",
            out_dir
        );
        if failed > 0 {
            std::process::exit(1);
        }
    }

    fn parse_args() -> Result<Args, String> {
        let raw: Vec<String> = std::env::args().skip(1).collect();
        let mut model: Option<PathBuf> = None;
        let mut metadata: Option<PathBuf> = None;
        let mut art: Option<PathBuf> = None;
        let mut out: Option<PathBuf> = None;
        let mut art_dir: Option<PathBuf> = None;
        let mut out_dir: Option<PathBuf> = None;
        let mut threshold: Option<f32> = None;
        let mut dilate: Option<u32> = None;
        let mut overwrite = false;

        let mut i = 0usize;
        while i < raw.len() {
            match raw[i].as_str() {
                "--model" => {
                    i += 1;
                    model = Some(PathBuf::from(next(&raw, i, "--model")?));
                }
                "--metadata" => {
                    i += 1;
                    metadata = Some(PathBuf::from(next(&raw, i, "--metadata")?));
                }
                "--art" => {
                    i += 1;
                    art = Some(PathBuf::from(next(&raw, i, "--art")?));
                }
                "--out" => {
                    i += 1;
                    out = Some(PathBuf::from(next(&raw, i, "--out")?));
                }
                "--art-dir" => {
                    i += 1;
                    art_dir = Some(PathBuf::from(next(&raw, i, "--art-dir")?));
                }
                "--out-dir" => {
                    i += 1;
                    out_dir = Some(PathBuf::from(next(&raw, i, "--out-dir")?));
                }
                "--threshold" => {
                    i += 1;
                    let value = next(&raw, i, "--threshold")?;
                    threshold = Some(
                        value
                            .parse::<f32>()
                            .map_err(|e| format!("--threshold: invalid float: {e}"))?,
                    );
                }
                "--dilate" => {
                    i += 1;
                    let value = next(&raw, i, "--dilate")?;
                    dilate = Some(
                        value
                            .parse::<u32>()
                            .map_err(|e| format!("--dilate: invalid integer: {e}"))?,
                    );
                }
                "--overwrite" => overwrite = true,
                "--help" | "-h" => {
                    print_help();
                    std::process::exit(0);
                }
                other => return Err(format!("unknown argument: {other}")),
            }
            i += 1;
        }

        if art.is_some() && art_dir.is_some() {
            return Err("--art and --art-dir are mutually exclusive".to_string());
        }
        if art.is_none() && art_dir.is_none() {
            return Err("either --art or --art-dir is required".to_string());
        }

        Ok(Args {
            model: model.ok_or("--model is required")?,
            metadata,
            art,
            out,
            art_dir,
            out_dir,
            threshold,
            dilate,
            overwrite,
        })
    }

    fn next<'a>(args: &'a [String], i: usize, flag: &str) -> Result<&'a str, String> {
        args.get(i)
            .map(|s| s.as_str())
            .ok_or_else(|| format!("{flag} requires a value"))
    }

    fn collect_art_images(dir: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
        let mut images = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && is_art_image(&path) {
                images.push(path);
            }
        }
        images.sort_by_key(|path| path.file_name().map(|s| s.to_os_string()));
        Ok(images)
    }

    fn is_art_image(path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| {
                matches!(
                    ext.to_ascii_lowercase().as_str(),
                    "jpg" | "jpeg" | "png" | "webp"
                )
            })
            .unwrap_or(false)
    }

    fn print_help() {
        eprintln!(
            r#"generate_mask — generate black/white effect protection masks from card art

USAGE
  generate_mask --model <ONNX> --art <FILE> --out <PNG> [OPTIONS]
  generate_mask --model <ONNX> --art-dir <DIR> --out-dir <DIR> [OPTIONS]

OPTIONS
  --model <ONNX>      TinyMaskNet ONNX model, e.g. model/ygo-mask-medium-640.onnx
  --metadata <JSON>   optional metadata JSON; defaults to model path with .json extension
  --art <FILE>        single art image
  --out <PNG>         single mask output
  --art-dir <DIR>     batch art directory; scans jpg/png/webp files
  --out-dir <DIR>     batch output directory; writes <stem>.png masks
  --threshold <F>     subject threshold; defaults to model metadata
  --dilate <PX>       subject dilation in model pixels; defaults to model metadata
  --overwrite         replace existing mask outputs
  --help              show this message

Mask semantics: black protects subject pixels from effects; white allows effects.
"#
        );
    }

    fn fatal(msg: &str) -> ! {
        eprintln!("fatal: {msg}");
        std::process::exit(1);
    }
}

#[cfg(feature = "onnx-mask")]
fn main() {
    app::main();
}

#[cfg(not(feature = "onnx-mask"))]
fn main() {
    eprintln!("generate_mask requires the `onnx-mask` feature.");
    eprintln!("Run: cargo run --features onnx-mask --bin generate_mask -- --help");
    std::process::exit(2);
}
