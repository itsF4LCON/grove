//! Fetches repo stats from the GitHub GraphQL API.

use crate::model::{Ci, RepoStats};
use anyhow::{Context, Result, bail};
use chrono::{DateTime, Duration, SecondsFormat, Utc};
use serde::Deserialize;
use serde_json::{Value, json};

const ENDPOINT: &str = "https://api.github.com/graphql";
const MAX_PAGES: usize = 20;

pub const QUERY: &str = r#"query($login:String!,$after:String,$isFork:Boolean,$isArchived:Boolean,$since:GitTimestamp!){
  repositoryOwner(login:$login){
    repositories(first:50, after:$after, ownerAffiliations:[OWNER], isFork:$isFork, isArchived:$isArchived, orderBy:{field:PUSHED_AT,direction:DESC}){
      pageInfo{hasNextPage endCursor}
      nodes{
        name nameWithOwner description url stargazerCount forkCount isArchived isFork isEmpty createdAt pushedAt
        primaryLanguage{name color}
        issues(states:OPEN){totalCount}
        pullRequests(states:OPEN){totalCount}
        defaultBranchRef{ target{ ... on Commit {
          history(first:0){totalCount}
          recent: history(first:0, since:$since){totalCount}
          statusCheckRollup{state}
        } } }
      }
    }
  }
}"#;

#[derive(Clone, Debug, PartialEq)]
pub struct FetchOpts {
    pub owner: Option<String>,
    pub include_forks: bool,
    pub include_archived: bool,
}

impl FetchOpts {
    pub fn cache_key(&self) -> String {
        format!(
            "{}|forks={}|archived={}",
            self.owner.as_deref().unwrap_or("@me"),
            self.include_forks,
            self.include_archived
        )
    }
}

pub fn token_from(env: Option<String>, gh: impl FnOnce() -> Option<String>) -> Result<String> {
    if let Some(t) = env.map(|t| t.trim().to_string()).filter(|t| !t.is_empty()) {
        return Ok(t);
    }
    if let Some(t) = gh().map(|t| t.trim().to_string()).filter(|t| !t.is_empty()) {
        return Ok(t);
    }
    bail!("no GitHub token: set GITHUB_TOKEN or run `gh auth login`")
}

pub fn token() -> Result<String> {
    token_from(std::env::var("GITHUB_TOKEN").ok(), || {
        let out = std::process::Command::new("gh")
            .args(["auth", "token"])
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    })
}

pub fn variables(
    opts: &FetchOpts,
    login: &str,
    after: Option<&str>,
    since: DateTime<Utc>,
) -> Value {
    json!({
        "login": login,
        "after": after,
        "isFork": if opts.include_forks { Value::Null } else { json!(false) },
        "isArchived": if opts.include_archived { Value::Null } else { json!(false) },
        "since": since.to_rfc3339_opts(SecondsFormat::Secs, true),
    })
}

#[derive(Deserialize)]
struct Resp<T> {
    data: Option<T>,
    errors: Option<Vec<GqlError>>,
}

#[derive(Deserialize)]
struct GqlError {
    message: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OwnerData {
    repository_owner: Option<Owner>,
}

#[derive(Deserialize)]
struct Owner {
    repositories: Conn,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Conn {
    page_info: PageInfo,
    nodes: Vec<Node>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageInfo {
    has_next_page: bool,
    end_cursor: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Node {
    name: String,
    name_with_owner: String,
    description: Option<String>,
    url: String,
    stargazer_count: u32,
    fork_count: u32,
    is_archived: bool,
    is_fork: bool,
    is_empty: bool,
    created_at: DateTime<Utc>,
    pushed_at: Option<DateTime<Utc>>,
    primary_language: Option<Lang>,
    issues: Count,
    pull_requests: Count,
    default_branch_ref: Option<BranchRef>,
}

#[derive(Deserialize)]
struct Lang {
    name: String,
    color: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Count {
    total_count: u32,
}

#[derive(Deserialize)]
struct BranchRef {
    target: Option<Target>,
}

/// Non-commit targets deserialize as `{}`, leaving every field `None`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Target {
    history: Option<Count>,
    recent: Option<Count>,
    status_check_rollup: Option<Rollup>,
}

#[derive(Deserialize)]
struct Rollup {
    state: String,
}

fn ci_from(state: &str) -> Ci {
    match state {
        "SUCCESS" => Ci::Passing,
        "FAILURE" | "ERROR" => Ci::Failing,
        "PENDING" | "EXPECTED" => Ci::Pending,
        _ => Ci::Unknown,
    }
}

fn check_errors<T>(resp: Resp<T>) -> Result<T> {
    if let Some(errs) = resp.errors.filter(|e| !e.is_empty()) {
        let msgs: Vec<String> = errs.into_iter().map(|e| e.message).collect();
        bail!("GitHub API error: {}", msgs.join("; "));
    }
    resp.data.context("GitHub API returned no data")
}

fn to_stats(n: Node) -> RepoStats {
    let target = n.default_branch_ref.and_then(|b| b.target);
    let (total, recent, ci) = match target {
        Some(t) => (
            t.history.map_or(0, |c| c.total_count),
            t.recent.map_or(0, |c| c.total_count),
            t.status_check_rollup
                .map_or(Ci::Unknown, |r| ci_from(&r.state)),
        ),
        None => (0, 0, Ci::Unknown),
    };
    RepoStats {
        name_with_owner: n.name_with_owner,
        name: n.name,
        description: n.description,
        url: n.url,
        stars: n.stargazer_count,
        forks: n.fork_count,
        archived: n.is_archived,
        is_fork: n.is_fork,
        language: n.primary_language.as_ref().map(|l| l.name.clone()),
        language_color: n.primary_language.and_then(|l| l.color),
        open_issues: n.issues.total_count,
        open_prs: n.pull_requests.total_count,
        total_commits: total,
        recent_commits: recent,
        ci,
        pushed_at: n.pushed_at.unwrap_or(n.created_at),
        created_at: n.created_at,
        empty: n.is_empty,
    }
}

pub fn parse_page(body: &str) -> Result<(Vec<RepoStats>, Option<String>)> {
    let resp: Resp<OwnerData> =
        serde_json::from_str(body).context("unexpected response from GitHub")?;
    let owner = check_errors(resp)?
        .repository_owner
        .context("no GitHub user or organization with that login")?;
    let conn = owner.repositories;
    let next = if conn.page_info.has_next_page {
        conn.page_info.end_cursor
    } else {
        None
    };
    Ok((conn.nodes.into_iter().map(to_stats).collect(), next))
}

fn post(client: &reqwest::blocking::Client, token: &str, body: &Value) -> Result<String> {
    let resp = client
        .post(ENDPOINT)
        .bearer_auth(token)
        .header("User-Agent", "grove")
        .json(body)
        .send()
        .context("could not reach api.github.com")?;
    let status = resp.status();
    let text = resp.text().context("reading GitHub response")?;
    if status.as_u16() == 401 {
        bail!("GitHub rejected the token (401): run `gh auth login` or set a valid GITHUB_TOKEN");
    }
    if !status.is_success() {
        bail!(
            "GitHub API returned {status}: {}",
            text.chars().take(200).collect::<String>()
        );
    }
    Ok(text)
}

#[derive(Deserialize)]
struct ViewerData {
    viewer: Viewer,
}

#[derive(Deserialize)]
struct Viewer {
    login: String,
}

pub fn fetch(
    opts: &FetchOpts,
    now: DateTime<Utc>,
    timeout: std::time::Duration,
) -> Result<(String, Vec<RepoStats>)> {
    let token = token()?;
    let client = reqwest::blocking::Client::builder()
        .timeout(timeout)
        .build()?;
    let login = match &opts.owner {
        Some(l) => l.clone(),
        None => {
            let body = post(&client, &token, &json!({ "query": "{viewer{login}}" }))?;
            let resp: Resp<ViewerData> =
                serde_json::from_str(&body).context("unexpected response from GitHub")?;
            check_errors(resp)?.viewer.login
        }
    };
    let since = now - Duration::days(90);
    let mut repos = Vec::new();
    let mut after: Option<String> = None;
    for _ in 0..MAX_PAGES {
        let body = json!({ "query": QUERY, "variables": variables(opts, &login, after.as_deref(), since) });
        let (mut page, next) = parse_page(&post(&client, &token, &body)?)?;
        repos.append(&mut page);
        match next {
            Some(c) => after = Some(c),
            None => break,
        }
    }
    Ok((login, repos))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    const PAGE: &str = include_str!("../tests/fixtures/page.json");

    #[test]
    fn parses_full_and_null_heavy_nodes() {
        let (repos, next) = parse_page(PAGE).unwrap();
        assert_eq!(next.as_deref(), Some("CURSOR1"));
        assert_eq!(repos.len(), 3);

        let h = &repos[0];
        assert_eq!(h.name_with_owner, "itsF4LCON/hivelab");
        assert_eq!((h.stars, h.forks, h.open_issues, h.open_prs), (7, 1, 2, 1));
        assert_eq!((h.total_commits, h.recent_commits), (16, 16));
        assert_eq!(h.ci, Ci::Failing);
        assert_eq!(h.language_color.as_deref(), Some("#89e051"));

        let f = &repos[1];
        assert_eq!(f.language, None);
        assert_eq!(f.ci, Ci::Unknown);
        assert_eq!(f.description, None);

        let e = &repos[2];
        assert!(e.empty);
        assert_eq!(e.total_commits, 0);
        assert_eq!(
            e.pushed_at, e.created_at,
            "null pushedAt falls back to createdAt"
        );
    }

    #[test]
    fn last_page_has_no_cursor() {
        let body = PAGE.replace("\"hasNextPage\":true", "\"hasNextPage\":false");
        assert_eq!(parse_page(&body).unwrap().1, None);
    }

    #[test]
    fn unknown_owner_is_a_clear_error() {
        let err = parse_page(r#"{"data":{"repositoryOwner":null}}"#).unwrap_err();
        assert!(format!("{err:#}").contains("no GitHub user or organization"));
    }

    #[test]
    fn graphql_errors_are_surfaced() {
        let err =
            parse_page(r#"{"data":null,"errors":[{"message":"Bad credentials"}]}"#).unwrap_err();
        assert!(format!("{err:#}").contains("Bad credentials"));
    }

    #[test]
    fn ci_states_map() {
        for (s, ci) in [
            ("SUCCESS", Ci::Passing),
            ("FAILURE", Ci::Failing),
            ("ERROR", Ci::Failing),
            ("PENDING", Ci::Pending),
            ("EXPECTED", Ci::Pending),
            ("WEIRD", Ci::Unknown),
        ] {
            assert_eq!(ci_from(s), ci);
        }
    }

    #[test]
    fn variables_respect_filters() {
        let since = Utc.with_ymd_and_hms(2026, 7, 3, 0, 0, 0).unwrap();
        let mut o = FetchOpts {
            owner: None,
            include_forks: false,
            include_archived: false,
        };
        let v = variables(&o, "me", None, since);
        assert_eq!(v["isFork"], serde_json::json!(false));
        assert_eq!(v["isArchived"], serde_json::json!(false));
        assert_eq!(v["after"], serde_json::Value::Null);
        assert_eq!(v["since"], "2026-07-03T00:00:00Z");
        o.include_forks = true;
        o.include_archived = true;
        let v = variables(&o, "me", Some("C"), since);
        assert_eq!(v["isFork"], serde_json::Value::Null);
        assert_eq!(v["isArchived"], serde_json::Value::Null);
        assert_eq!(v["after"], "C");
    }

    #[test]
    fn token_resolution_order() {
        assert_eq!(
            token_from(Some(" abc ".into()), || panic!("gh not needed")).unwrap(),
            "abc"
        );
        assert_eq!(
            token_from(Some("  ".into()), || Some("fromgh".into())).unwrap(),
            "fromgh"
        );
        assert_eq!(
            token_from(None, || Some("fromgh\n".into())).unwrap(),
            "fromgh"
        );
        let err = token_from(None, || None).unwrap_err();
        assert!(format!("{err:#}").contains("gh auth login"));
    }

    #[test]
    fn cache_key_distinguishes_options() {
        let a = FetchOpts {
            owner: None,
            include_forks: false,
            include_archived: false,
        };
        let b = FetchOpts {
            owner: Some("ratatui".into()),
            ..a.clone()
        };
        let c = FetchOpts {
            include_forks: true,
            ..a.clone()
        };
        assert_ne!(a.cache_key(), b.cache_key());
        assert_ne!(a.cache_key(), c.cache_key());
    }
}
