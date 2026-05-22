use crate::document::{EffectStyle, EffectTargetWeight};

pub(super) fn sanitize_effect_style(effect: EffectStyle) -> EffectStyle {
    let opacity = sanitize_opacity(effect_opacity(effect));
    effect_with_opacity(effect, opacity)
}

pub(super) fn effect_opacity(effect: EffectStyle) -> f32 {
    match effect {
        EffectStyle::RainbowFoil { opacity }
        | EffectStyle::DotGrid { opacity }
        | EffectStyle::OpticalSer { opacity }
        | EffectStyle::OpticalSerSimple { opacity }
        | EffectStyle::OpticalScr { opacity }
        | EffectStyle::OpticalScrSimple { opacity }
        | EffectStyle::SecretWeave { opacity }
        | EffectStyle::SecretFoil { opacity }
        | EffectStyle::Holographic { opacity }
        | EffectStyle::BrightBorder { opacity }
        | EffectStyle::GoldWash { opacity }
        | EffectStyle::FrostedFoil { opacity }
        | EffectStyle::ConcentricEngrave { opacity }
        | EffectStyle::ReliefEngrave { opacity }
        | EffectStyle::DiamondFoil { opacity } => opacity,
    }
}

pub(super) fn composite_base_opacity(effect: EffectStyle, targets: &[EffectTargetWeight]) -> f32 {
    targets
        .iter()
        .map(|target| sanitize_opacity(target.opacity))
        .fold(effect_opacity(effect), f32::max)
}

pub(super) fn effect_with_opacity(effect: EffectStyle, opacity: f32) -> EffectStyle {
    let opacity = sanitize_opacity(opacity);
    match effect {
        EffectStyle::RainbowFoil { .. } => EffectStyle::RainbowFoil { opacity },
        EffectStyle::DotGrid { .. } => EffectStyle::DotGrid { opacity },
        EffectStyle::OpticalSer { .. } => EffectStyle::OpticalSer { opacity },
        EffectStyle::OpticalSerSimple { .. } => EffectStyle::OpticalSerSimple { opacity },
        EffectStyle::OpticalScr { .. } => EffectStyle::OpticalScr { opacity },
        EffectStyle::OpticalScrSimple { .. } => EffectStyle::OpticalScrSimple { opacity },
        EffectStyle::SecretWeave { .. } => EffectStyle::SecretWeave { opacity },
        EffectStyle::SecretFoil { .. } => EffectStyle::SecretFoil { opacity },
        EffectStyle::Holographic { .. } => EffectStyle::Holographic { opacity },
        EffectStyle::BrightBorder { .. } => EffectStyle::BrightBorder { opacity },
        EffectStyle::GoldWash { .. } => EffectStyle::GoldWash { opacity },
        EffectStyle::FrostedFoil { .. } => EffectStyle::FrostedFoil { opacity },
        EffectStyle::ConcentricEngrave { .. } => EffectStyle::ConcentricEngrave { opacity },
        EffectStyle::ReliefEngrave { .. } => EffectStyle::ReliefEngrave { opacity },
        EffectStyle::DiamondFoil { .. } => EffectStyle::DiamondFoil { opacity },
    }
}

pub(super) fn sanitize_opacity(opacity: f32) -> f32 {
    if opacity.is_finite() {
        opacity.clamp(0.0, 1.0)
    } else {
        0.0
    }
}
