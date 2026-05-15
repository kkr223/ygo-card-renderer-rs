use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use image::{ColorType, ImageBuffer, ImageReader, Rgba, codecs::webp::WebPEncoder};
use resvg::{render, tiny_skia::Pixmap, usvg};
use serde_json::{Value, json};

const MAGIC: &[u8; 4] = b"YGOC";
const VERSION: u32 = 1;
const ATLAS_PADDING: u32 = 2;

const LAYOUT_JSON: &str = r#"{"card":{"width":1394,"height":2031},"base":{"name":{"x":103,"width_with_attribute":1033,"width_without_attribute":1161,"height":200},"attribute":{"x":1163,"y":96},"level":{"asset":"level.webp","star_width":88,"y":247,"right_lt_13":147,"right_ge_13":101,"gap":4,"max":13},"rank":{"asset":"rank.webp","star_width":88,"y":247,"left_lt_13":147,"left_ge_13":101,"gap":4,"max":13},"spell_trap":{"icon_asset_prefix":"icon-","icon_asset_suffix":".webp"},"image":{"normal":{"x":170,"y":375,"width":1054,"height":1054},"pendulum":{"x":94,"y":364,"width":1205,"height":1205}},"mask":{"normal":{"asset":"card-mask.webp","x":117,"y":322},"pendulum":{"asset":"card-mask-pendulum.webp","x":68,"y":342}},"out_frame":{"image":{"x":105,"y":311,"width":1184,"height":1183},"name_block":{"asset":"name-block.webp","x":76,"y":82},"effect_box":{"asset":"eblock-border.webp","x":77,"y":1501},"effect_box_colored":{"asset":"eblock-border-o.webp","x":77,"y":1501}},"pendulum_scale":{"left":{"astral":{"x":144,"y":1389},"default":{"x":145,"y":1370}},"right":{"astral":{"x":1250,"y":1389},"default":{"x":1249,"y":1370}}},"pendulum_description":{"x":221,"y":0,"width":950,"height":230},"package":{"font_family":"ygo-password","font_size":40,"pendulum":{"x":116,"y":1859},"default":{"right":148,"y":1455},"link":{"right":252,"y":1455}},"link_arrows":{"up":{"on":{"asset":"arrow-up-on.webp","x":555,"y":278},"off":{"asset":"arrow-up-off.webp","x":555,"y":278}},"right_up":{"on":{"asset":"arrow-right-up-on.webp","x":1130,"y":299},"off":{"asset":"arrow-right-up-off.webp","x":1130,"y":299}},"right":{"on":{"asset":"arrow-right-on.webp","x":1223,"y":761},"off":{"asset":"arrow-right-off.webp","x":1223,"y":761}},"right_down":{"on":{"asset":"arrow-right-down-on.webp","x":1130,"y":1336},"off":{"asset":"arrow-right-down-off.webp","x":1130,"y":1336}},"down":{"on":{"asset":"arrow-down-on.webp","x":555,"y":1428},"off":{"asset":"arrow-down-off.webp","x":555,"y":1428}},"left_down":{"on":{"asset":"arrow-left-down-on.webp","x":95,"y":1336},"off":{"asset":"arrow-left-down-off.webp","x":95,"y":1336}},"left":{"on":{"asset":"arrow-left-on.webp","x":71,"y":758},"off":{"asset":"arrow-left-off.webp","x":71,"y":758}},"left_up":{"on":{"asset":"arrow-left-up-on.webp","x":95,"y":299},"off":{"asset":"arrow-left-up-off.webp","x":95,"y":299}}},"effect":{"x":109,"width":1175,"height":100},"description":{"x":109,"width":1175,"base_height":385,"atk_bar_height":60},"atk_def_link":{"background":{"x":109,"y":1844},"atk":{"astral":{"x":898,"y":1850,"font_size":49},"default":{"x":999,"y":1846,"font_size":62}},"def":{"astral":{"x":1279,"y":1850,"font_size":49},"default":{"x":1282,"y":1846,"font_size":62}},"link":{"astral":{"x":1279,"y":1850,"font_size":49},"default":{"x":1274,"y":1860,"font_size":44,"scale_x":1.3}}},"password":{"x":66,"y":1932,"font_family":"ygo-password","font_size":40},"copyright":{"right":141,"y":1936},"laser":{"x":1276,"y":1913},"attribute_rare":{"asset":"attribute-rare.webp","x":1163,"y":96},"twentieth":{"asset":"20th.webp","x":472,"y":1532},"twenty_fifth":{"asset":"25th.webp","x":503,"y":1496}},"styles":{"sc":{"fontFamily":"ygo-sc","name":{"top":107,"fontSize":108},"spellTrap":{"top":254,"fontSize":76,"right":134,"letterSpacing":2,"icon":{"marginTop":8,"marginLeft":10}},"pendulumDescription":{"top":1282,"fontSize":36,"letterSpacing":2,"lineHeight":1.2},"effect":{"top":1528,"fontSize":44,"letterSpacing":2,"lineHeight":1.2},"description":{"fontSize":36,"letterSpacing":2,"lineHeight":1.2}},"tc":{"fontFamily":"ygo-tc","name":{"top":91,"fontSize":108},"spellTrap":{"top":250,"fontSize":76,"right":138,"icon":{"marginTop":12,"marginLeft":10}},"pendulumDescription":{"top":1280,"fontSize":36,"lineHeight":1.2},"effect":{"top":1525,"fontSize":44,"lineHeight":1.2,"minHeight":10},"description":{"fontSize":36,"lineHeight":1.2}},"jp":{"fontFamily":"ygo-jp","name":{"top":98,"fontSize":108,"rtFontSize":20,"rtTop":-2},"spellTrap":{"top":253,"fontSize":80,"right":130,"icon":{"marginTop":10},"rtFontSize":20,"rtTop":-8,"rtFontScaleX":1.2},"pendulumDescription":{"top":1288,"fontSize":36,"lineHeight":1.17,"rtFontSize":12,"rtTop":-5},"effect":{"top":1528,"fontSize":46,"lineHeight":1.17,"textIndent":-18.4,"minHeight":16,"rtFontSize":14,"rtTop":-6},"description":{"fontSize":38,"lineHeight":1.17,"rtFontSize":13,"rtTop":-6}},"kr":{"fontFamily":"ygo-kr","name":{"fontFamily":"ygo-kr-name","top":90,"fontSize":106,"letterSpacing":4,"wordSpacing":-20,"rtFontSize":18,"rtTop":6},"spellTrap":{"fontFamily":"ygo-kr-race","top":253,"fontSize":88,"wordSpacing":5,"scaleY":0.75,"right":142,"icon":{"marginTop":6,"marginLeft":12,"marginRight":12}},"pendulumDescription":{"top":1282,"fontSize":36,"lineHeight":1.19,"wordSpacing":5},"effect":{"fontFamily":"ygo-kr-race","top":1526,"fontSize":48,"lineHeight":1.19,"wordSpacing":12,"minHeight":8},"description":{"fontSize":36,"lineHeight":1.19,"wordSpacing":5}},"en":{"fontFamily":"ygo-en","name":{"fontFamily":"ygo-en-name","top":52,"fontSize":158,"letterSpacing":1},"spellTrap":{"fontFamily":"ygo-en-race","top":254,"fontSize":74,"right":145,"letterSpacing":1,"icon":{"marginTop":10,"marginLeft":10}},"pendulumDescription":{"top":1282,"fontSize":42,"lineHeight":1.02},"effect":{"fontFamily":"ygo-en-race","top":1527,"fontSize":56,"letterSpacing":1,"lineHeight":1.02},"description":{"fontSize":42,"lineHeight":1.02,"smallFontSize":36}},"astral":{"fontFamily":"ygo-astral","name":{"top":107,"fontSize":103},"spellTrap":{"top":258,"fontSize":76,"right":144,"icon":{"marginTop":4}},"pendulumDescription":{"top":1284,"fontSize":42,"lineHeight":1.04},"effect":{"top":1533,"fontSize":44,"lineHeight":1.04},"description":{"fontSize":42,"lineHeight":1.04}},"custom1":{"fontFamily":"custom1","name":{"top":92,"fontSize":108},"spellTrap":{"top":250,"fontSize":76,"right":110,"icon":{"marginTop":12,"marginLeft":10}},"pendulumDescription":{"top":1279,"fontSize":38,"lineHeight":1.15},"effect":{"top":1525,"fontSize":46,"lineHeight":1.15,"textIndent":-18.4,"minHeight":10},"description":{"fontSize":38,"lineHeight":1.15}},"custom2":{"fontFamily":"custom2","name":{"top":92,"fontSize":108},"spellTrap":{"top":250,"fontSize":76,"right":104,"icon":{"marginTop":12,"marginLeft":10}},"pendulumDescription":{"top":1280,"fontSize":36,"lineHeight":1.2},"effect":{"top":1525,"fontSize":44,"lineHeight":1.2,"textIndent":-17.6,"minHeight":10},"description":{"fontSize":36,"lineHeight":1.2}}},"resource_rules":{"base_image":"yugioh/image","card_asset":"card-{cardType}.webp","pendulum_asset":"card-{pendulumType}.webp","attribute_asset":"attribute-{attribute}{suffix}.webp","spell_trap_attribute_asset":"attribute-{type}{suffix}.webp","rare_asset":"rare-{rare}{suffix}.webp","copyright_asset":"copyright-{copyright}-{color}.svg","laser_asset":"{laser}.webp","atk_def_asset":{"default":"atk-def.svg","astral":"atk-def-astral.svg"},"atk_link_asset":{"default":"atk-link.svg","astral":"atk-link-astral.svg"}},"sources":{"component":"yugioh-card/packages/src/yugioh-card/index.js","styles":["yugioh-card/packages/src/yugioh-card/style/sc-style.js","yugioh-card/packages/src/yugioh-card/style/tc-style.js","yugioh-card/packages/src/yugioh-card/style/jp-style.js","yugioh-card/packages/src/yugioh-card/style/kr-style.js","yugioh-card/packages/src/yugioh-card/style/en-style.js","yugioh-card/packages/src/yugioh-card/style/astral-style.js","yugioh-card/packages/src/yugioh-card/style/custom1-style.js","yugioh-card/packages/src/yugioh-card/style/custom2-style.js"]}"#;

#[derive(Debug, Clone)]
struct Args {
    root: PathBuf,
    out: PathBuf,
    atlas_width: u32,
    max_sprite_dim: u32,
    max_sprite_area: u32,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_args()?.resolve()?;
    build_bundle(&args)?;
    Ok(())
}

fn parse_args() -> Result<Args, String> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut root = manifest_dir.join("assets/yugioh");
    let mut out = manifest_dir.join("resources/yugioh_bundle.bin");
    let mut atlas_width = 2048u32;
    let mut max_sprite_dim = 320u32;
    let mut max_sprite_area = 100000u32;

    let raw: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < raw.len() {
        match raw[i].as_str() {
            "--root" => {
                i += 1;
                root = PathBuf::from(next(&raw, i, "--root")?);
            }
            "--out" => {
                i += 1;
                out = PathBuf::from(next(&raw, i, "--out")?);
            }
            "--atlas-width" => {
                i += 1;
                atlas_width = next(&raw, i, "--atlas-width")?
                    .parse()
                    .map_err(|e| format!("--atlas-width: {e}"))?;
            }
            "--max-sprite-dim" => {
                i += 1;
                max_sprite_dim = next(&raw, i, "--max-sprite-dim")?
                    .parse()
                    .map_err(|e| format!("--max-sprite-dim: {e}"))?;
            }
            "--max-sprite-area" => {
                i += 1;
                max_sprite_area = next(&raw, i, "--max-sprite-area")?
                    .parse()
                    .map_err(|e| format!("--max-sprite-area: {e}"))?;
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
        root,
        out,
        atlas_width,
        max_sprite_dim,
        max_sprite_area,
    })
}

impl Args {
    fn resolve(mut self) -> Result<Self, Box<dyn std::error::Error>> {
        self.root = self.root.canonicalize()?;
        self.out = absolutize_output_path(&self.out)?;
        Ok(self)
    }
}

fn next<'a>(args: &'a [String], i: usize, flag: &str) -> Result<&'a str, String> {
    args.get(i)
        .map(|s| s.as_str())
        .ok_or_else(|| format!("{flag} requires a value"))
}

fn print_help() {
    println!(
        "build_bundle --root <DIR> --out <FILE> --atlas-width <N> --max-sprite-dim <N> --max-sprite-area <N>"
    );
}

fn build_bundle(args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    let image_dir = args.root.join("image");
    let font_dir = args.root.join("font");
    if !image_dir.exists() {
        return Err(format!("image dir not found: {}", image_dir.display()).into());
    }
    if !font_dir.exists() {
        return Err(format!("font dir not found: {}", font_dir.display()).into());
    }

    let image_filelist = load_filelist(&image_dir)?;
    let font_filelist = load_filelist(&font_dir)?;

    let image_entries = if let Some(filelist) = image_filelist {
        filelist
    } else {
        let mut entries = Vec::new();
        for p in read_sorted_files(&image_dir)? {
            if matches_ext(&p, &["webp", "svg"]) {
                let name = file_name(&p)?.to_string();
                entries.push((name, p.strip_prefix(&image_dir)?.to_path_buf()));
            }
        }
        entries
    };
    let font_entries = if let Some(filelist) = font_filelist {
        filelist
    } else {
        let mut entries = Vec::new();
        for p in read_sorted_files(&font_dir)? {
            if p.is_file() && matches_ext(&p, &["woff2", "woff", "ttf", "otf"]) {
                let name = stem(&p)?.to_string();
                entries.push((name, p.strip_prefix(&font_dir)?.to_path_buf()));
            }
        }
        entries
    };

    let mut payload = Vec::new();
    let mut images = serde_json::Map::new();
    let mut fonts = serde_json::Map::new();

    let mut sprite_entries = Vec::new();
    let mut standalone_raster = Vec::new();
    let mut svg_entries = Vec::new();
    let mut seen_names = HashSet::new();
    for (name, rel) in &image_entries {
        let p = image_dir.join(rel);
        ensure_unique_name(&mut seen_names, &name, "image")?;
        validate_supported_image(&p)?;
        if matches_ext(&p, &["svg"]) {
            svg_entries.push((name.clone(), rel.clone()));
        } else if is_small_sprite(&p, args.max_sprite_dim, args.max_sprite_area)? {
            sprite_entries.push((name.clone(), rel.clone()));
        } else {
            standalone_raster.push((name.clone(), rel.clone()));
        }
    }

    let mut atlas_meta =
        json!({"buffer": serde_json::Value::Null, "width": 0, "height": 0, "sprites": {}});
    if !sprite_entries.is_empty() {
        println!(
            "Packing {} small raster images into sprite atlas...",
            sprite_entries.len()
        );
        let (buf, sprites, size) = pack_atlas(&image_dir, &sprite_entries, args.atlas_width)?;
        atlas_meta["buffer"] = buffer_ptr(&mut payload, &buf);
        atlas_meta["width"] = json!(size.0);
        atlas_meta["height"] = json!(size.1);
        atlas_meta["sprites"] = serde_json::to_value(sprites)?;
    }

    for (name, _) in &sprite_entries {
        images.insert(
            name.clone(),
            json!({"kind":"raster","storage":"atlas","atlas": atlas_meta["sprites"][name].clone()}),
        );
    }
    let standalone_count = standalone_raster.len();
    println!("Packing {standalone_count} standalone raster images...");
    for (name, rel) in standalone_raster {
        let p = image_dir.join(&rel);
        let (w, h) = image_size(&p)?;
        let b = fs::read(&p)?;
        images.insert(name, json!({"kind":"raster","storage":"buffer","size":{"w":w,"h":h},"buffer": buffer_ptr(&mut payload,&b)}));
    }
    let svg_count = svg_entries.len();
    println!("Rasterizing {} SVG images...", svg_count);
    for (name, rel) in svg_entries {
        let p = image_dir.join(rel);
        let (webp, w, h) = rasterize_svg(&p)?;
        images.insert(name, json!({"kind":"raster","storage":"buffer","size":{"w":w,"h":h},"buffer": buffer_ptr(&mut payload,&webp)}));
    }

    // Pre-compute derived effect masks so the renderer can load them directly
    // instead of scanning authored assets at runtime.
    let layout_bytes =
        build_layout_with_generated_masks(&image_dir, &image_entries, &mut images, &mut payload)?;

    let layout_info = json!({"buffer": buffer_ptr(&mut payload, &layout_bytes)});

    println!("Packing {} font files...", font_entries.len());
    let mut seen_font_names = HashSet::new();
    for (name, rel) in font_entries {
        ensure_unique_name(&mut seen_font_names, &name, "font")?;
        let p = font_dir.join(&rel);
        validate_supported_font(&p)?;
        let b = fs::read(&p)?;
        fonts.insert(
            name,
            json!({"file": rel.to_string_lossy(), "buffer": buffer_ptr(&mut payload, &b)}),
        );
    }

    let font_list: Value = if font_dir.join("font-list.json").exists() {
        serde_json::from_slice(&fs::read(font_dir.join("font-list.json"))?)?
    } else {
        Value::Array(vec![])
    };
    let index = json!({
        "meta": {"root": args.root.to_string_lossy(), "version": VERSION, "atlas_width": args.atlas_width, "max_sprite_dim": args.max_sprite_dim, "max_sprite_area": args.max_sprite_area, "counts": {"sprite_raster": sprite_entries.len(), "standalone_raster": standalone_count, "svg": svg_count, "fonts": fonts.len()}},
        "atlas": atlas_meta,
        "layout": layout_info,
        "images": images,
        "fonts": fonts,
        "font_list": font_list,
    });

    let json_bytes = serde_json::to_vec(&index)?;
    if let Some(parent) = args.out.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    let mut out = Vec::with_capacity(12 + json_bytes.len() + payload.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(&json_bytes);
    out.extend_from_slice(&payload);
    println!("Writing bundle: {}", args.out.display());
    fs::write(&args.out, out)?;
    println!("Done!");
    println!("Index size: {} bytes", json_bytes.len());
    println!(
        "Payload size: {:.2} MB",
        payload.len() as f64 / 1024.0 / 1024.0
    );
    Ok(())
}

fn load_filelist(dir: &Path) -> Result<Option<Vec<(String, PathBuf)>>, Box<dyn std::error::Error>> {
    for name in ["filelist.json", "filelist.csv", "filelist.tsv", "filelist"] {
        let path = dir.join(name);
        if path.exists() {
            return Ok(Some(parse_filelist(&path, dir)?));
        }
    }
    Ok(None)
}

fn parse_filelist(
    path: &Path,
    base: &Path,
) -> Result<Vec<(String, PathBuf)>, Box<dyn std::error::Error>> {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let text = fs::read_to_string(path)?;
    let mut entries = Vec::new();
    if ext == "json" {
        let v: Value = serde_json::from_str(&text)?;
        let arr = v.as_array().ok_or("filelist.json must be an array")?;
        for item in arr {
            match item {
                Value::Object(m) => {
                    let name = m
                        .get("name")
                        .and_then(|v| v.as_str())
                        .ok_or("filelist json entry missing name")?;
                    let rel = m
                        .get("path")
                        .and_then(|v| v.as_str())
                        .ok_or("filelist json entry missing path")?;
                    entries.push((name.to_string(), PathBuf::from(rel)));
                }
                Value::Array(a) if a.len() == 2 => {
                    entries.push((
                        a[0].as_str()
                            .ok_or("filelist json name must be string")?
                            .to_string(),
                        PathBuf::from(a[1].as_str().ok_or("filelist json path must be string")?),
                    ));
                }
                _ => return Err("unsupported filelist.json entry".into()),
            }
        }
    } else {
        let mut saw_header = false;
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let sep = if ext == "tsv" { '\t' } else { ',' };
            let parts: Vec<_> = line.split(sep).map(|s| s.trim()).collect();
            if !saw_header
                && parts.len() == 2
                && matches!(parts[0], "name" | "resource")
                && parts[1] == "path"
            {
                saw_header = true;
                continue;
            }
            if parts.len() != 2 {
                return Err(format!("invalid filelist line: {line}").into());
            }
            saw_header = true;
            entries.push((parts[0].to_string(), PathBuf::from(parts[1])));
        }
    }
    for (_, rel) in &entries {
        let p = base.join(rel);
        if !p.exists() {
            return Err(format!("filelist entry missing file: {}", p.display()).into());
        }
    }
    Ok(entries)
}

fn read_sorted_files(dir: &Path) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut paths: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file())
        .collect();
    paths.sort();
    Ok(paths)
}

fn ensure_unique_name(
    seen: &mut HashSet<String>,
    name: &str,
    kind: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if !seen.insert(name.to_string()) {
        return Err(format!("duplicate {kind} resource name: {name}").into());
    }
    Ok(())
}

fn validate_supported_image(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if !matches_ext(path, &["webp", "svg"]) {
        return Err(format!("unsupported image extension: {}", path.display()).into());
    }
    Ok(())
}

fn validate_supported_font(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if !matches_ext(path, &["woff2", "woff", "ttf", "otf"]) {
        return Err(format!("unsupported font extension: {}", path.display()).into());
    }
    Ok(())
}

fn buffer_ptr(payload: &mut Vec<u8>, data: &[u8]) -> Value {
    let offset = payload.len() as u32;
    payload.extend_from_slice(data);
    json!({"offset": offset, "len": data.len() as u32})
}

fn build_layout_with_generated_masks(
    image_dir: &Path,
    image_entries: &[(String, PathBuf)],
    images: &mut serde_json::Map<String, Value>,
    payload: &mut Vec<u8>,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut json_str = LAYOUT_JSON.to_string();
    json_str.push('}');
    let mut layout: Value = serde_json::from_str(&json_str)?;

    add_pendulum_art_effect_mask(image_dir, image_entries, &mut layout, images, payload)?;
    add_pendulum_border_effect_mask(image_dir, image_entries, &mut layout);

    let Some(arrows) = layout["base"]["link_arrows"].as_object_mut() else {
        return Ok(layout.to_string().into_bytes());
    };
    for (dir, arrow) in arrows.iter_mut() {
        let on_asset = arrow["on"]["asset"].as_str().unwrap_or("");
        let off_asset = arrow["off"]["asset"].as_str().unwrap_or("");
        let on_path = image_dir.join(on_asset);
        let off_path = image_dir.join(off_asset);
        if !on_path.exists() || !off_path.exists() {
            continue;
        }
        let Ok(on_rgba) = image::open(&on_path).map(|img| img.to_rgba8()) else {
            continue;
        };
        let Ok(off_rgba) = image::open(&off_path).map(|img| img.to_rgba8()) else {
            continue;
        };
        if on_rgba.dimensions() != off_rgba.dimensions() {
            continue;
        }
        let (w, h) = on_rgba.dimensions();
        let mut mask = ImageBuffer::<Rgba<u8>, _>::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let o = on_rgba.get_pixel(x, y);
                let f = off_rgba.get_pixel(x, y);
                let dr = o[0] as i32 - f[0] as i32;
                let dg = o[1] as i32 - f[1] as i32;
                let db = o[2] as i32 - f[2] as i32;
                let diff = dr.abs() + dg.abs() + db.abs();
                let a = if diff > 120 { 255u8 } else { 0u8 };
                mask.put_pixel(x, y, Rgba([a, a, a, a]));
            }
        }
        let mask_name = format!("arrow-{dir}-red.webp");
        let mut webp_buf = Vec::new();
        let encoder = WebPEncoder::new_lossless(&mut webp_buf);
        encoder.encode(&mask, w, h, ColorType::Rgba8.into())?;
        images.insert(
            mask_name.clone(),
            json!({"kind":"raster","storage":"buffer","size":{"w":w,"h":h},"buffer": buffer_ptr(payload, &webp_buf)}),
        );
        arrow["red_mask"] =
            json!({"asset": mask_name, "x": arrow["on"]["x"], "y": arrow["on"]["y"]});
    }
    Ok(layout.to_string().into_bytes())
}

fn add_pendulum_border_effect_mask(
    image_dir: &Path,
    image_entries: &[(String, PathBuf)],
    layout: &mut Value,
) {
    const ASSET: &str = "rare-pser-print-pendulum.webp";
    if resource_path(image_dir, image_entries, ASSET).is_some() {
        layout["base"]["mask"]["pendulum_border"] = json!({"asset": ASSET, "x": 0, "y": 0});
    }
}

fn add_pendulum_art_effect_mask(
    image_dir: &Path,
    image_entries: &[(String, PathBuf)],
    layout: &mut Value,
    images: &mut serde_json::Map<String, Value>,
    payload: &mut Vec<u8>,
) -> Result<(), Box<dyn std::error::Error>> {
    let art = &layout["base"]["image"]["pendulum"];
    let frame_mask = &layout["base"]["mask"]["pendulum"];
    let art_x = json_u32(art, "x")?;
    let art_y = json_u32(art, "y")?;
    let art_w = json_u32(art, "width")?;
    let art_h = json_u32(art, "height")?;
    let mask_x = json_u32(frame_mask, "x")?;
    let mask_y = json_u32(frame_mask, "y")?;
    let mask_asset = frame_mask["asset"]
        .as_str()
        .ok_or("missing pendulum mask asset")?;
    let Some(mask_path) = resource_path(image_dir, image_entries, mask_asset) else {
        return Ok(());
    };

    let frame_rgba = image::open(&mask_path)?.to_rgba8();
    let (mask_w, mask_h) = frame_rgba.dimensions();
    let x0 = art_x.max(mask_x);
    let y0 = art_y.max(mask_y);
    let x1 = art_x
        .saturating_add(art_w)
        .min(mask_x.saturating_add(mask_w));
    let y1 = art_y
        .saturating_add(art_h)
        .min(mask_y.saturating_add(mask_h));
    if x0 >= x1 || y0 >= y1 {
        return Ok(());
    }

    let w = x1 - x0;
    let h = y1 - y0;
    let mut mask = ImageBuffer::<Rgba<u8>, _>::new(w, h);
    for local_y in 0..h {
        let src_y = y0 + local_y - mask_y;
        for local_x in 0..w {
            let src_x = x0 + local_x - mask_x;
            let alpha = frame_rgba.get_pixel(src_x, src_y)[3];
            // Only the transparent hole of card-mask-pendulum is real art.
            // Semi-transparent pendulum scale/effect panels are frame pixels,
            // so they must not receive Art-target rare effects.
            let allow = if alpha <= 8 { 255u8 } else { 0u8 };
            mask.put_pixel(local_x, local_y, Rgba([allow, allow, allow, allow]));
        }
    }

    let mask_name = "card-mask-pendulum-art-effect.webp".to_string();
    let mut webp_buf = Vec::new();
    let encoder = WebPEncoder::new_lossless(&mut webp_buf);
    encoder.encode(&mask, w, h, ColorType::Rgba8.into())?;
    images.insert(
        mask_name.clone(),
        json!({"kind":"raster","storage":"buffer","size":{"w":w,"h":h},"buffer": buffer_ptr(payload, &webp_buf)}),
    );
    layout["base"]["mask"]["pendulum_art"] = json!({"asset": mask_name, "x": x0, "y": y0});
    Ok(())
}

fn resource_path(
    image_dir: &Path,
    image_entries: &[(String, PathBuf)],
    asset: &str,
) -> Option<PathBuf> {
    image_entries
        .iter()
        .find(|(name, _)| name == asset)
        .map(|(_, rel)| image_dir.join(rel))
}

fn json_u32(value: &Value, key: &str) -> Result<u32, Box<dyn std::error::Error>> {
    value[key]
        .as_u64()
        .and_then(|v| u32::try_from(v).ok())
        .ok_or_else(|| format!("missing or invalid u32 field: {key}").into())
}

fn matches_ext(path: &Path, exts: &[&str]) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| exts.iter().any(|e| s.eq_ignore_ascii_case(e)))
}
fn file_name(path: &Path) -> Result<&str, Box<dyn std::error::Error>> {
    Ok(path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("invalid file name")?)
}
fn stem(path: &Path) -> Result<&str, Box<dyn std::error::Error>> {
    Ok(path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or("invalid file stem")?)
}
fn image_size(path: &Path) -> Result<(u32, u32), Box<dyn std::error::Error>> {
    Ok(ImageReader::open(path)?
        .with_guessed_format()?
        .into_dimensions()?)
}

fn absolutize_output_path(path: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    Ok(std::env::current_dir()?.join(path))
}
fn is_small_sprite(
    path: &Path,
    max_dim: u32,
    max_area: u32,
) -> Result<bool, Box<dyn std::error::Error>> {
    let (w, h) = image_size(path)?;
    Ok(w <= max_dim && h <= max_dim && w.saturating_mul(h) <= max_area)
}

fn pack_atlas(
    base: &Path,
    sprite_entries: &[(String, PathBuf)],
    atlas_width: u32,
) -> Result<(Vec<u8>, serde_json::Map<String, Value>, (u32, u32)), Box<dyn std::error::Error>> {
    let mut images = Vec::new();
    for (_, rel) in sprite_entries {
        let path = base.join(rel);
        let img = ImageReader::open(&path)?
            .with_guessed_format()?
            .decode()?
            .to_rgba8();
        images.push((path, img));
    }
    let estimated_height = images
        .iter()
        .map(|(_, i)| i.height() + ATLAS_PADDING)
        .sum::<u32>()
        + ATLAS_PADDING;
    let mut atlas: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_pixel(
        atlas_width,
        estimated_height.max(atlas_width),
        Rgba([0, 0, 0, 0]),
    );
    let mut x = ATLAS_PADDING;
    let mut y = ATLAS_PADDING;
    let mut row_height = 0u32;
    let mut sprites = serde_json::Map::new();
    for ((name, _), (p, img)) in sprite_entries.iter().zip(images.into_iter()) {
        let (w, h) = (img.width(), img.height());
        if w + ATLAS_PADDING * 2 > atlas_width {
            return Err(format!(
                "sprite {} is too wide ({w}px) for atlas width {atlas_width}",
                p.display()
            )
            .into());
        }
        if x + w + ATLAS_PADDING > atlas_width {
            x = ATLAS_PADDING;
            y += row_height + ATLAS_PADDING;
            row_height = 0;
        }
        for yy in 0..h {
            for xx in 0..w {
                atlas.put_pixel(x + xx, y + yy, *img.get_pixel(xx, yy));
            }
        }
        sprites.insert(name.clone(), json!({"x":x,"y":y,"w":w,"h":h}));
        x += w + ATLAS_PADDING;
        row_height = row_height.max(h);
    }
    let used_height = y + row_height + ATLAS_PADDING;
    let atlas = image::imageops::crop_imm(&atlas, 0, 0, atlas_width, used_height).to_image();
    let mut buf = Vec::new();
    WebPEncoder::new_lossless(&mut buf).encode(
        &atlas,
        atlas.width(),
        atlas.height(),
        ColorType::Rgba8.into(),
    )?;
    Ok((buf, sprites, (atlas.width(), atlas.height())))
}

fn rasterize_svg(path: &Path) -> Result<(Vec<u8>, u32, u32), Box<dyn std::error::Error>> {
    let data = fs::read(path)?;
    let opt = usvg::Options::default();
    let rtree = usvg::Tree::from_data(&data, &opt)?;
    let size = rtree.size().to_int_size();
    let mut pixmap = Pixmap::new(size.width(), size.height()).ok_or("failed to create pixmap")?;
    render(&rtree, usvg::Transform::default(), &mut pixmap.as_mut());
    let mut buf = Vec::new();
    let rgba = unpremultiply_rgba(pixmap.data());
    WebPEncoder::new_lossless(&mut buf).encode(
        &rgba,
        pixmap.width(),
        pixmap.height(),
        ColorType::Rgba8.into(),
    )?;
    Ok((buf, pixmap.width(), pixmap.height()))
}

fn unpremultiply_rgba(data: &[u8]) -> Vec<u8> {
    let mut out = data.to_vec();
    for px in out.chunks_exact_mut(4) {
        let alpha = px[3];
        if alpha == 0 {
            px[0] = 0;
            px[1] = 0;
            px[2] = 0;
        } else if alpha < 255 {
            let a = alpha as u32;
            px[0] = ((px[0] as u32 * 255 + a / 2) / a).min(255) as u8;
            px[1] = ((px[1] as u32 * 255 + a / 2) / a).min(255) as u8;
            px[2] = ((px[2] as u32 * 255 + a / 2) / a).min(255) as u8;
        }
    }
    out
}
