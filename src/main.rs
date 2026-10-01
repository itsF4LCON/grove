use chrono::Utc;
use clap::Parser;
use grove::canvas::ColorDepth;
use grove::demo::demo_repos;
use grove::print::render_print;

#[derive(Parser)]
#[command(name = "grove", version, about = "Your GitHub repos, grown as a bonsai forest")]
struct Cli {
    /// Print one static snapshot and exit
    #[arg(long)]
    print: bool,
    /// Output width for --print (defaults to the terminal width)
    #[arg(long)]
    width: Option<usize>,
    /// Use built-in sample repos instead of GitHub
    #[arg(long)]
    demo: bool,
}

fn main() {
    let cli = Cli::parse();
    let now = Utc::now();
    if cli.print && cli.demo {
        let width = cli
            .width
            .or_else(|| ratatui::crossterm::terminal::size().ok().map(|(w, _)| w as usize))
            .unwrap_or(100);
        print!("{}", render_print(&demo_repos(now), now, width, ColorDepth::detect()));
    } else {
        eprintln!("grove: only --demo --print is implemented so far");
    }
}
