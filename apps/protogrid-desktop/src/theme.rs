use std::sync::Arc;

use egui::{
    Color32, FontData, FontDefinitions, FontFamily, FontId, Stroke, TextStyle, Theme, Visuals,
};

pub const ACCENT: Color32 = Color32::from_rgb(59, 130, 246);
pub const CYAN: Color32 = Color32::from_rgb(34, 211, 238);
pub const GREEN: Color32 = Color32::from_rgb(16, 185, 129);
pub const AMBER: Color32 = Color32::from_rgb(245, 158, 11);
pub const RED: Color32 = Color32::from_rgb(239, 68, 68);
pub const VIOLET: Color32 = Color32::from_rgb(167, 139, 250);

fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub dark: bool,
    pub bg: Color32,
    pub bg_2: Color32,
    pub sidebar: Color32,
    pub panel: Color32,
    pub panel_2: Color32,
    pub panel_3: Color32,
    pub line: Color32,
    pub line_strong: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub faint: Color32,
    pub accent: Color32,
    pub accent_2: Color32,
    pub accent_soft: Color32,
    pub accent_line: Color32,
    pub cyan: Color32,
    pub cyan_soft: Color32,
    pub green: Color32,
    pub green_soft: Color32,
    pub amber: Color32,
    pub amber_soft: Color32,
    pub red: Color32,
    pub red_soft: Color32,
    pub violet: Color32,
    pub violet_soft: Color32,
    pub chart_grid: Color32,
    pub code_bg: Color32,
}

impl Palette {
    pub fn new(dark: bool) -> Self {
        if dark {
            Self {
                dark: true,
                bg: Color32::from_rgb(15, 17, 21),
                bg_2: Color32::from_rgb(12, 14, 18),
                sidebar: Color32::from_rgb(20, 22, 27),
                panel: Color32::from_rgb(25, 28, 34),
                panel_2: Color32::from_rgb(31, 35, 43),
                panel_3: Color32::from_rgb(37, 42, 52),
                line: Color32::from_rgb(39, 44, 53),
                line_strong: Color32::from_rgb(53, 60, 72),
                text: Color32::from_rgb(232, 237, 245),
                muted: Color32::from_rgb(160, 169, 183),
                faint: Color32::from_rgb(111, 121, 136),
                accent: ACCENT,
                accent_2: Color32::from_rgb(96, 165, 250),
                accent_soft: rgba(59, 130, 246, 36),
                accent_line: rgba(59, 130, 246, 107),
                cyan: CYAN,
                cyan_soft: rgba(34, 211, 238, 31),
                green: GREEN,
                green_soft: rgba(16, 185, 129, 31),
                amber: AMBER,
                amber_soft: rgba(245, 158, 11, 31),
                red: RED,
                red_soft: rgba(239, 68, 68, 26),
                violet: VIOLET,
                violet_soft: rgba(167, 139, 250, 33),
                chart_grid: rgba(255, 255, 255, 15),
                code_bg: Color32::from_rgb(12, 14, 18),
            }
        } else {
            Self {
                dark: false,
                bg: Color32::from_rgb(243, 245, 249),
                bg_2: Color32::from_rgb(248, 250, 252),
                sidebar: Color32::WHITE,
                panel: Color32::WHITE,
                panel_2: Color32::from_rgb(246, 248, 251),
                panel_3: Color32::from_rgb(238, 242, 247),
                line: Color32::from_rgb(229, 233, 241),
                line_strong: Color32::from_rgb(215, 221, 232),
                text: Color32::from_rgb(17, 24, 39),
                muted: Color32::from_rgb(91, 102, 119),
                faint: Color32::from_rgb(139, 149, 165),
                accent: Color32::from_rgb(37, 99, 235),
                accent_2: Color32::from_rgb(59, 130, 246),
                accent_soft: rgba(37, 99, 235, 23),
                accent_line: rgba(37, 99, 235, 89),
                cyan: Color32::from_rgb(8, 145, 178),
                cyan_soft: rgba(8, 145, 178, 22),
                green: Color32::from_rgb(5, 150, 105),
                green_soft: rgba(5, 150, 105, 22),
                amber: Color32::from_rgb(217, 119, 6),
                amber_soft: rgba(217, 119, 6, 24),
                red: Color32::from_rgb(220, 38, 38),
                red_soft: rgba(220, 38, 38, 20),
                violet: Color32::from_rgb(124, 58, 237),
                violet_soft: rgba(124, 58, 237, 20),
                chart_grid: rgba(15, 23, 42, 18),
                code_bg: Color32::from_rgb(248, 250, 252),
            }
        }
    }

    pub fn from_ctx(ctx: &egui::Context) -> Self {
        Self::new(ctx.theme() == egui::Theme::Dark)
    }
}

pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "inter".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/Inter-Regular.ttf"
        ))),
    );
    fonts.font_data.insert(
        "inter-medium".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/Inter-Medium.ttf"
        ))),
    );
    fonts.font_data.insert(
        "inter-semibold".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/Inter-SemiBold.ttf"
        ))),
    );
    fonts.font_data.insert(
        "inter-bold".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/Inter-Bold.ttf"
        ))),
    );
    // JetBrains Mono is the monospace face used by the reference concept.
    fonts.font_data.insert(
        "jetbrains".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/JetBrainsMono-Regular.ttf"
        ))),
    );
    fonts.font_data.insert(
        "jetbrains-medium".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/JetBrainsMono-Medium.ttf"
        ))),
    );
    fonts.font_data.insert(
        "jetbrains-bold".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/JetBrainsMono-Bold.ttf"
        ))),
    );
    // DejaVu Sans Mono stays as a glyph fallback behind JetBrains Mono.
    fonts.font_data.insert(
        "mono".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/DejaVuSansMono.ttf"
        ))),
    );

    let proportional = fonts.families.entry(FontFamily::Proportional).or_default();
    proportional.insert(0, "inter".to_owned());

    let monospace = fonts.families.entry(FontFamily::Monospace).or_default();
    monospace.insert(0, "jetbrains".to_owned());
    monospace.insert(1, "mono".to_owned());

    let semibold = FontFamily::Name("Semibold".into());
    fonts.families.insert(
        semibold,
        vec!["inter-semibold".to_owned(), "inter".to_owned()],
    );
    let medium = FontFamily::Name("Medium".into());
    fonts
        .families
        .insert(medium, vec!["inter-medium".to_owned(), "inter".to_owned()]);
    let bold = FontFamily::Name("Bold".into());
    fonts
        .families
        .insert(bold, vec!["inter-bold".to_owned(), "inter".to_owned()]);
    let mono_medium = FontFamily::Name("MonoMedium".into());
    fonts.families.insert(
        mono_medium,
        vec![
            "jetbrains-medium".to_owned(),
            "jetbrains".to_owned(),
            "mono".to_owned(),
        ],
    );
    let mono_bold = FontFamily::Name("MonoBold".into());
    fonts.families.insert(
        mono_bold,
        vec![
            "jetbrains-bold".to_owned(),
            "jetbrains".to_owned(),
            "mono".to_owned(),
        ],
    );

    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    // Inter ships Private Use Area glyphs that would shadow the Phosphor
    // codepoints; the bundled Inter files have those cmap entries stripped, so
    // the icon font can safely sit right after the text font.
    for family in [
        FontFamily::Proportional,
        FontFamily::Monospace,
        FontFamily::Name("Medium".into()),
        FontFamily::Name("Semibold".into()),
        FontFamily::Name("Bold".into()),
        FontFamily::Name("MonoMedium".into()),
        FontFamily::Name("MonoBold".into()),
    ] {
        if let Some(keys) = fonts.families.get_mut(&family) {
            keys.retain(|key| key != "phosphor");
            let position = keys.len().min(1);
            keys.insert(position, "phosphor".into());
        }
    }
    ctx.set_fonts(fonts);
}

pub fn install(ctx: &egui::Context, dark: bool) {
    install_fonts(ctx);
    ctx.set_style_of(Theme::Dark, style_for(Theme::Dark));
    ctx.set_style_of(Theme::Light, style_for(Theme::Light));
    ctx.set_theme(if dark {
        egui::ThemePreference::Dark
    } else {
        egui::ThemePreference::Light
    });
}

fn style_for(theme: Theme) -> egui::Style {
    let palette = Palette::new(theme == Theme::Dark);
    let mut style = egui::Style {
        visuals: visuals(&palette),
        ..Default::default()
    };
    style.spacing.item_spacing = egui::vec2(7.0, 7.0);
    style.spacing.button_padding = egui::vec2(11.0, 5.0);
    style.spacing.window_margin = egui::Margin::same(18);
    style.spacing.menu_margin = egui::Margin::same(8);
    style.spacing.indent = 16.0;
    style.spacing.scroll.bar_width = 8.0;
    style.spacing.scroll.floating = true;
    style.spacing.scroll.floating_width = 5.0;
    style.spacing.scroll.floating_allocated_width = 0.0;
    style.spacing.scroll.foreground_color = false;
    style.spacing.scroll.dormant_handle_opacity = 0.0;
    style.spacing.scroll.active_handle_opacity = 0.5;
    style.spacing.scroll.interact_handle_opacity = 0.8;
    style.spacing.scroll.dormant_background_opacity = 0.0;
    style.spacing.scroll.active_background_opacity = 0.0;
    style.spacing.scroll.interact_background_opacity = 0.12;
    style.text_styles = [
        (
            TextStyle::Heading,
            FontId::new(19.0, FontFamily::Name("Semibold".into())),
        ),
        (TextStyle::Body, FontId::new(13.0, FontFamily::Proportional)),
        (
            TextStyle::Button,
            FontId::new(12.5, FontFamily::Name("Medium".into())),
        ),
        (
            TextStyle::Small,
            FontId::new(10.5, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(11.5, FontFamily::Monospace),
        ),
    ]
    .into();
    style
}

fn visuals(p: &Palette) -> Visuals {
    let mut v = if p.dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    v.panel_fill = p.bg;
    v.window_fill = p.panel;
    v.extreme_bg_color = p.bg_2;
    v.text_edit_bg_color = Some(p.panel_2);
    v.faint_bg_color = p.panel_2;
    v.code_bg_color = p.code_bg;
    v.override_text_color = Some(p.text);
    v.weak_text_color = Some(p.muted);
    v.hyperlink_color = p.accent_2;
    v.warn_fg_color = p.amber;
    v.error_fg_color = p.red;
    v.selection.bg_fill = p.accent_soft;
    v.selection.stroke = Stroke::new(1.0, p.accent);
    v.widgets.noninteractive.bg_fill = p.panel;
    v.widgets.noninteractive.weak_bg_fill = p.panel;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.line);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    v.widgets.noninteractive.corner_radius = egui::CornerRadius::same(8);
    v.widgets.inactive.bg_fill = p.panel_2;
    v.widgets.inactive.weak_bg_fill = p.panel_2;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, p.line);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, p.text);
    v.widgets.inactive.corner_radius = egui::CornerRadius::same(8);
    v.widgets.hovered.bg_fill = p.panel_3;
    v.widgets.hovered.weak_bg_fill = p.panel_3;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, p.line_strong);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, p.text);
    v.widgets.hovered.corner_radius = egui::CornerRadius::same(8);
    v.widgets.active.bg_fill = p.accent_soft;
    v.widgets.active.weak_bg_fill = p.accent_soft;
    v.widgets.active.bg_stroke = Stroke::new(1.0, p.accent);
    v.widgets.active.fg_stroke = Stroke::new(1.0, p.text);
    v.widgets.active.corner_radius = egui::CornerRadius::same(8);
    v.widgets.open.bg_fill = p.panel_2;
    v.widgets.open.weak_bg_fill = p.panel_2;
    v.widgets.open.bg_stroke = Stroke::new(1.0, p.line_strong);
    v.widgets.open.fg_stroke = Stroke::new(1.0, p.text);
    v.widgets.open.corner_radius = egui::CornerRadius::same(8);
    v.window_stroke = Stroke::new(1.0, p.line_strong);
    v.window_corner_radius = egui::CornerRadius::same(16);
    v.menu_corner_radius = egui::CornerRadius::same(10);
    v
}

pub fn heading(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text.into())
        .family(FontFamily::Name("Semibold".into()))
        .size(19.0)
}

pub fn semibold(text: impl Into<String>, size: f32) -> egui::RichText {
    egui::RichText::new(text.into())
        .family(FontFamily::Name("Semibold".into()))
        .size(size)
}

pub fn bold(text: impl Into<String>, size: f32) -> egui::RichText {
    egui::RichText::new(text.into())
        .family(FontFamily::Name("Bold".into()))
        .size(size)
}

pub fn medium(text: impl Into<String>, size: f32) -> egui::RichText {
    egui::RichText::new(text.into())
        .family(FontFamily::Name("Medium".into()))
        .size(size)
}
