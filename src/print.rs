//! One-shot rendering of the forest to a string (for `--print` and shell greetings).

use crate::canvas::{Canvas, ColorDepth, Rgb};
use crate::forest::{layout, render, Scene, LABEL_MAX};
use crate::mapping::params_for;
use crate::model::RepoStats;
use crate::tree::grow::{grow, Tree};
use chrono::{DateTime, Utc};

pub const PRINT_TREE_HEIGHT: i32 = 22;

pub fn build_trees(repos: &[RepoStats], now: DateTime<Utc>, max_height: i32) -> Vec<Tree> {
    repos.iter().map(|r| grow(&params_for(r, now), max_height)).collect()
}

pub fn labels(repos: &[RepoStats]) -> Vec<(String, Option<Rgb>)> {
    repos
        .iter()
        .map(|r| (r.name.clone(), r.language_color.as_deref().and_then(Rgb::from_hex)))
        .collect()
}

pub fn render_print(repos: &[RepoStats], now: DateTime<Utc>, width: usize, depth: ColorDepth) -> String {
    if repos.is_empty() {
        return "grove: no repositories to grow\n".to_string();
    }
    let all = build_trees(repos, now, PRINT_TREE_HEIGHT);
    let full = layout(&all);
    // Keep trees whose trunk and label fit; always keep at least one.
    let fit = full
        .bases
        .iter()
        .take_while(|&&b| b + LABEL_MAX as i32 / 2 + 2 <= width as i32)
        .count()
        .max(1);
    let trees = &all[..fit];
    let lay = layout(trees);
    let names = labels(&repos[..fit]);
    let tallest = trees.iter().map(|t| t.height).max().unwrap_or(0);
    let mut canvas = Canvas::new(width, tallest as usize + 3);
    render(&Scene { trees, layout: &lay, labels: &names, revealed: None, selected: None }, &mut canvas, 0);
    canvas.to_ansi(depth)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::demo_repos;
    use chrono::{TimeZone, Utc};

    fn now() -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 1, 12, 0, 0).unwrap()
    }

    #[test]
    fn empty_repo_list_is_friendly() {
        assert_eq!(render_print(&[], now(), 80, ColorDepth::None), "grove: no repositories to grow\n");
    }

    #[test]
    fn no_line_exceeds_width() {
        let repos = demo_repos(now());
        for width in [1, 8, 40, 120, 300] {
            let out = render_print(&repos, now(), width, ColorDepth::None);
            for line in out.lines() {
                assert!(line.chars().count() <= width, "width {width}: {line:?}");
            }
        }
    }

    #[test]
    fn deterministic_and_shows_names() {
        let repos = demo_repos(now());
        let a = render_print(&repos, now(), 200, ColorDepth::None);
        assert_eq!(a, render_print(&repos, now(), 200, ColorDepth::None));
        assert!(a.contains("old-faithful"));
    }

    #[test]
    fn approved_look_snapshot() {
        let out = render_print(&demo_repos(now()), now(), 160, ColorDepth::None);
        insta::assert_snapshot!(out);
    }

    #[test]
    fn color_depth_none_has_no_escapes() {
        let out = render_print(&demo_repos(now()), now(), 120, ColorDepth::None);
        assert!(!out.contains('\x1b'));
        let colored = render_print(&demo_repos(now()), now(), 120, ColorDepth::TrueColor);
        assert!(colored.contains("\x1b[38;2;"));
    }
}
