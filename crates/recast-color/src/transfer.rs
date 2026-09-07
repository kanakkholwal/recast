#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(rename_all = "kebab-case")
)]
pub enum TransferFunction {
    #[default]
    Srgb,
    Linear,
    Gamma22,
    Rec709,
    Pq,
    Hlg,
}

impl TransferFunction {
    pub fn to_linear(self, encoded: f32) -> f32 {
        match self {
            Self::Srgb => srgb_to_linear(encoded),
            Self::Linear => encoded,
            Self::Gamma22 => signed_powf(encoded, 2.2),
            Self::Rec709 => rec709_to_linear(encoded),
            Self::Pq => pq_to_linear(encoded),
            Self::Hlg => hlg_to_linear(encoded),
        }
    }

    pub fn from_linear(self, linear: f32) -> f32 {
        match self {
            Self::Srgb => linear_to_srgb(linear),
            Self::Linear => linear,
            Self::Gamma22 => signed_powf(linear, 1.0 / 2.2),
            Self::Rec709 => linear_to_rec709(linear),
            Self::Pq => linear_to_pq(linear),
            Self::Hlg => linear_from_hlg(linear),
        }
    }
}

fn signed_powf(v: f32, exp: f32) -> f32 {
    if v < 0.0 {
        -(-v).powf(exp)
    } else {
        v.powf(exp)
    }
}

pub fn srgb_to_linear(c: f32) -> f32 {
    if c < 0.0 {
        -srgb_to_linear(-c)
    } else if c <= 0.040_45 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

pub fn linear_to_srgb(c: f32) -> f32 {
    if c < 0.0 {
        -linear_to_srgb(-c)
    } else if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

pub fn rec709_to_linear(c: f32) -> f32 {
    if c < 0.0 {
        -rec709_to_linear(-c)
    } else if c < 0.081 {
        c / 4.5
    } else {
        ((c + 0.099) / 1.099).powf(1.0 / 0.45)
    }
}

pub fn linear_to_rec709(c: f32) -> f32 {
    if c < 0.0 {
        -linear_to_rec709(-c)
    } else if c < 0.018 {
        c * 4.5
    } else {
        1.099 * c.powf(0.45) - 0.099
    }
}

const PQ_M1: f32 = 0.159_301_76;
const PQ_M2: f32 = 78.843_75;
const PQ_C1: f32 = 0.835_937_5;
const PQ_C2: f32 = 18.851_562;
const PQ_C3: f32 = 18.687_5;

/// Normalised to 1.0 == 10000 cd/m², the PQ reference peak.
pub fn pq_to_linear(c: f32) -> f32 {
    let e = c.clamp(0.0, 1.0).powf(1.0 / PQ_M2);
    let num = (e - PQ_C1).max(0.0);
    let den = PQ_C2 - PQ_C3 * e;
    if den <= 0.0 {
        0.0
    } else {
        (num / den).powf(1.0 / PQ_M1)
    }
}

pub fn linear_to_pq(c: f32) -> f32 {
    let y = c.clamp(0.0, 1.0).powf(PQ_M1);
    ((PQ_C1 + PQ_C2 * y) / (1.0 + PQ_C3 * y)).powf(PQ_M2)
}

const HLG_A: f32 = 0.178_832_77;
const HLG_B: f32 = 0.284_668_92;
const HLG_C: f32 = 0.559_910_7;

pub fn hlg_to_linear(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.5 {
        c * c / 3.0
    } else {
        (((c - HLG_C) / HLG_A).exp() + HLG_B) / 12.0
    }
}

pub fn linear_from_hlg(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 1.0 / 12.0 {
        (3.0 * c).sqrt()
    } else {
        HLG_A * (12.0 * c - HLG_B).ln() + HLG_C
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trips(f: TransferFunction, tolerance: f32) {
        for step in 0..=100 {
            let encoded = step as f32 / 100.0;
            let back = f.from_linear(f.to_linear(encoded));
            assert!(
                (back - encoded).abs() < tolerance,
                "{f:?} at {encoded}: got {back}"
            );
        }
    }

    #[test]
    fn every_transfer_function_round_trips() {
        round_trips(TransferFunction::Srgb, 1e-5);
        round_trips(TransferFunction::Linear, 1e-6);
        round_trips(TransferFunction::Gamma22, 1e-5);
        round_trips(TransferFunction::Rec709, 1e-5);
        round_trips(TransferFunction::Pq, 1e-4);
        round_trips(TransferFunction::Hlg, 1e-4);
    }

    #[test]
    fn srgb_endpoints_are_exact() {
        assert_eq!(srgb_to_linear(0.0), 0.0);
        assert!((srgb_to_linear(1.0) - 1.0).abs() < 1e-6);
        assert!((linear_to_srgb(1.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn srgb_mid_grey_matches_the_published_value() {
        assert!((srgb_to_linear(0.5) - 0.214_041_14).abs() < 1e-5);
    }

    #[test]
    fn the_srgb_curve_is_continuous_at_the_segment_join() {
        let below = srgb_to_linear(0.040_44);
        let above = srgb_to_linear(0.040_46);
        assert!((above - below).abs() < 1e-5);
    }

    #[test]
    fn srgb_is_defined_for_negative_input_so_a_wide_gamut_sample_survives() {
        assert!((srgb_to_linear(-0.5) + srgb_to_linear(0.5)).abs() < 1e-9);
    }

    #[test]
    fn pq_peak_is_the_reference_white() {
        assert!((linear_to_pq(1.0) - 1.0).abs() < 1e-5);
        assert!((pq_to_linear(1.0) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn srgb_is_not_the_same_curve_as_plain_gamma_22() {
        let a = TransferFunction::Srgb.to_linear(0.5);
        let b = TransferFunction::Gamma22.to_linear(0.5);
        assert!((a - b).abs() > 1e-3);
    }
}
