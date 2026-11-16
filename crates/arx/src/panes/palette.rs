use ratatui::style::Color;

use crate::config::PaletteConfig;

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub operator: Color,
    pub rule: Color,
    pub gutter: Color,
    pub tool: Color,
}

impl Default for Palette {
    fn default() -> Self {
        Palette {
            operator: Color::Rgb(0x8f, 0x8f, 0x4f),
            rule: Color::Rgb(0x8f, 0x8f, 0x4f),
            gutter: Color::Rgb(0x8f, 0x8f, 0x4f),
            tool: Color::Rgb(0xb0, 0x50, 0x3c),
        }
    }
}

fn parse_color(field: &str, value: &str) -> Result<Color, String> {
    value.parse::<Color>().map_err(|_| format!("[terminal.palette] {field} is not a color: {value}"))
}

impl Palette {
    pub fn from_config(cfg: &Option<PaletteConfig>) -> Result<Palette, String> {
        let mut palette = Palette::default();
        let Some(cfg) = cfg else {
            return Ok(palette);
        };
        if let Some(value) = &cfg.operator {
            palette.operator = parse_color("operator", value)?;
        }
        if let Some(value) = &cfg.rule {
            palette.rule = parse_color("rule", value)?;
        }
        if let Some(value) = &cfg.gutter {
            palette.gutter = parse_color("gutter", value)?;
        }
        if let Some(value) = &cfg.tool {
            palette.tool = parse_color("tool", value)?;
        }
        Ok(palette)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hex_override_parses_to_the_exact_rgb() {
        let cfg = Some(PaletteConfig { operator: Some("#112233".to_string()), ..Default::default() });
        let palette = Palette::from_config(&cfg).unwrap();
        assert_eq!(palette.operator, Color::Rgb(0x11, 0x22, 0x33));
    }

    #[test]
    fn a_named_color_override_parses_to_the_ratatui_variant() {
        let cfg = Some(PaletteConfig { tool: Some("red".to_string()), ..Default::default() });
        let palette = Palette::from_config(&cfg).unwrap();
        assert_eq!(palette.tool, Color::Red);
    }

    #[test]
    fn an_unrecognized_color_name_errors_rather_than_falling_back() {
        let cfg = Some(PaletteConfig { rule: Some("olive".to_string()), ..Default::default() });
        assert!(Palette::from_config(&cfg).is_err());
    }

    #[test]
    fn no_config_gives_the_defaults() {
        let palette = Palette::from_config(&None).unwrap();
        assert_eq!(palette.operator, Color::Rgb(0x8f, 0x8f, 0x4f));
        assert_eq!(palette.tool, Color::Rgb(0xb0, 0x50, 0x3c));
    }
}
