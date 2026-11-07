//! Settings (`SettingsActivity`, DESIGN §7.4): Appearance, General and App, with the phone's 14 rows in its order.
//! The rows, the section labels and the 379.4 dp column are the captures'. Each row is its own 16 dp card.

pub(crate) mod sheets;
pub(crate) mod ui;

use crate::appearance;
use crate::glass;
use crate::phone;
use crate::settings::sheets::Title;
use crate::settings::ui::Trail;
use crate::{icons, load_settings, save_setting, t, App, Page};
use eframe::egui::{Pos2, Rect, Ui, Vec2};

/// Settings › Language: (locale tag or "en" for the English default table, native name); "" is the system default.
pub const LANGUAGES: &[(&str, &str)] = &[
    ("", ""), ("en", "English"), ("zh-CN", "简体中文"), ("zh-TW", "繁體中文"), ("ja", "日本語"), ("ko", "한국어"),
    ("in", "Bahasa Indonesia"), ("ms", "Bahasa Melayu"), ("cs", "Čeština"), ("de", "Deutsch"), ("es", "Español"),
    ("fil", "Filipino"), ("fr", "Français"), ("it", "Italiano"), ("hu", "Magyar"), ("nl", "Nederlands"),
    ("pl", "Polski"), ("pt", "Português"), ("ro", "Română"), ("sv", "Svenska"), ("vi", "Tiếng Việt"),
    ("tr", "Türkçe"), ("el", "Ελληνικά"), ("ru", "Русский"), ("uk", "Українська"), ("hi", "हिन्दी"),
    ("bn", "বাংলা"), ("th", "ไทย"),
];

#[derive(Default)]
pub struct State {
    /// The background-service warning sheet is open.
    bg_warn: bool,
}

pub fn show(app: &mut App, ui: &mut Ui, body: Rect) -> f32 {
    let tr = app.tr.clone();
    let light = crate::palette::is_light_system(ui.ctx());
    let accent = crate::palette::active(light, app.style == glass::style::Style::Material).accent;
    let w = phone::content(body);
    let x0 = body.left();
    let st = load_settings();
    let flag = |key: &str, default: bool| st[key].as_bool().unwrap_or(default);
    let mut y = body.top();
    let mut go: Option<Page> = None;

    // Appearance.
    ui::section(ui, x0, y, t(&tr, "settings_appearance"), accent);
    y += phone::SECTION;
    let preset = appearance::preset_name(&tr, &crate::palette::active_id(light));
    let (h, clicked, _) = ui::row_card(ui, "theme", x0, y, w, Some(icons::PALETTE), t(&tr, "theme_title"), &preset, Trail::Chevron, true);
    if clicked { go = Some(Page::Appearance); }
    y += h + 8.0;
    let (h, clicked, _) = ui::row_card(ui, "layout", x0, y, w, Some(icons::LAYOUT), t(&tr, "settings_layout_title"), t(&tr, "settings_layout_sub"), Trail::Chevron, true);
    if clicked { go = Some(Page::Layout); }
    y += h;

    // General.
    ui::section(ui, x0, y, t(&tr, "settings_general"), accent);
    y += phone::SECTION;
    let (h, clicked, _) = ui::row_card(ui, "language", x0, y, w, Some(icons::LANGUAGE), t(&tr, "settings_language_title"), t(&tr, "settings_language_sub"), Trail::Chevron, true);
    if clicked { go = Some(Page::Language); }
    y += h + 8.0;

    let haptics = flag("haptics", true);
    let (h, _, toggled) = ui::row_card(ui, "haptics", x0, y, w, Some(ui::HAPTIC), t(&tr, "settings_haptics_title"), t(&tr, "settings_haptics_sub"), Trail::Switch(haptics), true);
    if toggled { save_setting("haptics", (!haptics).into()); }
    y += h + 8.0;

    let background = flag("background_service", true);
    let (h, _, toggled) = ui::row_card(ui, "background", x0, y, w, Some(icons::POWER), t(&tr, "settings_background_title"), t(&tr, "settings_background_sub"), Trail::Switch(background), true);
    if toggled { if background { app.settings.bg_warn = true; } else { save_setting("background_service", true.into()); } }
    y += h + 8.0;

    let clear = flag("clear_battery", false);
    let (h, _, toggled) = ui::row_card(ui, "clear-battery", x0, y, w, Some(icons::BATTERY), t(&tr, "settings_clear_battery_title"), t(&tr, "settings_clear_battery_sub"), Trail::Switch(clear), true);
    if toggled { save_setting("clear_battery", (!clear).into()); }
    y += h + 8.0;

    let devtools = flag("dev_tools_button", true);
    let (h, _, toggled) = ui::row_card(ui, "devtools", x0, y, w, Some(icons::DEV_TOOLS), t(&tr, "settings_devtools_title"), t(&tr, "settings_devtools_sub"), Trail::Switch(devtools), true);
    if toggled { save_setting("dev_tools_button", (!devtools).into()); }
    y += h + 8.0;

    let (h, clicked, _) = ui::row_card(ui, "setup", x0, y, w, Some(icons::INFO), t(&tr, "settings_setup_title"), "", Trail::Chevron, true);
    if clicked { app.setup.page = 0; app.page = Page::Setup; }
    y += h;

    // App.
    ui::section(ui, x0, y, t(&tr, "settings_app"), accent);
    y += phone::SECTION;
    let installed = t(&tr, "settings_update_sub").replace("%1$s", env!("CARGO_PKG_VERSION"));
    let (h, clicked, _) = ui::row_card(ui, "update", x0, y, w, Some(icons::UPDATE), t(&tr, "row_update_title"), &installed, Trail::Chevron, true);
    if clicked { go = Some(Page::Update); }
    y += h + 8.0;
    let (h, clicked, _) = ui::row_card(ui, "test", x0, y, w, Some(icons::EARBUD), t(&tr, "test_title"), t(&tr, "test_sub"), Trail::Chevron, true);
    if clicked { go = Some(Page::Test); }
    y += h + 8.0;
    let (h, clicked, _) = ui::row_card(ui, "feedback", x0, y, w, Some(icons::DEV_TOOLS), t(&tr, "problem_title"), t(&tr, "problem_sub"), Trail::Chevron, true);
    if clicked { go = Some(Page::Feedback); }
    y += h + 8.0;
    let (h, clicked, _) = ui::row_card(ui, "about", x0, y, w, Some(icons::INFO), t(&tr, "settings_about_title"), t(&tr, "settings_about_sub"), Trail::Chevron, true);
    if clicked { go = Some(Page::About); }
    y += h + 8.0;
    let (h, clicked, _) = ui::row_card(ui, "issues", x0, y, w, Some(icons::INFO), t(&tr, "settings_issues_title"), t(&tr, "settings_issues_sub"), Trail::Chevron, true);
    if clicked { go = Some(Page::Issues); }
    y += h;

    if let Some(p) = go { app.page = p; }

    // The background-service warning: Cancel keeps it on, "Turn off" writes the setting.
    if app.settings.bg_warn {
        let mut decision = None;
        let open = sheets::sheet(ui.ctx(), "background-warn", t(&tr, "settings_background_warn_title"), Title::Start, Vec2::new(sheets::WIDTH, 263.2), |ui, inner| {
            let body_rect = Rect::from_min_max(inner.min + Vec2::new(17.0, 4.0), Pos2::new(inner.right(), inner.bottom() - 66.0));
            phone::paragraph(ui, body_rect, t(&tr, "settings_background_warn_body"), 14.0, glass::white(0.80), true);
            decision = sheets::actions(ui, inner, t(&tr, "dialog_cancel"), t(&tr, "settings_background_warn_action"));
        });
        if decision == Some(true) { save_setting("background_service", false.into()); }
        if decision.is_some() || !open { app.settings.bg_warn = false; }
    }

    y + phone::GAP
}
