#!/usr/bin/env python3
"""
SeR (Secret Rare) foil effect — Python tuning prototype.

Usage (from project root):
    uv run python scripts/tune_ser.py

Hot-reload: edit the SER_PARAMS dict or the shader functions below, re-run, and
compare the output in `scripts/export/`.  Once you're happy, port the final
params back to `src/rare_effect.rs` → `draw_secret_weave`.
"""

from __future__ import annotations

import math
import os
import sys
from dataclasses import dataclass, field
from typing import Callable

import numpy as np
from PIL import Image

# ──────────────────────────────────────────────────────────────────────────────
# Paths
# ──────────────────────────────────────────────────────────────────────────────
PROJECT_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RESOURCES_DIR = os.path.join(PROJECT_ROOT, "resources")
OUT_DIR = os.path.join(PROJECT_ROOT, "scripts", "export")
os.makedirs(OUT_DIR, exist_ok=True)
PREVIEW_SCALE = 0.5

# ──────────────────────────────────────────────────────────────────────────────
# Tunable parameters (all in floating-point — will map to Rust equivalents)
# ──────────────────────────────────────────────────────────────────────────────
@dataclass
class SerParams:
    # Virtual point light position (fraction of art rect)
    light_ux: float = 0.76   # x fraction
    light_uy: float = 0.22   # y fraction

    # "#" grid: 2 horizontal + 2 vertical band centres (fraction of art rect)
    band_y0: float = 0.220
    band_y1: float = 0.735
    band_x0: float = 0.245
    band_x1: float = 0.820

    # Band profiles (Gaussian sigmas — fraction of the shorter art dimension)
    core_sigma: float = 0.010   # narrow bright core near the orange cross
    core_sigma_far: float = 0.034
    halo_sigma: float = 0.040    # tighter halo near the orange cross
    halo_sigma_far: float = 0.105

    # Core / halo blend weight
    core_weight: float = 0.48
    halo_weight: float = 0.36
    line_core_lift: float = 0.52
    line_core_lift_far: float = 0.34
    line_core_power: float = 1.32
    line_core_width_mult: float = 0.78
    line_core_width_mult_far: float = 1.18

    # Rainbow glow near light source
    glow_falloff_dist: float = 0.56   # fraction of max_dist where glow reaches zero
    glow_power: float = 3.0           # exponent for near_light attenuation
    glow_strength: float = 0.20       # base glow amplitude
    glow_skirt_falloff: float = 0.72  # wider skirt zero-distance fraction
    glow_skirt_power: float = 4.0
    glow_skirt_strength: float = 0.08

    # Glow blending
    glow_bg_mult: float = 0.16   # outside-band glow multiplier
    glow_skirt_bg_mult: float = 0.10
    glow_inband_mult: float = 0.16  # inside-band glow+skirt multiplier

    # Angular hue dispersion
    glow_hue_base: float = 0.040
    glow_hue_range: float = 0.34   # restrained sweep; near-light is orange-red

    # Diffraction colour gradient: left-top cross point 橙红 → outward 蓝紫
    hue_near: float = 0.035  # left-top intersection (orange-red)
    hue_far: float = 0.780   # far from that intersection (blue-purple)
    hue_origin_x: float = 0.245
    hue_origin_y: float = 0.280
    hue_falloff: float = 0.74
    hue_blue_bias: float = 0.34
    band_saturation: float = 0.98
    band_saturation_far: float = 0.92
    band_value_near: float = 1.00
    band_value_far: float = 0.88
    diffraction_light_z: float = 0.72
    diffraction_view_tilt_x: float = -0.10
    diffraction_view_tilt_y: float = 0.06
    diffraction_period_h_nm: float = 1360.0
    diffraction_period_v_nm: float = 1180.0
    diffraction_bandwidth_nm: float = 44.0
    diffraction_pitch_h: float = 0.135
    diffraction_pitch_v: float = 0.155
    diffraction_coherence: float = 7.5
    diffraction_surface_boost: float = 0.72
    diffraction_preset_boost: float = 0.050
    reflection_cloud_boost: float = 1.18
    reflection_speckle_boost: float = 0.48
    diffraction_texture_mix: float = 0.10
    prism_fragment_strength: float = 0.64
    prism_scan_strength: float = 0.48
    prism_cross_strength: float = 0.78
    prism_hue_jitter: float = 0.20
    prism_value_floor: float = 0.36
    prism_value_boost: float = 1.16
    reflect_color_mix: float = 0.94
    reflect_line_mix: float = 1.58
    reflect_lit_mix: float = 0.54
    reflect_brightness_alpha: float = 0.66
    alpha_lit_boost: float = 0.66
    alpha_line_boost: float = 1.08
    alpha_bright_relief: float = 0.18
    alpha_cap: float = 0.99
    color_overlay_cap: float = 0.98

    # Element boost
    elem_brightness_boost: float = 1.08

    # Overall opacity (maps to opacity param in Rust)
    opacity: float = 1.00

    # Screen blend — luminance compensation
    ink_luma_max: float = 1.16
    ink_visibility_min: float = 0.34

    # Sparkle
    spark_core_threshold: float = 0.58
    spark_probability: int = 58   # out of 0x7ff = 2047

    # Minimum strength to emit a pixel (skip completely below this)
    min_strength: float = 0.012

    # Cell / element geometry
    cell_size: int = 6
    unit_diameter: float = 5.0
    capsule_height: float = 15.0

    # Surface-wide micro foil: vertical rain, diagonal scratches, and pin glitter.
    foil_floor: float = 0.040
    cell_strength: float = 0.28
    vertical_grating_strength: float = 0.060
    diagonal_grating_strength: float = 0.018
    pin_spark_strength: float = 0.16
    dark_region_mult: float = 0.88
    lit_region_mult: float = 1.08

    # Base foil layer: every cell contributes a darkened copy of the underlying
    # art colour. Light later dyes these same units into rainbow foil.
    base_unit_alpha: float = 0.52
    base_unit_darkening: float = 0.46
    base_grating_alpha: float = 0.26
    base_pin_alpha: float = 0.22


SER_PARAMS = SerParams()

# ──────────────────────────────────────────────────────────────────────────────
# Utility: portable pixel hash (matches Rust `pixel_hash`)
# ──────────────────────────────────────────────────────────────────────────────
def pixel_hash(x: int, y: int) -> int:
    h = (x & 0xFFFF) | ((y & 0xFFFF) << 16)
    h = ((h ^ (h >> 8)) * 0x6eed0e9da1f1e3) & 0xFFFFFFFFFFFFFFFF
    h ^= h >> 31
    h = (h * 0x6eed0e9da1f1e3) & 0xFFFFFFFFFFFFFFFF
    h ^= h >> 31
    return h


def hash01(x: int, y: int) -> float:
    return (pixel_hash(x, y) & 0xFFFF) / 65535.0


def value_noise(u: float, v: float, scale: float, seed_x: int, seed_y: int) -> float:
    """Cheap bilinear value noise for broad foil reflectance patches."""
    px = u * scale + seed_x * 17.0
    py = v * scale + seed_y * 19.0
    ix = math.floor(px)
    iy = math.floor(py)
    fx = px - ix
    fy = py - iy
    sx = fx * fx * (3.0 - 2.0 * fx)
    sy = fy * fy * (3.0 - 2.0 * fy)
    a = hash01(ix, iy)
    b = hash01(ix + 1, iy)
    c = hash01(ix, iy + 1)
    d = hash01(ix + 1, iy + 1)
    return (a * (1.0 - sx) + b * sx) * (1.0 - sy) + (c * (1.0 - sx) + d * sx) * sy


def hsv_to_rgb(h: float, s: float, v: float):
    """h, s, v all in [0, 1] → (r, g, b) in [0, 1]."""
    h = h % 1.0
    i = int(h * 6.0)
    f = h * 6.0 - i
    p = v * (1.0 - s)
    q = v * (1.0 - f * s)
    t = v * (1.0 - (1.0 - f) * s)
    if i == 0:
        return v, t, p
    elif i == 1:
        return q, v, p
    elif i == 2:
        return p, v, t
    elif i == 3:
        return p, q, v
    elif i == 4:
        return t, p, v
    else:
        return v, p, q


def wavelength_to_rgb(wavelength_nm: float):
    """Approximate visible wavelength (nm) to linear-ish RGB."""
    w = wavelength_nm
    if w < 380.0:
        return 0.0, 0.0, 0.0
    if w < 440.0:
        r, g, b = -(w - 440.0) / (440.0 - 380.0), 0.0, 1.0
    elif w < 490.0:
        r, g, b = 0.0, (w - 440.0) / (490.0 - 440.0), 1.0
    elif w < 510.0:
        r, g, b = 0.0, 1.0, -(w - 510.0) / (510.0 - 490.0)
    elif w < 580.0:
        r, g, b = (w - 510.0) / (580.0 - 510.0), 1.0, 0.0
    elif w < 645.0:
        r, g, b = 1.0, -(w - 645.0) / (645.0 - 580.0), 0.0
    elif w <= 700.0:
        r, g, b = 1.0, 0.0, 0.0
    else:
        return 0.0, 0.0, 0.0

    if w < 420.0:
        factor = 0.30 + 0.70 * (w - 380.0) / (420.0 - 380.0)
    elif w <= 645.0:
        factor = 1.0
    else:
        factor = 0.30 + 0.70 * (700.0 - w) / (700.0 - 645.0)

    gamma = 0.82
    return (
        (max(r, 0.0) * factor) ** gamma,
        (max(g, 0.0) * factor) ** gamma,
        (max(b, 0.0) * factor) ** gamma,
    )


def spectral_lobe_rgb(center_nm: float, bandwidth_nm: float):
    """Small spectral integration around the predicted diffraction wavelength."""
    samples = (-1.0, -0.45, 0.0, 0.45, 1.0)
    weights = (0.18, 0.72, 1.0, 0.72, 0.18)
    r = g = b = total = 0.0
    for s, weight in zip(samples, weights):
        wl = center_nm + s * bandwidth_nm
        sr, sg, sb = wavelength_to_rgb(wl)
        warm = math.exp(-((wl - 590.0) / 48.0) ** 2)
        blue = math.exp(-((wl - 468.0) / 42.0) ** 2)
        cyan = math.exp(-((wl - 500.0) / 34.0) ** 2)
        material = warm * 1.22 + blue * 1.08 + cyan * 0.24
        material *= 1.0 - 0.60 * math.exp(-((wl - 540.0) / 34.0) ** 2)
        material *= 1.0 - 0.72 * math.exp(-((wl - 410.0) / 30.0) ** 2)
        material = max(material, 0.035)
        w = weight * material
        r += sr * w
        g += sg * w
        b += sb * w
        total += w
    if total <= 0.0:
        return 0.0, 0.0, 0.0
    rr = r / total
    gg = g / total
    bb = b / total
    gg *= 0.72 + 0.28 * max(bb, rr)
    return min(rr, 1.0), min(gg, 1.0), min(bb, 1.0)


def spectral_phase_rgb(phase: float, saturation: float, value: float):
    """Saturated camera-like rainbow used by the SeR prism fragments."""
    h = phase % 1.0
    r, g, b = hsv_to_rgb(h, saturation, value)

    # Real foil photos tend to punch warm/yellow and cyan-blue harder than the
    # mathematically even HSV wheel, while pure green is a little less dominant.
    warm = math.exp(-(((h - 0.075 + 0.5) % 1.0 - 0.5) / 0.075) ** 2)
    cyan = math.exp(-(((h - 0.535 + 0.5) % 1.0 - 0.5) / 0.095) ** 2)
    blue = math.exp(-(((h - 0.650 + 0.5) % 1.0 - 0.5) / 0.090) ** 2)
    green = math.exp(-(((h - 0.335 + 0.5) % 1.0 - 0.5) / 0.090) ** 2)

    r *= 1.00 + warm * 0.34 + blue * 0.08
    g *= 0.88 + warm * 0.14 + cyan * 0.24 - green * 0.10
    b *= 0.96 + cyan * 0.18 + blue * 0.20
    return min(max(r, 0.0), 1.0), min(max(g, 0.0), 1.0), min(max(b, 0.0), 1.0)


def avoid_magenta_phase(phase: float, warm_bias: float) -> float:
    """Fold magenta/purple phases toward orange-red or blue-cyan."""
    h = phase % 1.0
    if h < 0.76 or h > 0.965:
        return h

    t = (h - 0.76) / 0.205
    if warm_bias > 0.55:
        return (0.030 + t * 0.070) % 1.0

    blue_target = 0.610 - t * 0.070
    warm_target = 0.045 + t * 0.045
    use_warm = smoothstep(0.56, 0.88, t) * (0.38 + warm_bias * 0.62)
    return blue_target * (1.0 - use_warm) + warm_target * use_warm


def prism_peak(phase: float, power: float = 7.0) -> float:
    """Narrow reflective lobe from a periodic foil groove phase."""
    return (math.sin(phase * 2.0 * math.pi) * 0.5 + 0.5) ** power


def screen_channel(dst: float, src: float, alpha: float) -> float:
    """Screen blend one scalar channel."""
    s = src * alpha
    return 1.0 - (1.0 - dst) * (1.0 - s)


def luminance_rgb(r: float, g: float, b: float) -> float:
    return 0.2126 * r + 0.7152 * g + 0.0864 * b  # approximate Rec.709


def smoothstep(edge0: float, edge1: float, x: float) -> float:
    t = max(0.0, min((x - edge0) / (edge1 - edge0), 1.0))
    return t * t * (3.0 - 2.0 * t)


def lerp_rgb(a, b, t: float):
    t = max(0.0, min(t, 1.0))
    return (
        a[0] * (1.0 - t) + b[0] * t,
        a[1] * (1.0 - t) + b[1] * t,
        a[2] * (1.0 - t) + b[2] * t,
    )


def palette_ramp(stops, t: float):
    t = max(0.0, min(t, 1.0))
    for i in range(len(stops) - 1):
        p0, c0 = stops[i]
        p1, c1 = stops[i + 1]
        if t <= p1:
            local = 0.0 if p1 <= p0 else (t - p0) / (p1 - p0)
            return lerp_rgb(c0, c1, smoothstep(0.0, 1.0, local))
    return stops[-1][1]


def ser_grid_distribution_rgb(u: float, v: float, x0: float, x1: float, y0: float, y1: float, h_weight: float, v_weight: float, grain: float):
    """Reference-style fixed colour map for the two horizontal and two vertical SER bands."""
    cyan = (0.00, 0.66, 0.96)
    blue = (0.02, 0.12, 1.00)
    deep_blue = (0.02, 0.02, 0.92)
    green = (0.28, 0.92, 0.02)
    lime = (0.78, 1.00, 0.00)
    yellow = (1.00, 0.96, 0.00)
    orange = (1.00, 0.52, 0.00)
    red_orange = (1.00, 0.18, 0.03)

    top_h = palette_ramp(
        (
            (0.00, cyan),
            (x0, green),
            (x0 + (x1 - x0) * 0.42, cyan),
            (x1, blue),
            (1.00, deep_blue),
        ),
        u,
    )
    bottom_h = palette_ramp(
        (
            (0.00, yellow),
            (x0 * 0.68, yellow),
            (x0, orange),
            (x0 + (x1 - x0) * 0.52, yellow),
            (x1 - (x1 - x0) * 0.10, lime),
            (x1, green),
            (1.00, cyan),
        ),
        u,
    )
    left_v = palette_ramp(
        (
            (0.00, cyan),
            (y0, green),
            (y0 + (y1 - y0) * 0.24, lime),
            (y0 + (y1 - y0) * 0.50, yellow),
            (y0 + (y1 - y0) * 0.78, red_orange),
            (y1, yellow),
            (1.00, yellow),
        ),
        v,
    )
    right_v = palette_ramp(
        (
            (0.00, deep_blue),
            (y0, blue),
            (y0 + (y1 - y0) * 0.50, cyan),
            (y0 + (y1 - y0) * 0.70, lime),
            (y1, cyan),
            (1.00, cyan),
        ),
        v,
    )

    top_near = math.exp(-((v - y0) ** 2) / (2.0 * 0.115 * 0.115))
    bottom_near = math.exp(-((v - y1) ** 2) / (2.0 * 0.115 * 0.115))
    left_near = math.exp(-((u - x0) ** 2) / (2.0 * 0.105 * 0.105))
    right_near = math.exp(-((u - x1) ** 2) / (2.0 * 0.105 * 0.105))

    h_color = lerp_rgb(top_h, bottom_h, bottom_near / max(top_near + bottom_near, 0.001))
    v_color = lerp_rgb(left_v, right_v, right_near / max(left_near + right_near, 0.001))

    hw = max(h_weight, 0.0) * (0.35 + top_near + bottom_near)
    vw = max(v_weight, 0.0) * (0.35 + left_near + right_near)
    vertical_mix = vw / max(hw + vw, 0.001)
    rgb = lerp_rgb(h_color, v_color, vertical_mix)

    # Very slight local glint keeps the fixed colour map from looking flat.
    cool_glint = lerp_rgb(cyan, blue, smoothstep(0.35, 0.86, grain))
    warm_glint = lerp_rgb(yellow, red_orange, smoothstep(0.68, 1.0, grain))
    warm_gate = left_near * bottom_near + bottom_near * (1.0 - right_near) * 0.45
    glint = lerp_rgb(cool_glint, warm_glint, min(warm_gate, 1.0))
    return lerp_rgb(rgb, glint, 0.045 + grain * 0.035)


def ser_independent_speckle_rgb(u: float, v: float, phase_a: float, phase_b: float, phase_c: float, warm_bias: float):
    """Micro foil grains may flash their own colour instead of following the big grid."""
    phase = (
        phase_a * 0.42
        + phase_b * 0.31
        + phase_c * 0.27
        + math.sin(u * 38.0 + v * 17.0) * 0.055
        + u * 0.18
        - v * 0.11
    ) % 1.0
    phase = avoid_magenta_phase(phase, warm_bias)
    r, g, b = spectral_phase_rgb(phase, 0.99, 1.0)

    # A second sparse warm/cool glint makes neighbouring grains disagree in hue.
    cool = spectral_phase_rgb(0.57 + phase_b * 0.12, 0.96, 1.0)
    warm = spectral_phase_rgb(0.045 + phase_c * 0.060, 0.99, 1.0)
    warm_mix = smoothstep(0.62, 0.94, phase_a) * (0.35 + warm_bias * 0.65)
    gr, gg, gb = lerp_rgb(cool, warm, warm_mix)
    return lerp_rgb((r, g, b), (gr, gg, gb), 0.38)


def crisp_circle_mask(radius: float, dist: float) -> float:
    """Tight antialiasing for foil units; wide blur makes the pattern sparse."""
    return 1.0 - smoothstep(radius - 0.08, radius + 0.18, dist)


def hard_unit_mask(radius: float, dist: float) -> float:
    """Pixel-hard foil unit mask: no blur, no antialiasing."""
    return 1.0 if dist <= radius else 0.0


# ──────────────────────────────────────────────────────────────────────────────
# Core shader: Secret Rare Weave (mirrors Rust `draw_secret_weave`)
# ──────────────────────────────────────────────────────────────────────────────
def draw_secret_weave(
    card: np.ndarray,  # H×W×3 float [0,1]
    art_x: int,
    art_y: int,
    art_w: int,
    art_h: int,
    params: SerParams,
) -> np.ndarray:
    """Return a copy of `card` with SeR weave drawn over the art rect."""
    out = card.copy()
    H, W = out.shape[:2]
    rect_w = float(max(art_w, 1))
    rect_h = float(max(art_h, 1))
    max_dist = math.sqrt(rect_w * rect_w + rect_h * rect_h)
    sigma_basis = min(rect_w, rect_h)

    # Light position
    light_x = rect_w * params.light_ux
    light_y = rect_h * params.light_uy
    light_z = sigma_basis * params.diffraction_light_z
    view_len = math.sqrt(
        params.diffraction_view_tilt_x * params.diffraction_view_tilt_x
        + params.diffraction_view_tilt_y * params.diffraction_view_tilt_y
        + 1.0
    )
    view_x = params.diffraction_view_tilt_x / view_len
    view_y = params.diffraction_view_tilt_y / view_len

    # Band centres
    band_y0 = rect_h * params.band_y0
    band_y1 = rect_h * params.band_y1
    band_x0 = rect_w * params.band_x0
    band_x1 = rect_w * params.band_x1

    x_end = min(art_x + art_w, W)
    y_end = min(art_y + art_h, H)

    for y in range(art_y, y_end):
        for x in range(art_x, x_end):
            local_x = x - art_x
            local_y = y - art_y
            xf = float(local_x)
            yf = float(local_y)

            # ── Cell identity ──────────────────────────────────────────
            cell_x = local_x // params.cell_size
            stagger_y = local_y + (params.cell_size // 2 if (cell_x & 1) else 0)
            cell_y = stagger_y // params.cell_size
            in_cell_x = local_x % params.cell_size
            in_cell_y = stagger_y % params.cell_size
            ch = pixel_hash(cell_x, cell_y)
            cs = float(params.cell_size)
            cx = cs * 0.5
            cy = cs * 0.5
            in_xf = float(in_cell_x)
            in_yf = float(in_cell_y)

            # ── Element shape: hard 7px dots / hard 7x15px capsules ───────
            # Example colour is irrelevant; only the registered, pixel-hard
            # mask matters. Capsules are taller than a cell, so sample neighbor
            # cells as well instead of clipping them to their own tile.
            radius = params.unit_diameter * 0.5
            capsule_half_segment = max(params.capsule_height * 0.5 - radius, 0.0)
            elem_mask = 0.0
            for cxi in range(cell_x - 1, cell_x + 2):
                if cxi < 0:
                    continue
                offset_y = params.cell_size // 2 if (cxi & 1) else 0
                center_x = cxi * params.cell_size + cx
                for cyi in range(cell_y - 2, cell_y + 3):
                    if cyi < 0:
                        continue
                    unit_hash = pixel_hash(cxi, cyi)
                    center_y = cyi * params.cell_size + cy - offset_y
                    dx = abs(xf - center_x)
                    if (unit_hash % 9) != 0:
                        dist = math.sqrt(dx * dx + (yf - center_y) * (yf - center_y))
                    else:
                        dy = max(abs(yf - center_y) - capsule_half_segment, 0.0)
                        dist = math.sqrt(dx * dx + dy * dy)
                    if hard_unit_mask(radius, dist) > 0.0:
                        elem_mask = 1.0
                        break
                if elem_mask >= 1.0:
                    break

            # Sub-cell foil texture. Secret rare is never just four lines:
            # the whole surface has vertical “rain” and tiny prism grains.
            u = xf / rect_w
            v = yf / rect_h
            vertical_grating = 1.0 - smoothstep(0.68, 1.55, abs(in_xf - cx))
            diagonal_grating = (
                (math.sin((xf + yf) * 0.11) * 0.5 + 0.5) ** 10.0
            )
            pin_hash = pixel_hash(local_x // 2, local_y // 2)
            pin_spark = 1.0 if (pin_hash & 0x1FF) < 11 else 0.0
            broad_wave = (
                math.sin(u * 8.0 - v * 5.3 + math.sin(v * 17.0) * 0.35) * 0.5 + 0.5
            ) ** 1.35

            # ── "#" diffraction bands ───────────────────────────────────
            dy0 = abs(yf - band_y0)
            dy1 = abs(yf - band_y1)
            dx0 = abs(xf - band_x0)
            dx1 = abs(xf - band_x1)
            dy_band = min(dy0, dy1)
            dx_band = min(dx0, dx1)
            to_band = min(dy_band, dx_band)

            # ── Rainbow glow near light ─────────────────────────────────
            to_lx = light_x - xf
            to_ly = light_y - yf
            light_dist2 = to_lx * to_lx + to_ly * to_ly
            light_dist = math.sqrt(light_dist2)
            near_light = 1.0 - min(light_dist / (max_dist * params.glow_falloff_dist), 1.0)

            glow = near_light ** params.glow_power * params.glow_strength
            glow_skirt = (
                (1.0 - min(light_dist / (max_dist * params.glow_skirt_falloff), 1.0))
                ** params.glow_skirt_power
                * params.glow_skirt_strength
            )

            glow_angle = math.atan2(to_ly, to_lx)
            glow_phase = (glow_angle + math.pi) / (2.0 * math.pi)

            # ── Prism diffraction response ──────────────────────────────
            hue_origin_x = rect_w * params.hue_origin_x
            hue_origin_y = rect_h * params.hue_origin_y
            hue_dx = xf - hue_origin_x
            hue_dy = yf - hue_origin_y
            hue_dist = math.sqrt(hue_dx * hue_dx + hue_dy * hue_dy)
            hue_max = max_dist * params.hue_falloff
            hue_t = min(hue_dist / hue_max, 1.0)
            hue_t = hue_t * hue_t * (3.0 - 2.0 * hue_t)
            hue_t = min(hue_t + params.hue_blue_bias * hue_t * (1.0 - hue_t), 1.0)

            # Real SeR bands are tight and hot around the orange cross, then
            # broaden into blue/purple diffraction as they move right/outward.
            width_t = max(hue_t, min(max((xf / rect_w - 0.18) / 0.82, 0.0), 1.0) * 0.72)
            core_s = (
                params.core_sigma
                + (params.core_sigma_far - params.core_sigma) * width_t
            ) * sigma_basis
            halo_s = (
                params.halo_sigma
                + (params.halo_sigma_far - params.halo_sigma) * width_t
            ) * sigma_basis
            line_core_width_mult = (
                params.line_core_width_mult
                + (params.line_core_width_mult_far - params.line_core_width_mult) * width_t
            )
            line_core_lift = (
                params.line_core_lift
                + (params.line_core_lift_far - params.line_core_lift) * width_t
            )

            line_core_s = core_s * line_core_width_mult
            preset_core = math.exp(-to_band * to_band / (2.0 * core_s * core_s))
            preset_line_core = math.exp(-to_band * to_band / (2.0 * line_core_s * line_core_s))
            preset_halo = math.exp(-to_band * to_band / (2.0 * halo_s * halo_s))
            preset_h = math.exp(-dy_band * dy_band / (2.0 * line_core_s * line_core_s))
            preset_v = math.exp(-dx_band * dx_band / (2.0 * line_core_s * line_core_s))

            cloud_a = value_noise(u, v, 2.4, 3, 11)
            cloud_b = value_noise(u, v, 5.6, 7, 13)
            cloud_c = value_noise(u, v, 9.0, 17, 5)
            cloud = min(max(cloud_a * 0.55 + cloud_b * 0.30 + cloud_c * 0.15, 0.0), 1.0)
            patch = smoothstep(0.28, 0.76, cloud)

            # Real-card diffraction reads as tiny prism tiles that light up in
            # short horizontal/vertical runs, not as one smooth wavelength field.
            h_center0 = 0.165 + math.sin(u * 9.0 + 0.8) * 0.018 + (cloud_a - 0.5) * 0.030
            h_center1 = 0.318 + math.sin(u * 7.4 + 2.1) * 0.024 + (cloud_b - 0.5) * 0.034
            h_center2 = 0.545 + math.sin(u * 7.8 + 1.4) * 0.030 + (cloud_c - 0.5) * 0.038
            h_center3 = 0.725 + math.sin(u * 8.7 + 3.2) * 0.022 + (cloud_a - 0.5) * 0.030
            h_center4 = 0.875 + math.sin(u * 8.1 + 5.0) * 0.020 + (cloud_b - 0.5) * 0.028
            h_band0 = math.exp(-((v - h_center0) ** 2) / (2.0 * 0.030 * 0.030))
            h_band1 = math.exp(-((v - h_center1) ** 2) / (2.0 * 0.038 * 0.038))
            h_band2 = math.exp(-((v - h_center2) ** 2) / (2.0 * 0.050 * 0.050))
            h_band3 = math.exp(-((v - h_center3) ** 2) / (2.0 * 0.042 * 0.042))
            h_band4 = math.exp(-((v - h_center4) ** 2) / (2.0 * 0.032 * 0.032))

            slant = v - (0.84 - u * 0.48 + math.sin(u * 8.4) * 0.026)
            slant_band = math.exp(-(slant * slant) / (2.0 * 0.046 * 0.046))
            v_lobe0 = math.exp(-((u - (0.185 + math.sin(v * 6.2) * 0.026)) ** 2) / (2.0 * 0.052 * 0.052))
            v_lobe1 = math.exp(-((u - (0.525 + math.sin(v * 5.8 + 1.1) * 0.035)) ** 2) / (2.0 * 0.072 * 0.072))
            v_lobe2 = math.exp(-((u - (0.815 + math.sin(v * 6.8 + 2.4) * 0.034)) ** 2) / (2.0 * 0.066 * 0.066))

            h_cloud = min(
                h_band0 * 0.28
                + h_band1 * 0.40
                + h_band2 * 0.44
                + h_band3 * 0.36
                + h_band4 * 0.24
                + slant_band * 0.22,
                1.0,
            )
            v_cloud = min(v_lobe0 * 0.22 + v_lobe1 * 0.28 + v_lobe2 * 0.24 + slant_band * 0.16, 1.0)
            granular = ((ch >> 10) & 0xFF) / 255.0
            fine_hash = ((pixel_hash(local_x // 2, local_y // 2) >> 11) & 0xFF) / 255.0
            cell_phase_a = ((ch >> 17) & 0xFFFF) / 65535.0
            cell_phase_b = ((ch >> 33) & 0xFFFF) / 65535.0
            cell_phase_c = ((ch >> 49) & 0x7FFF) / 32767.0

            h_prism = prism_peak(u * 31.0 + v * 1.6 + cell_phase_a * 1.7 + cloud_a * 0.42, 8.0)
            h_prism += prism_peak(u * 53.0 - v * 2.4 + cell_phase_b * 1.9 + cloud_c * 0.35, 10.0) * 0.58
            v_prism = prism_peak(v * 35.0 - u * 1.9 + cell_phase_b * 1.6 + cloud_b * 0.40, 8.0)
            v_prism += prism_peak(v * 59.0 + u * 2.1 + cell_phase_c * 2.0 + cloud_a * 0.36, 10.0) * 0.52
            diag_prism = prism_peak((u + v) * 26.0 + cell_phase_c * 1.8 + cloud_c * 0.50, 9.0)
            speckle = smoothstep(0.50, 0.96, granular * 0.36 + fine_hash * 0.30 + cloud_c * 0.34)
            cloud_gate = 0.36 + patch * 0.64
            unit_gate = 0.46 + elem_mask * 0.54 + vertical_grating * 0.14 + pin_spark * 0.18
            h_scan_gate = 0.18 + min(max(h_prism, speckle), 1.0) * 0.82
            v_scan_gate = 0.18 + min(max(v_prism, speckle), 1.0) * 0.82
            line_foil_gate = min(
                elem_mask * 0.72
                + vertical_grating * 0.18
                + diagonal_grating * 0.08
                + speckle * 0.34
                + max(h_prism, v_prism) * 0.24
                + pin_spark * 0.30,
                1.0,
            )
            line_reflect_gate = 0.08 + line_foil_gate * 0.92

            h_response = min(
                h_cloud * cloud_gate * h_scan_gate * params.prism_scan_strength
                + h_prism * unit_gate * params.prism_fragment_strength
                + diag_prism * 0.16
                + speckle * params.reflection_speckle_boost
                + preset_h * params.prism_cross_strength * line_reflect_gate,
                1.0,
            )
            v_response = min(
                v_cloud * cloud_gate * v_scan_gate * (params.prism_scan_strength * 0.82)
                + v_prism * unit_gate * (params.prism_fragment_strength * 0.88)
                + diag_prism * 0.18
                + speckle * (params.reflection_speckle_boost * 0.74)
                + preset_v * (params.prism_cross_strength * 0.78) * line_reflect_gate,
                1.0,
            )
            grain_response = min(
                speckle * 0.72
                + max(h_prism, v_prism) * 0.42
                + pin_spark * 0.34
                + diag_prism * 0.22,
                1.0,
            )
            line_bright = min(
                (preset_line_core * 0.82 + max(preset_h, preset_v) * 0.42)
                * line_reflect_gate,
                1.0,
            )

            near_warm_bias = smoothstep(0.20, 0.78, near_light)
            warm_response = min(
                (
                    h_band1 * 0.30
                    + h_band2 * 0.22
                    + preset_h * 0.22 * line_reflect_gate
                    + preset_v * 0.12 * line_reflect_gate
                    + near_warm_bias * 0.22
                )
                * (0.38 + h_prism * 0.48 + speckle * 0.14)
                * (1.06 - width_t * 0.24),
                1.0,
            )
            grid_r, grid_g, grid_b = ser_grid_distribution_rgb(
                u,
                v,
                params.band_x0,
                params.band_x1,
                params.band_y0,
                params.band_y1,
                h_response + preset_h * 0.55 * line_reflect_gate,
                v_response + preset_v * 0.55 * line_reflect_gate,
                grain_response * 0.72 + fine_hash * 0.28,
            )
            warm_r, warm_g, warm_b = spectral_phase_rgb(0.042 + u * 0.045 + cell_phase_a * 0.020, 0.99, 1.0)
            warm_mix = min(warm_response * 0.28 + near_warm_bias * 0.20, 0.46)
            base_diff_r = grid_r * (1.0 - warm_mix) + warm_r * warm_mix
            base_diff_g = grid_g * (1.0 - warm_mix) + warm_g * warm_mix
            base_diff_b = grid_b * (1.0 - warm_mix) + warm_b * warm_mix
            speckle_r, speckle_g, speckle_b = ser_independent_speckle_rgb(
                u,
                v,
                cell_phase_a,
                cell_phase_b,
                cell_phase_c,
                near_warm_bias * 0.45 + warm_response * 0.35,
            )
            speckle_mix = min(
                grain_response * (0.18 + speckle * 0.24)
                + pin_spark * 0.28
                + max(h_prism, v_prism) * 0.070,
                0.48,
            )
            diff_r = base_diff_r * (1.0 - speckle_mix) + speckle_r * speckle_mix
            diff_g = base_diff_g * (1.0 - speckle_mix) + speckle_g * speckle_mix
            diff_b = base_diff_b * (1.0 - speckle_mix) + speckle_b * speckle_mix

            line_core = min(
                preset_line_core * 0.62 * line_reflect_gate
                + max(h_response, v_response) * 0.64
                + grain_response * 0.26,
                1.0,
            )
            core = min(
                max(h_response, v_response) * 0.88
                + grain_response * 0.28
                + warm_response * 0.16
                + preset_core * 0.045
                + line_bright * 0.16,
                1.0,
            )
            halo = min(max(h_cloud, v_cloud) * 0.20 + patch * 0.08 + preset_halo * 0.035, 1.0)

            # ── Combine strengths ───────────────────────────────────────
            strength = core * params.core_weight + halo * params.halo_weight
            in_band = strength

            if in_band < 0.01:
                ambient_glow = glow * params.glow_bg_mult + glow_skirt * params.glow_skirt_bg_mult
            else:
                ambient_glow = (glow + glow_skirt) * params.glow_inband_mult

            surface = (
                params.foil_floor
                + elem_mask * params.cell_strength
                + vertical_grating * params.vertical_grating_strength
                + diagonal_grating * params.diagonal_grating_strength
                + pin_spark * params.pin_spark_strength
            ) * (0.72 + broad_wave * 0.34)

            lit_gate = min(line_core * 0.70 + core * 0.55 + halo * 0.18 + ambient_glow * 1.10, 1.0)
            surface *= params.dark_region_mult + lit_gate * (params.lit_region_mult - params.dark_region_mult)

            # Bands/glow lift the surface-wide foil instead of replacing it.
            strength = min(
                surface * (0.54 + strength * 1.34)
                + ambient_glow
                + (line_core ** params.line_core_power) * line_core_lift,
                0.92,
            )
            strength = min(strength + line_bright * 0.055, 0.94)

            if elem_mask > 0.2:
                strength *= params.elem_brightness_boost
            unit_mask = min(
                elem_mask
                + vertical_grating * params.base_grating_alpha
                + diagonal_grating * (params.base_grating_alpha * 0.55)
                + pin_spark * params.base_pin_alpha,
                1.0,
            )

            if strength < params.min_strength and unit_mask < 0.01:
                continue

            # Sparse sparkle
            spark_hash = pixel_hash(local_x // 3, local_y // 3)
            spark_on = core > params.spark_core_threshold and (spark_hash & 0x7FF) < params.spark_probability

            if strength <= 0.0 and not spark_on and unit_mask < 0.01:
                continue

            # ── Colour ──────────────────────────────────────────────────
            dr = float(out[y, x, 0])
            dg = float(out[y, x, 1])
            db = float(out[y, x, 2])
            lum = 0.2126 * dr + 0.7152 * dg + 0.0722 * db
            ink_vis = max(min(params.ink_luma_max - lum, 1.0), params.ink_visibility_min)
            dark_foil_vis = smoothstep(0.92, 0.24, lum)

            # Dark base foil: same hue as the art under it, just dimmer and
            # constrained to the procedural unit pattern.
            base_unit_alpha = min(
                unit_mask * params.base_unit_alpha * (0.70 + broad_wave * 0.30),
                1.0,
            )
            base_keep = 1.0 - base_unit_alpha * params.base_unit_darkening
            br = dr * base_keep
            bg = dg * base_keep
            bb = db * base_keep

            if spark_on:
                r, g, b = 0.97, 0.94, 0.76
            else:
                gw = min(ambient_glow / max(strength, 0.001), 1.0)
                bw = 1.0 - gw

                cell_hue = ((ch >> 24) & 0xFF) / 255.0
                texture_hue = (
                    cell_hue * 0.62
                    + u * 0.16
                    - v * 0.12
                    + broad_wave * 0.18
                    + vertical_grating * 0.08
                ) % 1.0
                texture_hue = avoid_magenta_phase(texture_hue, 0.08)
                band_sat = params.band_saturation + (params.band_saturation_far - params.band_saturation) * width_t
                band_val_cap = params.band_value_near + (params.band_value_far - params.band_value_near) * width_t
                tex_r, tex_g, tex_b = hsv_to_rgb(texture_hue, band_sat, 1.0)
                texture_mix = params.diffraction_texture_mix
                prism_light = min(max(line_core, core, grain_response), 1.0)
                band_val = min(
                    max(
                        strength * params.prism_value_boost * (1.10 - width_t * 0.08),
                        params.prism_value_floor * (0.22 + prism_light * 0.78),
                    ),
                    band_val_cap,
                )
                band_r = (diff_r * (1.0 - texture_mix) + tex_r * texture_mix) * band_val
                band_g = (diff_g * (1.0 - texture_mix) + tex_g * texture_mix) * band_val
                band_b = (diff_b * (1.0 - texture_mix) + tex_b * texture_mix) * band_val

                # Glow colour: near-light areas skew orange-red like real SER foil.
                rainbow_glow_hue = avoid_magenta_phase(
                    params.glow_hue_base + bw * 0.04 + glow_phase * params.glow_hue_range,
                    near_warm_bias,
                )
                warm_glow_hue = 0.030 + broad_wave * 0.030 + cell_phase_a * 0.020
                glow_hue = rainbow_glow_hue * (1.0 - near_warm_bias) + warm_glow_hue * near_warm_bias
                glow_sat = 0.88 + near_light * 0.12
                glow_val = min(strength * (0.92 + near_warm_bias * 0.18), 0.92)
                glow_r, glow_g, glow_b = hsv_to_rgb(glow_hue, glow_sat, glow_val)

                r = band_r * bw + glow_r * gw
                g = band_g * bw + glow_g * gw
                b = band_b * bw + glow_b * gw

            alpha = min(strength * ink_vis * (0.64 + dark_foil_vis * 0.42 + line_bright * 0.18), 1.0) * params.opacity
            alpha *= 1.0 + lit_gate * params.alpha_lit_boost + line_core * params.alpha_line_boost
            alpha *= 1.0 + line_bright * 0.18
            alpha = min(alpha, params.alpha_cap)
            if spark_on:
                alpha = 0.82 * params.opacity

            # Foil reflection is primarily chroma: keep local luminance, push hue
            # and saturation toward the foil colour, then add only a restrained
            # screen highlight on top.
            src_lum = max(0.2126 * r + 0.7152 * g + 0.0722 * b, 0.001)
            reflect_lum = min(
                max(
                    lum * (0.70 + line_core * 0.10)
                    + strength * 0.22
                    + grain_response * 0.08,
                    0.16 + line_core * 0.14 + line_bright * 0.18,
                ),
                0.98,
            )
            reflect_scale = reflect_lum / src_lum
            rr = min(max(r * reflect_scale, 0.0), 1.0)
            rg = min(max(g * reflect_scale, 0.0), 1.0)
            rb = min(max(b * reflect_scale, 0.0), 1.0)

            screen_alpha = alpha * params.reflect_brightness_alpha
            screen_alpha = min(screen_alpha * (1.0 + line_bright * 0.32), 1.0)
            sr = screen_channel(br, r, screen_alpha)
            sg = screen_channel(bg, g, screen_alpha)
            sb = screen_channel(bb, b, screen_alpha)
            color_overlay = alpha * (
                params.reflect_color_mix * (0.35 + lit_gate * 0.65)
                + line_core * params.reflect_line_mix
                + lit_gate * params.reflect_lit_mix
                + line_bright * 0.52
            )
            color_overlay = min(color_overlay, params.color_overlay_cap)
            screen_keep = 1.0 - color_overlay
            out[y, x, 0] = sr * screen_keep + rr * color_overlay
            out[y, x, 1] = sg * screen_keep + rg * color_overlay
            out[y, x, 2] = sb * screen_keep + rb * color_overlay

    return out


# ──────────────────────────────────────────────────────────────────────────────
# Main: load a card image and apply the SeR weave
# ──────────────────────────────────────────────────────────────────────────────
def main():
    card_names = os.environ.get("SER_CARDS", "2511-card,483-card").split(",")
    card_names = [name.strip() for name in card_names if name.strip()]
    for name in card_names:
        src_path = os.path.join(RESOURCES_DIR, f"{name}.png")
        if not os.path.exists(src_path):
            print(f"SKIP: {src_path} not found")
            continue

        img = Image.open(src_path).convert("RGBA")
        if PREVIEW_SCALE != 1.0:
            preview_size = (
                max(1, int(round(img.width * PREVIEW_SCALE))),
                max(1, int(round(img.height * PREVIEW_SCALE))),
            )
            img = img.resize(preview_size, Image.Resampling.LANCZOS)
        card = np.array(img, dtype=np.float32) / 255.0

        # Match the Rust default normal-monster art rect from build_bundle.py.
        H, W = card.shape[:2]
        art_x, art_y, art_w, art_h = 170, 375, 1054, 1054
        if W != 1394 or H != 2031:
            sx = W / 1394.0
            sy = H / 2031.0
            art_x = int(round(art_x * sx))
            art_y = int(round(art_y * sy))
            art_w = int(round(art_w * sx))
            art_h = int(round(art_h * sy))

        print(f"Rendering {name} ({W}×{H})…")
        result = draw_secret_weave(card, art_x, art_y, art_w, art_h, SER_PARAMS)

        result_u8 = (np.clip(result, 0, 1) * 255).astype(np.uint8)
        out_path = os.path.join(OUT_DIR, f"tune-ser-{name}.png")
        Image.fromarray(result_u8, "RGBA").save(out_path)
        print(f"  → {out_path}")

    print("\nDone. Open scripts/export/ to inspect.")


if __name__ == "__main__":
    main()
