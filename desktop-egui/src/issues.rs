//! Known issues (`KnownIssuesActivity`, DESIGN §6.4 and §7.20): the limits of the earbuds and the computer, one card each.
//! The keys come from the model's `knownIssues` in `models.json` and from two checks on the rows, as on the phone.

use crate::glass;
use crate::phone;
use crate::session::Snapshot;
use crate::types;
use crate::{t, App};
use eframe::egui::{Align2, Pos2, Rect, Ui, Vec2};

/// The high-quality codecs, best first (`ModelCatalog.bestHiResCodec`).
const HIRES_CODECS: [&str; 6] = ["lhdc5", "lhdc", "ldac", "aptx-adaptive", "aptx-hd", "aptx"];
/// The phone's order.
const ALL: [&str; 4] = ["spatial_silent", "samsung_codec", "hires_spatial", "hires_no_codec"];

fn listed(s: &Snapshot, key: &str) -> bool {
    s.model.is_some_and(|m| m["knownIssues"].as_array().is_some_and(|k| k.iter().any(|x| x.as_str() == Some(key))))
}

/// The issues of the current model and computer, in the phone's order.
pub fn keys(s: &Snapshot) -> Vec<&'static str> {
    if s.model.is_none() { return Vec::new(); }
    let hires = types::has_feature(&s.features, types::FEATURE_HIRES) || crate::home_logic::codec_picker(s);
    let spatial = types::has_feature(&s.features, types::FEATURE_SPATIAL) || crate::home_logic::spatial_by_type(s);
    let mut out: Vec<&'static str> = Vec::new();
    if listed(s, "spatial_silent") { out.push("spatial_silent"); }
    // No Samsung phone runs this build, so `samsung_codec` never applies here.
    if hires && spatial && crate::home_logic::exclusive(s) { out.push("hires_spatial"); }
    let no_codec = s.model.is_some_and(|m| m["codecs"].as_array().is_some_and(|c| !c.iter().any(|x| x.as_str().is_some_and(|t| HIRES_CODECS.contains(&t)))));
    if hires && no_codec { out.push("hires_no_codec"); }
    out.retain(|k| ALL.contains(k));
    out
}

pub fn show(app: &App, ui: &mut Ui, body: Rect) -> f32 {
    let tr = &app.tr;
    let w = phone::content(body);
    let x0 = body.left();
    // The title box ends at the body top; the first card sits 8 dp below it (the phone's margin).
    let mut y = body.top() + 8.0;
    let issues = keys(&app.snap);
    if issues.is_empty() {
        let text = t(tr, "issues_none");
        let h = phone::wrap_height(ui, text, 15.0, w - 8.0);
        phone::paragraph(ui, Rect::from_min_size(Pos2::new(x0 + 4.0, y + 16.0), Vec2::new(w - 8.0, h)), text, 15.0, glass::white(0.74), false);
        return y + 16.0 + h + phone::GAP;
    }
    for key in issues {
        let title = t(tr, &format!("issue_{key}_title"));
        let body = t(tr, &format!("issue_{key}_body"));
        let body_h = phone::wrap_height(ui, body, 14.0, w - 32.0);
        let h = 14.0 + 23.2 + 2.0 + body_h + 14.0;
        let card = Rect::from_min_size(Pos2::new(x0, y), Vec2::new(w, h));
        glass::flat_at(ui, card, phone::RADIUS, 1.0);
        glass::text(ui, Pos2::new(x0 + phone::MARGIN, y + 14.0 + 11.6), Align2::LEFT_CENTER, title, 16.0, 1.0);
        phone::paragraph(ui, Rect::from_min_size(Pos2::new(x0 + phone::MARGIN, y + 39.2), Vec2::new(w - 32.0, body_h)), body, 14.0, glass::white(0.74), false);
        y += h + 8.0;
    }
    y + phone::GAP
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unknown_model_has_no_issues() {
        assert!(keys(&Snapshot::default()).is_empty());
    }

    /// Hi-Res and 3D audio on a model that lists both: the phone's `hires_spatial` rule.
    #[test]
    fn hires_and_spatial_is_one_issue() {
        let model = types::models().iter().find(|m| m["knownIssues"].as_array().is_some_and(|k| !k.is_empty()));
        let Some(model) = model else { return };
        let s = Snapshot { model: Some(model), features: vec![(types::FEATURE_HIRES, 0), (types::FEATURE_SPATIAL, 0)], ..Default::default() };
        let keys = keys(&s);
        assert!(!keys.contains(&"samsung_codec"), "not an Android phone");
        assert!(keys.contains(&"hires_spatial"), "one switch turns the other off");
    }

    #[test]
    fn realme_air7_pro_lists_its_documented_silent_spatial_issue() {
        let model = types::models().iter().find(|m| m["id"].as_str() == Some("064C12"));
        let Some(model) = model else { panic!("Air 7 Pro profile is missing") };
        let s = Snapshot { model: Some(model), features: vec![(types::FEATURE_SPATIAL, 1)], ..Default::default() };
        assert_eq!(keys(&s), vec!["spatial_silent"]);
    }
}
