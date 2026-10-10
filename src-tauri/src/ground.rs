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
    fn no_monitors_means_no_ground() {
        assert!(ground_for(0.0, 0.0, &[]).is_none());
    }
}
