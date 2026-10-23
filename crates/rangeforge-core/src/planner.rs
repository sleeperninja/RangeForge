//! Gain planning: how much louder or quieter each track should be.
//!
//! Pure arithmetic on measurements we already have, so changing the target in the UI never
//! needs to re-measure or re-decode anything.
//!
//! The gain is a single linear offset in dB, which keeps the track's dynamics intact. An
//! optional true-peak ceiling caps it so the result never clips.

use thiserror::Error;

/// What was measured for one track.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Measurement {
    /// Integrated loudness in LUFS.
    pub integrated_lufs: f64,
    /// True peak in dBTP.
    pub true_peak_dbtp: f64,
    /// Loudness range in LU.
    pub loudness_range_lu: f64,
}

/// What the user asked for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Target {
    pub loudness_lufs: f64,
    /// When set, gain is capped so the true peak does not exceed this level.
    pub peak_ceiling_dbtp: Option<f64>,
}

/// The decision for one track.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GainPlan {
    /// Gain to apply, in dB. Negative means quieter.
    pub gain_db: f64,
    /// True when the peak ceiling reduced the gain below what the loudness target wanted.
    pub limited_by_peak: bool,
    /// Integrated loudness after the gain is applied.
    pub resulting_lufs: f64,
    /// True peak after the gain is applied.
    pub resulting_peak_dbtp: f64,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PlanError {
    #[error("the measurement is not usable (silent audio or a non-finite value)")]
    NotMeasurable,
    #[error("the target is not usable (a non-finite value)")]
    InvalidTarget,
}

/// Works out the gain for one track.
///
/// If a ceiling is set and the track's true peak is already above it, the gain goes negative
/// so the peak lands on the ceiling. That is the point of peak protection.
pub fn plan_gain(measurement: Measurement, target: Target) -> Result<GainPlan, PlanError> {
    if !measurement.integrated_lufs.is_finite() || !measurement.true_peak_dbtp.is_finite() {
        return Err(PlanError::NotMeasurable);
    }
    if !target.loudness_lufs.is_finite() || target.peak_ceiling_dbtp.is_some_and(|c| !c.is_finite())
    {
        return Err(PlanError::InvalidTarget);
    }

    let wanted_db = target.loudness_lufs - measurement.integrated_lufs;
    let (gain_db, limited_by_peak) = match target.peak_ceiling_dbtp {
        Some(ceiling) => {
            let headroom_db = ceiling - measurement.true_peak_dbtp;
            if wanted_db > headroom_db {
                (headroom_db, true)
            } else {
                (wanted_db, false)
            }
        }
        None => (wanted_db, false),
    };

    Ok(GainPlan {
        gain_db,
        limited_by_peak,
        resulting_lufs: measurement.integrated_lufs + gain_db,
        resulting_peak_dbtp: measurement.true_peak_dbtp + gain_db,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    fn measured(lufs: f64, peak: f64) -> Measurement {
        Measurement {
            integrated_lufs: lufs,
            true_peak_dbtp: peak,
            loudness_range_lu: 10.0,
        }
    }

    const TARGET: Target = Target {
        loudness_lufs: -22.0,
        peak_ceiling_dbtp: Some(-0.6),
    };

    #[test]
    fn matches_the_mockup_adjust_column() {
        // Values from "1. UI Mockups - Main.png", target -22 LUFS.
        let cases = [
            (-14.2, -1.65, -7.8),
            (-19.53, -1.0, -2.47),
            (-18.1, -1.0, -3.9),
        ];
        for (lufs, peak, expected) in cases {
            let plan = plan_gain(measured(lufs, peak), TARGET).unwrap();
            assert!(close(plan.gain_db, expected), "{lufs}: {}", plan.gain_db);
            assert!(!plan.limited_by_peak);
            assert!(close(plan.resulting_lufs, -22.0));
        }
    }

    #[test]
    fn ceiling_caps_a_boost() {
        let target = Target {
            loudness_lufs: -14.0,
            peak_ceiling_dbtp: Some(-0.6),
        };
        let plan = plan_gain(measured(-20.0, -1.0), target).unwrap();
        assert!(close(plan.gain_db, 0.4));
        assert!(plan.limited_by_peak);
        assert!(close(plan.resulting_peak_dbtp, -0.6));
        assert!(close(plan.resulting_lufs, -19.6));
    }

    #[test]
    fn ceiling_attenuates_a_peak_that_is_already_too_high() {
        let target = Target {
            loudness_lufs: -10.0,
            peak_ceiling_dbtp: Some(-0.6),
        };
        let plan = plan_gain(measured(-10.0, -0.2), target).unwrap();
        assert!(close(plan.gain_db, -0.4));
        assert!(plan.limited_by_peak);
    }

    #[test]
    fn without_a_ceiling_the_target_is_always_met() {
        let target = Target {
            loudness_lufs: -14.0,
            peak_ceiling_dbtp: None,
        };
        let plan = plan_gain(measured(-20.0, -1.0), target).unwrap();
        assert!(close(plan.gain_db, 6.0));
        assert!(!plan.limited_by_peak);
        assert!(plan.resulting_peak_dbtp > 0.0);
    }

    #[test]
    fn silence_and_garbage_are_rejected() {
        assert_eq!(
            plan_gain(measured(f64::NEG_INFINITY, -1.0), TARGET),
            Err(PlanError::NotMeasurable)
        );
        assert_eq!(
            plan_gain(measured(-14.0, f64::NAN), TARGET),
            Err(PlanError::NotMeasurable)
        );
        let bad_target = Target {
            loudness_lufs: f64::NAN,
            peak_ceiling_dbtp: None,
        };
        assert_eq!(
            plan_gain(measured(-14.0, -1.0), bad_target),
            Err(PlanError::InvalidTarget)
        );
    }
}
