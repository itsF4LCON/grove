//! Draws the app state into a ratatui frame.

use crate::canvas::{Canvas, ColorDepth};
use crate::forest::{Scene, ground_y, render};
use crate::print::labels;
use crate::ui::app::{App, LEGEND, Overlay, detail_lines};
use chrono::Utc;
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Clear, Paragraph, Widget, Wrap};

struct CanvasWidget<'a>(&'a Canvas, ColorDepth);

impl Widget for CanvasWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let (cv, depth) = (self.0, self.1);
        for y in 0..area.height.min(cv.height() as u16) {
            for x in 0..area.width.min(cv.width() as u16) {
                let Some(c) = cv.get(x as usize, y as usize) else {
                    continue;
                };
                let fg = match depth {
                    ColorDepth::TrueColor => Color::Rgb(c.fg.0, c.fg.1, c.fg.2),
                    ColorDepth::Ansi256 => Color::Indexed(c.fg.to_ansi256()),
                    ColorDepth::None => Color::Reset,
                };
                let mut style = Style::new().fg(fg);
                if c.bold {
                    style = style.add_modifier(Modifier::BOLD);
                }
                buf[(area.x + x, area.y + y)]
                    .set_char(c.ch)
                    .set_style(style);
            }
        }
    }
}

fn popup(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 2,
        width: w,
        height: h,
    }
}

pub fn draw(frame: &mut Frame, app: &App, depth: ColorDepth) {
    let [main, status] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(frame.area());

    if app.repos.is_empty() {
        let msg = Paragraph::new("no repositories to grow").centered();
        frame.render_widget(msg, popup(main, 30, 1));
    } else {
        let mut canvas = Canvas::new(main.width as usize, main.height as usize);
        let names = labels(&app.repos);
        let scene = Scene {
            trees: &app.trees,
            layout: &app.layout,
            labels: &names,
            revealed: Some(&app.revealed),
            selected: Some(app.selected),
        };
        render(&scene, &mut canvas, app.scroll_x);
        let gy = ground_y(&canvas);
        for p in &app.particles {
            canvas.set(p.x - app.scroll_x, gy + p.y, p.cell);
        }
        frame.render_widget(CanvasWidget(&canvas, depth), main);
    }

    let bar = format!(
        " grove · {} · {} repos · sort: {} · {}   ←/→ select  ⏎ details  o open  s sort  r refresh  ? legend  q quit",
        app.owner,
        app.repos.len(),
        app.sort.label(),
        app.status
    );
    frame.render_widget(
        Paragraph::new(bar).style(Style::new().fg(Color::DarkGray)),
        status,
    );

    let overlay = match (app.overlay, app.selected_repo()) {
        (Overlay::Detail, Some(r)) => {
            Some((r.name_with_owner.clone(), detail_lines(r, Utc::now())))
        }
        (Overlay::Legend, _) => Some((
            "legend".to_string(),
            LEGEND.iter().map(|s| s.to_string()).collect(),
        )),
        _ => None,
    };
    if let Some((title, lines)) = overlay {
        let area = popup(main, 64, lines.len() as u16 + 2);
        frame.render_widget(Clear, area);
        let body: Vec<Line> = lines.into_iter().map(Line::from).collect();
        frame.render_widget(
            Paragraph::new(body)
                .wrap(Wrap { trim: false })
                .block(Block::bordered().title(format!(" {title} "))),
            area,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::demo_repos;
    use crate::ui::app::{App, Overlay};
    use chrono::Utc;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn screen(app: &App, w: u16, h: u16) -> String {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| draw(f, app, ColorDepth::TrueColor)).unwrap();
        term.backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect()
    }

    #[test]
    fn draws_forest_and_status() {
        let app = App::new("demo".into(), demo_repos(Utc::now()), Utc::now(), 18, true);
        let s = screen(&app, 100, 24);
        assert!(s.contains("grove"));
        assert!(s.contains("old-faithful") || s.contains("dotfiles"));
    }

    #[test]
    fn overlays_render() {
        let mut app = App::new("demo".into(), demo_repos(Utc::now()), Utc::now(), 18, true);
        app.toggle(Overlay::Detail);
        assert!(screen(&app, 100, 24).contains("last push"));
        app.toggle(Overlay::Legend);
        assert!(screen(&app, 100, 24).contains("blossoms"));
    }

    #[test]
    fn empty_and_tiny_do_not_panic() {
        let empty = App::new("nobody".into(), vec![], Utc::now(), 18, true);
        assert!(screen(&empty, 80, 24).contains("no repositories to grow"));
        let mut app = App::new("demo".into(), demo_repos(Utc::now()), Utc::now(), 3, true);
        app.toggle(Overlay::Detail);
        for (w, h) in [(1, 1), (10, 5), (20, 3)] {
            screen(&app, w, h);
        }
    }
}
