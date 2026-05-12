"""
Physical SER (Secret Rare) holographic effect prototype.

Uses:
- Procedural normal map generation (5×5 unit, corners removed, flat-top + beveled edges)
- Anisotropic specular (Ward model)
- Fresnel effect (Schlick approximation)
- Chromatic dispersion (per-channel normal perturbation)
- Parameterized viewing angle (theta, phi)

Microstructure:
  5×5 pixel unit with 4 corner pixels removed:
    0 1 1 1 0
    1 1 1 1 1
    1 1 1 1 1
    1 1 1 1 1
    0 1 1 1 0
  1px gap between units, staggered (brick) layout.
  Tile period: horizontal = 6px, vertical offset for odd columns = 3px.
"""

import numpy as np
from PIL import Image
import os
import sys
from typing import Optional

# ─────────────────────────────────────────────────────────────────────────────
# Configuration
# ─────────────────────────────────────────────────────────────────────────────

UNIT_SIZE = 5          # 5×5 pixel unit
GAP = 1               # 1px gap between units
CELL_SIZE = UNIT_SIZE + GAP  # 6px total cell pitch
STAGGER_OFFSET = 3    # half-cell vertical offset for odd columns (6//2=3)

# Unit shape mask (5×5, corners removed)
UNIT_MASK = np.array([
    [0, 1, 1, 1, 0],
    [1, 1, 1, 1, 1],
    [1, 1, 1, 1, 1],
    [1, 1, 1, 1, 1],
    [0, 1, 1, 1, 0],
], dtype=np.float32)

# Edge angle for beveled edges (degrees)
DEFAULT_EDGE_ANGLE = 38.0


# ─────────────────────────────────────────────────────────────────────────────
# Part 1: Normal Map Generation
# ─────────────────────────────────────────────────────────────────────────────

def generate_unit_normals(edge_angle_deg: float = DEFAULT_EDGE_ANGLE) -> np.ndarray:
    """
    Generate normal vectors for a single 5×5 unit cell.
    
    Returns: (5, 5, 3) array of normalized normal vectors.
    
    Strategy: flat-top + beveled edges
    - Center 3×3: normal = (0, 0, 1) (flat top)
    - Edge pixels: normal tilted outward by edge_angle
    - Corner (removed) pixels: normal = (0, 0, 1) but masked out
    """
    normals = np.zeros((UNIT_SIZE, UNIT_SIZE, 3), dtype=np.float32)
    normals[:, :, 2] = 1.0  # default: pointing up
    
    edge_angle = np.radians(edge_angle_deg)
    sin_a = np.sin(edge_angle)
    cos_a = np.cos(edge_angle)
    
    center = (UNIT_SIZE - 1) / 2.0  # 2.0 for 5×5
    
    for y in range(UNIT_SIZE):
        for x in range(UNIT_SIZE):
            if UNIT_MASK[y, x] == 0:
                continue
            
            # Check if this is an edge pixel (not in center 3×3)
            is_center = (1 <= x <= 3) and (1 <= y <= 3)
            
            if not is_center:
                # Edge pixel: compute direction from center
                dx = x - center
                dy = y - center
                dist = np.sqrt(dx * dx + dy * dy)
                if dist > 0:
                    # Normalize direction
                    nx = (dx / dist) * sin_a
                    ny = (dy / dist) * sin_a
                    nz = cos_a
                    # Normalize the normal vector
                    length = np.sqrt(nx*nx + ny*ny + nz*nz)
                    normals[y, x] = [nx/length, ny/length, nz/length]
    
    return normals


def generate_normal_map(width: int, height: int, edge_angle_deg: float = DEFAULT_EDGE_ANGLE) -> tuple:
    """
    Generate a full normal map and active mask for the given dimensions.
    
    Uses staggered (brick) layout with 1px gaps.
    
    Returns:
        normal_map: (H, W, 3) float32 array of normalized normals
        active_mask: (H, W) float32 array, 1.0 for active foil pixels, 0.0 for gaps
    """
    normal_map = np.zeros((height, width, 3), dtype=np.float32)
    normal_map[:, :, 2] = 1.0  # default up-facing normal
    active_mask = np.zeros((height, width), dtype=np.float32)
    
    unit_normals = generate_unit_normals(edge_angle_deg)
    
    # Iterate over cells
    # Horizontal period: CELL_SIZE = 6
    # Vertical period: CELL_SIZE = 6
    # Odd columns stagger by STAGGER_OFFSET = 3
    
    cols = (width + CELL_SIZE - 1) // CELL_SIZE + 1
    rows = (height + CELL_SIZE - 1) // CELL_SIZE + 1
    
    for col in range(cols):
        x_start = col * CELL_SIZE
        y_offset = STAGGER_OFFSET if (col % 2 == 1) else 0
        
        for row in range(-1, rows):  # -1 to handle stagger overflow
            y_start = row * CELL_SIZE + y_offset
            
            # Place unit at (x_start, y_start)
            for uy in range(UNIT_SIZE):
                for ux in range(UNIT_SIZE):
                    px = x_start + ux
                    py = y_start + uy
                    
                    if 0 <= px < width and 0 <= py < height:
                        if UNIT_MASK[uy, ux] > 0:
                            normal_map[py, px] = unit_normals[uy, ux]
                            active_mask[py, px] = 1.0
    
    return normal_map, active_mask


def generate_normal_map_vectorized(width: int, height: int, edge_angle_deg: float = DEFAULT_EDGE_ANGLE) -> tuple:
    """
    Vectorized version of normal map generation using numpy tiling.
    Much faster for large images.
    
    Returns:
        normal_map: (H, W, 3) float32
        active_mask: (H, W) float32
    """
    unit_normals = generate_unit_normals(edge_angle_deg)
    
    # Build a tile that covers one full period of the staggered pattern
    # Horizontal period: CELL_SIZE (6px per column)
    # Vertical period: CELL_SIZE * 2 (12px, because stagger repeats every 2 columns)
    # Tile width: CELL_SIZE * 2 = 12px (even + odd column)
    # Tile height: CELL_SIZE * 2 = 12px (to accommodate stagger)
    
    tile_w = CELL_SIZE * 2  # 12
    tile_h = CELL_SIZE * 2  # 12 (LCM of vertical period with stagger)
    
    tile_normals = np.zeros((tile_h, tile_w, 3), dtype=np.float32)
    tile_normals[:, :, 2] = 1.0
    tile_mask = np.zeros((tile_h, tile_w), dtype=np.float32)
    
    # Place units in the tile
    # Column 0 (even): unit at (0, 0) and (0, 6)
    # Column 1 (odd): unit at (6, 3) and (6, 9)
    placements = [
        (0, 0),              # col 0, row 0
        (0, CELL_SIZE),      # col 0, row 1
        (CELL_SIZE, STAGGER_OFFSET),              # col 1, row 0 (staggered)
        (CELL_SIZE, STAGGER_OFFSET + CELL_SIZE),  # col 1, row 1 (staggered)
    ]
    
    for (px_start, py_start) in placements:
        for uy in range(UNIT_SIZE):
            for ux in range(UNIT_SIZE):
                tx = px_start + ux
                ty = (py_start + uy) % tile_h
                if tx < tile_w:
                    if UNIT_MASK[uy, ux] > 0:
                        tile_normals[ty, tx] = unit_normals[uy, ux]
                        tile_mask[ty, tx] = 1.0
    
    # Tile across the full image
    reps_x = (width + tile_w - 1) // tile_w + 1
    reps_y = (height + tile_h - 1) // tile_h + 1
    
    full_normals = np.tile(tile_normals, (reps_y, reps_x, 1))[:height, :width, :]
    full_mask = np.tile(tile_mask, (reps_y, reps_x))[:height, :width]
    
    return full_normals, full_mask


# ─────────────────────────────────────────────────────────────────────────────
# Part 2: PBR Shading Pipeline
# ─────────────────────────────────────────────────────────────────────────────

def normalize_vec3(v: np.ndarray) -> np.ndarray:
    """Normalize 3D vectors. v shape: (..., 3)"""
    length = np.sqrt(np.sum(v * v, axis=-1, keepdims=True))
    length = np.maximum(length, 1e-8)
    return v / length


def ward_anisotropic_specular(
    N: np.ndarray,      # (H, W, 3) normals
    V: np.ndarray,      # (3,) view direction (toward viewer)
    L: np.ndarray,      # (3,) light direction (toward light)
    T: np.ndarray,      # (H, W, 3) tangent directions
    alpha_x: float,     # roughness along tangent
    alpha_y: float,     # roughness along bitangent
) -> np.ndarray:
    """
    Ward anisotropic specular model.
    
    Returns: (H, W) specular intensity
    """
    # Half vector
    H = normalize_vec3((L + V)[np.newaxis, np.newaxis, :] * np.ones_like(N))
    
    # Bitangent
    B = normalize_vec3(np.cross(N, T))
    
    # Dot products
    NdotL = np.sum(N * L, axis=-1)  # (H, W)
    NdotV = np.sum(N * V, axis=-1)  # (H, W)
    NdotH = np.sum(N * H, axis=-1)  # (H, W)
    HdotT = np.sum(H * T, axis=-1)  # (H, W)
    HdotB = np.sum(H * B, axis=-1)  # (H, W)
    
    # Ward model
    # spec = exp(-2 * ((HdotT/αx)² + (HdotB/αy)²) / (1 + NdotH))
    #         / (4π * αx * αy * sqrt(NdotL * NdotV))
    
    exponent = -2.0 * ((HdotT / alpha_x)**2 + (HdotB / alpha_y)**2) / (1.0 + NdotH + 1e-8)
    
    denom = 4.0 * np.pi * alpha_x * alpha_y * np.sqrt(
        np.maximum(NdotL, 0.0) * np.maximum(NdotV, 0.0) + 1e-8
    )
    
    spec = np.exp(exponent) / (denom + 1e-8)
    
    # Mask out back-facing
    spec = np.where((NdotL > 0) & (NdotV > 0), spec, 0.0)
    
    return spec


def fresnel_schlick(cos_theta: np.ndarray, f0: float = 0.04) -> np.ndarray:
    """
    Schlick's Fresnel approximation.
    
    cos_theta: (H, W) cosine of angle between view and normal
    f0: base reflectance at normal incidence
    
    Returns: (H, W) Fresnel reflectance
    """
    cos_theta = np.clip(cos_theta, 0.0, 1.0)
    return f0 + (1.0 - f0) * (1.0 - cos_theta) ** 5


def compute_tangent_field(width: int, height: int, tangent_angle_deg: float = 0.0) -> np.ndarray:
    """
    Generate tangent vectors for the surface.
    
    For SER foil, the primary tangent direction is vertical (along Y),
    which creates horizontal rainbow bands when combined with anisotropic specular.
    
    tangent_angle_deg: rotation of tangent field from vertical (0 = vertical tangent → horizontal bands)
    
    Returns: (H, W, 3) normalized tangent vectors
    """
    angle = np.radians(tangent_angle_deg)
    # Tangent in XY plane
    tx = np.sin(angle)
    ty = np.cos(angle)
    tz = 0.0
    
    tangents = np.zeros((height, width, 3), dtype=np.float32)
    tangents[:, :, 0] = tx
    tangents[:, :, 1] = ty
    tangents[:, :, 2] = tz
    
    return tangents


# ─────────────────────────────────────────────────────────────────────────────
# Part 3: Chromatic Dispersion + View Parameterization
# ─────────────────────────────────────────────────────────────────────────────

def spherical_to_cartesian(theta_deg: float, phi_deg: float) -> np.ndarray:
    """
    Convert spherical coordinates to unit vector.
    theta: polar angle from Z axis (0 = looking straight down at surface)
    phi: azimuthal angle in XY plane
    
    Returns: (3,) unit vector pointing toward the viewer/light
    """
    theta = np.radians(theta_deg)
    phi = np.radians(phi_deg)
    x = np.sin(theta) * np.cos(phi)
    y = np.sin(theta) * np.sin(phi)
    z = np.cos(theta)
    return np.array([x, y, z], dtype=np.float32)


def perturb_normals_for_dispersion(
    normal_map: np.ndarray,
    channel_offset: float,
    dispersion_direction: Optional[np.ndarray] = None,
) -> np.ndarray:
    """
    Slightly perturb normals to simulate chromatic dispersion.
    
    Different wavelengths (R, G, B) see slightly different effective normals,
    causing rainbow color separation.
    
    channel_offset: angular offset in radians (negative for R, 0 for G, positive for B)
    dispersion_direction: (3,) direction of dispersion in tangent space
    
    Returns: (H, W, 3) perturbed and re-normalized normals
    """
    if dispersion_direction is None:
        dispersion_direction = np.array([1.0, 0.0, 0.0], dtype=np.float32)
    
    # Perturb by rotating normal slightly toward dispersion direction
    perturbed = normal_map.copy()
    perturbed[:, :, 0] += channel_offset * dispersion_direction[0]
    perturbed[:, :, 1] += channel_offset * dispersion_direction[1]
    
    # Re-normalize
    return normalize_vec3(perturbed)


def render_ser_physical(
    base_image: np.ndarray,
    theta: float = 25.0,
    phi: float = 135.0,
    light_theta: float = 30.0,
    light_phi: float = 120.0,
    roughness_x: float = 0.05,
    roughness_y: float = 0.30,
    dispersion: float = 0.035,
    edge_angle: float = 38.0,
    foil_intensity: float = 0.85,
    tangent_angle: float = 0.0,
    f0: float = 0.04,
    darken: float = 0.62,
) -> np.ndarray:
    """
    Render the physical SER holographic effect on a base image.
    
    Parameters:
        base_image: (H, W, 3) float32 array in [0, 1]
        theta: view polar angle (degrees), 0 = straight on
        phi: view azimuthal angle (degrees)
        light_theta: light polar angle (degrees)
        light_phi: light azimuthal angle (degrees)
        roughness_x: anisotropic roughness along tangent
        roughness_y: anisotropic roughness along bitangent
        dispersion: chromatic dispersion strength (radians)
        edge_angle: bevel angle of unit edges (degrees)
        foil_intensity: overall effect strength [0, 1]
        tangent_angle: tangent field rotation (degrees)
        f0: base Fresnel reflectance
        darken: base darkening factor applied to whole art region (0=black, 1=no change)
                Real SER foil printing darkens the underlying art due to the metallic layer.
    
    Returns: (H, W, 3) float32 composited result in [0, 1]
    """
    height, width = base_image.shape[:2]
    
    # Step 0: Darken the base image to simulate the metallic foil layer
    # Real SER cards have a noticeably darker base because the holographic
    # metallic substrate absorbs/scatters light differently from plain paper.
    base_image = base_image * darken
    
    # Generate normal map and active mask
    print(f"  Generating normal map ({width}x{height})...")
    normal_map, active_mask = generate_normal_map_vectorized(width, height, edge_angle)
    
    # Generate tangent field
    tangent_field = compute_tangent_field(width, height, tangent_angle)
    
    # View and light directions
    V = spherical_to_cartesian(theta, phi)
    L = spherical_to_cartesian(light_theta, light_phi)
    
    print(f"  View direction: {V}")
    print(f"  Light direction: {L}")
    
    # Dispersion direction (perpendicular to tangent in the surface plane)
    disp_dir = np.array([np.cos(np.radians(tangent_angle + 90)),
                         np.sin(np.radians(tangent_angle + 90)), 0.0], dtype=np.float32)
    
    # Compute specular for each color channel with chromatic dispersion.
    # Note: this is a prototype for printed diffraction foil rather than a
    # physically-perfect BRDF. The micro normals still drive reflection, but the
    # final color is intentionally amplified because real SER foil is a highly
    # reflective micro-grating material, not ordinary dielectric paint.
    print("  Computing anisotropic specular (R channel)...")
    N_r = perturb_normals_for_dispersion(normal_map, -dispersion, disp_dir)
    spec_r = ward_anisotropic_specular(N_r, V, L, tangent_field, roughness_x, roughness_y)
    
    print("  Computing anisotropic specular (G channel)...")
    spec_g = ward_anisotropic_specular(normal_map, V, L, tangent_field, roughness_x, roughness_y)
    
    print("  Computing anisotropic specular (B channel)...")
    N_b = perturb_normals_for_dispersion(normal_map, +dispersion, disp_dir)
    spec_b = ward_anisotropic_specular(N_b, V, L, tangent_field, roughness_x, roughness_y)
    
    # Fresnel
    print("  Computing Fresnel...")
    NdotV = np.sum(normal_map * V, axis=-1)
    fresnel = fresnel_schlick(NdotV, f0)
    
    # Directional diffraction phase. This is derived from view/light/tangent
    # terms plus the local bevel normal, so tilting the view changes color bands.
    yy, xx = np.mgrid[0:height, 0:width].astype(np.float32)
    H = normalize_vec3((L + V)[np.newaxis, np.newaxis, :] * np.ones_like(normal_map))
    h_dot_t = np.sum(H * tangent_field, axis=-1)
    bitangent = normalize_vec3(np.cross(normal_map, tangent_field))
    h_dot_b = np.sum(H * bitangent, axis=-1)
    bevel = np.sqrt(normal_map[:, :, 0] ** 2 + normal_map[:, :, 1] ** 2)

    # # 形衍射：极低频横向 + 纵向光栅，整图只出现 1-2 条带
    # 频率极低：整个 art 区域（~1098px）只走约 1.5 个周期
    freq = 1.5 / max(width, height)
    phase_h = (yy * freq + h_dot_t * 0.8 + normal_map[:, :, 1] * 0.3) % 1.0
    phase_v = (xx * freq + h_dot_b * 0.8 + normal_map[:, :, 0] * 0.3) % 1.0

    # 激活掩码：只在条带中心附近激活，其余区域压暗
    # 用 cos 使条带柔和，只取峰值附近（>0.6）
    band_h = np.clip((np.cos(phase_h * 2 * np.pi) - 0.2) / 0.8, 0.0, 1.0) ** 2
    band_v = np.clip((np.cos(phase_v * 2 * np.pi) - 0.2) / 0.8, 0.0, 1.0) ** 2

    # # 形：横带 OR 竖带激活（取最大值），交叉处最亮
    band_mask = np.maximum(band_h, band_v)

    # 随机稀疏散点（模拟参考图中的随机亮点）
    rng2 = np.random.default_rng(seed=7)
    scatter = rng2.random((height, width), dtype=np.float32)
    scatter_mask = np.clip((scatter - 0.97) / 0.03, 0.0, 1.0)  # 只有最亮的 3% 随机点

    # 合并：条带 + 散点
    band_mask = np.clip(band_mask + scatter_mask * 0.6, 0.0, 1.0)

    # phase 用于决定颜色（横竖各自的相位叠加）
    phase = (phase_h * band_h + phase_v * band_v) / (band_h + band_v + 1e-6)

    # Smooth spectral ramp, avoiding excessive magenta dominance.
    spectral = hsv_to_rgb_array(phase, 0.98, 1.0)

    # SER-like activation: all active cells shimmer, bevel/specular cells are brighter.
    # Base is raised to 0.25 so even flat-top cells show color (uniform foil coverage).
    spec_lobe = np.maximum.reduce([spec_r, spec_g, spec_b])
    spec_lobe = np.tanh(spec_lobe * 7.5)
    activation = (
        0.55
        + bevel * 0.80
        + spec_lobe * 1.00
        + fresnel * 1.00
    )
    activation = np.clip(activation, 0.0, 2.0)
    # 乘以条带掩码：只在 # 形条带区域激活彩虹
    activation = activation * band_mask

    # Channel-specific lobe gives subtle chromatic displacement; spectral hue
    # gives the large rainbow response.
    reflect_r = spectral[:, :, 0] * activation * (0.62 + np.tanh(spec_r * 4.0) * 0.38)
    reflect_g = spectral[:, :, 1] * activation * (0.62 + np.tanh(spec_g * 4.0) * 0.38)
    reflect_b = spectral[:, :, 2] * activation * (0.62 + np.tanh(spec_b * 4.0) * 0.38)
    
    # Apply active mask (only foil units reflect)
    reflect_r *= active_mask
    reflect_g *= active_mask
    reflect_b *= active_mask
    
    # Stack into RGB
    reflection = np.stack([reflect_r, reflect_g, reflect_b], axis=-1)
    
    # Composite: additive blend on top of darkened base.
    # Real SER foil: metallic layer darkens the art, then rainbow reflections
    # add bright color on top. Additive blend matches this physical behavior.
    overlay = reflection * foil_intensity * active_mask[:, :, np.newaxis]
    result = np.clip(base_image + overlay, 0.0, 1.0)
    
    # Add subtle sparkle points
    print("  Adding sparkle points...")
    result = add_sparkles(result, active_mask, spec_g, foil_intensity * 0.6)
    
    return np.clip(result, 0.0, 1.0)


def hsv_to_rgb_array(h: np.ndarray, s: float, v: float) -> np.ndarray:
    """Vectorized HSV->RGB for h in [0,1]. Returns (...,3)."""
    h = h % 1.0
    i = np.floor(h * 6.0).astype(np.int32)
    f = h * 6.0 - i
    p = v * (1.0 - s)
    q = v * (1.0 - f * s)
    t = v * (1.0 - (1.0 - f) * s)

    i_mod = i % 6
    r = np.select(
        [i_mod == 0, i_mod == 1, i_mod == 2, i_mod == 3, i_mod == 4],
        [v, q, p, p, t],
        default=v,
    )
    g = np.select(
        [i_mod == 0, i_mod == 1, i_mod == 2, i_mod == 3, i_mod == 4],
        [t, v, v, q, p],
        default=p,
    )
    b = np.select(
        [i_mod == 0, i_mod == 1, i_mod == 2, i_mod == 3, i_mod == 4],
        [p, p, t, v, v],
        default=q,
    )
    return np.stack([r, g, b], axis=-1).astype(np.float32)


def default_art_rect(width: int, height: int) -> tuple[int, int, int, int]:
    """
    Art rect for current generated non-pendulum card output.

    Renderer canvas is 1394×2031. These values match the normal illustration
    frame visible in tmp/2511-card.png and tmp/483-card.png. If the renderer
    layout changes, pass explicit coords here or port from BaseLayout.
    """
    if (width, height) == (1394, 2031):
        return (148, 354, 1098, 1098)
    # Fallback: approximate the same relative position.
    return (
        round(width * 148 / 1394),
        round(height * 354 / 2031),
        round(width * 1098 / 1394),
        round(height * 1098 / 2031),
    )


def apply_ser_to_card_art(base: np.ndarray, **kwargs) -> np.ndarray:
    """Apply the SER prototype only to the card illustration area."""
    h, w = base.shape[:2]
    x, y, aw, ah = default_art_rect(w, h)
    x2 = min(w, x + aw)
    y2 = min(h, y + ah)

    result = base.copy()
    art = base[y:y2, x:x2, :]
    effected = render_ser_physical(art, **kwargs)
    result[y:y2, x:x2, :] = effected
    return result


def add_sparkles(
    image: np.ndarray,
    active_mask: np.ndarray,
    spec_intensity: np.ndarray,
    intensity: float = 0.5,
    density: float = 0.003,
) -> np.ndarray:
    """
    Add random sparkle/glint points on high-specular areas.
    """
    height, width = image.shape[:2]
    
    # Deterministic random based on pixel position
    rng = np.random.default_rng(seed=42)
    noise = rng.random((height, width), dtype=np.float32)
    
    # Sparkle where: active, high specular, and random threshold
    threshold = 1.0 - density
    sparkle_mask = (noise > threshold) & (active_mask > 0.5) & (spec_intensity > 0.1)
    
    # Sparkle color: warm white
    sparkle_strength = intensity * spec_intensity * sparkle_mask.astype(np.float32)
    
    image[:, :, 0] = np.clip(image[:, :, 0] + sparkle_strength * 0.97, 0, 1)
    image[:, :, 1] = np.clip(image[:, :, 1] + sparkle_strength * 0.94, 0, 1)
    image[:, :, 2] = np.clip(image[:, :, 2] + sparkle_strength * 0.76, 0, 1)
    
    return image


# ─────────────────────────────────────────────────────────────────────────────
# Main: Load image, render effect, save output
# ─────────────────────────────────────────────────────────────────────────────

def main():
    script_dir = os.path.dirname(os.path.abspath(__file__))

    # Test on images generated by the current card renderer. The photo-like
    # reference files image.png/img2.png are intentionally not used as inputs.
    input_files = ["2511-card.png", "483-card.png"]

    # Render with multiple viewing angles for comparison.
    angles = [
        (25.0, 135.0, "default"),
        (35.0, 45.0, "tilted_left"),
    ]

    for input_name in input_files:
        input_path = os.path.join(script_dir, input_name)
        if not os.path.exists(input_path):
            print(f"Skipping missing input: {input_path}")
            continue

        print(f"\nLoading generated card: {input_path}")
        img = Image.open(input_path).convert("RGB")
        base = np.array(img, dtype=np.float32) / 255.0
        print(f"Image size: {base.shape[1]}x{base.shape[0]}")

        stem = os.path.splitext(input_name)[0]
        for theta, phi, label in angles:
            print(f"\nRendering {stem}: theta={theta}, phi={phi} ({label})")
            result = apply_ser_to_card_art(
                base,
                theta=theta,
                phi=phi,
                light_theta=30.0,
                light_phi=phi - 15.0,  # light slightly offset from view
                roughness_x=0.055,
                roughness_y=0.34,
                dispersion=0.045,
                edge_angle=38.0,
                foil_intensity=1.80,
                tangent_angle=0.0,
                f0=0.18,
                darken=0.60,
            )

            output_path = os.path.join(script_dir, f"{stem}-ser-physical-{label}.png")
            out_img = Image.fromarray((result * 255).astype(np.uint8))
            out_img.save(output_path)
            print(f"  Saved: {output_path}")
    
    print("\nDone! Check tmp/ folder for output images.")


if __name__ == "__main__":
    main()
