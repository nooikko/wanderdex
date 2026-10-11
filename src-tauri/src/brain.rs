//! One pet's behavior: a walk / pause rhythm with occasional moves and naps, plus drag, fall and land.
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
/// Attack/Swing/Double are left out: they lunge the body around the frame, which looks wrong on a desktop.
pub const FLOURISH_PREFERENCE: [&str; 4] = ["Pose", "Hop", "Rotate", "Charge"];

/// Tuning (spec §5.2). A pet keeps a calm rhythm: walk, pause, walk again. Moves are an occasional treat
/// at the end of a pause, and it naps once it has been awake for a while.
const PAUSE_SECS: std::ops::Range<f64> = 2.0..5.0;
const WALK_PTS: std::ops::Range<f64> = 40.0..240.0;
const WALK_PTS_PER_SEC_PER_SCALE: f64 = 10.0;
/// Seconds after a move before the next one may come on its own (a click doesn't wait).
const MOVE_COOLDOWN_SECS: std::ops::Range<f64> = 20.0..40.0;
/// Chance that a pause whose move cooldown is over ends in a move.
const MOVE_CHANCE: f64 = 0.35;
/// Share of moves that are the signature move, when there are others to choose from.
const SIGNATURE_SHARE: f64 = 0.5;
/// Seconds awake before getting sleepy; the first pause after that ends in a nap.
const AWAKE_SECS: std::ops::Range<f64> = 300.0..480.0;
const NAP_SECS: std::ops::Range<f64> = 30.0..90.0;
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

/// What a sprite can do. Walk and Idle are guaranteed (the index only lists variants with both); every
/// other behavior drops out of the rhythm when its animation is missing.
#[derive(Debug, Clone)]
struct Abilities {
    /// Signature first (if the sprite has it), then the flourishes it has.
    moves: Vec<String>,
    signature: Option<String>,
    nap: bool,
    hop: bool,
}

impl Abilities {
    fn of(anims: &AnimSet, signature: Option<String>) -> Self {
        let signature = signature.filter(|s| anims.has(s));
        let mut moves: Vec<String> = signature.iter().cloned().collect();
        for f in FLOURISH_PREFERENCE {
            if anims.has(f) && !moves.iter().any(|m| m == f) {
                moves.push(f.to_string());
            }
        }
        Abilities { moves, signature, nap: anims.has("Sleep"), hop: anims.has("Hop") }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Mode {
    Walk { remaining_pts: f64 },
    /// Standing and looking around between walks (the Idle animation).
    Pause { remaining: f64 },
    Move { anim: String, remaining: f64 },
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
    can: Abilities,
    scale: f64,
    extent: (f64, f64),
    seq: u64,
    drag_offset: (f64, f64),
    last_drag: (f64, f64),
    /// Seconds this brain has ticked; the timers below are on this clock.
    clock: f64,
    next_move_at: f64,
    last_move: Option<String>,
    bedtime: f64,
}

impl Brain {
    pub fn new(seed: u64, anims: AnimSet, signature: Option<String>, scale: u32, extent: (f64, f64), x: f64, ground: &Ground) -> Brain {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let next_move_at = rng.random_range(MOVE_COOLDOWN_SECS);
        let bedtime = rng.random_range(AWAKE_SECS);
        let mut b = Brain {
            mode: Mode::Pause { remaining: 1.0 },
            x,
            y: ground.y,
            facing: DOWN,
            rng,
            can: Abilities::of(&anims, signature),
            anims,
            scale: scale as f64,
            extent,
            seq: 0,
            drag_offset: (0.0, 0.0),
            last_drag: (0.0, 0.0),
            clock: 0.0,
            next_move_at,
            last_move: None,
            bedtime,
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

    fn start_pause(&mut self) {
        let remaining = self.rng.random_range(PAUSE_SECS);
        self.set_mode(Mode::Pause { remaining }, DOWN);
    }

    fn start_walk(&mut self) {
        let facing = if self.rng.random_bool(0.5) { LEFT } else { RIGHT };
        let remaining_pts = self.rng.random_range(WALK_PTS);
        self.set_mode(Mode::Walk { remaining_pts }, facing);
    }

    /// Plays a move now and restarts the move cooldown.
    fn start_move(&mut self, anim: String) {
        let remaining = self.anims.secs(&anim);
        self.next_move_at = self.clock + remaining + self.rng.random_range(MOVE_COOLDOWN_SECS);
        self.last_move = Some(anim.clone());
        self.set_mode(Mode::Move { anim, remaining }, DOWN);
    }

    /// A move other than the last one (unless it's the only one); the signature gets its share.
    fn pick_move(&mut self) -> Option<String> {
        let fresh: Vec<&String> = self.can.moves.iter().filter(|m| self.can.moves.len() == 1 || Some(*m) != self.last_move.as_ref()).collect();
        if fresh.is_empty() {
            return None;
        }
        if let Some(sig) = self.can.signature.as_ref().filter(|s| fresh.contains(s)) {
            if fresh.len() == 1 || self.rng.random_bool(SIGNATURE_SHARE) {
                return Some(sig.clone());
            }
            let others: Vec<&String> = fresh.into_iter().filter(|m| *m != sig).collect();
            return Some(others[self.rng.random_range(0..others.len())].clone());
        }
        Some(fresh[self.rng.random_range(0..fresh.len())].clone())
    }

    /// The end of a pause: nap if it's past bedtime, maybe a move, otherwise walk on.
    fn after_pause(&mut self, room_to_walk: bool) {
        if self.can.nap && self.clock >= self.bedtime {
            let remaining = self.rng.random_range(NAP_SECS);
            self.set_mode(Mode::Nap { remaining }, DOWN);
        } else if self.clock >= self.next_move_at && self.rng.random_bool(MOVE_CHANCE) {
            match self.pick_move() {
                Some(anim) => self.start_move(anim),
                None => self.walk_or_keep_pausing(room_to_walk),
            }
        } else {
            self.walk_or_keep_pausing(room_to_walk);
        }
    }

    /// Walks on; with no room to walk it pauses instead (an ongoing pause just continues, so it never flickers).
    fn walk_or_keep_pausing(&mut self, room_to_walk: bool) {
        if room_to_walk {
            self.start_walk();
        } else if matches!(self.mode, Mode::Pause { .. }) {
            self.mode = Mode::Pause { remaining: self.rng.random_range(PAUSE_SECS) };
        } else {
            self.start_pause();
        }
    }

    fn land(&mut self) {
        if self.can.hop {
            let remaining = self.anims.secs("Hop");
            self.set_mode(Mode::Landing { remaining }, DOWN);
        } else {
            self.start_pause();
        }
    }

    pub fn tick(&mut self, dt: f64, g: &Ground) {
        self.clock += dt;
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
                    *remaining_pts = 0.0; // no room to walk
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
            Mode::Pause { remaining } | Mode::Nap { remaining } | Mode::Move { remaining, .. } | Mode::Landing { remaining } => {
                *remaining -= dt;
                *remaining <= 0.0
            }
            Mode::Dragged | Mode::Falling { .. } => false,
        };
        if !done {
            return;
        }
        match self.mode {
            Mode::Walk { .. } | Mode::Landing { .. } => self.start_pause(),
            Mode::Pause { .. } => self.after_pause(lo < hi),
            Mode::Move { .. } => self.walk_or_keep_pausing(lo < hi),
            Mode::Nap { .. } => {
                self.bedtime = self.clock + self.rng.random_range(AWAKE_SECS);
                self.start_pause();
            }
            Mode::Dragged | Mode::Falling { .. } => {}
        }
    }

    pub fn visual(&self) -> Visual {
        let (anim, looped) = match &self.mode {
            Mode::Walk { .. } | Mode::Dragged | Mode::Falling { .. } => ("Walk".to_string(), true),
            Mode::Pause { .. } => ("Idle".to_string(), true),
            Mode::Move { anim, .. } => (anim.clone(), false),
            Mode::Nap { .. } => ("Sleep".to_string(), true),
            Mode::Landing { .. } => ("Hop".to_string(), false),
        };
        Visual { anim, dir: self.facing, looped, seq: self.seq }
    }

    /// Plays a move right away (no cooldown wait), unless the pet is being carried or falling.
    pub fn click(&mut self) {
        if matches!(self.mode, Mode::Dragged | Mode::Falling { .. }) {
            return;
        }
        if let Some(anim) = self.pick_move() {
            self.start_move(anim);
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
    const TWO_HOURS: f64 = 2.0 * 3600.0;
    /// Spec §5.2 numbers, restated so the tests check the behavior rather than echo the constants.
    const PAUSE_MAX: f64 = 5.0;
    const MOVE_COOLDOWN_MIN: f64 = 20.0;
    const BEDTIME: std::ops::RangeInclusive<f64> = 300.0..=480.0;
    /// Slack for "at the first pause after bedtime": a walk (240 pts at scale 3 = 8 s) plus a pause.
    const PAUSE_SLACK: f64 = 8.0 + PAUSE_MAX + 0.1;

    fn ground() -> Ground {
        Ground { left: 0.0, right: 1000.0, y: 800.0, px_per_pt: 1.0 }
    }

    fn full_set() -> AnimSet {
        AnimSet::from_secs(&[("Walk", 0.6), ("Idle", 1.2), ("Sleep", 1.1), ("Hop", 0.4), ("Withdraw", 0.73), ("Rotate", 0.3)])
    }

    fn brain(anims: AnimSet, signature: Option<&str>) -> Brain {
        Brain::new(42, anims, signature.map(String::from), 3, (8.0, 8.0), 500.0, &ground())
    }

    /// Ticks for `secs`, logging each new behavior as (start time, animation).
    fn run(b: &mut Brain, g: &Ground, secs: f64) -> Vec<(f64, String)> {
        let (mut log, mut seq, mut t) = (Vec::new(), u64::MAX, 0.0);
        while t < secs {
            b.tick(DT, g);
            t += DT;
            let v = b.visual();
            if v.seq != seq {
                seq = v.seq;
                log.push((t, v.anim));
            }
        }
        log
    }

    fn is_move(anim: &str) -> bool {
        !matches!(anim, "Walk" | "Idle" | "Sleep")
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
    fn flourishes_exclude_lunging_attacks() {
        // Attack/Swing/Double move the body around the frame (Wobbuffet's Swing reaches 21 px below his feet).
        for lunge in ["Attack", "Swing", "Double"] {
            assert!(!behavior_anims(None).iter().any(|n| n == lunge), "{lunge}");
        }
        let mut b = brain(AnimSet::from_secs(&[("Walk", 0.6), ("Idle", 1.2), ("Swing", 0.3)]), None);
        b.click();
        assert_ne!(b.visual().anim, "Swing");
    }

    #[test]
    fn behavior_anims_include_signature_once() {
        let a = behavior_anims(Some("Withdraw"));
        assert!(a.contains(&"Walk".to_string()) && a.contains(&"Withdraw".to_string()));
        assert_eq!(a.iter().filter(|n| *n == "Hop").count(), 1);
    }

    #[test]
    fn rhythm_is_walk_then_pause_never_two_in_a_row() {
        let mut b = brain(full_set(), Some("Withdraw"));
        let log = run(&mut b, &ground(), TWO_HOURS);
        for pair in log.windows(2) {
            let (a, b) = (pair[0].1.as_str(), pair[1].1.as_str());
            match a {
                "Walk" => assert_eq!(b, "Idle", "a walk always ends in a pause"),
                "Idle" => assert!(b != "Idle", "pauses don't stack"),
                "Sleep" => assert_eq!(b, "Idle", "waking up starts with a pause"),
                m => assert_eq!(b, "Walk", "after a move ({m}) the pet walks on"),
            }
        }
    }

    #[test]
    fn pauses_are_short() {
        let mut b = brain(full_set(), Some("Withdraw"));
        let log = run(&mut b, &ground(), TWO_HOURS);
        for pair in log.windows(2) {
            if pair[0].1 == "Idle" {
                let secs = pair[1].0 - pair[0].0;
                assert!(secs <= PAUSE_MAX + DT, "a pause lasted {secs:.2} s at {:.0} s", pair[0].0);
            }
        }
    }

    #[test]
    fn moves_are_spaced_out_and_never_repeat_back_to_back() {
        let mut b = brain(full_set(), Some("Withdraw"));
        let moves: Vec<(f64, String)> = run(&mut b, &ground(), TWO_HOURS).into_iter().filter(|(_, a)| is_move(a)).collect();
        assert!(moves.len() > 40, "moves should still happen regularly: {}", moves.len());
        for pair in moves.windows(2) {
            assert!(pair[1].0 - pair[0].0 >= MOVE_COOLDOWN_MIN, "moves {:.1} s apart", pair[1].0 - pair[0].0);
            assert_ne!(pair[0].1, pair[1].1, "same move twice in a row at {:.0} s", pair[1].0);
        }
    }

    #[test]
    fn signature_gets_about_half_the_moves() {
        let mut b = brain(full_set(), Some("Withdraw"));
        let moves: Vec<String> = run(&mut b, &ground(), TWO_HOURS).into_iter().map(|(_, a)| a).filter(|a| is_move(a)).collect();
        let share = moves.iter().filter(|a| *a == "Withdraw").count() as f64 / moves.len() as f64;
        assert!((0.35..=0.65).contains(&share), "signature share {share:.2}");
    }

    #[test]
    fn naps_come_after_five_to_eight_minutes_awake() {
        let mut b = brain(full_set(), None);
        let log = run(&mut b, &ground(), TWO_HOURS);
        let mut awake_since = 0.0;
        let mut naps = 0;
        for pair in log.windows(2) {
            if pair[1].1 == "Sleep" {
                let awake = pair[1].0 - awake_since;
                assert!(awake >= *BEDTIME.start() && awake <= *BEDTIME.end() + PAUSE_SLACK, "napped after {awake:.0} s awake");
                naps += 1;
            }
            if pair[0].1 == "Sleep" {
                let nap = pair[1].0 - pair[0].0;
                assert!((30.0..=90.0 + DT).contains(&nap), "nap lasted {nap:.0} s");
                awake_since = pair[1].0;
            }
        }
        assert!(naps >= 10, "about one nap every 6-10 minutes over two hours: {naps}");
    }

    #[test]
    fn every_behavior_eventually_happens_with_a_full_set() {
        let mut b = brain(full_set(), None);
        let seen: std::collections::HashSet<String> = run(&mut b, &ground(), TWO_HOURS).into_iter().map(|(_, a)| a).collect();
        for anim in ["Walk", "Idle", "Sleep", "Hop", "Rotate"] {
            assert!(seen.contains(anim), "{anim} never shown; saw {seen:?}");
        }
    }

    // Sprites with fewer animations: every behavior whose animation is missing drops out of the rhythm.

    #[test]
    fn walk_and_idle_only_alternate_and_stay_in_range() {
        let mut b = brain(AnimSet::from_secs(&[("Walk", 0.6), ("Idle", 1.2)]), None);
        let g = ground();
        let mut prev = String::new();
        let mut changes = 0;
        for _ in 0..(TWO_HOURS / DT) as usize {
            b.tick(DT, &g);
            let v = b.visual();
            assert!(v.anim == "Walk" || v.anim == "Idle", "{}", v.anim);
            assert!(b.x >= 24.0 && b.x <= 976.0 && b.x.is_finite());
            assert_eq!(b.y, 800.0);
            if v.anim != prev {
                changes += 1;
                prev = v.anim;
            }
        }
        assert!(changes > 100, "keeps walking and pausing: {changes}");
        let seq = b.visual().seq;
        b.click(); // no move available: nothing happens
        assert_eq!(b.visual().seq, seq);
    }

    #[test]
    fn without_moves_it_still_naps() {
        let mut b = brain(AnimSet::from_secs(&[("Walk", 0.6), ("Idle", 1.2), ("Sleep", 1.1)]), None);
        let log = run(&mut b, &ground(), TWO_HOURS);
        assert!(log.iter().all(|(_, a)| !is_move(a)));
        assert!(log.iter().any(|(_, a)| a == "Sleep"));
    }

    #[test]
    fn a_single_move_may_repeat_but_stays_spaced_out() {
        let mut b = brain(AnimSet::from_secs(&[("Walk", 0.6), ("Idle", 1.2), ("Rotate", 0.3)]), None);
        let moves: Vec<f64> = run(&mut b, &ground(), TWO_HOURS).into_iter().filter(|(_, a)| a == "Rotate").map(|(t, _)| t).collect();
        assert!(moves.len() > 40, "{}", moves.len());
        assert!(moves.windows(2).all(|w| w[1] - w[0] >= MOVE_COOLDOWN_MIN));
    }

    #[test]
    fn no_room_to_walk_means_a_long_pause_not_a_flicker() {
        let narrow = Ground { left: 100.0, right: 120.0, y: 800.0, px_per_pt: 1.0 };
        let mut b = Brain::new(1, AnimSet::from_secs(&[("Walk", 0.6), ("Idle", 1.2)]), None, 4, (40.0, 40.0), 0.0, &narrow);
        for _ in 0..(600.0 / DT) as usize {
            b.tick(DT, &narrow);
            assert_eq!(b.x, 110.0);
            assert_ne!(b.visual().anim, "Walk", "no room: never shows a walk");
        }
    }

    #[test]
    fn click_plays_a_move_at_once_even_during_the_cooldown() {
        let mut b = brain(full_set(), Some("Withdraw"));
        b.click();
        let first = b.visual();
        assert!(is_move(&first.anim) && !first.looped && first.dir == DOWN, "{first:?}");
        b.click();
        let second = b.visual();
        assert!(is_move(&second.anim) && second.seq != first.seq, "clicks skip the cooldown");
        assert_ne!(second.anim, first.anim, "but still don't repeat a move");
    }

    #[test]
    fn a_click_restarts_the_move_cooldown() {
        let mut b = brain(full_set(), Some("Withdraw"));
        let g = ground();
        run(&mut b, &g, 600.0);
        b.click();
        let log = run(&mut b, &g, 600.0);
        let next = log.iter().find(|(t, a)| *t > 1.0 && is_move(a)).map(|(t, _)| *t).unwrap();
        assert!(next >= MOVE_COOLDOWN_MIN, "next move {next:.1} s after the click");
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
    fn drag_faces_movement_then_drop_falls_lands_with_hop_and_pauses() {
        let mut b = brain(full_set(), None);
        b.drag_start((500.0, 790.0));
        b.drag_move((450.0, 400.0)); // up-left-ish, mostly up
        assert!(b.is_dragged());
        assert_eq!(b.visual().anim, "Walk");
        assert_eq!(b.visual().dir, dir_from_delta(-50.0, -390.0));
        assert_eq!((b.x, b.y), (450.0, 410.0)); // keeps the grab offset (+10 px)
        b.drop_pet();
        let log = run(&mut b, &ground(), 3.0);
        let anims: Vec<&str> = log.iter().map(|(_, a)| a.as_str()).collect();
        assert_eq!(&anims[..3], ["Walk", "Hop", "Idle"], "falls, lands with a hop, then pauses");
        assert!(log[1].0 < 2.0, "should land within 2 s");
        assert_eq!(b.y, 800.0);
    }

    #[test]
    fn drop_without_hop_pauses() {
        let mut b = brain(AnimSet::from_secs(&[("Walk", 0.6), ("Idle", 1.2)]), None);
        b.drag_start((500.0, 800.0));
        b.drag_move((500.0, 700.0)); // lift it 100 px
        b.drop_pet();
        let log = run(&mut b, &ground(), 2.0);
        assert_eq!(log.iter().map(|(_, a)| a.as_str()).take(2).collect::<Vec<_>>(), ["Walk", "Idle"]);
        assert_eq!(b.y, 800.0);
        assert!(!b.is_dragged());
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
