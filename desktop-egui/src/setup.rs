//! The setup guide (`SetupActivity`, DESIGN §7.3): ten pages in the capture's order, a 43.8 dp bar and
//! 8 dp dots. The last page starts Home. A page change fades and slides 28 dp in 260 ms.

use crate::glass::{self, white};
use crate::palette;
use crate::phone::{self, Trail};
use crate::glass::style::Style;
use crate::{icons, load_settings, save_setting, t, App, Page};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Ui, Vec2};

/// The bar's top edge on the phone: 851.4 dp from the screen top. The page body starts 60.2 dp down.
const BAR_TOP: f32 = 851.4 - 60.2;
const BAR: f32 = 43.8;
const PAGE_W: f32 = 347.4;
/// The phone's picker order.
const STYLES: [Style; 4] = [Style::Material, Style::Classic, Style::Pixel, Style::Glass];

#[derive(Default)]
pub struct State {
    pub page: usize,
    /// 1 when the page came from the left, -1 from the right.
    dir: f32,
    /// The wallpaper switch; Android only, so it is not saved.
    wallpaper: bool,
    started: bool,
}

fn fit(ui: &Ui, s: &str, size: f32, width: f32) -> String {
    let font = FontId::proportional(size);
    let measure = |text: &str| ui.painter().layout_no_wrap(text.to_string(), font.clone(), Color32::WHITE).size().x;
    if measure(s) <= width { return s.to_string(); }
    let mut cut = s.to_string();
    while !cut.is_empty() {
        cut.pop();
        let shown = format!("{}…", cut.trim_end());
        if measure(&shown) <= width { return shown; }
    }
    String::new()
}

/// A page title at 24 sp and its body at 14 sp. Returns the bottom of the body.
fn heading(ui: &Ui, x: f32, y: f32, w: f32, title: &str, body: &str) -> f32 {
    let th = phone::wrap_height(ui, title, 24.0, w);
    ui.painter().text(Pos2::new(x, y), Align2::LEFT_TOP, title, FontId::proportional(24.0), white(1.0));
    if body.is_empty() { return y + th; }
    let by = y + th + 8.0;
    let galley = ui.painter().layout(body.to_string(), FontId::proportional(14.0), white(0.74), w);
    let h = galley.size().y;
    ui.painter().galley(Pos2::new(x, by), galley, white(0.74));
    by + h
}

/// A row of `h` with a title and a 36 dp pill at the end, or the allowed label.
fn pill_row(ui: &mut Ui, id: &str, rect: Rect, title: &str, action: &str, allowed: bool, accent: Color32) -> bool {
    glass::flat_at(ui, rect, 16.0, 1.0);
    let x = rect.left() + 16.0;
    let pill_w = 72.0;
    let text_w = rect.width() - 32.0 - pill_w - 8.0;
    glass::text(ui, Pos2::new(x, rect.center().y), Align2::LEFT_CENTER, &fit(ui, title, 16.0, text_w), 16.0, 1.0);
    if allowed {
        ui.painter().text(Pos2::new(rect.right() - 16.0, rect.center().y), Align2::RIGHT_CENTER, action, FontId::proportional(14.0), white(0.74));
        false
    } else {
        let r = Rect::from_min_size(Pos2::new(rect.right() - 16.0 - pill_w, rect.center().y - 18.0), Vec2::new(pill_w, 36.0));
        phone::pill(ui, id, r, action, true, accent)
    }
}

/// A row that opens another page.
fn nav_row(ui: &mut Ui, rect: Rect, title: &str, sub: &str, accent: Color32) -> bool {
    phone::row_at(ui, title, rect, phone::Row::new(title, sub, Trail::Chevron, accent), true).0
}

pub fn show(app: &mut App, ui: &mut Ui, body: Rect) -> f32 {
    let tr = app.tr.clone();
    let light = palette::is_light_system(ui.ctx());
    let accent = palette::active(light, app.style == Style::Material).accent;
    let x0 = body.left() + 16.0;
    let w = phone::content(body).min(PAGE_W);
    let bar_y = body.top() + BAR_TOP;
    let page = app.setup.page.min(9);
    if !app.setup.started { app.setup.started = true; app.setup.wallpaper = true; }

    // The page, clipped above the bar. A page change fades in and slides from its side.
    let area = Rect::from_min_max(Pos2::new(body.left(), body.top()), Pos2::new(body.right(), bar_y));
    let m = crate::motion::run(ui.ctx(), egui::Id::NULL.with(("setup", page)), crate::motion::PAGE_ENTER_FADE);
    let off = (1.0 - m) * 28.0 * app.setup.dir;
    let mut page_ui = ui.new_child(egui::UiBuilder::new().max_rect(area));
    page_ui.set_clip_rect(area.intersect(ui.clip_rect()));
    page_ui.set_opacity(0.35 + 0.65 * m);
    let p = &mut page_ui;
    let mut y = area.top() + 4.0 + off;
    let mut go: Option<Page> = None;
    match page {
        0 => {
            let icon = Rect::from_center_size(Pos2::new(x0 + w / 2.0, y + 66.0), Vec2::splat(120.0));
            glass::flat_at(p, Rect::from_center_size(icon.center(), Vec2::new(140.0, 66.0)), 33.0, 1.0);
            glass::icon_at(p, icons::LAUNCHER_FOREGROUND, icon, white(1.0));
            y += 132.0 + 20.0;
            y = heading(p, x0, y, w, t(&tr, "welcome_title"), t(&tr, "about_tagline"));
            y += 16.0;
            phone::paragraph(p, Rect::from_min_size(Pos2::new(x0, y), Vec2::new(w, 80.0)), t(&tr, "welcome_body"), 14.0, white(0.74), false);
        }
        1 => {
            y = heading(p, x0, y, w, t(&tr, "setup_look_title"), t(&tr, "setup_look_body")) + 8.0;
            phone::label(p, x0, y, t(&tr, "widget_style_title"), white(0.70));
            y += phone::SECTION;
            let labels: Vec<String> = STYLES.iter().map(|s| t(&tr, s.name_key()).to_string()).collect();
            let cur = STYLES.iter().position(|s| *s == app.style);
            if let Some(i) = phone::segments(p, "setup-style", Rect::from_min_size(Pos2::new(x0, y), Vec2::new(w, phone::SEGMENT)), &labels, cur, accent) {
                save_setting("egui_style", STYLES[i].key().into());
                app.reload_style(ui.ctx());
            }
            y += phone::SEGMENT + 2.0;
            phone::label(p, x0, y, t(&tr, "theme_builtin"), white(0.70));
            y += phone::SECTION;
            let labels: Vec<String> = palette::BUILT_IN.iter().map(|id| crate::appearance::preset_name(&tr, id)).collect();
            let id = palette::active_id(light);
            let cur = if palette::auto() { None } else { palette::BUILT_IN.iter().position(|p| *p == id) };
            if let Some(i) = phone::segments(p, "setup-theme", Rect::from_min_size(Pos2::new(x0, y), Vec2::new(w, phone::SEGMENT)), &labels, cur, accent) {
                palette::set_active(palette::BUILT_IN[i]);
                app.reload_style(ui.ctx());
            }
            y += phone::SEGMENT + phone::GAP;
            let wall = Rect::from_min_size(Pos2::new(x0, y), Vec2::new(w, phone::ROW));
            glass::flat_at(p, Rect::from_min_size(Pos2::new(x0, y), Vec2::new(w, phone::ROW * 2.0 + 8.0)), phone::RADIUS, 1.0);
            if phone::row_at(p, "setup-wall", wall, phone::Row::new(t(&tr, "theme_wallpaper"), t(&tr, "theme_wallpaper_sub"), Trail::Switch(app.setup.wallpaper), accent), true).1 {
                app.setup.wallpaper = !app.setup.wallpaper;
            }
            y += phone::ROW + 8.0;
            let row = Rect::from_min_size(Pos2::new(x0, y), Vec2::new(w, phone::ROW));
            let sub = crate::appearance::preset_name(&tr, &id);
            if phone::row_at(p, "setup-themes", row, phone::Row::new(t(&tr, "theme_title"), &sub, Trail::Chevron, accent), true).0 {
                save_setting("setup_done", true.into());
                go = Some(Page::Appearance);
            }
        }
        2 => {
            y = heading(p, x0, y, w, t(&tr, "setup_perm_title"), t(&tr, "setup_perm_body")) + 12.0;
            let card = Rect::from_min_size(Pos2::new(x0, y), Vec2::new(w, 59.8 * 2.0 + 8.0));
            glass::flat_at(p, card, phone::RADIUS, 1.0);
            for (i, key) in ["setup_perm_bt", "setup_perm_notif"].iter().enumerate() {
                let r = Rect::from_min_size(Pos2::new(x0, y + (59.8 + 8.0) * i as f32), Vec2::new(w, 59.8));
                pill_row(p, key, r, t(&tr, key), t(&tr, "setup_allowed"), true, accent);
            }
        }
        3 => {
            y = heading(p, x0, y, w, t(&tr, "setup_earbuds_title"), t(&tr, "setup_earbuds_body")) + 12.0;
            let names: Vec<String> = app.snap.devices.iter().map(|d| d.name.clone()).collect();
            if names.is_empty() {
                glass::text(p, Pos2::new(x0 + 4.0, y + 12.0), Align2::LEFT_CENTER, t(&tr, "setup_earbuds_none"), 14.0, 0.74);
                y += 32.0;
            } else {
                let card = Rect::from_min_size(Pos2::new(x0, y), Vec2::new(w, 59.8 * names.len() as f32 + 8.0 * (names.len() - 1) as f32));
                glass::flat_at(p, card, phone::RADIUS, 1.0);
                for (i, name) in names.iter().enumerate() {
                    let r = Rect::from_min_size(Pos2::new(x0, y + (59.8 + 8.0) * i as f32), Vec2::new(w, 59.8));
                    glass::icon_at(p, if i == 0 { icons::EARBUD } else { icons::DEVICES }, Rect::from_center_size(Pos2::new(r.left() + 16.0 + 12.0, r.center().y), Vec2::splat(24.0)), white(0.9));
                    glass::text(p, Pos2::new(r.left() + 53.7, r.center().y), Align2::LEFT_CENTER, name, 16.0, 1.0);
                    glass::icon_at(p, icons::CHECK, Rect::from_center_size(Pos2::new(r.right() - 20.0, r.center().y), Vec2::splat(22.0)), accent);
                }
                y = card.bottom();
            }
            y += 12.0;
            let r = Rect::from_min_size(Pos2::new(x0, y), Vec2::new(w, 59.8));
            if nav_row(p, r, t(&tr, "setup_earbuds_pair"), "", accent) { /* Desktop: Bluetooth settings. Not wired. */ }
        }
        4 => {
            y = heading(p, x0, y, w, t(&tr, "setup_bg_title"), t(&tr, "setup_bg_body")) + 12.0;
            let st = load_settings();
            let bg = st["background_service"].as_bool().unwrap_or(true);
            glass::flat_at(p, Rect::from_min_size(Pos2::new(x0, y), Vec2::new(w, 59.8 * 2.0 + 8.0)), phone::RADIUS, 1.0);
            if phone::row_at(p, "setup-bg", Rect::from_min_size(Pos2::new(x0, y), Vec2::new(w, 59.8)), phone::Row::new(t(&tr, "settings_background_title"), t(&tr, "settings_background_sub"), Trail::Switch(bg), accent), true).1 {
                save_setting("background_service", (!bg).into());
            }
            y += 59.8 + 8.0;
            pill_row(p, "setup-battery", Rect::from_min_size(Pos2::new(x0, y), Vec2::new(w, 59.8)), t(&tr, "setup_battery_title"), t(&tr, "setup_allow"), true, accent);
            y += 59.8 + phone::GAP;
            let up = st["update_auto"].as_bool().unwrap_or(true);
            let r = Rect::from_min_size(Pos2::new(x0, y), Vec2::new(w, 59.8));
            glass::flat_at(p, r, 16.0, 1.0);
            if phone::row_at(p, "setup-update", r, phone::Row::new(t(&tr, "update_auto_title"), t(&tr, "update_auto_sub"), Trail::Switch(up), accent), true).1 {
                save_setting("update_auto", (!up).into());
            }
        }
        5 => {
            y = heading(p, x0, y, w, t(&tr, "setup_widgets_title"), t(&tr, "setup_widgets_body")) + 12.0;
            for key in ["widget_name", "widget_pages_name"] {
                glass::flat_at(p, Rect::from_min_size(Pos2::new(x0, y), Vec2::new(w, 59.8)), 16.0, 1.0);
                glass::text(p, Pos2::new(x0 + 16.0, y + 29.9), Align2::LEFT_CENTER, t(&tr, key), 16.0, 1.0);
                let r = Rect::from_min_size(Pos2::new(x0 + w - 16.0 - 72.0, y + 11.9), Vec2::new(72.0, 36.0));
                if phone::pill(p, key, r, t(&tr, "setup_widgets_add"), true, accent) { /* Android only. Not wired. */ }
                y += 59.8 + 8.0;
            }
        }
        6 => {
            y = heading(p, x0, y, w, t(&tr, "setup_feedback_title"), t(&tr, "setup_feedback_body")) + 12.0;
            for (key, sub, page) in [("problem_title", "problem_sub", Page::Feedback), ("test_title", "test_sub", Page::Test)] {
                let r = Rect::from_min_size(Pos2::new(x0, y), Vec2::new(w, 59.8));
                if nav_row(p, r, t(&tr, key), t(&tr, sub), accent) { go = Some(page); }
                y += 59.8 + 8.0;
            }
        }
        7 => {
            y = heading(p, x0, y, w, t(&tr, "settings_issues_title"), t(&tr, "setup_issues_body")) + 12.0;
            let r = Rect::from_min_size(Pos2::new(x0, y), Vec2::new(w, 59.8));
            if nav_row(p, r, t(&tr, "settings_issues_title"), t(&tr, "settings_issues_sub"), accent) { go = Some(Page::Issues); }
        }
        8 => {
            y = heading(p, x0, y, w, t(&tr, "about_privacy_title"), t(&tr, "about_privacy_body")) + 12.0;
            phone::paragraph(p, Rect::from_min_size(Pos2::new(x0, y), Vec2::new(w, 60.0)), t(&tr, "about_privacy_reply"), 14.0, white(0.74), false);
            y += 68.0;
        }
        _ => {
            let icon = Rect::from_center_size(Pos2::new(x0 + w / 2.0, y + 66.0), Vec2::splat(120.0));
            glass::flat_at(p, Rect::from_center_size(icon.center(), Vec2::new(140.0, 66.0)), 33.0, 1.0);
            glass::icon_at(p, icons::LAUNCHER_FOREGROUND, icon, white(1.0));
            y += 132.0 + 20.0;
            heading(p, x0, y, w, t(&tr, "setup_ready_title"), t(&tr, "setup_ready_body"));
        }
    }

    // The fixed bar: Back / Skip, the dots, Next / Done. It sits at the body bottom edge, outside the page's clip.
    let mut bar = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(Pos2::new(body.left(), bar_y), Vec2::new(body.width(), BAR))));
    bar.set_clip_rect(ui.clip_rect());
    let p = &mut bar;
    let left = t(&tr, if page == 0 { "setup_skip" } else { "setup_back" });
    let right = t(&tr, if page == 9 { "welcome_done" } else { "setup_next" });
    let rw = (right.chars().count() as f32 * 8.5 + 36.0).max(72.0);
    let next = Rect::from_min_size(Pos2::new(x0 + w - rw, bar_y), Vec2::new(rw, BAR));
    let lw = (left.chars().count() as f32 * 8.5 + 24.0).max(44.0);
    let back = Rect::from_min_size(Pos2::new(x0, bar_y), Vec2::new(lw, BAR));
    if phone::pill(p, "setup-back", back, left, false, accent) {
        if page == 0 {
            save_setting("setup_done", true.into());
            app.page = Page::Overview;
            return bar_y + BAR;
        }
        app.setup.dir = -1.0;
        app.setup.page = page - 1;
    }
    if phone::pill(p, "setup-next", next, right, true, accent) {
        if page == 9 {
            save_setting("setup_done", true.into());
            app.page = Page::Overview;
            return bar_y + BAR;
        }
        app.setup.dir = 1.0;
        app.setup.page = page + 1;
    }
    // Dots: 8 x 8, the current 18 x 8, 3 dp apart.
    let total = 18.0 + 8.0 * 9.0 + 3.0 * 9.0;
    let mut dx = x0 + (w - total) / 2.0;
    for i in 0..10 {
        let wd = if i == page { 18.0 } else { 8.0 };
        let r = Rect::from_min_size(Pos2::new(dx, bar_y + BAR / 2.0 - 4.0), Vec2::new(wd, 8.0));
        if i == page { p.painter().rect_filled(r, 4.0, accent); } else { p.painter().rect_stroke(r, 4.0, egui::Stroke::new(1.0, white(0.35)), egui::StrokeKind::Inside); }
        dx += wd + 3.0;
    }
    if let Some(page) = go { app.page = page; }
    bar_y + BAR
}
