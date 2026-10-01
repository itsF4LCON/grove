use anyhow::Result;
use chrono::Utc;
use clap::Parser;
use grove::cache;
use grove::canvas::ColorDepth;
use grove::demo::demo_repos;
use grove::github::{self, FetchOpts};
use grove::model::RepoStats;
use grove::print::render_print;
use grove::source::{Freshness, Loaded, PrintPlan, load_with, print_plan};
use grove::ui::{self, Refresher, app::App, status_for};
use std::process::ExitCode;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

#[derive(Parser)]
#[command(
    name = "grove",
    version,
    about = "Your GitHub repos, grown as a bonsai forest"
)]
struct Cli {
    /// Print one static snapshot and exit (good for a shell greeting)
    #[arg(long)]
    print: bool,
    /// Output width for --print (defaults to the terminal width)
    #[arg(long)]
    width: Option<usize>,
    /// Use built-in sample repos instead of GitHub
    #[arg(long)]
    demo: bool,
    /// Show this user's repos instead of your own
    #[arg(long, conflicts_with = "org")]
    user: Option<String>,
    /// Show an organization's repos
    #[arg(long)]
    org: Option<String>,
    /// Include forked repos
    #[arg(long)]
    include_forks: bool,
    /// Include archived repos (they grow as bare winter trees)
    #[arg(long)]
    include_archived: bool,
    /// Ignore the cache and fetch from GitHub now
    #[arg(long)]
    refresh: bool,
    /// Disable growth animation and falling leaves
    #[arg(long)]
    still: bool,
    /// Internal: refresh the cache silently (spawned by --print when it is stale)
    #[arg(long, hide = true)]
    warm_cache: bool,
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("grove: {e:#}");
            ExitCode::FAILURE
        }
    }
}

/// Loads repos from cache or GitHub.
fn loader(opts: FetchOpts) -> impl Fn(bool) -> Result<Loaded> + Send + Sync + 'static {
    move |refresh| {
        let now = Utc::now();
        let path = cache::default_path();
        load_with(path.as_deref(), &opts.cache_key(), now, refresh, |_| {
            github::fetch(&opts, now, Duration::from_secs(20))
        })
    }
}

/// Repos for `--print`: the cache when there is one (warming it in the background if stale),
/// otherwise a blocking fetch.
fn print_repos(
    opts: &FetchOpts,
    refresh: bool,
    load: &impl Fn(bool) -> Result<Loaded>,
) -> Result<Vec<RepoStats>> {
    let now = Utc::now();
    let cached = cache::default_path().and_then(|p| cache::load(&p, &opts.cache_key()));
    match print_plan(cached.as_ref(), now, refresh) {
        PrintPlan::UseCache { warm } => {
            if warm {
                spawn_warmer(opts);
            }
            Ok(cached.map(|c| c.repos).unwrap_or_default())
        }
        PrintPlan::Fetch => Ok(load(refresh)?.repos),
    }
}

/// Detached `grove --warm-cache` with the same repo filters; failures are silent by design.
fn spawn_warmer(opts: &FetchOpts) {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let mut cmd = Command::new(exe);
    cmd.arg("--warm-cache");
    if let Some(o) = &opts.owner {
        cmd.args(["--user", o]);
    }
    if opts.include_forks {
        cmd.arg("--include-forks");
    }
    if opts.include_archived {
        cmd.arg("--include-archived");
    }
    let _ = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

fn run(cli: Cli) -> Result<()> {
    let now = Utc::now();
    let depth = ColorDepth::detect();
    let opts = FetchOpts {
        owner: cli.user.clone().or(cli.org.clone()),
        include_forks: cli.include_forks,
        include_archived: cli.include_archived,
    };
    let load = loader(opts.clone());

    if cli.warm_cache {
        load(true)?;
        return Ok(());
    }

    if cli.print {
        let repos = if cli.demo {
            demo_repos(now)
        } else {
            print_repos(&opts, cli.refresh, &load)?
        };
        let width = cli
            .width
            .or_else(|| {
                ratatui::crossterm::terminal::size()
                    .ok()
                    .map(|(w, _)| w as usize)
            })
            .unwrap_or(100);
        print!("{}", render_print(&repos, now, width, depth));
        return Ok(());
    }

    let (app, refresher): (App, Refresher) = if cli.demo {
        let mut app = App::new("demo".into(), demo_repos(now), now, 20, cli.still);
        app.status = "demo data".into();
        let r: Refresher = Arc::new(|| {
            let now = Utc::now();
            Ok(Loaded {
                owner: "demo".into(),
                repos: demo_repos(now),
                fetched_at: now,
                freshness: Freshness::Fetched,
            })
        });
        (app, r)
    } else {
        eprintln!("grove: growing your forest…");
        let loaded = load(cli.refresh)?;
        let mut app = App::new(
            loaded.owner.clone(),
            loaded.repos.clone(),
            now,
            20,
            cli.still,
        );
        app.status = status_for(&loaded, now);
        (app, Arc::new(move || load(true)))
    };
    ui::run(app, depth, refresher)
}
