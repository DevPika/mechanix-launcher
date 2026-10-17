/// Speed, in px/s, for an eased transition that starts at the release
/// velocity `v_px_per_ms`: the release speed (floored at `min_px_per_s`)
/// divided by the easing curve's slope at t = 0.
pub fn throw_speed(v_px_per_ms: f32, min_px_per_s: f32, slope: f32) -> f32 {
    (v_px_per_ms.abs() * 1000.0).max(min_px_per_s) / slope
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn throw_speed_uses_min_and_slope() {
        assert_eq!(throw_speed(0.1, 1500.0, 3.0), 500.0);
        assert_eq!(throw_speed(-3.0, 1500.0, 3.0), 1000.0);
    }
}
