//! Plants trees side by side on a shared ground line and draws them to a canvas.

use crate::canvas::{Canvas, Cell, Rgb};
use crate::tree::grow::Tree;
use crate::tree::rng::{Rng, hash_str};

pub const LABEL_MAX: usize = 12;
pub const MIN_SPACING: i32 = 16;

const GRASS: [Rgb; 4] = [
    Rgb::hex(0x3A5A40),
    Rgb::hex(0x588157),
    Rgb::hex(0x6A994E),
    Rgb::hex(0x344E41),
];
const SOIL: [Rgb; 3] = [Rgb::hex(0x5C4033), Rgb::hex(0x6F4E37), Rgb::hex(0x4A3728)];
const LABEL: Rgb = Rgb::hex(0xA8B0A0);
const LABEL_SELECTED: Rgb = Rgb::hex(0xFFE066);

#[derive(Clone, Debug, PartialEq, Default)]
pub struct ForestLayout {
    pub bases: Vec<i32>,
    pub total_width: i32,
}

/// Canopies may overlap slightly: a tenth of the gap at which they would just touch.
pub fn layout(trees: &[Tree]) -> ForestLayout {
    let mut bases = Vec::with_capacity(trees.len());
    let mut x = 0;
    for (i, t) in trees.iter().enumerate() {
        if i == 0 {
            x = (-t.min_x).max(MIN_SPACING / 2);
        } else {
            let touching = trees[i - 1].max_x - t.min_x + 1;
            x += (touching * 9 / 10).max(MIN_SPACING);
        }
        bases.push(x);
    }
    let total_width = match (trees.last(), bases.last()) {
        (Some(t), Some(&b)) => b + t.max_x.max(MIN_SPACING / 2) + 1,
        _ => 0,
    };
    ForestLayout { bases, total_width }
}

pub fn label(name: &str) -> String {
    if name.chars().count() <= LABEL_MAX {
        name.to_string()
    } else {
        name.chars()
            .take(LABEL_MAX - 1)
            .chain(std::iter::once('…'))
            .collect()
    }
}

pub struct Scene<'a> {
    pub trees: &'a [Tree],
    pub layout: &'a ForestLayout,
    pub labels: &'a [(String, Option<Rgb>)],
    /// Writes shown per tree (growth animation); `None` shows everything.
    pub revealed: Option<&'a [usize]>,
    pub selected: Option<usize>,
}

pub fn ground_y(canvas: &Canvas) -> i32 {
    canvas.height() as i32 - 3
}

pub fn render(scene: &Scene, canvas: &mut Canvas, scroll_x: i32) {
    if canvas.height() < 3 {
        return;
    }
    let gy = ground_y(canvas);
    draw_ground(canvas, gy, scroll_x);

    // Back to front: tall trees behind, the selected tree always on top.
    let mut order: Vec<usize> = (0..scene.trees.len()).collect();
    order.sort_by_key(|&i| {
        (
            scene.selected == Some(i),
            std::cmp::Reverse(scene.trees[i].height),
        )
    });
    for i in order {
        let t = &scene.trees[i];
        let base = scene.layout.bases[i] - scroll_x;
        if base + t.max_x < 0 || base + t.min_x >= canvas.width() as i32 {
            continue;
        }
        let n = scene
            .revealed
            .map_or(t.writes.len(), |r| r[i].min(t.writes.len()));
        let dim = scene.selected.is_some() && scene.selected != Some(i);
        for w in &t.writes[..n] {
            let mut cell = w.cell;
            if dim {
                cell.fg = cell.fg.scale(0.72);
            }
            canvas.set(base + w.x, gy + w.y, cell);
        }
    }

    for (i, (name, dot)) in scene.labels.iter().enumerate() {
        let text = label(name);
        let len = text.chars().count() as i32 + if dot.is_some() { 2 } else { 0 };
        let mut x = scene.layout.bases[i] - scroll_x - len / 2;
        let selected = scene.selected == Some(i);
        if let Some(c) = dot {
            canvas.set(
                x,
                gy + 2,
                Cell {
                    ch: '●',
                    fg: *c,
                    bold: false,
                },
            );
            x += 2;
        }
        canvas.put_str(
            x,
            gy + 2,
            &text,
            if selected { LABEL_SELECTED } else { LABEL },
            selected,
        );
    }
}

fn draw_ground(canvas: &mut Canvas, gy: i32, scroll_x: i32) {
    for sx in 0..canvas.width() as i32 {
        // Seeded by world x so the ground scrolls with the trees instead of shimmering.
        let wx = (sx + scroll_x) as i64 as u64;
        let mut r = Rng::new(hash_str("grove-ground") ^ wx.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let grass = *r.pick(&['"', '\'', ',', '.', '`', '"', ',']);
        canvas.set(
            sx,
            gy,
            Cell {
                ch: grass,
                fg: *r.pick(&GRASS),
                bold: false,
            },
        );
        let soil = *r.pick(&['~', '.', ':', '-', '~', '.']);
        canvas.set(
            sx,
            gy + 1,
            Cell {
                ch: soil,
                fg: *r.pick(&SOIL),
                bold: false,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapping::{Season, TreeParams};
    use crate::tree::grow::grow;

    fn tree(seed: u64, life: u32) -> Tree {
        grow(
            &TreeParams {
                seed,
                life,
                multiplier: 5,
                leaf_density: 0.8,
                season: Season::Summer,
                blossoms: 0.0,
                blight: false,
                tint: None,
                seedling: false,
            },
            18,
        )
    }

    #[test]
    fn bases_increase_with_minimum_spacing() {
        let trees: Vec<Tree> = (0..6).map(|i| tree(i, 10 + i as u32 * 5)).collect();
        let l = layout(&trees);
        assert_eq!(l.bases.len(), 6);
        for w in l.bases.windows(2) {
            assert!(w[1] - w[0] >= MIN_SPACING);
        }
        assert!(l.total_width > *l.bases.last().unwrap());
    }

    #[test]
    fn empty_layout() {
        let l = layout(&[]);
        assert!(l.bases.is_empty());
        assert_eq!(l.total_width, 0);
    }

    #[test]
    fn labels_truncate_by_chars() {
        assert_eq!(label("grove"), "grove");
        let long = label("a-really-long-repository-name");
        assert_eq!(long.chars().count(), LABEL_MAX);
        assert!(long.ends_with('…'));
        let uni = label("ñandú-çafé-über-straße");
        assert_eq!(uni.chars().count(), LABEL_MAX);
    }

    #[test]
    fn render_places_labels_on_last_row() {
        let trees = vec![tree(1, 20), tree(2, 20)];
        let l = layout(&trees);
        let labels = vec![
            ("alpha".to_string(), None),
            ("beta".to_string(), Some(Rgb(1, 2, 3))),
        ];
        let mut cv = Canvas::new(60, 24);
        render(
            &Scene {
                trees: &trees,
                layout: &l,
                labels: &labels,
                revealed: None,
                selected: Some(1),
            },
            &mut cv,
            0,
        );
        let plain = cv.to_plain();
        let last = plain.lines().last().unwrap();
        assert!(last.contains("alpha") && last.contains("beta"));
        assert!(last.contains('●'));
    }

    #[test]
    fn revealed_zero_draws_no_tree() {
        let trees = vec![tree(1, 20)];
        let l = layout(&trees);
        let labels = vec![("a".to_string(), None)];
        let mut cv = Canvas::new(40, 24);
        render(
            &Scene {
                trees: &trees,
                layout: &l,
                labels: &labels,
                revealed: Some(&[0usize][..]),
                selected: None,
            },
            &mut cv,
            0,
        );
        let plain = cv.to_plain();
        let above_ground: String = plain.lines().take(24 - 3).collect();
        assert!(above_ground.trim().is_empty());
    }

    #[test]
    fn tiny_canvas_does_not_panic() {
        let trees = vec![tree(1, 30)];
        let l = layout(&trees);
        let labels = vec![("a".to_string(), None)];
        for (w, h) in [(0, 0), (1, 1), (3, 2), (10, 5)] {
            let mut cv = Canvas::new(w, h);
            render(
                &Scene {
                    trees: &trees,
                    layout: &l,
                    labels: &labels,
                    revealed: None,
                    selected: Some(0),
                },
                &mut cv,
                5,
            );
        }
    }
}
