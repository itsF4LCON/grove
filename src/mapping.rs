//! Turns repo activity into the knobs the tree grower understands.

use crate::canvas::Rgb;
use crate::model::{Ci, RepoStats};
use crate::tree::rng::hash_str;
use chrono::{DateTime, Utc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Season {
    Spring,
    Summer,
    Autumn,
    LateAutumn,
    Winter,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TreeParams {
    pub seed: u64,
    /// Trunk length in growth steps; overall tree size.
    pub life: u32,
    /// Branching rhythm; varies per repo for shape variety.
    pub multiplier: u32,
    /// 0.35..=1.0 chance that a foliage step places a leaf.
    pub leaf_density: f32,
    pub season: Season,
    /// Chance that a leaf is a blossom instead (0 when no stars).
    pub blossoms: f32,
    pub blight: bool,
    pub tint: Option<Rgb>,
    pub seedling: bool,
}

pub fn season_for(days_since_push: i64, archived: bool) -> Season {
    if archived {
        return Season::Winter;
    }
    match days_since_push {
        ..=14 => Season::Spring,
        15..=60 => Season::Summer,
        61..=180 => Season::Autumn,
        181..=365 => Season::LateAutumn,
        _ => Season::Winter,
    }
}

pub fn params_for(repo: &RepoStats, now: DateTime<Utc>) -> TreeParams {
    let seed = hash_str(&repo.name_with_owner);
    let days = (now - repo.pushed_at).num_days();
    let age_years = (now - repo.created_at).num_days().max(0) as f32 / 365.0;
    let life = (4.0 + 5.0 * (repo.total_commits as f32).ln_1p() + 2.0 * age_years)
        .round()
        .clamp(8.0, 40.0) as u32;
    let leaf_density = 0.35 + 0.65 * ((repo.recent_commits as f32).ln_1p() / 61f32.ln()).min(1.0);
    let blossoms = if repo.stars == 0 {
        0.0
    } else {
        (0.05 + 0.05 * (repo.stars as f32).ln_1p()).min(0.35)
    };
    TreeParams {
        seed,
        life,
        multiplier: 4 + (seed % 3) as u32,
        leaf_density,
        season: season_for(days, repo.archived),
        blossoms,
        blight: repo.ci == Ci::Failing,
        tint: repo.language_color.as_deref().and_then(Rgb::from_hex),
        seedling: repo.empty || repo.total_commits == 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Ci, RepoStats};
    use chrono::{Duration, TimeZone, Utc};

    fn now() -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 1, 12, 0, 0).unwrap()
    }

    #[test]
    fn seasons_follow_days_since_push() {
        assert_eq!(season_for(0, false), Season::Spring);
        assert_eq!(season_for(14, false), Season::Spring);
        assert_eq!(season_for(15, false), Season::Summer);
        assert_eq!(season_for(60, false), Season::Summer);
        assert_eq!(season_for(61, false), Season::Autumn);
        assert_eq!(season_for(180, false), Season::Autumn);
        assert_eq!(season_for(181, false), Season::LateAutumn);
        assert_eq!(season_for(365, false), Season::LateAutumn);
        assert_eq!(season_for(366, false), Season::Winter);
        assert_eq!(season_for(0, true), Season::Winter);
    }

    #[test]
    fn seed_is_stable_per_repo_and_differs_between_repos() {
        let a = RepoStats::sample("a", now());
        let b = RepoStats::sample("b", now());
        assert_eq!(
            params_for(&a, now()).seed,
            params_for(&a.clone(), now()).seed
        );
        assert_ne!(params_for(&a, now()).seed, params_for(&b, now()).seed);
    }

    #[test]
    fn life_grows_with_commits_and_is_clamped() {
        let mut r = RepoStats::sample("x", now());
        r.total_commits = 5;
        let small = params_for(&r, now()).life;
        r.total_commits = 2000;
        let big = params_for(&r, now()).life;
        r.total_commits = 1_000_000;
        r.created_at = now() - Duration::days(20 * 365);
        let huge = params_for(&r, now()).life;
        assert!(small < big);
        assert!((8..=40).contains(&small));
        assert_eq!(huge, 40);
    }

    #[test]
    fn leaf_density_tracks_recent_commits() {
        let mut r = RepoStats::sample("x", now());
        r.recent_commits = 0;
        assert!((params_for(&r, now()).leaf_density - 0.35).abs() < 1e-4);
        r.recent_commits = 5000;
        assert!((params_for(&r, now()).leaf_density - 1.0).abs() < 1e-4);
    }

    #[test]
    fn blossoms_from_stars() {
        let mut r = RepoStats::sample("x", now());
        r.stars = 0;
        assert_eq!(params_for(&r, now()).blossoms, 0.0);
        r.stars = 10;
        let ten = params_for(&r, now()).blossoms;
        r.stars = 100_000;
        let lots = params_for(&r, now()).blossoms;
        assert!(ten > 0.0 && ten < lots);
        assert!(lots <= 0.35);
    }

    #[test]
    fn blight_tint_seedling() {
        let mut r = RepoStats::sample("x", now());
        r.ci = Ci::Failing;
        r.language_color = Some("#3178c6".into());
        let p = params_for(&r, now());
        assert!(p.blight);
        assert_eq!(p.tint, Some(Rgb(0x31, 0x78, 0xc6)));
        r.ci = Ci::Passing;
        r.language_color = None;
        assert!(!params_for(&r, now()).blight);
        assert_eq!(params_for(&r, now()).tint, None);
        r.empty = true;
        assert!(params_for(&r, now()).seedling);
    }

    #[test]
    fn pushed_in_future_does_not_panic() {
        let mut r = RepoStats::sample("x", now());
        r.pushed_at = now() + Duration::days(3);
        assert_eq!(params_for(&r, now()).season, Season::Spring);
    }
}
