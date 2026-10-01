//! Interactive state: selection, scrolling, sorting, growth reveal, falling leaves.

use crate::canvas::Cell;
use crate::forest::{ForestLayout, layout};
use crate::mapping::{Season, params_for};
use crate::model::{Ci, RepoStats};
use crate::print::build_trees;
use crate::tree::grow::{Part, Tree};
use crate::tree::rng::Rng;
use chrono::{DateTime, Utc};

pub const MAX_PARTICLES: usize = 40;
/// Ticks for one tree to finish growing (~1.5 s at 30 fps).
const GROW_TICKS: usize = 45;

pub const LEGEND: &[&str] = &[
    "size          total commits and age",
    "leaf density  commits in the last 90 days",
    "season        days since last push:",
    "              spring ≤14d · summer ≤60d · autumn ≤6mo",
    "              late autumn ≤1y · winter >1y or archived",
    "❀ ✿ blossoms  stars",
    "x  blight     failing CI",
    "●  dot        primary language colour",
    "\\ /  seedling empty repo",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortKey {
    Pushed,
    Activity,
    Stars,
    Name,
}

impl SortKey {
    pub fn next(self) -> SortKey {
        match self {
            SortKey::Pushed => SortKey::Activity,
            SortKey::Activity => SortKey::Stars,
            SortKey::Stars => SortKey::Name,
            SortKey::Name => SortKey::Pushed,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SortKey::Pushed => "recent push",
            SortKey::Activity => "activity",
            SortKey::Stars => "stars",
            SortKey::Name => "name",
        }
    }
}

pub fn sort_repos(repos: &mut [RepoStats], key: SortKey) {
    match key {
        SortKey::Pushed => repos.sort_by_key(|r| std::cmp::Reverse(r.pushed_at)),
        SortKey::Activity => repos.sort_by(|a, b| {
            b.recent_commits
                .cmp(&a.recent_commits)
                .then(b.pushed_at.cmp(&a.pushed_at))
        }),
        SortKey::Stars => repos.sort_by(|a, b| b.stars.cmp(&a.stars).then(a.name.cmp(&b.name))),
        SortKey::Name => repos.sort_by_key(|r| r.name.to_lowercase()),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Overlay {
    None,
    Detail,
    Legend,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Particle {
    pub x: i32,
    pub y: i32,
    pub cell: Cell,
    pub age: u32,
}

pub struct App {
    pub owner: String,
    pub repos: Vec<RepoStats>,
    pub trees: Vec<Tree>,
    pub layout: ForestLayout,
    pub selected: usize,
    pub scroll_x: i32,
    pub sort: SortKey,
    pub revealed: Vec<usize>,
    pub overlay: Overlay,
    pub still: bool,
    pub particles: Vec<Particle>,
    pub status: String,
    pub tree_height: i32,
    pub loading: bool,
    sheds: Vec<bool>,
    ticks: u64,
    rng: Rng,
    now: DateTime<Utc>,
}

impl App {
    pub fn new(
        owner: String,
        repos: Vec<RepoStats>,
        now: DateTime<Utc>,
        tree_height: i32,
        still: bool,
    ) -> App {
        let mut app = App {
            owner,
            repos,
            trees: Vec::new(),
            layout: ForestLayout::default(),
            selected: 0,
            scroll_x: 0,
            sort: SortKey::Pushed,
            revealed: Vec::new(),
            overlay: Overlay::None,
            still,
            particles: Vec::new(),
            status: String::new(),
            tree_height: tree_height.clamp(3, 60),
            loading: false,
            sheds: Vec::new(),
            ticks: 0,
            rng: Rng::new(0x0067_726f_7665),
            now,
        };
        app.rebuild();
        app
    }

    fn rebuild(&mut self) {
        let keep = self.selected_repo().map(|r| r.name_with_owner.clone());
        sort_repos(&mut self.repos, self.sort);
        self.trees = build_trees(&self.repos, self.now, self.tree_height);
        self.layout = layout(&self.trees);
        self.selected = keep
            .and_then(|n| self.repos.iter().position(|r| r.name_with_owner == n))
            .unwrap_or(0);
        self.revealed = if self.still {
            self.trees.iter().map(|t| t.writes.len()).collect()
        } else {
            vec![0; self.trees.len()]
        };
        self.sheds = self
            .repos
            .iter()
            .map(|r| {
                let p = params_for(r, self.now);
                matches!(p.season, Season::Autumn | Season::LateAutumn)
                    || (p.blossoms > 0.0 && p.season != Season::Winter)
            })
            .collect();
        self.particles.clear();
        self.ticks = 0;
    }

    pub fn set_repos(&mut self, owner: String, repos: Vec<RepoStats>, now: DateTime<Utc>) {
        self.owner = owner;
        self.repos = repos;
        self.now = now;
        self.rebuild();
    }

    pub fn set_height(&mut self, h: i32) {
        let h = h.clamp(3, 60);
        if h != self.tree_height {
            self.tree_height = h;
            self.rebuild();
        }
    }

    pub fn selected_repo(&self) -> Option<&RepoStats> {
        self.repos.get(self.selected)
    }

    pub fn select_next(&mut self) {
        if self.selected + 1 < self.repos.len() {
            self.selected += 1;
        }
    }

    pub fn select_prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn cycle_sort(&mut self) {
        self.sort = self.sort.next();
        self.rebuild();
    }

    pub fn toggle(&mut self, o: Overlay) {
        self.overlay = if self.overlay == o { Overlay::None } else { o };
    }

    /// Scrolls just enough to keep the selected trunk away from the edges.
    pub fn follow(&mut self, view_width: i32) {
        let Some(&b) = self.layout.bases.get(self.selected) else {
            return;
        };
        let margin = (view_width / 4).min(20);
        if b - self.scroll_x < margin {
            self.scroll_x = b - margin;
        }
        if b - self.scroll_x > view_width - margin {
            self.scroll_x = b - view_width + margin;
        }
        let max_scroll = (self.layout.total_width - view_width).max(0);
        self.scroll_x = self
            .scroll_x
            .clamp(0, max_scroll.max(b - view_width + margin).max(0));
    }

    pub fn tick(&mut self, view_width: i32) {
        self.ticks += 1;
        if self.still {
            return;
        }
        for (i, t) in self.trees.iter().enumerate() {
            let start = (i as u64 * 4).min(60);
            if self.ticks > start {
                let step = t.writes.len().div_ceil(GROW_TICKS).max(1);
                self.revealed[i] = (self.revealed[i] + step).min(t.writes.len());
            }
        }
        for p in &mut self.particles {
            p.age += 1;
            if p.age % 3 == 0 {
                p.y += 1;
                p.x += self.rng.range(-1, 1);
            }
        }
        self.particles.retain(|p| p.y < 0);
        if self.particles.len() < MAX_PARTICLES && self.rng.chance(0.25) {
            self.spawn_particle(view_width);
        }
    }

    fn spawn_particle(&mut self, view_width: i32) {
        let candidates: Vec<usize> = (0..self.trees.len())
            .filter(|&i| {
                let b = self.layout.bases[i] - self.scroll_x;
                self.sheds[i]
                    && b > -20
                    && b < view_width + 20
                    && self.revealed[i] == self.trees[i].writes.len()
            })
            .collect();
        if candidates.is_empty() {
            return;
        }
        let i = *self.rng.pick(&candidates);
        let leafy: Vec<_> = self.trees[i]
            .writes
            .iter()
            .filter(|w| matches!(w.part, Part::Leaf | Part::Blossom))
            .collect();
        if leafy.is_empty() {
            return;
        }
        let w = **self.rng.pick(&leafy);
        if w.y + 1 < 0 {
            self.particles.push(Particle {
                x: self.layout.bases[i] + w.x,
                y: w.y + 1,
                cell: w.cell,
                age: 0,
            });
        }
    }
}

pub fn ago(d: chrono::Duration) -> String {
    let m = d.num_minutes().max(0);
    match m {
        0 => "just now".into(),
        1..=59 => format!("{m}m ago"),
        60..=1439 => format!("{}h ago", m / 60),
        1440..=43_199 => format!("{}d ago", m / 1440),
        43_200..=525_599 => format!("{}mo ago", m / 43_200),
        _ => format!("{}y ago", m / 525_600),
    }
}

pub fn detail_lines(r: &RepoStats, now: DateTime<Utc>) -> Vec<String> {
    let ci = match r.ci {
        Ci::Passing => "passing",
        Ci::Failing => "failing",
        Ci::Pending => "pending",
        Ci::Unknown => "—",
    };
    let mut lines = vec![
        r.description
            .clone()
            .unwrap_or_else(|| "(no description)".into()),
        String::new(),
        format!("language     {}", r.language.as_deref().unwrap_or("—")),
        format!("stars/forks  ★ {}   forks {}", r.stars, r.forks),
        format!("open         {} PRs · {} issues", r.open_prs, r.open_issues),
        format!("CI           {ci}"),
        format!("last push    {}", ago(now - r.pushed_at)),
        format!(
            "commits      {} in last 90d · {} total",
            r.recent_commits, r.total_commits
        ),
    ];
    if r.archived {
        lines.push("status       archived".into());
    }
    lines.push(String::new());
    lines.push("o open in browser · esc close".into());
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::demo_repos;
    use chrono::{Duration, TimeZone};

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 1, 12, 0, 0).unwrap()
    }

    fn app(still: bool) -> App {
        App::new("demo".into(), demo_repos(now()), now(), 20, still)
    }

    #[test]
    fn selection_clamps() {
        let mut a = app(true);
        a.select_prev();
        assert_eq!(a.selected, 0);
        for _ in 0..100 {
            a.select_next();
        }
        assert_eq!(a.selected, a.repos.len() - 1);
    }

    #[test]
    fn sort_keeps_selected_repo() {
        let mut a = app(true);
        a.select_next();
        a.select_next();
        let name = a.selected_repo().unwrap().name.clone();
        a.cycle_sort();
        assert_eq!(a.sort, SortKey::Activity);
        assert_eq!(a.selected_repo().unwrap().name, name);
    }

    #[test]
    fn name_and_stars_sort() {
        let mut repos = demo_repos(now());
        sort_repos(&mut repos, SortKey::Name);
        let names: Vec<_> = repos.iter().map(|r| r.name.to_lowercase()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
        sort_repos(&mut repos, SortKey::Stars);
        assert_eq!(repos[0].name, "old-faithful");
    }

    #[test]
    fn follow_keeps_selection_in_view() {
        let mut a = app(true);
        for _ in 0..a.repos.len() {
            a.select_next();
            a.follow(60);
            let b = a.layout.bases[a.selected] - a.scroll_x;
            assert!((0..60).contains(&b), "base {b} out of view");
        }
        assert!(a.scroll_x >= 0);
    }

    #[test]
    fn growth_animation_completes() {
        let mut a = app(false);
        assert!(a.revealed.iter().all(|&n| n == 0));
        for _ in 0..200 {
            a.tick(200);
        }
        for (i, t) in a.trees.iter().enumerate() {
            assert_eq!(a.revealed[i], t.writes.len());
        }
    }

    #[test]
    fn still_mode_is_fully_grown_and_calm() {
        let mut a = app(true);
        for (i, t) in a.trees.iter().enumerate() {
            assert_eq!(a.revealed[i], t.writes.len());
        }
        for _ in 0..300 {
            a.tick(200);
        }
        assert!(a.particles.is_empty());
    }

    #[test]
    fn particles_are_bounded_and_fall_to_ground() {
        let mut a = app(false);
        let mut seen = false;
        for _ in 0..2000 {
            a.tick(400);
            assert!(a.particles.len() <= MAX_PARTICLES);
            assert!(a.particles.iter().all(|p| p.y < 0));
            seen |= !a.particles.is_empty();
        }
        assert!(seen, "autumn and blossoming demo trees should shed");
    }

    #[test]
    fn empty_forest_is_safe() {
        let mut a = App::new("nobody".into(), vec![], now(), 20, false);
        a.select_next();
        a.select_prev();
        a.cycle_sort();
        a.follow(80);
        a.tick(80);
        assert!(a.selected_repo().is_none());
    }

    #[test]
    fn set_height_rebuilds_within_bounds() {
        let mut a = app(true);
        a.set_height(1);
        assert_eq!(a.tree_height, 3);
        assert!(a.trees.iter().all(|t| t.height <= 3));
        a.set_height(500);
        assert_eq!(a.tree_height, 60);
    }

    #[test]
    fn overlay_toggles() {
        let mut a = app(true);
        a.toggle(Overlay::Detail);
        assert_eq!(a.overlay, Overlay::Detail);
        a.toggle(Overlay::Legend);
        assert_eq!(a.overlay, Overlay::Legend);
        a.toggle(Overlay::Legend);
        assert_eq!(a.overlay, Overlay::None);
    }

    #[test]
    fn ago_formats() {
        assert_eq!(ago(Duration::seconds(30)), "just now");
        assert_eq!(ago(Duration::minutes(5)), "5m ago");
        assert_eq!(ago(Duration::hours(3)), "3h ago");
        assert_eq!(ago(Duration::days(4)), "4d ago");
        assert_eq!(ago(Duration::days(65)), "2mo ago");
        assert_eq!(ago(Duration::days(800)), "2y ago");
        assert_eq!(ago(Duration::days(-1)), "just now");
    }

    #[test]
    fn detail_lines_cover_the_stats() {
        let mut r = RepoStats::sample("x", now());
        r.ci = Ci::Failing;
        r.open_prs = 2;
        let text = detail_lines(&r, now()).join("\n");
        assert!(text.contains("(no description)"));
        assert!(text.contains("failing"));
        assert!(text.contains("2 PRs"));
        assert!(text.contains("3d ago"));
    }
}
