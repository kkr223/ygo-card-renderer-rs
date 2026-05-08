from __future__ import annotations

import argparse
import io
import json
import struct
from pathlib import Path

from PIL import Image


ROOT_DIR = Path(__file__).resolve().parent.parent  # repo root
YUGIOH_DIR = ROOT_DIR / "assets" / "yugioh"
IMAGE_DIR = YUGIOH_DIR / "image"
FONT_DIR = YUGIOH_DIR / "font"
DEFAULT_OUT = ROOT_DIR / "resources" / "yugioh_bundle.bin"
MAGIC = b"YGOC"
VERSION = 1
ATLAS_PADDING = 2


def build_layout_payload() -> bytes:
    layout = {
        "card": {"width": 1394, "height": 2031},
        "base": {
            "name": {"x": 103, "width_with_attribute": 1033, "width_without_attribute": 1161, "height": 200},
            "attribute": {"x": 1163, "y": 96},
            "level": {"asset": "level.webp", "star_width": 88, "y": 247, "right_lt_13": 147, "right_ge_13": 101, "gap": 4, "max": 13},
            "rank": {"asset": "rank.webp", "star_width": 88, "y": 247, "left_lt_13": 147, "left_ge_13": 101, "gap": 4, "max": 13},
            "spell_trap": {"icon_asset_prefix": "icon-", "icon_asset_suffix": ".webp"},
            "image": {
                "normal": {"x": 170, "y": 375, "width": 1054, "height": 1054},
                "pendulum": {"x": 94, "y": 364, "width": 1205, "height": 1205},
            },
            "mask": {
                "normal": {"asset": "card-mask.webp", "x": 117, "y": 322},
                "pendulum": {"asset": "card-mask-pendulum.webp", "x": 68, "y": 342},
            },
            "out_frame": {
                "image": {"x": 105, "y": 311, "width": 1184, "height": 1183},
                "name_block": {"asset": "name-block.webp", "x": 76, "y": 82},
                "effect_box": {"asset": "eblock-border.webp", "x": 77, "y": 1501},
                "effect_box_colored": {"asset": "eblock-border-o.webp", "x": 77, "y": 1501},
            },
            "pendulum_scale": {
                "left": {"astral": {"x": 144, "y": 1389}, "default": {"x": 145, "y": 1370}},
                "right": {"astral": {"x": 1250, "y": 1389}, "default": {"x": 1249, "y": 1370}},
            },
            "pendulum_description": {"x": 221, "y": 0, "width": 950, "height": 230},
            "package": {
                "font_family": "ygo-password",
                "font_size": 40,
                "pendulum": {"x": 116, "y": 1859},
                "default": {"right": 148, "y": 1455},
                "link": {"right": 252, "y": 1455},
            },
            "link_arrows": {
                "up": {"on": {"asset": "arrow-up-on.webp", "x": 555, "y": 278}, "off": {"asset": "arrow-up-off.webp", "x": 555, "y": 278}},
                "right_up": {"on": {"asset": "arrow-right-up-on.webp", "x": 1130, "y": 299}, "off": {"asset": "arrow-right-up-off.webp", "x": 1130, "y": 299}},
                "right": {"on": {"asset": "arrow-right-on.webp", "x": 1223, "y": 761}, "off": {"asset": "arrow-right-off.webp", "x": 1223, "y": 761}},
                "right_down": {"on": {"asset": "arrow-right-down-on.webp", "x": 1130, "y": 1336}, "off": {"asset": "arrow-right-down-off.webp", "x": 1130, "y": 1336}},
                "down": {"on": {"asset": "arrow-down-on.webp", "x": 555, "y": 1428}, "off": {"asset": "arrow-down-off.webp", "x": 555, "y": 1428}},
                "left_down": {"on": {"asset": "arrow-left-down-on.webp", "x": 95, "y": 1336}, "off": {"asset": "arrow-left-down-off.webp", "x": 95, "y": 1336}},
                "left": {"on": {"asset": "arrow-left-on.webp", "x": 71, "y": 758}, "off": {"asset": "arrow-left-off.webp", "x": 71, "y": 758}},
                "left_up": {"on": {"asset": "arrow-left-up-on.webp", "x": 95, "y": 299}, "off": {"asset": "arrow-left-up-off.webp", "x": 95, "y": 299}},
            },
            "effect": {"x": 109, "width": 1175, "height": 100},
            "description": {"x": 109, "width": 1175, "base_height": 385, "atk_bar_height": 60},
            "atk_def_link": {
                "background": {"x": 109, "y": 1844},
                "atk": {"astral": {"x": 898, "y": 1850, "font_size": 49}, "default": {"x": 999, "y": 1846, "font_size": 62}},
                "def": {"astral": {"x": 1279, "y": 1850, "font_size": 49}, "default": {"x": 1282, "y": 1846, "font_size": 62}},
                "link": {"astral": {"x": 1279, "y": 1850, "font_size": 49}, "default": {"x": 1274, "y": 1860, "font_size": 44, "scale_x": 1.3}},
            },
            "password": {"x": 66, "y": 1932, "font_family": "ygo-password", "font_size": 40},
            "copyright": {"right": 141, "y": 1936},
            "laser": {"x": 1276, "y": 1913},
            "attribute_rare": {"asset": "attribute-rare.webp", "x": 1163, "y": 96},
            "twentieth": {"asset": "20th.webp", "x": 472, "y": 1532},
            "twenty_fifth": {"asset": "25th.webp", "x": 503, "y": 1496},
        },
        "styles": {
            "sc": {
                "fontFamily": "ygo-sc",
                "name": {"top": 107, "fontSize": 108},
                "spellTrap": {"top": 254, "fontSize": 76, "right": 134, "letterSpacing": 2, "icon": {"marginTop": 8, "marginLeft": 10}},
                "pendulumDescription": {"top": 1282, "fontSize": 36, "letterSpacing": 2, "lineHeight": 1.2},
                "effect": {"top": 1528, "fontSize": 44, "letterSpacing": 2, "lineHeight": 1.2},
                "description": {"fontSize": 36, "letterSpacing": 2, "lineHeight": 1.2},
            },
            "tc": {
                "fontFamily": "ygo-tc",
                "name": {"top": 91, "fontSize": 108},
                "spellTrap": {"top": 250, "fontSize": 76, "right": 138, "icon": {"marginTop": 12, "marginLeft": 10}},
                "pendulumDescription": {"top": 1280, "fontSize": 36, "lineHeight": 1.2},
                "effect": {"top": 1525, "fontSize": 44, "lineHeight": 1.2, "minHeight": 10},
                "description": {"fontSize": 36, "lineHeight": 1.2},
            },
            "jp": {
                "fontFamily": "ygo-jp",
                "name": {"top": 98, "fontSize": 108, "rtFontSize": 20, "rtTop": -2},
                "spellTrap": {"top": 253, "fontSize": 80, "right": 130, "icon": {"marginTop": 10}, "rtFontSize": 20, "rtTop": -8, "rtFontScaleX": 1.2},
                "pendulumDescription": {"top": 1288, "fontSize": 36, "lineHeight": 1.17, "rtFontSize": 12, "rtTop": -5},
                "effect": {"top": 1528, "fontSize": 46, "lineHeight": 1.17, "textIndent": -18.4, "minHeight": 16, "rtFontSize": 14, "rtTop": -6},
                "description": {"fontSize": 38, "lineHeight": 1.17, "rtFontSize": 13, "rtTop": -6},
            },
            "kr": {
                "fontFamily": "ygo-kr",
                "name": {"fontFamily": "ygo-kr-name", "top": 90, "fontSize": 106, "letterSpacing": 4, "wordSpacing": -20, "rtFontSize": 18, "rtTop": 6},
                "spellTrap": {"fontFamily": "ygo-kr-race", "top": 253, "fontSize": 88, "wordSpacing": 5, "scaleY": 0.75, "right": 142, "icon": {"marginTop": 6, "marginLeft": 12, "marginRight": 12}},
                "pendulumDescription": {"top": 1282, "fontSize": 36, "lineHeight": 1.19, "wordSpacing": 5},
                "effect": {"fontFamily": "ygo-kr-race", "top": 1526, "fontSize": 48, "lineHeight": 1.19, "wordSpacing": 12, "minHeight": 8},
                "description": {"fontSize": 36, "lineHeight": 1.19, "wordSpacing": 5},
            },
            "en": {
                "fontFamily": "ygo-en",
                "name": {"fontFamily": "ygo-en-name", "top": 52, "fontSize": 158, "letterSpacing": 1},
                "spellTrap": {"fontFamily": "ygo-en-race", "top": 254, "fontSize": 74, "right": 145, "letterSpacing": 1, "icon": {"marginTop": 10, "marginLeft": 10}},
                "pendulumDescription": {"top": 1282, "fontSize": 42, "lineHeight": 1.02},
                "effect": {"fontFamily": "ygo-en-race", "top": 1527, "fontSize": 56, "letterSpacing": 1, "lineHeight": 1.02},
                "description": {"fontSize": 42, "lineHeight": 1.02, "smallFontSize": 36},
            },
            "astral": {
                "fontFamily": "ygo-astral",
                "name": {"top": 107, "fontSize": 103},
                "spellTrap": {"top": 258, "fontSize": 76, "right": 144, "icon": {"marginTop": 4}},
                "pendulumDescription": {"top": 1284, "fontSize": 42, "lineHeight": 1.04},
                "effect": {"top": 1533, "fontSize": 44, "lineHeight": 1.04},
                "description": {"fontSize": 42, "lineHeight": 1.04},
            },
            "custom1": {
                "fontFamily": "custom1",
                "name": {"top": 92, "fontSize": 108},
                "spellTrap": {"top": 250, "fontSize": 76, "right": 110, "icon": {"marginTop": 12, "marginLeft": 10}},
                "pendulumDescription": {"top": 1279, "fontSize": 38, "lineHeight": 1.15},
                "effect": {"top": 1525, "fontSize": 46, "lineHeight": 1.15, "textIndent": -18.4, "minHeight": 10},
                "description": {"fontSize": 38, "lineHeight": 1.15},
            },
            "custom2": {
                "fontFamily": "custom2",
                "name": {"top": 92, "fontSize": 108},
                "spellTrap": {"top": 250, "fontSize": 76, "right": 104, "icon": {"marginTop": 12, "marginLeft": 10}},
                "pendulumDescription": {"top": 1280, "fontSize": 36, "lineHeight": 1.2},
                "effect": {"top": 1525, "fontSize": 44, "lineHeight": 1.2, "textIndent": -17.6, "minHeight": 10},
                "description": {"fontSize": 36, "lineHeight": 1.2},
            },
        },
        "resource_rules": {
            "base_image": "yugioh/image",
            "card_asset": "card-{cardType}.webp",
            "pendulum_asset": "card-{pendulumType}.webp",
            "attribute_asset": "attribute-{attribute}{suffix}.webp",
            "spell_trap_attribute_asset": "attribute-{type}{suffix}.webp",
            "rare_asset": "rare-{rare}{suffix}.webp",
            "copyright_asset": "copyright-{copyright}-{color}.svg",
            "laser_asset": "{laser}.webp",
            "atk_def_asset": {"default": "atk-def.svg", "astral": "atk-def-astral.svg"},
            "atk_link_asset": {"default": "atk-link.svg", "astral": "atk-link-astral.svg"},
        },
        "sources": {
            "component": "yugioh-card/packages/src/yugioh-card/index.js",
            "styles": [
                "yugioh-card/packages/src/yugioh-card/style/sc-style.js",
                "yugioh-card/packages/src/yugioh-card/style/tc-style.js",
                "yugioh-card/packages/src/yugioh-card/style/jp-style.js",
                "yugioh-card/packages/src/yugioh-card/style/kr-style.js",
                "yugioh-card/packages/src/yugioh-card/style/en-style.js",
                "yugioh-card/packages/src/yugioh-card/style/astral-style.js",
                "yugioh-card/packages/src/yugioh-card/style/custom1-style.js",
                "yugioh-card/packages/src/yugioh-card/style/custom2-style.js",
            ],
        },
    }
    return json.dumps(layout, ensure_ascii=False, separators=(",", ":")).encode("utf-8")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Bundle yugioh-card/yugioh images and fonts into one binary."
    )
    parser.add_argument(
        "--root",
        type=Path,
        default=YUGIOH_DIR,
        help="Root folder containing image/ and font/ (default: assets/yugioh).",
    )
    parser.add_argument(
        "--out",
        type=Path,
        default=DEFAULT_OUT,
        help="Output bundle path.",
    )
    parser.add_argument(
        "--atlas-width",
        type=int,
        default=2048,
        help="Atlas width used for small raster images.",
    )
    parser.add_argument(
        "--max-sprite-dim",
        type=int,
        default=320,
        help="Max width or height to be packed into the sprite atlas.",
    )
    parser.add_argument(
        "--max-sprite-area",
        type=int,
        default=100000,
        help="Max pixel area to be packed into the sprite atlas.",
    )
    return parser.parse_args()


def append_bytes(payload: bytearray, data: bytes) -> dict[str, int]:
    offset = len(payload)
    payload.extend(data)
    return {"offset": offset, "len": len(data)}


def read_bytes(path: Path) -> bytes:
    with open(path, "rb") as f:
        return f.read()


def rasterize_svg_to_webp(path: Path) -> tuple[bytes, tuple[int, int]]:
    """Rasterize a fixed-size SVG at build time and encode it as lossless WebP."""
    try:
        import cairosvg  # type: ignore[import-not-found]
    except ImportError as exc:
        raise RuntimeError(
            "SVG rasterization requires the optional Python package 'cairosvg'. "
            "Install it or remove SVG assets before building the bundle."
        ) from exc

    png_bytes = cairosvg.svg2png(bytestring=read_bytes(path))
    with Image.open(io.BytesIO(png_bytes)) as image:
        rgba = image.convert("RGBA")
        size = rgba.size
        buffer = io.BytesIO()
        rgba.save(buffer, format="WEBP", lossless=True, quality=100)
        return buffer.getvalue(), size


def is_small_sprite(path: Path, *, max_dim: int, max_area: int) -> bool:
    with Image.open(path) as image:
        width, height = image.size
    return width <= max_dim and height <= max_dim and width * height <= max_area


def pack_sprite_atlas(
    sprite_paths: list[Path],
    *,
    atlas_width: int,
) -> tuple[bytes, dict[str, dict[str, int]], tuple[int, int]]:
    images: list[tuple[Path, Image.Image]] = []
    for path in sprite_paths:
        with Image.open(path) as image:
            images.append((path, image.convert("RGBA").copy()))

    estimated_height = sum(img.height + ATLAS_PADDING for _, img in images) + ATLAS_PADDING
    atlas = Image.new("RGBA", (atlas_width, max(atlas_width, estimated_height)), (0, 0, 0, 0))

    x = ATLAS_PADDING
    y = ATLAS_PADDING
    row_height = 0
    atlas_entries: dict[str, dict[str, int]] = {}

    for path, image in images:
        width, height = image.size
        if x + width + ATLAS_PADDING > atlas_width:
            x = ATLAS_PADDING
            y += row_height + ATLAS_PADDING
            row_height = 0

        atlas.alpha_composite(image, (x, y))
        atlas_entries[path.name] = {"x": x, "y": y, "w": width, "h": height}

        x += width + ATLAS_PADDING
        row_height = max(row_height, height)

    used_height = y + row_height + ATLAS_PADDING
    atlas = atlas.crop((0, 0, atlas_width, used_height))

    buffer = io.BytesIO()
    atlas.save(buffer, format="WEBP", lossless=True, quality=100)
    return buffer.getvalue(), atlas_entries, atlas.size


def build_bundle(
    *,
    root: Path,
    out_path: Path,
    atlas_width: int,
    max_sprite_dim: int,
    max_sprite_area: int,
) -> None:
    image_dir = root / "image"
    font_dir = root / "font"
    if not image_dir.exists():
        raise FileNotFoundError(f"image dir not found: {image_dir}")
    if not font_dir.exists():
        raise FileNotFoundError(f"font dir not found: {font_dir}")

    raster_paths = sorted(image_dir.glob("*.webp"))
    svg_paths = sorted(image_dir.glob("*.svg"))
    font_paths = sorted(
        path
        for path in font_dir.iterdir()
        if path.is_file() and path.suffix.lower() in {".woff2", ".woff", ".ttf", ".otf"}
    )
    font_list_path = font_dir / "font-list.json"

    sprite_paths: list[Path] = []
    standalone_raster_paths: list[Path] = []
    for path in raster_paths:
        if is_small_sprite(path, max_dim=max_sprite_dim, max_area=max_sprite_area):
            sprite_paths.append(path)
        else:
            standalone_raster_paths.append(path)

    payload = bytearray()
    atlas_info = {
        "buffer": None,
        "width": 0,
        "height": 0,
        "sprites": {},
    }

    if sprite_paths:
        print(f"Packing {len(sprite_paths)} small raster images into sprite atlas...")
        atlas_bytes, sprite_entries, atlas_size = pack_sprite_atlas(
            sprite_paths,
            atlas_width=atlas_width,
        )
        atlas_info["buffer"] = append_bytes(payload, atlas_bytes)
        atlas_info["width"], atlas_info["height"] = atlas_size
        atlas_info["sprites"] = sprite_entries

    layout_bytes = build_layout_payload()
    layout_info = {
        "buffer": append_bytes(payload, layout_bytes),
    }

    images_index: dict[str, dict[str, object]] = {}

    for path in sprite_paths:
        sprite = atlas_info["sprites"][path.name]
        images_index[path.name] = {
            "kind": "raster",
            "storage": "atlas",
            "atlas": sprite,
        }

    print(f"Packing {len(standalone_raster_paths)} standalone raster images...")
    for path in standalone_raster_paths:
        with Image.open(path) as image:
            width, height = image.size
        images_index[path.name] = {
            "kind": "raster",
            "storage": "buffer",
            "size": {"w": width, "h": height},
            "buffer": append_bytes(payload, read_bytes(path)),
        }

    print(f"Rasterizing {len(svg_paths)} SVG images...")
    for path in svg_paths:
        webp_bytes, (width, height) = rasterize_svg_to_webp(path)
        images_index[path.name] = {
            "kind": "raster",
            "storage": "buffer",
            "size": {"w": width, "h": height},
            "buffer": append_bytes(payload, webp_bytes),
        }

    fonts_index: dict[str, dict[str, object]] = {}
    print(f"Packing {len(font_paths)} font files...")
    for path in font_paths:
        fonts_index[path.stem] = {
            "file": path.name,
            "buffer": append_bytes(payload, read_bytes(path)),
        }

    font_list = []
    if font_list_path.exists():
        with open(font_list_path, "r", encoding="utf-8") as f:
            font_list = json.load(f)

    index = {
        "meta": {
            "root": str(root),
            "version": VERSION,
            "atlas_width": atlas_width,
            "max_sprite_dim": max_sprite_dim,
            "max_sprite_area": max_sprite_area,
            "counts": {
                "sprite_raster": len(sprite_paths),
                "standalone_raster": len(standalone_raster_paths),
                "svg": len(svg_paths),
                "fonts": len(font_paths),
            },
        },
        "atlas": atlas_info,
        "layout": layout_info,
        "images": images_index,
        "fonts": fonts_index,
        "font_list": font_list,
    }

    json_bytes = json.dumps(index, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
    out_path.parent.mkdir(parents=True, exist_ok=True)

    print(f"Writing bundle: {out_path}")
    with open(out_path, "wb") as f:
        f.write(MAGIC)
        f.write(struct.pack("<I", VERSION))
        f.write(struct.pack("<I", len(json_bytes)))
        f.write(json_bytes)
        f.write(payload)

    print("Done!")
    print(f"Index size: {len(json_bytes)} bytes")
    print(f"Payload size: {len(payload) / 1024 / 1024:.2f} MB")


def main() -> int:
    args = parse_args()
    build_bundle(
        root=args.root.resolve(),
        out_path=args.out.resolve(),
        atlas_width=args.atlas_width,
        max_sprite_dim=args.max_sprite_dim,
        max_sprite_area=args.max_sprite_area,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
