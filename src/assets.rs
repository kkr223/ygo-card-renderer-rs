use std::{fs, path::Path};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use resvg::usvg;

use crate::{
    card_logic::{
        attribute_rel_path, background_rel_path_rush, background_rel_path_yugioh, uses_rank,
    },
    constants::{RUSH_DUEL_FONT_SPECS, YUGIOH_FONT_SPECS},
    model::{CardKind, RenderError, RenderRequest},
};

pub(crate) fn resolve_background_data_uri(
    request: &RenderRequest,
) -> Result<Option<String>, RenderError> {
    let relative = match request.kind {
        CardKind::Yugioh => background_rel_path_yugioh(&request.card),
        CardKind::RushDuel => background_rel_path_rush(&request.card),
    };
    let path = request.options.resource_path.join(relative);
    if path.exists() {
        Ok(Some(read_data_uri(&path)?))
    } else {
        Ok(None)
    }
}

pub(crate) fn resolve_mask_data_uri(
    request: &RenderRequest,
) -> Result<Option<String>, RenderError> {
    let relative = if request.card.is_pendulum() {
        "yugioh/image/card-mask-pendulum.png"
    } else {
        "yugioh/image/card-mask.png"
    };
    let path = request.options.resource_path.join(relative);
    if path.exists() {
        Ok(Some(read_data_uri(&path)?))
    } else {
        Ok(None)
    }
}

pub(crate) fn resolve_attribute_data_uri(
    request: &RenderRequest,
) -> Result<Option<String>, RenderError> {
    let relative = attribute_rel_path(&request.card)?;
    let Some(relative) = relative else {
        return Ok(None);
    };
    let path = request.options.resource_path.join(relative);
    if path.exists() {
        Ok(Some(read_data_uri(&path)?))
    } else {
        Ok(None)
    }
}

pub(crate) fn resolve_level_rank_data_uri(
    request: &RenderRequest,
) -> Result<Option<String>, RenderError> {
    let relative = if uses_rank(&request.card) {
        "yugioh/image/rank.png"
    } else {
        "yugioh/image/level.png"
    };
    let path = request.options.resource_path.join(relative);
    if path.exists() {
        Ok(Some(read_data_uri(&path)?))
    } else {
        Ok(None)
    }
}

pub(crate) fn resolve_atk_def_link_data_uri(
    request: &RenderRequest,
) -> Result<Option<String>, RenderError> {
    let relative = if request.card.is_link() {
        "yugioh/image/atk-link.svg"
    } else {
        "yugioh/image/atk-def.svg"
    };
    let path = request.options.resource_path.join(relative);
    if path.exists() {
        Ok(Some(read_data_uri(&path)?))
    } else {
        Ok(None)
    }
}

pub(crate) fn resolve_copyright_data_uri(
    request: &RenderRequest,
) -> Result<Option<String>, RenderError> {
    let color = if uses_rank(&request.card) {
        "white"
    } else {
        "black"
    };
    let path = request
        .options
        .resource_path
        .join(format!("yugioh/image/copyright-sc-{color}.svg"));
    if path.exists() {
        Ok(Some(read_data_uri(&path)?))
    } else {
        Ok(None)
    }
}

pub(crate) fn embed_font_faces(
    resource_root: &Path,
    kind: CardKind,
) -> Result<String, RenderError> {
    let mut css = String::from("<style>");
    for (family, relative) in font_specs(kind) {
        let path = resource_root.join(relative);
        if !path.exists() {
            continue;
        }
        let href = read_data_uri(&path)?;
        css.push_str(&format!(
            "@font-face{{font-family:'{family}';src:url('{href}') format('woff2');font-weight:normal;font-style:normal;}}"
        ));
    }
    css.push_str("</style>");
    Ok(css)
}

pub(crate) fn load_font_dirs(opt: &mut usvg::Options, resource_root: &Path, kind: CardKind) {
    let fontdb = opt.fontdb_mut();
    fontdb.load_system_fonts();
    match kind {
        CardKind::Yugioh => {
            fontdb.load_fonts_dir(resource_root.join("yugioh").join("font"));
        }
        CardKind::RushDuel => {
            fontdb.load_fonts_dir(resource_root.join("rush-duel").join("font"));
            let yugioh_dir = resource_root.join("yugioh").join("font");
            if yugioh_dir.exists() {
                fontdb.load_fonts_dir(yugioh_dir);
            }
        }
    }
}

pub(crate) fn read_data_uri(path: &Path) -> Result<String, RenderError> {
    let bytes = fs::read(path)?;
    let mime = match path
        .extension()
        .and_then(|item| item.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        _ => "application/octet-stream",
    };
    Ok(format!("data:{mime};base64,{}", STANDARD.encode(bytes)))
}

fn font_specs(kind: CardKind) -> &'static [(&'static str, &'static str)] {
    match kind {
        CardKind::Yugioh => YUGIOH_FONT_SPECS,
        CardKind::RushDuel => RUSH_DUEL_FONT_SPECS,
    }
}
