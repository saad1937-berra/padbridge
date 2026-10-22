// Analyse indicative de dérive, sur données BRUTES uniquement.
// Le test doit être déclenché volontairement lorsque les sticks sont relâchés.
use crate::devices::model::Stick;
use crate::error::{codes, AppError, AppResult};
use serde::{Deserialize, Serialize};

const MIN_SAMPLES: usize = 30;
const MAX_SAMPLES: usize = 240;
const NORMAL_THRESHOLD: f32 = 0.10;
const SUSPECT_THRESHOLD: f32 = 0.12;
const MOVEMENT_THRESHOLD: f32 = 0.55;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriftSample {
    pub left: Stick,
    pub right: Stick,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftStatus {
    Normal,
    PossibleDrift,
    MovementDetected,
    Inconclusive,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StickDriftResult {
    pub status: DriftStatus,
    pub mean_magnitude: f32,
    pub peak_magnitude: f32,
    pub suspect_ratio: f32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriftReport {
    pub sample_count: usize,
    pub left: StickDriftResult,
    pub right: StickDriftResult,
}

fn analyze_one(values: impl Iterator<Item = Stick> + Clone) -> StickDriftResult {
    let mut count = 0;
    let mut sum = 0.0_f32;
    let mut peak = 0.0_f32;
    let mut suspects = 0;
    let mut normal = 0;
    for value in values {
        let magnitude = value.x.hypot(value.y).min(1.0);
        count += 1;
        sum += magnitude;
        peak = peak.max(magnitude);
        if magnitude > SUSPECT_THRESHOLD { suspects += 1; }
        if magnitude <= NORMAL_THRESHOLD { normal += 1; }
    }
    let mean = sum / count as f32;
    let ratio = suspects as f32 / count as f32;
    let normal_ratio = normal as f32 / count as f32;
    let status = if peak >= MOVEMENT_THRESHOLD {
        DriftStatus::MovementDetected
    } else if ratio >= 0.75 && mean >= SUSPECT_THRESHOLD {
        DriftStatus::PossibleDrift
    } else if normal_ratio >= 0.85 && peak <= 0.15 {
        DriftStatus::Normal
    } else {
        DriftStatus::Inconclusive
    };
    StickDriftResult {
        status,
        mean_magnitude: (mean * 10000.0).round() / 10000.0,
        peak_magnitude: (peak * 10000.0).round() / 10000.0,
        suspect_ratio: (ratio * 10000.0).round() / 10000.0,
    }
}

pub fn analyze(samples: &[DriftSample]) -> AppResult<DriftReport> {
    if !(MIN_SAMPLES..=MAX_SAMPLES).contains(&samples.len()) {
        return Err(AppError::new(
            codes::INVALID_ARGUMENT,
            format!("diagnostic drift : entre {MIN_SAMPLES} et {MAX_SAMPLES} échantillons requis"),
        ));
    }
    for s in samples {
        for axis in [s.left.x, s.left.y, s.right.x, s.right.y] {
            if !axis.is_finite() || !(-1.0..=1.0).contains(&axis) {
                return Err(AppError::new(codes::INVALID_ARGUMENT, "axe hors plage [-1, 1]"));
            }
        }
    }
    Ok(DriftReport {
        sample_count: samples.len(),
        left: analyze_one(samples.iter().map(|s| s.left)),
        right: analyze_one(samples.iter().map(|s| s.right)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repeated(left: Stick, right: Stick) -> Vec<DriftSample> {
        vec![DriftSample { left, right }; 90]
    }

    #[test]
    fn neutral_sticks_are_normal() {
        let r = analyze(&repeated(Stick { x: 0.02, y: -0.01 }, Stick::default())).unwrap();
        assert_eq!(r.sample_count, 90);
        assert_eq!(r.left.status, DriftStatus::Normal);
        assert_eq!(r.right.status, DriftStatus::Normal);
    }

    #[test]
    fn persistent_offset_is_suspected_drift() {
        let r = analyze(&repeated(Stick { x: 0.20, y: 0.0 }, Stick::default())).unwrap();
        assert_eq!(r.left.status, DriftStatus::PossibleDrift);
        assert_eq!(r.right.status, DriftStatus::Normal);
    }

    #[test]
    fn deliberate_full_movement_requires_retest() {
        let r = analyze(&repeated(Stick { x: 0.95, y: 0.0 }, Stick::default())).unwrap();
        assert_eq!(r.left.status, DriftStatus::MovementDetected);
    }

    #[test]
    fn intermittent_deviation_is_inconclusive() {
        let mut data = repeated(Stick::default(), Stick::default());
        for s in data.iter_mut().take(40) { s.left.x = 0.20; }
        let r = analyze(&data).unwrap();
        assert_eq!(r.left.status, DriftStatus::Inconclusive);
    }

    #[test]
    fn too_few_samples_are_refused() {
        let err = analyze(&repeated(Stick::default(), Stick::default())[..10]).err().unwrap();
        assert_eq!(err.code, codes::INVALID_ARGUMENT);
    }

    #[test]
    fn malformed_axes_are_refused() {
        let samples = repeated(Stick { x: f32::NAN, y: 0.0 }, Stick::default());
        assert_eq!(analyze(&samples).err().unwrap().code, codes::INVALID_ARGUMENT);
        let samples = repeated(Stick { x: 1.5, y: 0.0 }, Stick::default());
        assert_eq!(analyze(&samples).err().unwrap().code, codes::INVALID_ARGUMENT);
    }
}
