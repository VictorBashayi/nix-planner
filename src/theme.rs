//! Colour tokens derived from a profile's seed colour.

/// Curated seed colours offered for profiles, in assignment order.
pub const SWATCHES: [&str; 8] = ["#B0703A", "#4F6FA8", "#4E8A5A", "#2F8A8A", "#7A5FA8", "#B0506A", "#5F6B7A", "#B07A1E"];


/// An sRGB colour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    /// Reads "#RRGGBB" or "RRGGBB".
    pub fn parse(hex: &str) -> Option<Rgb> {
        let h = hex.strip_prefix('#').unwrap_or(hex);
        if h.len() != 6 || !h.is_ascii() {
            return None;
        }
        let byte = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
        Some(Rgb(byte(0)?, byte(2)?, byte(4)?))
    }

    fn to_hsl(self) -> (f32, f32, f32) {
        let [r, g, b] = [self.0, self.1, self.2].map(|c| c as f32 / 255.0);
        let (max, min) = (r.max(g).max(b), r.min(g).min(b));
        let l = (max + min) / 2.0;
        let d = max - min;
        if d == 0.0 {
            return (0.0, 0.0, l * 100.0);
        }
        let s = d / (1.0 - (2.0 * l - 1.0).abs());
        let h = if max == r {
            60.0 * ((g - b) / d).rem_euclid(6.0)
        } else if max == g {
            60.0 * ((b - r) / d + 2.0)
        } else {
            60.0 * ((r - g) / d + 4.0)
        };
        (h, s * 100.0, l * 100.0)
    }
}

/// HSL with `s` and `l` in percent.
fn hsl(h: f32, s: f32, l: f32) -> Rgb {
    let (s, l) = (s.clamp(0.0, 100.0) / 100.0, l.clamp(0.0, 100.0) / 100.0);
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    let (r, g, b) = match (h.rem_euclid(360.0) / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    let to = |v: f32| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    Rgb(to(r), to(g), to(b))
}

fn luminance(c: &Rgb) -> f32 {
    let lin = |v: u8| {
        let v = v as f32 / 255.0;
        if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * lin(c.0) + 0.7152 * lin(c.1) + 0.0722 * lin(c.2)
}

/// WCAG 2 contrast ratio, 1 to 21.
pub fn contrast(a: &Rgb, b: &Rgb) -> f32 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

/// Every colour role the UI uses; see docs/nectar-style.md.
#[derive(Clone, Debug, PartialEq)]
pub struct Tokens {
    pub chrome: Rgb,
    pub canvas: Rgb,
    pub card: Rgb,
    pub card_alt: Rgb,
    pub accent_soft: Rgb,
    pub accent: Rgb,
    pub text: Rgb,
    pub text_muted: Rgb,
    pub hairline: Rgb,
    pub hover: Rgb,
    pub danger: Rgb,
    pub now: Rgb,
    pub soon: Rgb,
    pub later: Rgb,
    pub done: Rgb,
}

/// The token set for a seed colour in light or dark mode.
pub fn tokens(seed: Rgb, dark: bool) -> Tokens {
    let (h, s, l) = seed.to_hsl();
    let tone = |light: (f32, f32), dark_: (f32, f32)| {
        let (ts, tl) = if dark { dark_ } else { light };
        hsl(h, ts, tl)
    };
    let canvas = tone((18.0, 95.0), (10.0, 14.0));
    let card = tone((22.0, 98.0), (8.0, 22.0));
    // Lightness moving away from the surfaces: darker in light mode, lighter in dark.
    let step = if dark { 1.0 } else { -1.0 };
    let readable = |c: &Rgb, ratio: f32| contrast(c, &canvas) >= ratio && contrast(c, &card) >= ratio;
    // The palest tone of the seed that reaches `ratio`, searched from the surface side.
    let reach = |start: f32, ratio: f32| {
        let mut tl = start;
        while !readable(&hsl(h, s, tl), ratio) && (0.0..=100.0).contains(&tl) {
            tl += step;
        }
        hsl(h, s, tl)
    };
    let mut al = l;
    while !readable(&hsl(h, s, al), 3.0) && (0.0..=100.0).contains(&al) {
        al += step;
    }
    let surface_l = if dark { 14.0 } else { 95.0 };
    Tokens {
        chrome: tone((12.0, 93.0), (8.0, 19.0)),
        canvas,
        card,
        card_alt: tone((20.0, 92.0), (10.0, 17.0)),
        accent_soft: tone((55.0, 89.0), (28.0, 28.0)),
        accent: hsl(h, s, al),
        text: tone((20.0, 12.0), (10.0, 92.0)),
        text_muted: tone((12.0, 36.0), (8.0, 70.0)),
        hairline: tone((18.0, 82.0), (10.0, 31.0)),
        hover: tone((25.0, 99.0), (8.0, 26.0)),
        danger: if dark { Rgb(0xE0, 0x7A, 0x62) } else { Rgb(0xB4, 0x44, 0x2F) },
        now: reach(surface_l, 4.0),
        soon: reach(surface_l, 3.4),
        later: reach(surface_l, 3.0),
        done: tone((6.0, 62.0), (6.0, 45.0)),
    }
}

/// Whether Windows apps are set to dark mode (Settings > Personalization > Colors).
pub fn system_dark() -> bool {
    use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
    let key = wide(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");
    let name = wide("AppsUseLightTheme");
    let mut value: u32 = 1;
    let mut size = std::mem::size_of::<u32>() as u32;
    let rc = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            (&mut value as *mut u32).cast(),
            &mut size,
        )
    };
    rc == 0 && value == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_swatch_is_readable_in_both_modes() {
        for hex in SWATCHES {
            for dark in [false, true] {
                let t = tokens(Rgb::parse(hex).unwrap(), dark);
                for bg in [&t.card, &t.canvas] {
                    assert!(contrast(&t.text, bg) >= 4.5, "{hex} dark={dark} text");
                    assert!(contrast(&t.text_muted, bg) >= 4.5, "{hex} dark={dark} muted");
                    for (n, c) in [("accent", &t.accent), ("now", &t.now), ("soon", &t.soon), ("later", &t.later)] {
                        assert!(contrast(c, bg) >= 3.0, "{hex} dark={dark} {n}");
                    }
                }
            }
        }
    }

    #[test]
    fn contrast_matches_wcag_reference() {
        let r = contrast(&Rgb(0, 0, 0), &Rgb(255, 255, 255));
        assert!((r - 21.0).abs() < 0.01);
    }

    #[test]
    fn fade_steps_get_paler_towards_someday() {
        let t = tokens(Rgb::parse("#B0703A").unwrap(), false);
        let d = |c: &Rgb| contrast(c, &t.canvas);
        assert!(d(&t.now) >= d(&t.soon) && d(&t.soon) >= d(&t.later));
        assert!(d(&t.now) > d(&t.later), "fade must actually fade");
    }

    #[test]
    fn parse_rejects_bad_hex() {
        assert!(Rgb::parse("#12345").is_none() && Rgb::parse("zzzzzz").is_none());
        assert!(Rgb::parse("#b0703a").is_some());
    }
}
