//! The repo data grove works from, independent of where it came from.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ci {
    Unknown,
    Passing,
    Failing,
    Pending,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RepoStats {
    pub name_with_owner: String,
    pub name: String,
    pub description: Option<String>,
    pub url: String,
    pub stars: u32,
    pub forks: u32,
    pub archived: bool,
    pub is_fork: bool,
    pub language: Option<String>,
    pub language_color: Option<String>,
    pub open_issues: u32,
    pub open_prs: u32,
    pub total_commits: u32,
    pub recent_commits: u32,
    pub ci: Ci,
    pub created_at: DateTime<Utc>,
    pub pushed_at: DateTime<Utc>,
    pub empty: bool,
}

impl RepoStats {
    /// A plausible mid-sized repo; callers override the fields they care about.
    pub fn sample(name: &str, now: DateTime<Utc>) -> RepoStats {
        RepoStats {
            name_with_owner: format!("demo/{name}"),
            name: name.to_string(),
            description: None,
            url: format!("https://github.com/demo/{name}"),
            stars: 0,
            forks: 0,
            archived: false,
            is_fork: false,
            language: None,
            language_color: None,
            open_issues: 0,
            open_prs: 0,
            total_commits: 100,
            recent_commits: 10,
            ci: Ci::Unknown,
            created_at: now - Duration::days(365),
            pushed_at: now - Duration::days(3),
            empty: false,
        }
    }
}
