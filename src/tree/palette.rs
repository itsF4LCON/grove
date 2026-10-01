//! Colour and glyph sets per season.

use crate::canvas::Rgb;
use crate::mapping::Season;

pub struct Palette {
    pub bark: [Rgb; 3],
    pub twig: Rgb,
    pub leaves: Vec<Rgb>,
    pub leaf_chars: &'static [char],
    /// Multiplies the repo's leaf density (bare seasons drop most leaves).
    pub leaf_factor: f32,
    pub blossom: [Rgb; 2],
    pub blossom_chars: [char; 2],
    pub blight: Rgb,
}

pub fn palette_for(season: Season, tint: Option<Rgb>) -> Palette {
    let (leaves, leaf_chars, leaf_factor): (Vec<u32>, &'static [char], f32) = match season {
        Season::Spring => (vec![0x9BE564, 0x7BD389, 0xB5E48C, 0x80ED99], &['&', '&', '%', '*'], 0.9),
        Season::Summer => (vec![0x2D6A4F, 0x40916C, 0x52B788, 0x1B4332, 0x74C69D], &['&', '&', '&', '%', '@'], 1.0),
        Season::Autumn => (vec![0xE76F51, 0xF4A261, 0xE9C46A, 0xD62828, 0xBC6C25], &['&', '%', '*', '&'], 0.8),
        Season::LateAutumn => (vec![0x7F5539, 0x9C6644, 0xB08968, 0xDDB892], &['\'', ',', '.', '`'], 0.35),
        Season::Winter => (vec![0xE0E6EF, 0xF8F9FA, 0xC9D6DF], &['*', '.', '\''], 0.15),
    };
    let leaves = leaves
        .into_iter()
        .map(Rgb::hex)
        .map(|c| match (season, tint) {
            (Season::Spring | Season::Summer, Some(t)) => c.mix(t, 0.15),
            _ => c,
        })
        .collect();
    Palette {
        bark: [Rgb::hex(0x6F4E37), Rgb::hex(0x8B5A2B), Rgb::hex(0xA0522D)],
        twig: Rgb::hex(0x8D7B68),
        leaves,
        leaf_chars,
        leaf_factor,
        blossom: [Rgb::hex(0xFFAFCC), Rgb::hex(0xFFF0F5)],
        blossom_chars: ['❀', '✿'],
        blight: Rgb::hex(0xC1121F),
    }
}
