//! Which bottom edge a pet stands on, from the monitors' work areas (physical pixels).

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ground {
    pub left: f64,
    pub right: f64,
    /// The bottom of the work area: pets' feet rest here.
    pub y: f64,
    /// Physical pixels per point on this monitor.
    pub px_per_pt: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MonitorInfo {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub scale: f64,
    pub primary: bool,
}

impl MonitorInfo {
    fn ground(&self) -> Ground {
        Ground { left: self.x, right: self.x + self.width, y: self.y + self.height, px_per_pt: self.scale }
    }
    fn contains_x(&self, px: f64) -> bool {
        px >= self.x && px < self.x + self.width
    }
}

/// Pets live in a "desktop space" shared by all monitors. macOS has no consistent physical space across
/// mixed-DPI monitors (tao reports each monitor in its own scale and the cursor in the primary's), so there
/// the space is logical points; Windows' physical virtual screen is consistent, so there it's physical pixels.
pub const LOGICAL_DESKTOP: bool = cfg!(target_os = "macos");

/// A monitor's work area (as tao reports it, in physical px of its own scale) in desktop space.
pub fn desktop_monitor(x: f64, y: f64, width: f64, height: f64, scale: f64, primary: bool, logical: bool) -> MonitorInfo {
    if logical {
        MonitorInfo { x: x / scale, y: y / scale, width: width / scale, height: height / scale, scale: 1.0, primary }
    } else {
        MonitorInfo { x, y, width, height, scale, primary }
    }
}

/// The cursor (as tao reports it, in physical px of the primary monitor's scale) in desktop space.
pub fn desktop_cursor(x: f64, y: f64, primary_scale: f64, logical: bool) -> (f64, f64) {
    if logical { (x / primary_scale, y / primary_scale) } else { (x, y) }
}

/// Desktop-space units per sprite pixel, rounded to whole units so pixels stay crisp at fractional DPI.
pub fn sprite_px(scale: u32, px_per_pt: f64) -> f64 {
    (scale as f64 * px_per_pt).round().max(1.0)
}

/// The ground under (px, py): the monitor containing the point just above the feet, else the monitor
/// whose column contains px, else the primary monitor, else the first.
pub fn ground_for(px: f64, py: f64, monitors: &[MonitorInfo]) -> Option<Ground> {
    let probe_y = py - 1.0;
    monitors
        .iter()
        .find(|m| m.contains_x(px) && probe_y >= m.y && probe_y < m.y + m.height)
        .or_else(|| monitors.iter().find(|m| m.contains_x(px)))
        .or_else(|| monitors.iter().find(|m| m.primary))
        .or_else(|| monitors.first())
        .map(MonitorInfo::ground)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitors() -> Vec<MonitorInfo> {
        vec![
            MonitorInfo { x: 0.0, y: 0.0, width: 2880.0, height: 1700.0, scale: 2.0, primary: true },
            MonitorInfo { x: 2880.0, y: 200.0, width: 1920.0, height: 1040.0, scale: 1.0, primary: false },
        ]
    }

    #[test]
    fn picks_the_monitor_under_the_pet() {
        let g = ground_for(3000.0, 900.0, &monitors()).unwrap();
        assert_eq!(g, Ground { left: 2880.0, right: 4800.0, y: 1240.0, px_per_pt: 1.0 });
    }

    #[test]
    fn feet_exactly_on_the_ground_stay_on_that_monitor() {
        assert_eq!(ground_for(3000.0, 1240.0, &monitors()).unwrap().right, 4800.0);
    }

    #[test]
    fn point_above_a_monitor_uses_that_monitors_column() {
        assert_eq!(ground_for(3000.0, -500.0, &monitors()).unwrap().left, 2880.0);
    }

    #[test]
    fn point_outside_every_monitor_falls_back_to_primary() {
        let g = ground_for(99_999.0, 99_999.0, &monitors()).unwrap();
        assert_eq!((g.left, g.y, g.px_per_pt), (0.0, 1700.0, 2.0));
    }

    #[test]
    fn macos_mixed_dpi_uses_logical_points_so_monitors_and_cursor_agree() {
        // Retina laptop @2x (1512x982 pt) + 1x external to its right at logical x=1512.
        // tao reports each monitor in its own scale and the cursor in the primary's scale.
        let laptop = desktop_monitor(0.0, 0.0, 3024.0, 1964.0, 2.0, true, true);
        let external = desktop_monitor(1512.0, 0.0, 1920.0, 1080.0, 1.0, false, true);
        assert_eq!((laptop.x, laptop.width, laptop.scale), (0.0, 1512.0, 1.0));
        assert_eq!((external.x, external.width, external.scale), (1512.0, 1920.0, 1.0));
        let (cx, cy) = desktop_cursor(4024.0, 1000.0, 2.0, true); // physical reading of logical (2012, 500)
        assert_eq!((cx, cy), (2012.0, 500.0));
        let g = ground_for(cx, cy, &[laptop, external]).unwrap();
        assert_eq!((g.left, g.right, g.y), (1512.0, 3432.0, 1080.0), "cursor over the external grounds there");
    }

    #[test]
    fn windows_keeps_physical_pixels() {
        let m = desktop_monitor(1920.0, 0.0, 2560.0, 1400.0, 1.5, false, false);
        assert_eq!((m.x, m.width, m.scale), (1920.0, 2560.0, 1.5));
        assert_eq!(desktop_cursor(2000.0, 10.0, 1.0, false), (2000.0, 10.0));
    }

    #[test]
    fn sprite_pixels_snap_to_whole_device_pixels() {
        assert_eq!(sprite_px(3, 1.25), 4.0); // 3.75 → 4 on 125% Windows
        assert_eq!(sprite_px(2, 1.5), 3.0);
        assert_eq!(sprite_px(3, 2.0), 6.0);
        assert_eq!(sprite_px(2, 0.25), 1.0); // never zero
    }

    #[test]
    fn no_monitors_means_no_ground() {
        assert!(ground_for(0.0, 0.0, &[]).is_none());
    }
}
