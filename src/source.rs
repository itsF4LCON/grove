//! Decides between cache and network.

use crate::cache::{self, CacheFile};
use crate::model::RepoStats;
use anyhow::Result;
use chrono::{DateTime, Utc};
use std::path::Path;

#[derive(Clone, Debug, PartialEq)]
pub enum Freshness {
    Fresh,
    Fetched,
    /// Network failed; carrying the reason.
    Stale(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Loaded {
    pub owner: String,
    pub repos: Vec<RepoStats>,
    pub fetched_at: DateTime<Utc>,
    pub freshness: Freshness,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrintPlan {
    /// Print the cache now; `warm` asks for a background refresh for next time.
    UseCache {
        warm: bool,
    },
    Fetch,
}

/// `--print` must be instant in a shell greeting, so it never waits on the network
/// when any cache exists; a stale cache is refreshed in the background instead.
pub fn print_plan(cached: Option<&CacheFile>, now: DateTime<Utc>, refresh: bool) -> PrintPlan {
    match cached {
        Some(c) if !refresh => PrintPlan::UseCache {
            warm: !cache::is_fresh(c, now),
        },
        _ => PrintPlan::Fetch,
    }
}

pub fn load_with(
    cache_path: Option<&Path>,
    key: &str,
    now: DateTime<Utc>,
    refresh: bool,
    fetch: impl FnOnce(bool) -> Result<(String, Vec<RepoStats>)>,
) -> Result<Loaded> {
    let cached = cache_path.and_then(|p| cache::load(p, key));
    if let Some(c) = cached
        .as_ref()
        .filter(|c| !refresh && cache::is_fresh(c, now))
    {
        return Ok(Loaded {
            owner: c.owner.clone(),
            repos: c.repos.clone(),
            fetched_at: c.fetched_at,
            freshness: Freshness::Fresh,
        });
    }
    match fetch(cached.is_some()) {
        Ok((owner, repos)) => {
            if let Some(p) = cache_path {
                // A cache write failure shouldn't stop the user seeing their forest.
                let _ = cache::save(
                    p,
                    &CacheFile {
                        key: key.into(),
                        owner: owner.clone(),
                        fetched_at: now,
                        repos: repos.clone(),
                    },
                );
            }
            Ok(Loaded {
                owner,
                repos,
                fetched_at: now,
                freshness: Freshness::Fetched,
            })
        }
        Err(e) => match cached {
            Some(c) => Ok(Loaded {
                owner: c.owner,
                repos: c.repos,
                fetched_at: c.fetched_at,
                freshness: Freshness::Stale(format!("{e:#}")),
            }),
            None => Err(e),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::{CacheFile, save};
    use crate::model::RepoStats;
    use anyhow::anyhow;
    use chrono::{Duration, TimeZone};

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 1, 12, 0, 0).unwrap()
    }

    fn seed_cache(path: &Path, age_min: i64) {
        save(
            path,
            &CacheFile {
                key: "k".into(),
                owner: "cached-owner".into(),
                fetched_at: now() - Duration::minutes(age_min),
                repos: vec![RepoStats::sample("cached", now())],
            },
        )
        .unwrap();
    }

    #[test]
    fn print_plan_prefers_cache_and_warms_when_stale() {
        let c = |age| CacheFile {
            key: "k".into(),
            owner: "o".into(),
            fetched_at: now() - Duration::minutes(age),
            repos: vec![],
        };
        assert_eq!(
            print_plan(Some(&c(1)), now(), false),
            PrintPlan::UseCache { warm: false }
        );
        assert_eq!(
            print_plan(Some(&c(600)), now(), false),
            PrintPlan::UseCache { warm: true }
        );
        assert_eq!(print_plan(Some(&c(1)), now(), true), PrintPlan::Fetch);
        assert_eq!(print_plan(None, now(), false), PrintPlan::Fetch);
    }

    #[test]
    fn fresh_cache_skips_fetch() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("c.json");
        seed_cache(&p, 1);
        let l = load_with(Some(&p), "k", now(), false, |_| panic!("should not fetch")).unwrap();
        assert_eq!(l.freshness, Freshness::Fresh);
        assert_eq!(l.repos[0].name, "cached");
    }

    #[test]
    fn refresh_forces_fetch_and_saves() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("c.json");
        seed_cache(&p, 1);
        let l = load_with(Some(&p), "k", now(), true, |have| {
            assert!(have);
            Ok(("me".into(), vec![RepoStats::sample("new", now())]))
        })
        .unwrap();
        assert_eq!(l.freshness, Freshness::Fetched);
        let again = load_with(Some(&p), "k", now(), false, |_| panic!("cached")).unwrap();
        assert_eq!(again.repos[0].name, "new");
    }

    #[test]
    fn stale_cache_used_when_fetch_fails() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("c.json");
        seed_cache(&p, 600);
        let l = load_with(Some(&p), "k", now(), false, |have| {
            assert!(have);
            Err(anyhow!("offline"))
        })
        .unwrap();
        assert!(matches!(l.freshness, Freshness::Stale(ref m) if m.contains("offline")));
        assert_eq!(l.owner, "cached-owner");
    }

    #[test]
    fn no_cache_and_fetch_fails_is_error() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("c.json");
        let err = load_with(Some(&p), "k", now(), false, |have| {
            assert!(!have);
            Err(anyhow!("no token"))
        })
        .unwrap_err();
        assert!(format!("{err:#}").contains("no token"));
    }

    #[test]
    fn works_without_a_cache_dir() {
        let l = load_with(None, "k", now(), false, |_| Ok(("me".into(), vec![]))).unwrap();
        assert!(l.repos.is_empty());
    }
}
