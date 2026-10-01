pub mod app;
pub mod view;

use crate::canvas::ColorDepth;
use crate::source::{Freshness, Loaded};
use anyhow::Result;
use app::{App, Overlay, ago};
use chrono::{DateTime, Utc};
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use std::process::{Command, Stdio};
use std::sync::{Arc, mpsc};
use std::time::Duration;

pub type Refresher = Arc<dyn Fn() -> Result<Loaded> + Send + Sync>;

pub fn status_for(l: &Loaded, now: DateTime<Utc>) -> String {
    match &l.freshness {
        Freshness::Fresh => format!("cached {}", ago(now - l.fetched_at)),
        Freshness::Fetched => "updated just now".into(),
        Freshness::Stale(_) => format!("offline · showing cache from {}", ago(now - l.fetched_at)),
    }
}

pub fn run(app: App, depth: ColorDepth, refresh: Refresher) -> Result<()> {
    let mut terminal = ratatui::init();
    let result = event_loop(&mut terminal, app, depth, refresh);
    ratatui::restore();
    result
}

fn event_loop(
    terminal: &mut DefaultTerminal,
    mut app: App,
    depth: ColorDepth,
    refresh: Refresher,
) -> Result<()> {
    let (tx, rx) = mpsc::channel::<Result<Loaded>>();
    loop {
        let size = terminal.size()?;
        // 1 status row + grass, soil, label rows.
        app.set_height(size.height as i32 - 4);
        app.follow(size.width as i32);
        terminal.draw(|f| view::draw(f, &app, depth))?;

        if event::poll(Duration::from_millis(33))?
            && let Event::Key(k) = event::read()?
        {
            if k.kind != KeyEventKind::Press {
                continue;
            }
            match k.code {
                KeyCode::Char('q') => return Ok(()),
                KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => return Ok(()),
                KeyCode::Esc if app.overlay != Overlay::None => app.overlay = Overlay::None,
                KeyCode::Esc => return Ok(()),
                KeyCode::Left | KeyCode::Char('h') => app.select_prev(),
                KeyCode::Right | KeyCode::Char('l') => app.select_next(),
                KeyCode::Enter => app.toggle(Overlay::Detail),
                KeyCode::Char('?') => app.toggle(Overlay::Legend),
                KeyCode::Char('s') => app.cycle_sort(),
                KeyCode::Char('o') => {
                    if let Some(r) = app.selected_repo() {
                        let url = r.url.clone();
                        let spawned = Command::new("xdg-open")
                            .arg(&url)
                            .stdin(Stdio::null())
                            .stdout(Stdio::null())
                            .stderr(Stdio::null())
                            .spawn();
                        app.status = match spawned {
                            Ok(_) => format!("opened {url}"),
                            Err(e) => format!("couldn't run xdg-open: {e}"),
                        };
                    }
                }
                KeyCode::Char('r') if !app.loading => {
                    app.loading = true;
                    app.status = "refreshing…".into();
                    let (tx, f) = (tx.clone(), refresh.clone());
                    std::thread::spawn(move || {
                        let _ = tx.send(f());
                    });
                }
                _ => {}
            }
        }

        if let Ok(res) = rx.try_recv() {
            app.loading = false;
            let now = Utc::now();
            match res {
                Ok(l) => {
                    app.status = status_for(&l, now);
                    app.set_repos(l.owner, l.repos, now);
                }
                Err(e) => app.status = format!("refresh failed: {e:#}"),
            }
        }
        app.tick(size.width as i32);
    }
}
