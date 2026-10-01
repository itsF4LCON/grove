//! A grid of coloured character cells, rendered to plain text or ANSI escapes.

use serde::{Deserialize, Serialize};
use std::fmt::Write as _;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub const fn hex(v: u32) -> Rgb {
        Rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
    }

    pub fn from_hex(s: &str) -> Option<Rgb> {
        let s = s.strip_prefix('#').unwrap_or(s);
        if s.len() != 6 || !s.is_ascii() {
            return None;
        }
        let p = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).ok();
        Some(Rgb(p(0)?, p(2)?, p(4)?))
    }

    pub fn mix(self, other: Rgb, t: f32) -> Rgb {
        let t = t.clamp(0.0, 1.0);
        let m = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Rgb(m(self.0, other.0), m(self.1, other.1), m(self.2, other.2))
    }

    pub fn scale(self, k: f32) -> Rgb {
        let s = |a: u8| (a as f32 * k).round().clamp(0.0, 255.0) as u8;
        Rgb(s(self.0), s(self.1), s(self.2))
    }

    /// Nearest colour in the xterm 6×6×6 cube.
    pub fn to_ansi256(self) -> u8 {
        let c = |v: u8| ((v as u16 * 5 + 127) / 255) as u8;
        16 + 36 * c(self.0) + 6 * c(self.1) + c(self.2)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorDepth {
    TrueColor,
    Ansi256,
    None,
}

impl ColorDepth {
    pub fn detect() -> ColorDepth {
        if std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()) {
            return ColorDepth::None;
        }
        match std::env::var("COLORTERM").as_deref() {
            Ok("truecolor") | Ok("24bit") => ColorDepth::TrueColor,
            _ => ColorDepth::Ansi256,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub fg: Rgb,
    pub bold: bool,
}

pub struct Canvas {
    width: usize,
    height: usize,
    cells: Vec<Option<Cell>>,
}

impl Canvas {
    pub fn new(width: usize, height: usize) -> Canvas {
        Canvas { width, height, cells: vec![None; width * height] }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    /// Writes a cell; coordinates outside the canvas are silently ignored.
    pub fn set(&mut self, x: i32, y: i32, cell: Cell) {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return;
        }
        self.cells[y as usize * self.width + x as usize] = Some(cell);
    }

    pub fn get(&self, x: usize, y: usize) -> Option<Cell> {
        if x >= self.width || y >= self.height {
            return None;
        }
        self.cells[y * self.width + x]
    }

    pub fn put_str(&mut self, x: i32, y: i32, s: &str, fg: Rgb, bold: bool) {
        for (i, ch) in s.chars().enumerate() {
            self.set(x + i as i32, y, Cell { ch, fg, bold });
        }
    }

    pub fn to_plain(&self) -> String {
        let mut out = String::new();
        for y in 0..self.height {
            let line: String = (0..self.width).map(|x| self.get(x, y).map_or(' ', |c| c.ch)).collect();
            out.push_str(line.trim_end());
            out.push('\n');
        }
        out
    }

    pub fn to_ansi(&self, depth: ColorDepth) -> String {
        if depth == ColorDepth::None {
            return self.to_plain();
        }
        let mut out = String::new();
        for y in 0..self.height {
            let last = (0..self.width).rev().find(|&x| self.get(x, y).is_some_and(|c| c.ch != ' '));
            if let Some(last) = last {
                let mut cur: Option<(Rgb, bool)> = None;
                for x in 0..=last {
                    match self.get(x, y) {
                        Some(c) => {
                            if cur != Some((c.fg, c.bold)) {
                                out.push_str("\x1b[0m");
                                if c.bold {
                                    out.push_str("\x1b[1m");
                                }
                                match depth {
                                    ColorDepth::TrueColor => {
                                        let _ = write!(out, "\x1b[38;2;{};{};{}m", c.fg.0, c.fg.1, c.fg.2);
                                    }
                                    _ => {
                                        let _ = write!(out, "\x1b[38;5;{}m", c.fg.to_ansi256());
                                    }
                                }
                                cur = Some((c.fg, c.bold));
                            }
                            out.push(c.ch);
                        }
                        None => out.push(' '),
                    }
                }
                out.push_str("\x1b[0m");
            }
            out.push('\n');
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(ch: char) -> Cell {
        Cell { ch, fg: Rgb(10, 20, 30), bold: false }
    }

    #[test]
    fn set_clips_out_of_bounds() {
        let mut cv = Canvas::new(3, 2);
        cv.set(-1, 0, c('a'));
        cv.set(3, 0, c('b'));
        cv.set(0, 2, c('c'));
        cv.set(2, 1, c('d'));
        assert_eq!(cv.to_plain(), "\n  d\n");
    }

    #[test]
    fn plain_trims_trailing_space() {
        let mut cv = Canvas::new(5, 1);
        cv.put_str(1, 0, "hi", Rgb(0, 0, 0), false);
        assert_eq!(cv.to_plain(), " hi\n");
    }

    #[test]
    fn ansi_truecolor_and_256() {
        let mut cv = Canvas::new(2, 1);
        cv.set(0, 0, Cell { ch: 'x', fg: Rgb(255, 0, 0), bold: true });
        let t = cv.to_ansi(ColorDepth::TrueColor);
        assert!(t.contains("\x1b[1m") && t.contains("\x1b[38;2;255;0;0mx"));
        let p = cv.to_ansi(ColorDepth::Ansi256);
        assert!(p.contains("\x1b[38;5;196mx"));
        assert_eq!(cv.to_ansi(ColorDepth::None), "x\n");
    }

    #[test]
    fn hex_parse_and_helpers() {
        assert_eq!(Rgb::from_hex("#dea584"), Some(Rgb(0xde, 0xa5, 0x84)));
        assert_eq!(Rgb::from_hex("3178c6"), Some(Rgb(0x31, 0x78, 0xc6)));
        assert_eq!(Rgb::from_hex("#xyz"), None);
        assert_eq!(Rgb::from_hex("#ééé"), None);
        assert_eq!(Rgb::hex(0x102030), Rgb(0x10, 0x20, 0x30));
        assert_eq!(Rgb(0, 0, 0).mix(Rgb(200, 100, 50), 0.5), Rgb(100, 50, 25));
        assert_eq!(Rgb(200, 200, 200).scale(2.0), Rgb(255, 255, 255));
        assert_eq!(Rgb(0, 0, 0).to_ansi256(), 16);
        assert_eq!(Rgb(255, 255, 255).to_ansi256(), 231);
    }
}
