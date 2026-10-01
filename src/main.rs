use anyhow::Result;
use chrono::Utc;
use clap::Parser;
use grove::cache;
use grove::canvas::ColorDepth;
use grove::demo::demo_repos;
use grove::github::{self, FetchOpts};
use grove::print::render_print;
use grove::source::{load_with, Loaded};
use std::process::ExitCode;
use std::time::Duration;

#[derive(Parser)]
#[command(name = "grove", version, about = "Your GitHub repos, grown as a bonsai forest")]
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

/// Loads repos from cache or GitHub. Short network timeout when a cache can cover for us.
fn loader(opts: FetchOpts, print: bool) -> impl Fn(bool) -> Result<Loaded> + Send + Sync + 'static {
    move |refresh| {
        let now = Utc::now();
        let path = cache::default_path();
        load_with(path.as_deref(), &opts.cache_key(), now, refresh, |have_cache| {
            let secs = if print && have_cache { 3 } else { 20 };
            github::fetch(&opts, now, Duration::from_secs(secs))
        })
    }
}

fn run(cli: Cli) -> Result<()> {
    let now = Utc::now();
    let depth = ColorDepth::detect();
    let opts = FetchOpts {
        owner: cli.user.clone().or(cli.org.clone()),
        include_forks: cli.include_forks,
        include_archived: cli.include_archived,
    };
    let load = loader(opts, cli.print);

    if cli.print {
        let repos = if cli.demo { demo_repos(now) } else { load(cli.refresh)?.repos };
        let width = cli
            .width
            .or_else(|| ratatui::crossterm::terminal::size().ok().map(|(w, _)| w as usize))
            .unwrap_or(100);
        print!("{}", render_print(&repos, now, width, depth));
        return Ok(());
    }

    anyhow::bail!("interactive mode is not implemented yet; use --print")
}
