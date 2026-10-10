//! One pet's behavior: walk / idle / flourish / nap on the ground, plus drag, fall and land.
//! Pure logic in physical pixels; the pets module moves windows to match.

use crate::animdata::AnimTable;
use crate::ground::Ground;
use rand::{RngExt, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::Serialize;
use std::collections::HashMap;

pub const DOWN: u8 = 0;
pub const RIGHT: u8 = 2;
pub const LEFT: u8 = 6;
pub const FLOURISH_PREFERENCE: [&str; 7] = ["Pose", "Hop", "Rotate", "Charge", "Double", "Swing", "Attack"];

/// Tuning (spec §5.2).
const WEIGHT_WALK: u32 = 50;
const WEIGHT_IDLE: u32 = 30;
const WEIGHT_FLOURISH: u32 = 15;
const WEIGHT_NAP: u32 = 5;
const IDLE_SECS: std::ops::Range<f64> = 2.0..8.0;
const NAP_SECS: std::ops::Range<f64> = 20.0..60.0;
const WALK_PTS: std::ops::Range<f64> = 40.0..240.0;
const WALK_PTS_PER_SEC_PER_SCALE: f64 = 10.0;
const GRAVITY_PTS_PER_SEC2: f64 = 2400.0;
const DRAG_TURN_PX: f64 = 6.0;

/// Screen-space movement (y down) to a PMD direction row.
pub fn dir_from_delta(dx: f64, dy: f64) -> u8 {
    let angle = dy.atan2(dx).to_degrees(); // 0 right, 90 down, ±180 left, -90 up
    (((2 - (angle / 45.0).round() as i32) % 8 + 8) % 8) as u8
}

/// Every animation a pet may show (for sizing its window).
pub fn behavior_anims(signature: Option<&str>) -> Vec<String> {
    let mut names: Vec<String> = ["Walk", "Idle", "Sleep", "Hop"].iter().map(|s| s.to_string()).collect();
    for n in FLOURISH_PREFERENCE.iter().copied().chain(signature) {
        if !names.iter().any(|x| x == n) {
            names.push(n.to_string());
        }
    }
    names
}

#[derive(Debug, Clone)]
pub struct AnimSet {
    secs: HashMap<String, f64>,
}

impl AnimSet {
    pub fn from_table(t: &AnimTable) -> Self {
        AnimSet { secs: t.anims.iter().map(|a| (a.name.clone(), a.total_secs())).collect() }
    }
    pub fn from_secs(list: &[(&str, f64)]) -> Self {
        AnimSet { secs: list.iter().map(|(n, s)| (n.to_string(), *s)).collect() }
    }
    fn has(&self, name: &str) -> bool {
        self.secs.contains_key(name)
    }
    fn secs(&self, name: &str) -> f64 {
        self.secs.get(name).copied().unwrap_or(0.0)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Visual {
    pub anim: String,
    pub dir: u8,
    pub looped: bool,
    /// Bumps on every mode change so the renderer restarts the animation.
    pub seq: u64,
}

#[derive(Debug, Clone, PartialEq)]
enum Mode {
    Walk { remaining_pts: f64 },
    Idle { remaining: f64 },
    OneShot { anim: String, remaining: f64 },
    Nap { remaining: f64 },
    Dragged,
    Falling { vy: f64 },
    Landing { remaining: f64 },
}

pub struct Brain {
    mode: Mode,
    /// Anchor (frame center) x, physical px.
    pub x: f64,
    /// Feet y (bottom of the window), physical px.
    pub y: f64,
    facing: u8,
    rng: ChaCha8Rng,
    anims: AnimSet,
    signature: Option<String>,
    scale: f64,
    extent: (f64, f64),
    seq: u64,
    drag_offset: (f64, f64),
    last_drag: (f64, f64),
}

impl Brain {
    pub fn new(seed: u64, anims: AnimSet, signature: Option<String>, scale: u32, extent: (f64, f64), x: f64, ground: &Ground) -> Brain {
        let signature = signature.filter(|s| anims.has(s));
        let mut b = Brain {
            mode: Mode::Idle { remaining: 1.0 },
            x,
            y: ground.y,
            facing: DOWN,
            rng: ChaCha8Rng::seed_from_u64(seed),
            anims,
            signature,
            scale: scale as f64,
            extent,
            seq: 0,
            drag_offset: (0.0, 0.0),
            last_drag: (0.0, 0.0),
        };
        b.clamp_x(ground);
        b
    }

    pub fn set_scale(&mut self, scale: u32) {
        self.scale = scale as f64;
    }

    pub fn is_dragged(&self) -> bool {
        self.mode == Mode::Dragged
    }

    fn x_range(&self, g: &Ground) -> (f64, f64) {
        let k = self.scale * g.px_per_pt;
        let lo = g.left + self.extent.0 * k;
        let hi = g.right - self.extent.1 * k;
        if lo > hi {
            let mid = (g.left + g.right) / 2.0;
            (mid, mid)
        } else {
            (lo, hi)
        }
    }

    fn clamp_x(&mut self, g: &Ground) {
        let (lo, hi) = self.x_range(g);
        self.x = self.x.clamp(lo, hi);
    }

    fn set_mode(&mut self, mode: Mode, facing: u8) {
        self.mode = mode;
        self.facing = facing;
        self.seq += 1;
    }

    fn start_idle(&mut self) {
        let remaining = self.rng.random_range(IDLE_SECS);
        self.set_mode(Mode::Idle { remaining }, DOWN);
    }

    fn start_walk(&mut self) {
        let facing = if self.rng.random_bool(0.5) { LEFT } else { RIGHT };
        let remaining_pts = self.rng.random_range(WALK_PTS);
        self.set_mode(Mode::Walk { remaining_pts }, facing);
    }

    fn start_one_shot(&mut self, anim: String) {
        let remaining = self.anims.secs(&anim);
        self.set_mode(Mode::OneShot { anim, remaining }, DOWN);
    }

    fn pick_flourish(&mut self) -> Option<String> {
        if let Some(sig) = &self.signature {
            return Some(sig.clone());
        }
        let options: Vec<&str> = FLOURISH_PREFERENCE.iter().copied().filter(|n| self.anims.has(n)).collect();
        if options.is_empty() {
            None
        } else {
            Some(options[self.rng.random_range(0..options.len())].to_string())
        }
    }

    fn choose_next(&mut self) {
        let flourish = self.pick_flourish();
        let can_nap = self.anims.has("Sleep");
        let weights = [
            WEIGHT_WALK,
            WEIGHT_IDLE,
            if flourish.is_some() { WEIGHT_FLOURISH } else { 0 },
            if can_nap { WEIGHT_NAP } else { 0 },
        ];
        let mut roll = self.rng.random_range(0..weights.iter().sum::<u32>());
        let mut choice = 1;
        for (i, w) in weights.iter().enumerate() {
            if roll < *w {
                choice = i;
                break;
            }
            roll -= w;
        }
        match choice {
            0 => self.start_walk(),
            2 => self.start_one_shot(flourish.expect("weight is 0 without a flourish")),
            3 => {
                let remaining = self.rng.random_range(NAP_SECS);
                self.set_mode(Mode::Nap { remaining }, DOWN);
            }
            _ => self.start_idle(),
        }
    }

    fn land(&mut self) {
        if self.anims.has("Hop") {
            let remaining = self.anims.secs("Hop");
            self.set_mode(Mode::Landing { remaining }, DOWN);
        } else {
            self.start_idle();
        }
    }

    pub fn tick(&mut self, dt: f64, g: &Ground) {
        match &mut self.mode {
            Mode::Dragged => return,
            Mode::Falling { vy } => {
                *vy += GRAVITY_PTS_PER_SEC2 * g.px_per_pt * dt;
                self.y += *vy * dt;
                self.clamp_x(g);
                if self.y >= g.y {
                    self.y = g.y;
                    self.land();
                }
                return;
            }
            _ => {}
        }
        self.y = g.y;
        self.clamp_x(g);
        let (lo, hi) = self.x_range(g);
        let speed_pts = WALK_PTS_PER_SEC_PER_SCALE * self.scale;
        let done = match &mut self.mode {
            Mode::Walk { remaining_pts } => {
                if lo >= hi {
                    *remaining_pts = 0.0; // no room to walk: pick something else
                }
                let step = speed_pts * dt;
                let sign = if self.facing == LEFT { -1.0 } else { 1.0 };
                self.x = (self.x + sign * step * g.px_per_pt).clamp(lo, hi);
                if self.x <= lo && self.facing == LEFT {
                    self.facing = RIGHT;
                } else if self.x >= hi && self.facing == RIGHT {
                    self.facing = LEFT;
                }
                *remaining_pts -= step;
                *remaining_pts <= 0.0
            }
            Mode::Idle { remaining } | Mode::Nap { remaining } | Mode::OneShot { remaining, .. } | Mode::Landing { remaining } => {
                *remaining -= dt;
                *remaining <= 0.0
            }
            Mode::Dragged | Mode::Falling { .. } => false,
        };
        if done {
            self.choose_next();
        }
    }

    pub fn visual(&self) -> Visual {
        let (anim, looped) = match &self.mode {
            Mode::Walk { .. } | Mode::Dragged | Mode::Falling { .. } => ("Walk".to_string(), true),
            Mode::Idle { .. } => ("Idle".to_string(), true),
            Mode::OneShot { anim, .. } => (anim.clone(), false),
            Mode::Nap { .. } => ("Sleep".to_string(), true),
            Mode::Landing { .. } => ("Hop".to_string(), false),
        };
        Visual { anim, dir: self.facing, looped, seq: self.seq }
    }

    pub fn click(&mut self) {
        if matches!(self.mode, Mode::Dragged | Mode::Falling { .. }) {
            return;
        }
        if let Some(anim) = self.pick_flourish() {
            self.start_one_shot(anim);
        }
    }

    pub fn drag_start(&mut self, cursor: (f64, f64)) {
        self.drag_offset = (self.x - cursor.0, self.y - cursor.1);
        self.last_drag = cursor;
        let facing = self.facing;
        self.set_mode(Mode::Dragged, facing);
    }

    pub fn drag_move(&mut self, cursor: (f64, f64)) {
        if self.mode != Mode::Dragged {
            return;
        }
        self.x = cursor.0 + self.drag_offset.0;
        self.y = cursor.1 + self.drag_offset.1;
        let (dx, dy) = (cursor.0 - self.last_drag.0, cursor.1 - self.last_drag.1);
        if dx.hypot(dy) >= DRAG_TURN_PX {
            self.facing = dir_from_delta(dx, dy);
            self.last_drag = cursor;
        }
    }

    pub fn drop_pet(&mut self) {
        if self.mode == Mode::Dragged {
            self.set_mode(Mode::Falling { vy: 0.0 }, DOWN);
        }
    }

    #[cfg(test)]
    fn force_walk(&mut self, facing: u8, pts: f64) {
        self.set_mode(Mode::Walk { remaining_pts: pts }, facing);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f64 = 1.0 / 60.0;

    fn ground() -> Ground {
        Ground { left: 0.0, right: 1000.0, y: 800.0, px_per_pt: 1.0 }
    }

    fn full_set() -> AnimSet {
        AnimSet::from_secs(&[("Walk", 0.6), ("Idle", 1.2), ("Sleep", 1.1), ("Hop", 0.4), ("Withdraw", 0.73), ("Rotate", 0.3)])
    }

    fn brain(anims: AnimSet, signature: Option<&str>) -> Brain {
        Brain::new(42, anims, signature.map(String::from), 3, (8.0, 8.0), 500.0, &ground())
    }

    #[test]
    fn directions_map_to_pmd_rows() {
        assert_eq!(dir_from_delta(0.0, 1.0), 0); // down
        assert_eq!(dir_from_delta(1.0, 1.0), 1);
        assert_eq!(dir_from_delta(1.0, 0.0), 2); // right
        assert_eq!(dir_from_delta(1.0, -1.0), 3);
        assert_eq!(dir_from_delta(0.0, -1.0), 4); // up
        assert_eq!(dir_from_delta(-1.0, -1.0), 5);
        assert_eq!(dir_from_delta(-1.0, 0.0), 6); // left
        assert_eq!(dir_from_delta(-1.0, 1.0), 7);
    }

    #[test]
    fn behavior_anims_include_signature_once() {
        let a = behavior_anims(Some("Withdraw"));
        assert!(a.contains(&"Walk".to_string()) && a.contains(&"Withdraw".to_string()));
        assert_eq!(a.iter().filter(|n| *n == "Hop").count(), 1);
    }

    #[test]
    fn walking_pet_turns_around_at_the_right_edge() {
        let mut b = brain(full_set(), None);
        b.force_walk(RIGHT, 10_000.0);
        b.x = 970.0; // right extent is 8 px * scale 3 = 24 → max x = 976
        for _ in 0..30 { b.tick(DT, &ground()); }
        assert_eq!(b.visual().dir, LEFT);
        assert!(b.x <= 976.0);
    }

    #[test]
    fn stays_on_moved_ground_and_inside_shrunken_range() {
        let mut b = brain(full_set(), None);
        b.x = 900.0;
        let smaller = Ground { left: 0.0, right: 600.0, y: 700.0, px_per_pt: 1.0 };
        b.tick(DT, &smaller);
        assert_eq!(b.y, 700.0);
        assert!(b.x <= 600.0 - 24.0);
    }

    #[test]
    fn minimal_set_never_panics_and_stays_in_range() {
        let mut b = brain(AnimSet::from_secs(&[("Walk", 0.6), ("Idle", 1.2)]), None);
        let g = ground();
        for _ in 0..20_000 {
            b.tick(DT, &g);
            let v = b.visual();
            assert!(v.anim == "Walk" || v.anim == "Idle", "{}", v.anim);
            assert!(b.x >= 24.0 && b.x <= 976.0 && b.x.is_finite());
            assert_eq!(b.y, 800.0);
        }
        b.click(); // no flourish available: nothing happens
        assert!(b.visual().anim == "Walk" || b.visual().anim == "Idle");
    }

    #[test]
    fn every_behavior_eventually_happens_with_a_full_set() {
        let mut b = brain(full_set(), None);
        let mut seen = std::collections::HashSet::new();
        for _ in 0..200_000 {
            b.tick(DT, &ground());
            seen.insert(b.visual().anim);
        }
        for anim in ["Walk", "Idle", "Sleep", "Hop", "Rotate"] {
            assert!(seen.contains(anim), "{anim} never shown; saw {seen:?}");
        }
    }

    #[test]
    fn click_plays_signature_once_then_moves_on() {
        let mut b = brain(full_set(), Some("Withdraw"));
        b.click();
        let v = b.visual();
        assert_eq!((v.anim.as_str(), v.looped, v.dir), ("Withdraw", false, DOWN));
        for _ in 0..60 { b.tick(DT, &ground()); } // 1 s > 0.73 s
        assert_ne!(b.visual().seq, v.seq);
    }

    #[test]
    fn drag_faces_movement_then_drop_falls_and_lands_with_hop() {
        let mut b = brain(full_set(), None);
        b.drag_start((500.0, 790.0));
        b.drag_move((450.0, 400.0)); // up-left-ish, mostly up
        assert!(b.is_dragged());
        assert_eq!(b.visual().anim, "Walk");
        assert_eq!(b.visual().dir, dir_from_delta(-50.0, -390.0));
        assert_eq!((b.x, b.y), (450.0, 410.0)); // keeps the grab offset (+10 px)
        b.drop_pet();
        let mut landed = false;
        for _ in 0..120 {
            b.tick(DT, &ground());
            if b.visual().anim == "Hop" { landed = true; break; }
        }
        assert!(landed, "should land within 2 s");
        assert_eq!(b.y, 800.0);
    }

    #[test]
    fn drop_without_hop_goes_idle() {
        let mut b = brain(AnimSet::from_secs(&[("Walk", 0.6), ("Idle", 1.2)]), None);
        b.drag_start((500.0, 800.0));
        b.drop_pet();
        for _ in 0..120 { b.tick(DT, &ground()); }
        assert_eq!(b.y, 800.0);
        assert!(!b.is_dragged());
    }

    #[test]
    fn pet_wider_than_ground_is_centered() {
        let narrow = Ground { left: 100.0, right: 120.0, y: 800.0, px_per_pt: 1.0 };
        let mut b = Brain::new(1, full_set(), None, 4, (40.0, 40.0), 0.0, &narrow);
        let mut prev = b.visual();
        for _ in 0..600 {
            b.tick(DT, &narrow);
            assert_eq!(b.x, 110.0);
            let v = b.visual();
            if v.seq == prev.seq {
                assert_eq!(v.dir, prev.dir, "direction must not flip without a mode change");
            }
            prev = v;
        }
    }

    #[test]
    fn same_seed_same_behavior() {
        let mut a = brain(full_set(), None);
        let mut b = brain(full_set(), None);
        for _ in 0..5_000 {
            a.tick(DT, &ground());
            b.tick(DT, &ground());
            assert_eq!(a.visual(), b.visual());
            assert_eq!(a.x, b.x);
        }
    }
}
