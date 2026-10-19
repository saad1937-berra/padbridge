// src-tauri/src/pipeline/deadzone.rs
//! Zones mortes continues : la sortie part de 0 à la limite de la zone, donc aucune
//! oscillation due au bruit analogique autour du seuil. Valeurs bornées à [0, 0.5].

use crate::devices::model::Stick;

/// Zone morte radiale avec remise à l'échelle de la plage restante.
pub fn radial(stick: Stick, deadzone: f32) -> Stick {
    let magnitude = (stick.x * stick.x + stick.y * stick.y).sqrt();
    if magnitude <= deadzone || magnitude == 0.0 {
        return Stick::default();
    }
    let scaled = ((magnitude.min(1.0) - deadzone) / (1.0 - deadzone)).clamp(0.0, 1.0);
    Stick {
        x: stick.x / magnitude * scaled,
        y: stick.y / magnitude * scaled,
    }
}

/// Seuil de gâchette avec remise à l'échelle.
pub fn trigger(value: f32, threshold: f32) -> f32 {
    if value <= threshold {
        0.0
    } else {
        ((value - threshold) / (1.0 - threshold)).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inside_zone_is_zero() {
        let s = radial(Stick { x: 0.05, y: -0.05 }, 0.1);
        assert_eq!(s, Stick::default());
    }

    #[test]
    fn full_deflection_stays_full() {
        let s = radial(Stick { x: 1.0, y: 0.0 }, 0.2);
        assert!((s.x - 1.0).abs() < 1e-6 && s.y == 0.0);
    }

    #[test]
    fn output_is_continuous_at_the_edge() {
        let just_outside = radial(Stick { x: 0.1001, y: 0.0 }, 0.1);
        assert!(just_outside.x < 0.01);
    }

    #[test]
    fn direction_is_preserved() {
        let s = radial(Stick { x: -0.6, y: 0.8 }, 0.1);
        assert!(s.x < 0.0 && s.y > 0.0);
        assert!(((s.y / s.x) - (0.8 / -0.6)).abs() < 1e-5);
    }

    #[test]
    fn corners_are_clamped() {
        let s = radial(Stick { x: 1.0, y: 1.0 }, 0.1);
        let m = (s.x * s.x + s.y * s.y).sqrt();
        assert!((m - 1.0).abs() < 1e-5);
    }

    #[test]
    fn trigger_threshold() {
        assert_eq!(trigger(0.02, 0.05), 0.0);
        assert_eq!(trigger(1.0, 0.05), 1.0);
        assert!((trigger(0.525, 0.05) - 0.5).abs() < 1e-6);
    }
}
