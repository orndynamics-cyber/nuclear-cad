//! The step dragged manipulators snap to: the arrows of Extrude and the other dialogs, a pull on
//! a face, the fillet radius handles and the move triad. The step menu beside the browser's title
//! chooses how it is found (`Prefs::manipulator_step`):
//! - Automatic follows the zoom where the arrow is, and stays fine enough for the size of the
//!   part being worked on, so a small part seen from far away still moves in small steps.
//! - Zoom follows the zoom alone, as before the menu.
//! - Fixed keeps one step (`Prefs::manipulator_step_mm`) whatever the view, and a pull on a face
//!   no longer changes it with sideways moves.
//!
//! The angle step (`Prefs::manipulator_angle_deg`) applies in every mode. Steps are physical
//! millimetres and degrees; the design's display units only change how they are written.
//!
//! The canvas and the gizmo do not carry `&SolveApp`, so the viewport notes the choice once per
//! frame ([`set_frame`]), the same way as [`crate::tool_modifiers::set_manipulator_free`].

use std::cell::{Cell, RefCell};

use solidcraft_engine::Sel;
use solidcraft_engine::geom::{Aabb3, Vec3};

use crate::SolveApp;
use crate::prefs::StepMode;
use crate::viewport::Proj;

/// Fixed steps the menu offers (mm), finest first.
pub const LINEAR_PRESETS: [f64; 7] = [0.01, 0.1, 0.5, 1.0, 5.0, 10.0, 50.0];
/// Angle steps the menu offers (degrees).
pub const ANGLE_PRESETS: [f64; 5] = [0.5, 1.0, 5.0, 15.0, 45.0];
/// The finest and coarsest steps a typed or saved value is brought into (mm).
pub const LINEAR_RANGE: std::ops::RangeInclusive<f64> = 1e-4..=1e4;
/// The finest and coarsest angle steps (degrees).
pub const ANGLE_RANGE: std::ops::RangeInclusive<f64> = 0.01..=180.0;
/// In Automatic, the step is at most this share of the part's largest extent.
const PART_SHARE: f64 = 1.0 / 20.0;
/// The view's height over the step: the Zoom step is a fortieth of the half height.
const VIEW_SHARE: f64 = 80.0;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Frame {
    mode: StepMode,
    fixed: f64,
    angle: f64,
    /// The largest extent of what is being worked on (0 when unknown).
    size: f64,
}

impl Default for Frame {
    fn default() -> Self {
        Self { mode: StepMode::Zoom, fixed: 1.0, angle: 5.0, size: 0.0 }
    }
}

type SizeCache = Option<(usize, Vec<Sel>, f64)>;

thread_local! {
    static FRAME: Cell<Frame> = Cell::new(Frame::default());
    /// The part size for a world state and selection, so a frame with a dialog open does not
    /// walk the selection again.
    static SIZE: RefCell<SizeCache> = const { RefCell::new(None) };
}

/// A step brought into range; anything that is not a usable number reads as `default`.
fn clamp(v: f64, range: &std::ops::RangeInclusive<f64>, default: f64) -> f64 {
    if v.is_finite() { v.clamp(*range.start(), *range.end()) } else { default }
}

/// The step a length setting asks for, in range.
pub fn fixed_mm(app: &SolveApp) -> f64 {
    clamp(app.preferences.manipulator_step_mm, &LINEAR_RANGE, 1.0)
}

/// The angle step the setting asks for, in range (degrees).
pub fn angle_setting(app: &SolveApp) -> f64 {
    clamp(app.preferences.manipulator_angle_deg, &ANGLE_RANGE, 5.0)
}

/// Note this frame's choice. Call from the viewport before the canvas and gizmos draw.
pub fn set_frame(app: &SolveApp) {
    let mode = app.preferences.manipulator_step;
    let size = if mode == StepMode::Automatic { part_size(app) } else { 0.0 };
    FRAME.with(|f| f.set(Frame { mode, fixed: fixed_mm(app), angle: angle_setting(app), size }));
}

/// The largest extent of what a manipulator works on: the selection while a tool is open,
/// otherwise the whole model.
fn part_size(app: &SolveApp) -> f64 {
    let busy = app.dialog.is_some() || app.tree.occurrence_move.is_some() || crate::drag_extrude::pressed(app);
    if !busy {
        return 0.0;
    }
    let st = crate::open_meshing::shown_world(app);
    let key = std::sync::Arc::as_ptr(&st) as usize;
    let cached = SIZE.with(|c| c.borrow().as_ref().filter(|(k, sel, _)| *k == key && *sel == app.session.selection).map(|(_, _, s)| *s));
    if let Some(s) = cached {
        return s;
    }
    let b = crate::viewport::selection_bounds(app).filter(|b| !b.is_empty()).unwrap_or_else(|| crate::viewport::model_bounds(app));
    let s = extent(&b);
    SIZE.with(|c| *c.borrow_mut() = Some((key, app.session.selection.clone(), s)));
    s
}

/// The largest side of a box; 0 for an empty or broken one.
fn extent(b: &Aabb3) -> f64 {
    if b.is_empty() {
        return 0.0;
    }
    let d = b.max - b.min;
    let s = d.x.max(d.y).max(d.z);
    if s.is_finite() { s.max(0.0) } else { 0.0 }
}

/// The nearest round step at or below `raw`: 1, 2 or 5 times a power of ten.
pub fn round_step(raw: f64) -> f64 {
    let raw = if raw.is_finite() { raw.max(1e-4) } else { 1.0 };
    let p = 10f64.powf(raw.log10().floor());
    if raw / p < 2.0 {
        p
    } else if raw / p < 5.0 {
        2.0 * p
    } else {
        5.0 * p
    }
}

/// `v` on the nearest whole number of steps, without the binary noise of the multiplication
/// (152 × 0.1 reads 15.2, not 15.200000000000001). Steps are counted from where the value is
/// measured from, so they never pile up rounding from frame to frame.
pub fn snap(v: f64, step: f64) -> f64 {
    if !(step > 0.0 && step.is_finite() && v.is_finite()) {
        return v;
    }
    let r = (v / step).round() * step;
    let clean = (r * 1e9).round() / 1e9;
    if clean.is_finite() { clean } else { r }
}

/// Automatic's step from the zoom at the arrow (`view`, in mm before rounding) and the part's
/// largest extent (0 when unknown): the zoom's step, but never coarser than a twentieth of the
/// part.
pub fn automatic(view: f64, size: f64) -> f64 {
    let raw = if size > 0.0 && size.is_finite() { view.min(size * PART_SHARE) } else { view };
    round_step(raw)
}

/// Millimetres the view shows per screen point at `at`, times the view's height over 80: the
/// half height over 40 in a parallel view (the old step), and deeper points coarser in a
/// perspective one.
fn view_raw(proj: &Proj, at: Vec3) -> Option<f64> {
    let mm = crate::gizmo::mm_per_px(proj, at)?;
    let raw = mm * f64::from(proj.rect.height()) / VIEW_SHARE;
    (raw.is_finite() && raw > 0.0).then_some(raw)
}

/// The step for a drag at `at` this frame (mm).
pub fn linear(proj: &Proj, at: Vec3) -> f64 {
    let f = FRAME.with(Cell::get);
    let zoom = || round_step(proj.cam.half_height() / 40.0);
    match f.mode {
        StepMode::Fixed => f.fixed,
        StepMode::Zoom => zoom(),
        StepMode::Automatic => view_raw(proj, at).map_or_else(zoom, |v| automatic(v, f.size)),
    }
}

#[cfg(test)]
/// The step for a drag this frame when no point is known (mm).
fn linear_for_view(half_height: f64) -> f64 {
    let f = FRAME.with(Cell::get);
    match f.mode {
        StepMode::Fixed => f.fixed,
        StepMode::Zoom => round_step(half_height / 40.0),
        StepMode::Automatic => automatic(half_height / 40.0, f.size),
    }
}

/// Whether the step is fixed this frame: sideways moves during a pull leave it alone.
pub fn fixed() -> bool {
    FRAME.with(Cell::get).mode == StepMode::Fixed
}

/// The angle step this frame (degrees).
pub fn angle_deg() -> f64 {
    FRAME.with(Cell::get).angle
}

/// The menu's button text: the mode, or the fixed step written in the design's units.
pub fn summary(app: &SolveApp) -> String {
    match app.preferences.manipulator_step {
        StepMode::Automatic => app.tr("ui.step_auto").to_string(),
        StepMode::Zoom => app.tr("ui.step_zoom").to_string(),
        StepMode::Fixed => crate::canvas::format_len(&app.session.doc, fixed_mm(app)),
    }
}

/// The step menu beside the browser's title: Automatic, Zoom, or a fixed step, and the angle
/// step. A choice is a preference, kept with the others.
pub fn menu(app: &mut SolveApp, ui: &mut egui::Ui) {
    use crate::ui_kit::{Menu, MenuItem};
    let label = crate::i18n::format("ui.step_button", &[("step", &summary(app))]);
    let r = Menu::text(ui, &label, |ui| {
        // Rows fill the popup: keep it as narrow as a context menu.
        ui.set_width(crate::context_menu::LIST_W);
        Menu::heading(ui, app.tr("ui.step_heading_length"));
        for (id, mode, key) in [("step:auto", StepMode::Automatic, "ui.step_auto_long"), ("step:zoom", StepMode::Zoom, "ui.step_zoom_long")] {
            let text = app.tr(key).to_string();
            let item = MenuItem { checked: Some(app.preferences.manipulator_step == mode), ..MenuItem::new(id, &text, "") };
            if item.show(ui).clicked() {
                app.preferences.manipulator_step = mode;
                ui.close();
            }
        }
        Menu::separator(ui);
        let current = fixed_mm(app);
        for mm in LINEAR_PRESETS {
            let text = crate::canvas::format_len(&app.session.doc, mm);
            let id = format!("step:{mm}");
            let on = app.preferences.manipulator_step == StepMode::Fixed && (current - mm).abs() < 1e-9;
            if (MenuItem { checked: Some(on), ..MenuItem::new(&id, &text, "") }).show(ui).clicked() {
                app.preferences.manipulator_step = StepMode::Fixed;
                app.preferences.manipulator_step_mm = mm;
                ui.close();
            }
        }
        let mut custom = current;
        let text = app.tr("ui.step_custom").to_string();
        let response = crate::ui_kit::row(ui, &text, |ui| crate::ui_kit::number_field(ui, &mut custom, LINEAR_RANGE, 0.01, " mm"));
        crate::scenario::publish_target("step:custom", response.rect);
        if response.changed() {
            app.preferences.manipulator_step = StepMode::Fixed;
            app.preferences.manipulator_step_mm = clamp(custom, &LINEAR_RANGE, 1.0);
        }
        Menu::separator(ui);
        Menu::heading(ui, app.tr("ui.step_heading_angle"));
        let angle = angle_setting(app);
        for deg in ANGLE_PRESETS {
            let text = format!("{deg}°");
            let id = format!("step:angle:{deg}");
            if (MenuItem { checked: Some((angle - deg).abs() < 1e-9), ..MenuItem::new(&id, &text, "") }).show(ui).clicked() {
                app.preferences.manipulator_angle_deg = deg;
                ui.close();
            }
        }
    });
    crate::scenario::publish_target("browser:step", r.response.rect);
    let tip = app.tr("ui.step_tip").to_string();
    let _ = r.response.on_hover_text(tip);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_steps_are_one_two_or_five_times_a_power_of_ten() {
        assert_eq!(round_step(1.0), 1.0);
        assert_eq!(round_step(2.5), 2.0);
        assert_eq!(round_step(9.9), 5.0);
        assert!((round_step(0.13) - 0.1).abs() < 1e-12);
        assert_eq!(round_step(f64::NAN), 1.0);
        assert_eq!(round_step(-3.0), 1e-4);
    }

    #[test]
    fn snapped_values_are_whole_steps_without_binary_noise() {
        assert_eq!(snap(15.234, 0.1), 15.2);
        assert_eq!(snap(-0.26, 0.1), -0.3);
        assert_eq!(snap(13.1, 5.0), 15.0);
        assert_eq!(snap(0.004, 0.01), 0.0);
        assert_eq!(snap(2.0, 0.0), 2.0);
        assert!(snap(f64::NAN, 1.0).is_nan());
    }

    #[test]
    fn automatic_follows_the_zoom_but_stays_fine_for_a_small_part() {
        // A large part, or none known: the zoom decides, as before.
        assert_eq!(automatic(1.0, 1000.0), 1.0);
        assert_eq!(automatic(1.0, 0.0), 1.0);
        assert_eq!(automatic(1.0, f64::NAN), 1.0);
        // A 4 mm part seen from far away: a fifth of a millimetre, not a whole one.
        assert!((automatic(1.0, 4.0) - 0.2).abs() < 1e-12);
        // Closer in, the zoom is finer than the part's share and wins again.
        assert!((automatic(0.05, 4.0) - 0.05).abs() < 1e-12);
    }

    #[test]
    fn fixed_and_angle_settings_are_brought_into_range() {
        assert_eq!(clamp(f64::NAN, &LINEAR_RANGE, 1.0), 1.0);
        assert_eq!(clamp(-5.0, &LINEAR_RANGE, 1.0), 1e-4);
        assert_eq!(clamp(1e9, &ANGLE_RANGE, 5.0), 180.0);
        assert_eq!(clamp(0.1, &LINEAR_RANGE, 1.0), 0.1);
    }

    fn extruded() -> crate::scenario::Harness {
        use serde_json::json;
        let mut h = crate::scenario::Harness::new();
        h.app.run("sketch.create", json!({"plane":"XY","name":"Base"})).unwrap();
        h.app.run("sketch.rectangle.two_point", json!({"p0":[0,0],"p1":[10,10]})).unwrap();
        h.app.run("sketch.finish", json!({})).unwrap();
        h.app.run("solid.extrude", json!({"sketch":"Base","distance":"10 mm","operation":"new","name":"Depth"})).unwrap();
        h.frames(4);
        h.step(&json!({"call":"ui.editFeature","params":{"feature":"Depth"}})).unwrap();
        h.frames(8);
        h
    }

    fn distance(h: &crate::scenario::Harness) -> String {
        let Some(crate::dialogs::Dialog { kind: crate::dialogs::Kind::Extrude { distance, .. }, .. }) = &h.app.dialog else {
            panic!("extrude dialog")
        };
        distance.clone()
    }

    #[test]
    fn the_old_zoom_step_stays_behind_the_zoom_choice() {
        assert_eq!(crate::prefs::Prefs::default().manipulator_step, StepMode::Automatic);
        for half_height in [4.0, 40.0, 100.0, 3000.0] {
            FRAME.with(|f| f.set(Frame { mode: StepMode::Zoom, ..Frame::default() }));
            assert_eq!(linear_for_view(half_height), crate::canvas::snap_step(half_height));
        }
    }

    #[test]
    fn a_fixed_step_locks_the_extrude_arrow_whatever_the_zoom() {
        use serde_json::json;
        let mut h = extruded();
        h.app.preferences.manipulator_step = StepMode::Fixed;
        h.app.preferences.manipulator_step_mm = 0.1;
        h.frames(2);
        h.step(&json!({"drag":[{"handle":"arrow"},{"world":[5,5,15.234]}]})).unwrap();
        assert_eq!(distance(&h), "15.2 mm");
        h.app.preferences.manipulator_step_mm = 5.0;
        h.frames(2);
        h.step(&json!({"drag":[{"handle":"arrow"},{"world":[5,5,13.1]}]})).unwrap();
        assert_eq!(distance(&h), "15 mm");
    }

    #[test]
    fn the_menu_beside_the_browser_title_sets_the_step() {
        use serde_json::json;
        let mut h = crate::scenario::Harness::new();
        h.frames(2);
        h.step(&json!({"click":{"handle":"browser:step"}})).unwrap();
        h.frames(2);
        h.step(&json!({"click":{"handle":"menu:step:0.1"}})).unwrap();
        h.frames(2);
        assert_eq!(h.app.preferences.manipulator_step, StepMode::Fixed);
        assert_eq!(h.app.preferences.manipulator_step_mm, 0.1);
        h.step(&json!({"click":{"handle":"browser:step"}})).unwrap();
        h.frames(2);
        h.step(&json!({"click":{"handle":"menu:step:angle:15"}})).unwrap();
        h.frames(2);
        assert_eq!(h.app.preferences.manipulator_angle_deg, 15.0);
        h.step(&json!({"click":{"handle":"browser:step"}})).unwrap();
        h.frames(2);
        h.step(&json!({"click":{"handle":"menu:step:auto"}})).unwrap();
        h.frames(2);
        assert_eq!(h.app.preferences.manipulator_step, StepMode::Automatic);
    }
}
