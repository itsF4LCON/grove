//! JSON cache of the last successful fetch.

use crate::model::RepoStats;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const TTL_MINUTES: i64 = 15;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CacheFile {
    pub key: String,
    pub owner: String,
    pub fetched_at: DateTime<Utc>,
    pub repos: Vec<RepoStats>,
}

pub fn default_path() -> Option<PathBuf> {
    dirs::cache_dir().map(|d| d.join("grove").join("repos.json"))
}

/// Missing, unreadable, corrupt, or for a different query: all treated as no cache.
pub fn load(path: &Path, key: &str) -> Option<CacheFile> {
    let text = std::fs::read_to_string(path).ok()?;
    let file: CacheFile = serde_json::from_str(&text).ok()?;
    (file.key == key).then_some(file)
}

pub fn save(path: &Path, file: &CacheFile) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec(file)?)?;
    std::fs::rename(tmp, path)
}

pub fn is_fresh(file: &CacheFile, now: DateTime<Utc>) -> bool {
    now - file.fetched_at < Duration::minutes(TTL_MINUTES)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::RepoStats;
    use chrono::{Duration, TimeZone};

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 1, 12, 0, 0).unwrap()
    }

    fn file(key: &str, at: DateTime<Utc>) -> CacheFile {
        CacheFile { key: key.into(), owner: "me".into(), fetched_at: at, repos: vec![RepoStats::sample("a", now())] }
    }

    #[test]
    fn round_trip_and_key_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/repos.json");
        let f = file("k1", now());
        save(&path, &f).unwrap();
        assert_eq!(load(&path, "k1"), Some(f));
        assert_eq!(load(&path, "k2"), None);
    }

    #[test]
    fn missing_or_corrupt_is_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("repos.json");
        assert_eq!(load(&path, "k"), None);
        std::fs::write(&path, "{not json").unwrap();
        assert_eq!(load(&path, "k"), None);
    }

    #[test]
    fn freshness_window() {
        assert!(is_fresh(&file("k", now() - Duration::minutes(14)), now()));
        assert!(!is_fresh(&file("k", now() - Duration::minutes(16)), now()));
    }
}
