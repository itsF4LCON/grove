//! Procedural bonsai growth.
//!
//! An independent implementation of the idea popularised by cbonsai (no code taken
//! from it): a trunk random-walks upward and throws off alternating side shoots;
//! shoot tips and the crown burst into wandering "dying"/"dead" twigs that carry
//! the foliage. Every write is recorded in order so the TUI can replay growth.

use crate::canvas::{Cell, Rgb};
use crate::mapping::{Season, TreeParams};
use crate::tree::palette::{palette_for, Palette};
use crate::tree::rng::Rng;

pub const MAX_WRITES: usize = 6000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Bark,
    Leaf,
    Blossom,
    Blight,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Write {
    pub x: i32,
    pub y: i32,
    pub cell: Cell,
    pub part: Part,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Tree {
    pub writes: Vec<Write>,
    pub min_x: i32,
    pub max_x: i32,
    pub height: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Trunk,
    ShootLeft,
    ShootRight,
    Dying,
    Dead,
}

pub fn grow(p: &TreeParams, max_height: i32) -> Tree {
    let max_height = max_height.max(3);
    let mut g = Grower {
        p,
        pal: palette_for(p.season, p.tint),
        rng: Rng::new(p.seed),
        top: -max_height,
        writes: Vec::new(),
        next_left: p.seed & 1 == 0,
    };
    if p.seedling {
        g.seedling();
    } else {
        // Keep the trunk from slamming into the ceiling on short canvases.
        let life = p.life.min(max_height as u32 * 5 / 4 + 4);
        g.branch(0, 0, Kind::Trunk, life);
    }
    g.finish()
}

struct Grower<'a> {
    p: &'a TreeParams,
    pal: Palette,
    rng: Rng,
    top: i32,
    writes: Vec<Write>,
    next_left: bool,
}

impl Grower<'_> {
    fn branch(&mut self, mut x: i32, mut y: i32, kind: Kind, total: u32) {
        let mut life = total;
        while life > 0 && self.writes.len() < MAX_WRITES {
            life -= 1;
            let age = total - life;
            let (dx, mut dy) = self.delta(kind, age);
            if y + dy < self.top {
                dy = 0;
            }
            if y + dy > -1 {
                dy = if y > -1 { -1 } else { 0 };
            }
            x += dx;
            y += dy;
            let thick = kind == Kind::Trunk && total >= 24 && age < total / 2;
            self.draw(kind, x, y, dx, dy, thick);
            self.spawn(kind, x, y, life, age, total);
        }
    }

    fn delta(&mut self, kind: Kind, age: u32) -> (i32, i32) {
        let r = &mut self.rng;
        match kind {
            Kind::Trunk => {
                if age <= 2 {
                    return (0, -1);
                }
                let dx = match r.below(4) {
                    0 => -1,
                    3 => 1,
                    _ => 0,
                };
                (dx, if r.chance(0.8) { -1 } else { 0 })
            }
            Kind::ShootLeft | Kind::ShootRight => {
                let dir = if kind == Kind::ShootLeft { -1 } else { 1 };
                let dx = if r.chance(0.8) { dir } else { 0 };
                let dy = match r.below(20) {
                    0..=6 => -1,
                    19 => 1,
                    _ => 0,
                };
                (dx, dy)
            }
            Kind::Dying => {
                let dx = match r.below(10) {
                    0 => -2,
                    1..=3 => -1,
                    6..=8 => 1,
                    9 => 2,
                    _ => 0,
                };
                let dy = match r.below(10) {
                    0..=3 => -1,
                    4..=7 => 0,
                    _ => 1,
                };
                (dx, dy)
            }
            Kind::Dead => (r.range(-1, 1), r.range(-1, 1)),
        }
    }

    fn spawn(&mut self, kind: Kind, x: i32, y: i32, life: u32, age: u32, total: u32) {
        let m = self.p.multiplier;
        match kind {
            Kind::Trunk => {
                if life < m {
                    self.branch(x, y, Kind::Dying, m + 4);
                } else if age > total / 4 && age % m == 0 && self.rng.chance(0.85) {
                    let k = if self.next_left { Kind::ShootLeft } else { Kind::ShootRight };
                    self.next_left = !self.next_left;
                    let shoot_life = (life / 2 + self.rng.below(m)).max(m + 2);
                    self.branch(x, y, k, shoot_life);
                }
            }
            Kind::ShootLeft | Kind::ShootRight => {
                if life < m / 2 + 1 {
                    self.branch(x, y, Kind::Dying, m + 1);
                } else if self.rng.chance(0.12) {
                    self.branch(x, y, Kind::Dying, m / 2 + 2);
                }
            }
            Kind::Dying => {
                if self.rng.chance(0.5) {
                    let l = self.rng.range(2, 5) as u32;
                    self.branch(x, y, Kind::Dead, l);
                }
            }
            Kind::Dead => {}
        }
    }

    fn draw(&mut self, kind: Kind, x: i32, y: i32, dx: i32, dy: i32, thick: bool) {
        if dx == 0 && dy == 0 {
            return;
        }
        match kind {
            Kind::Trunk | Kind::ShootLeft | Kind::ShootRight => {
                let fg = *self.rng.pick(&self.pal.bark);
                let bold = kind == Kind::Trunk;
                if thick {
                    // A thick trunk is drawn as a short stroke centred on the path.
                    let (stroke, from) = match dx.signum() {
                        -1 => ("\\|", 0),
                        1 => ("|/", -1),
                        _ if dy == 0 => ("~_", 0),
                        _ if self.rng.chance(0.5) => ("/|", -1),
                        _ => ("|\\", 0),
                    };
                    for (i, ch) in stroke.chars().enumerate() {
                        self.put(x + from + i as i32, y, ch, fg, bold, Part::Bark);
                    }
                } else {
                    let ch = self.bark_char(dx, dy);
                    self.put(x, y, ch, fg, bold, Part::Bark);
                }
            }
            Kind::Dying | Kind::Dead => self.foliage(x, y, dx, dy),
        }
    }

    fn bark_char(&mut self, dx: i32, dy: i32) -> char {
        match (dx.signum(), dy.signum()) {
            (0, _) => '|',
            (-1, -1) | (1, 1) => '\\',
            (1, -1) | (-1, 1) => '/',
            _ => {
                if self.rng.chance(0.5) {
                    '~'
                } else {
                    '_'
                }
            }
        }
    }

    fn foliage(&mut self, x: i32, y: i32, dx: i32, dy: i32) {
        let density = self.p.leaf_density * self.pal.leaf_factor;
        if self.rng.chance(density) {
            self.leaf(x, y);
            if self.rng.chance(density) {
                let px = x + self.rng.range(-1, 1);
                let py = y + self.rng.range(-1, 0);
                self.leaf(px, py);
            }
        } else if self.pal.leaf_factor < 0.5 && self.rng.chance(0.45) {
            // A bare twig shows through: this is what gives winter its skeleton.
            let ch = self.bark_char(dx, dy);
            let fg = self.pal.twig;
            self.put(x, y, ch, fg, false, Part::Bark);
        }
    }

    fn leaf(&mut self, x: i32, y: i32) {
        let blossoms = if self.p.season == Season::Winter { 0.0 } else { self.p.blossoms };
        if self.rng.chance(blossoms) {
            let i = self.rng.below(2) as usize;
            let (ch, fg) = (self.pal.blossom_chars[i], self.pal.blossom[i]);
            self.put(x, y, ch, fg, false, Part::Blossom);
        } else if self.p.blight && self.rng.chance(0.15) {
            let fg = self.pal.blight;
            self.put(x, y, 'x', fg, true, Part::Blight);
        } else {
            let ch = *self.rng.pick(self.pal.leaf_chars);
            let shade = 0.85 + 0.3 * self.rng.unit();
            let fg = self.rng.pick(&self.pal.leaves).scale(shade);
            let bold = self.rng.chance(0.3);
            self.put(x, y, ch, fg, bold, Part::Leaf);
        }
    }

    fn seedling(&mut self) {
        let stem = Rgb::hex(0x52B788);
        let leaf = Rgb::hex(0x80ED99);
        self.put(0, -1, '|', stem, false, Part::Bark);
        self.put(-1, -2, '\\', leaf, false, Part::Leaf);
        self.put(1, -2, '/', leaf, false, Part::Leaf);
    }

    fn put(&mut self, x: i32, y: i32, ch: char, fg: Rgb, bold: bool, part: Part) {
        if y > -1 || y < self.top || self.writes.len() >= MAX_WRITES {
            return;
        }
        self.writes.push(Write { x, y, cell: Cell { ch, fg, bold }, part });
    }

    fn finish(self) -> Tree {
        let min_x = self.writes.iter().map(|w| w.x).min().unwrap_or(0);
        let max_x = self.writes.iter().map(|w| w.x).max().unwrap_or(0);
        let height = self.writes.iter().map(|w| -w.y).max().unwrap_or(0);
        Tree { writes: self.writes, min_x, max_x, height }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapping::{Season, TreeParams};
    use unicode_width::UnicodeWidthChar;

    fn params(seed: u64, life: u32, season: Season) -> TreeParams {
        TreeParams {
            seed,
            life,
            multiplier: 5,
            leaf_density: 0.9,
            season,
            blossoms: 0.0,
            blight: false,
            tint: None,
            seedling: false,
        }
    }

    fn count(t: &Tree, part: Part) -> usize {
        t.writes.iter().filter(|w| w.part == part).count()
    }

    #[test]
    fn deterministic_per_seed() {
        let p = params(99, 30, Season::Summer);
        assert_eq!(grow(&p, 22), grow(&p, 22));
        assert_ne!(grow(&p, 22), grow(&params(100, 30, Season::Summer), 22));
    }

    #[test]
    fn stays_within_height_and_above_ground() {
        for seed in 0..30 {
            for max_h in [3, 5, 12, 22] {
                let t = grow(&params(seed, 40, Season::Summer), max_h);
                assert!(t.height <= max_h, "seed {seed} max {max_h} height {}", t.height);
                assert!(t.writes.iter().all(|w| w.y <= -1 && w.y >= -max_h));
            }
        }
    }

    #[test]
    fn bigger_life_means_bigger_tree() {
        let small = grow(&params(5, 10, Season::Summer), 30);
        let big = grow(&params(5, 40, Season::Summer), 30);
        assert!(big.writes.len() > small.writes.len());
        assert!(big.height >= small.height);
        assert!(big.max_x - big.min_x > small.max_x - small.min_x);
    }

    #[test]
    fn write_count_is_bounded() {
        let mut p = params(1, 40, Season::Summer);
        p.leaf_density = 1.0;
        p.multiplier = 6;
        assert!(grow(&p, 60).writes.len() <= MAX_WRITES);
    }

    #[test]
    fn seedling_is_tiny() {
        let mut p = params(1, 40, Season::Spring);
        p.seedling = true;
        let t = grow(&p, 22);
        assert_eq!(t.writes.len(), 3);
        assert_eq!(t.height, 2);
    }

    #[test]
    fn blossoms_only_with_stars_and_never_in_winter() {
        let mut p = params(3, 35, Season::Spring);
        assert_eq!(count(&grow(&p, 22), Part::Blossom), 0);
        p.blossoms = 0.3;
        assert!(count(&grow(&p, 22), Part::Blossom) > 0);
        p.season = Season::Winter;
        assert_eq!(count(&grow(&p, 22), Part::Blossom), 0);
    }

    #[test]
    fn blight_only_when_failing() {
        let mut p = params(4, 35, Season::Summer);
        assert_eq!(count(&grow(&p, 22), Part::Blight), 0);
        p.blight = true;
        assert!(count(&grow(&p, 22), Part::Blight) > 0);
    }

    #[test]
    fn winter_is_barer_than_summer() {
        let summer = grow(&params(8, 35, Season::Summer), 22);
        let winter = grow(&params(8, 35, Season::Winter), 22);
        let ratio = |t: &Tree| count(t, Part::Leaf) as f32 / t.writes.len() as f32;
        assert!(ratio(&winter) < ratio(&summer) / 2.0);
    }

    #[test]
    fn every_glyph_is_single_width() {
        for season in [Season::Spring, Season::Summer, Season::Autumn, Season::LateAutumn, Season::Winter] {
            let mut p = params(11, 40, season);
            p.blossoms = 0.3;
            p.blight = true;
            for w in grow(&p, 22).writes {
                assert_eq!(w.cell.ch.width(), Some(1), "{:?}", w.cell.ch);
            }
        }
    }
}
