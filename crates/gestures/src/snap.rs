/// Picks the page to settle on after a horizontal drag release.
pub fn snap_page(
    current: usize,
    offset: f32,
    velocity: f32,
    page_width: f32,
    count: usize,
    momentum_ms: f32,
    min_fraction: f32,
) -> usize {
    if count == 0 {
        return 0;
    }
    let projected = offset + velocity * momentum_ms;
    let mut delta = (-projected / page_width).round() as isize;
    if delta == 0 && offset.abs() > page_width * min_fraction {
        delta = if offset < 0.0 { 1 } else { -1 };
    }
    (current as isize + delta).clamp(0, count as isize - 1) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snap_follows_momentum() {
        assert_eq!(snap_page(1, -100.0, -1.0, 540.0, 5, 180.0, 0.16), 2);
    }

    #[test]
    fn snap_small_drag_with_no_velocity_stays() {
        assert_eq!(snap_page(1, -50.0, 0.0, 540.0, 5, 180.0, 0.16), 1);
    }

    #[test]
    fn snap_min_fraction_moves_one() {
        assert_eq!(snap_page(1, -90.0, 0.0, 540.0, 5, 180.0, 0.16), 2);
    }

    #[test]
    fn snap_clamps() {
        assert_eq!(snap_page(0, 2000.0, 3.0, 540.0, 5, 180.0, 0.16), 0);
        assert_eq!(snap_page(4, -2000.0, -3.0, 540.0, 5, 180.0, 0.16), 4);
    }
}
