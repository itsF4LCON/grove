//! Synthetic repos covering every visual state, for `--demo` and tests.

use crate::model::{Ci, RepoStats};
use chrono::{DateTime, Duration, Utc};

pub fn demo_repos(now: DateTime<Utc>) -> Vec<RepoStats> {
    let d = |n: i64| now - Duration::days(n);
    let lang = |r: &mut RepoStats, name: &str, color: &str| {
        r.language = Some(name.into());
        r.language_color = Some(color.into());
    };
    let mut out = Vec::new();

    let mut r = RepoStats::sample("old-faithful", now); // big, spring, blossoming
    lang(&mut r, "Rust", "#dea584");
    (r.stars, r.total_commits, r.recent_commits, r.pushed_at, r.created_at, r.ci) = (420, 2400, 80, d(1), d(2200), Ci::Passing);
    out.push(r);

    let mut r = RepoStats::sample("dotfiles", now); // summer
    lang(&mut r, "Shell", "#89e051");
    (r.stars, r.total_commits, r.recent_commits, r.pushed_at, r.created_at) = (3, 310, 25, d(20), d(1400));
    out.push(r);

    let mut r = RepoStats::sample("web-app", now); // failing CI blight
    lang(&mut r, "TypeScript", "#3178c6");
    (r.stars, r.total_commits, r.recent_commits, r.pushed_at, r.created_at, r.ci) = (40, 900, 40, d(5), d(600), Ci::Failing);
    out.push(r);

    let mut r = RepoStats::sample("side-quest", now); // autumn
    lang(&mut r, "Python", "#3572A5");
    (r.total_commits, r.recent_commits, r.pushed_at, r.created_at) = (120, 0, d(100), d(500));
    out.push(r);

    let mut r = RepoStats::sample("weekend-hack", now); // late autumn
    lang(&mut r, "Go", "#00ADD8");
    (r.stars, r.total_commits, r.recent_commits, r.pushed_at, r.created_at) = (1, 45, 0, d(250), d(400));
    out.push(r);

    let mut r = RepoStats::sample("legacy-api", now); // winter by neglect
    lang(&mut r, "Java", "#b07219");
    (r.stars, r.total_commits, r.recent_commits, r.pushed_at, r.created_at) = (12, 1500, 0, d(900), d(3000));
    out.push(r);

    let mut r = RepoStats::sample("archived-thing", now); // winter by archive
    (r.total_commits, r.recent_commits, r.pushed_at, r.created_at, r.archived) = (60, 0, d(400), d(1200), true);
    out.push(r);

    let mut r = RepoStats::sample("fresh-idea", now); // empty → seedling
    (r.total_commits, r.recent_commits, r.pushed_at, r.created_at, r.empty) = (0, 0, d(0), d(0), true);
    out.push(r);

    let mut r = RepoStats::sample("tiny-tool", now); // sapling
    lang(&mut r, "C", "#555555");
    (r.total_commits, r.recent_commits, r.pushed_at, r.created_at) = (6, 6, d(2), d(3));
    out.push(r);

    out
}
