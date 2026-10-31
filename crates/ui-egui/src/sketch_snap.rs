//! Reference acquisition and inference guides while a sketch drawing tool places points.
//!
//! Acquisition: near a meaningful place of the existing geometry (a point, a midpoint, a
//! crossing of two curves, a tangent or perpendicular from the line being drawn, or the foot on
//! a curve) the cursor sticks to it, and the click keeps the relation (midpoint, point on the
//! curve, tangent, perpendicular). Guides: a point, midpoint or crossing the cursor rested on is
//! tracked; moving away from it along one of eight directions (every 45°) puts the cursor on a
//! dashed ray from it, or on the crossing of two such rays. A guide only places the point: the
//! point it gives is a plain one, with no constraint.
//!
//! Everything works on the sketch's analytic curves (lines, circles, arcs) in sketch
//! coordinates, never on the display tessellation. Every radius is in screen pixels (`ppu`
//! converts), so the feel is the same at any zoom, and the order is stable: kind, then distance,
//! then key.

use std::cell::RefCell;

use egui::{Pos2, Stroke};
use solidcraft_engine::geom::Vec2;
use solidcraft_engine::sketch::{CurveKind, Shape, Sketch, intersections_within};

use crate::SolveApp;
use crate::theme::Tokens;
use crate::tools::Kind as ToolKind;
use crate::viewport::Proj;

/// Capture and release radius of a snap and of a guide (screen pixels).
pub const CAPTURE_PX: f64 = 8.0;
/// Curves near the cursor considered for a snap, so a hostile sketch cannot stall the pointer.
const MAX_CURVES: usize = 256;
/// How far a crossing may stray from both curves and still be on them (sketch units).
const ON_CURVE: f64 = 1e-4;
/// A virtual place this close to an existing point is that point (sketch units).
const SAME_POINT: f64 = 1e-6;
/// Tracked points kept, newest first.
const MAX_TRACKED: usize = 2;

/// What kind of place a snap is, best first: the derived order is the priority.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SnapKind {
    Point,
    Midpoint,
    Intersection,
    Tangent,
    Perpendicular,
    OnCurve,
}

impl SnapKind {
    /// Whether resting on it tracks the place for guides.
    pub fn tracks(self) -> bool {
        matches!(self, SnapKind::Point | SnapKind::Midpoint | SnapKind::Intersection)
    }
}

/// The finer role of a snap, which picks its glyph.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Endpoint,
    Center,
    Origin,
    Point,
    Midpoint,
    Intersection,
    Tangent,
    Perpendicular,
    OnLine,
    OnCurve,
}

/// A relation between the line being drawn and an existing curve, kept once the line exists.
#[derive(Clone, Debug, PartialEq)]
pub enum LineRelation {
    Tangent(String),
    Perpendicular(String),
}

/// A place the cursor can stick to.
#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    pub kind: SnapKind,
    pub role: Role,
    pub at: Vec2,
    /// Screen distance from the cursor (pixels).
    pub px: f64,
    /// Stable identity across frames (hysteresis, ties, tracking).
    pub key: String,
    /// The point argument the drawing commands take: an existing point's id, or a snap form
    /// (`mid:<line>`, `on:<curve>:x,y`, `x:<a>+<b>:x,y`) that makes the point with its relation.
    pub arg: Option<String>,
    /// Tangent and perpendicular: the relation the new line keeps with the curve.
    pub relation: Option<LineRelation>,
}

fn on(id: &str, p: Vec2) -> String {
    format!("on:{id}:{},{}", p.x, p.y)
}

/// The eight guide directions, exact on the axes so a horizontal guide keeps the very same y.
const DIRECTIONS: [Vec2; 8] = {
    let s = std::f64::consts::FRAC_1_SQRT_2;
    [
        Vec2::new(1.0, 0.0),
        Vec2::new(s, s),
        Vec2::new(0.0, 1.0),
        Vec2::new(-s, s),
        Vec2::new(-1.0, 0.0),
        Vec2::new(-s, -s),
        Vec2::new(0.0, -1.0),
        Vec2::new(s, -s),
    ]
};

/// Every place within [`CAPTURE_PX`] of the sketch point `lp`. `anchor`: the start of the line
/// being drawn (Line tool after its first click), for tangents and perpendiculars.
pub fn candidates(sk: &Sketch, lp: Vec2, ppu: f64, anchor: Option<Vec2>) -> Vec<Candidate> {
    let mut out = Vec::new();
    if !lp.is_finite() || !ppu.is_finite() || ppu <= 0.0 {
        return out;
    }
    let reach = CAPTURE_PX / ppu;
    let px = |p: Vec2| p.dist(lp) * ppu;
    let visible = solidcraft_engine::view::SketchPointVisibility::new(sk);
    let points: Vec<Vec2> = sk.points.iter().filter(|p| visible.takes(p)).map(|p| p.pos).filter(|p| p.is_finite()).collect();
    let occupied = |q: Vec2| points.iter().any(|p| p.dist(q) <= SAME_POINT);

    // Real points, with the role their curves give them.
    let mut role = vec![Role::Point; sk.points.len()];
    for curve in &sk.curves {
        if let Some((a, b)) = curve.kind.ends() {
            for i in [a, b] {
                if let Some(r) = role.get_mut(i)
                    && *r == Role::Point
                {
                    *r = Role::Endpoint;
                }
            }
        }
        if let CurveKind::Circle { c, .. } | CurveKind::Arc { c, .. } = curve.kind
            && let Some(r) = role.get_mut(c)
        {
            *r = Role::Center;
        }
    }
    for (i, p) in sk.points.iter().enumerate() {
        if !visible.takes(p) || !p.pos.is_finite() || px(p.pos) > CAPTURE_PX {
            continue;
        }
        let role = if p.id == "origin" { Role::Origin } else { role.get(i).copied().unwrap_or(Role::Point) };
        out.push(Candidate {
            kind: SnapKind::Point,
            role,
            at: p.pos,
            px: px(p.pos),
            key: format!("p:{}", p.id),
            arg: Some(p.id.clone()),
            relation: None,
        });
    }

    // Every candidate lies on a curve, so only curves within reach can offer one.
    let mut near: Vec<(&str, Shape, bool)> = Vec::new();
    for (i, curve) in sk.curves.iter().enumerate() {
        if near.len() >= MAX_CURVES {
            break;
        }
        if !solidcraft_engine::view::sketch_curve_visible(sk, i) {
            continue;
        }
        let Some(sh) = sk.shape(i) else { continue };
        if sh.dist(lp) <= reach {
            near.push((curve.id.as_str(), sh, curve.infinite));
        }
    }
    let virtual_at = |out: &mut Vec<Candidate>, c: Candidate| {
        if c.at.is_finite() && c.px <= CAPTURE_PX && !occupied(c.at) {
            out.push(c);
        }
    };

    for (id, sh, infinite) in &near {
        // Midpoints of lines and arcs.
        let mid = match *sh {
            Shape::Line { a, b } if !infinite => Some(((a + b) * 0.5, format!("mid:{id}"))),
            Shape::Round { c, r, start, sweep } if sweep < std::f64::consts::TAU - 1e-9 => {
                let m = c + Vec2::from_angle(start + sweep * 0.5) * r;
                // The commands hold a midpoint of a line; an arc's keeps the point on the arc.
                Some((m, on(id, m)))
            }
            _ => None,
        };
        if let Some((m, arg)) = mid {
            let c = Candidate {
                kind: SnapKind::Midpoint,
                role: Role::Midpoint,
                at: m,
                px: px(m),
                key: format!("m:{id}"),
                arg: Some(arg),
                relation: None,
            };
            virtual_at(&mut out, c);
        }
        // The foot on the curve.
        let foot = sh.project(lp);
        let role = if matches!(sh, Shape::Line { .. }) { Role::OnLine } else { Role::OnCurve };
        let c = Candidate { kind: SnapKind::OnCurve, role, at: foot, px: px(foot), key: format!("o:{id}"), arg: Some(on(id, foot)), relation: None };
        virtual_at(&mut out, c);
    }

    // Crossings of two curves, on both of them as drawn (trimmed).
    for (i, (ia, sa, _)) in near.iter().enumerate() {
        for (ib, sb, _) in near.iter().skip(i + 1) {
            for (k, q) in intersections_within(sa, sb, 1e-6).into_iter().enumerate() {
                if !q.is_finite() || sa.dist(q) > ON_CURVE || sb.dist(q) > ON_CURVE {
                    continue;
                }
                let c = Candidate {
                    kind: SnapKind::Intersection,
                    role: Role::Intersection,
                    at: q,
                    px: px(q),
                    key: format!("x:{ia}+{ib}:{k}"),
                    arg: Some(format!("x:{ia}+{ib}:{},{}", q.x, q.y)),
                    relation: None,
                };
                virtual_at(&mut out, c);
            }
        }
    }

    // From the start of the line being drawn: tangents to circles and arcs, perpendiculars to lines.
    if let Some(anchor) = anchor.filter(|a| a.is_finite()) {
        for (id, sh, infinite) in &near {
            match *sh {
                Shape::Round { c, r, .. } => {
                    let d = anchor.dist(c);
                    if r <= 1e-12 || d <= r + 1e-9 {
                        continue;
                    }
                    let base = (anchor - c).angle();
                    let half = (r / d).acos();
                    for (branch, s) in [-1.0, 1.0].into_iter().enumerate() {
                        let t = c + Vec2::from_angle(base + s * half) * r;
                        if sh.dist(t) > 1e-6 {
                            continue;
                        }
                        let p = px(t);
                        if t.is_finite() && p <= CAPTURE_PX {
                            out.push(Candidate {
                                kind: SnapKind::Tangent,
                                role: Role::Tangent,
                                at: t,
                                px: p,
                                key: format!("t:{id}:{branch}"),
                                arg: Some(on(id, t)),
                                relation: Some(LineRelation::Tangent(id.to_string())),
                            });
                        }
                    }
                }
                Shape::Line { a, b } => {
                    let d = b - a;
                    let len2 = d.len2();
                    if len2 <= 1e-18 {
                        continue;
                    }
                    let t = (anchor - a).dot(d) / len2;
                    let f = a + d * t;
                    let inside = *infinite || (t > 1e-9 && t < 1.0 - 1e-9);
                    let p = px(f);
                    if inside && f.is_finite() && f.dist(anchor) > 1e-9 && p <= CAPTURE_PX {
                        out.push(Candidate {
                            kind: SnapKind::Perpendicular,
                            role: Role::Perpendicular,
                            at: f,
                            px: p,
                            key: format!("n:{id}"),
                            arg: Some(on(id, f)),
                            relation: Some(LineRelation::Perpendicular(id.to_string())),
                        });
                    }
                }
            }
        }
    }
    out
}

/// The candidate to show: the best by kind, then distance, then key; the previous frame's
/// (`previous`, its key) is kept while it is still within reach and nothing of a better kind is.
pub fn choose(mut all: Vec<Candidate>, previous: Option<&str>) -> Option<Candidate> {
    all.sort_by(|a, b| a.kind.cmp(&b.kind).then(a.px.total_cmp(&b.px)).then_with(|| a.key.cmp(&b.key)));
    let best_kind = all.first()?.kind;
    if let Some(prev) = previous
        && let Some(i) = all.iter().position(|c| c.key == prev && c.kind <= best_kind)
    {
        return Some(all.swap_remove(i));
    }
    all.into_iter().next()
}

/// A place given by guides: where the cursor goes and the rays shown (tracked point → there).
#[derive(Clone, Debug, PartialEq)]
pub struct GuideSnap {
    pub at: Vec2,
    pub lines: Vec<(Vec2, Vec2)>,
}

/// The guide nearest the cursor from the tracked points (newest first): the crossing of two
/// guides near the cursor, else the one ray the cursor is closest to.
pub fn guide(tracked: &[Vec2], lp: Vec2, ppu: f64) -> Option<GuideSnap> {
    if !lp.is_finite() || !ppu.is_finite() || ppu <= 0.0 {
        return None;
    }
    // A ray counts only once the cursor has left its point (closer is the point's own snap).
    let away = |t: Vec2, q: Vec2| q.dist(t) * ppu >= CAPTURE_PX;
    if let [t1, t2, ..] = tracked {
        let mut best: Option<(f64, Vec2)> = None;
        for u1 in DIRECTIONS {
            for u2 in DIRECTIONS {
                let den = u1.cross(u2);
                if den.abs() < 1e-9 {
                    continue;
                }
                let w = *t2 - *t1;
                let (s, t) = (w.cross(u2) / den, w.cross(u1) / den);
                // Snap the axis-aligned coordinates exactly onto their points.
                let mut x = *t1 + u1 * s;
                if u1.y == 0.0 {
                    x.y = t1.y;
                } else if u1.x == 0.0 {
                    x.x = t1.x;
                }
                if u2.y == 0.0 {
                    x.y = t2.y;
                } else if u2.x == 0.0 {
                    x.x = t2.x;
                }
                if s <= 0.0 || t <= 0.0 || !x.is_finite() || !away(*t1, x) || !away(*t2, x) {
                    continue;
                }
                let d = x.dist(lp) * ppu;
                if d <= CAPTURE_PX * std::f64::consts::SQRT_2 && best.is_none_or(|b| d < b.0) {
                    best = Some((d, x));
                }
            }
        }
        if let Some((_, x)) = best {
            return Some(GuideSnap { at: x, lines: vec![(*t1, x), (*t2, x)] });
        }
    }
    let mut best: Option<(f64, Vec2, Vec2)> = None;
    for t in tracked {
        let d = lp - *t;
        if !away(*t, lp) {
            continue;
        }
        for u in DIRECTIONS {
            let along = d.dot(u);
            let off = d.cross(u).abs() * ppu;
            if along <= 0.0 || off > CAPTURE_PX || best.is_some_and(|b| off >= b.0) {
                continue;
            }
            let mut f = *t + u * along;
            if u.y == 0.0 {
                f.y = t.y;
            } else if u.x == 0.0 {
                f.x = t.x;
            }
            best = Some((off, *t, f));
        }
    }
    best.map(|(_, t, f)| GuideSnap { at: f, lines: vec![(t, f)] })
}

/// A tracked place: its key (no duplicates) and where it is.
#[derive(Clone, Debug, PartialEq)]
pub struct Tracked {
    pub key: String,
    pub at: Vec2,
}

/// Rest on `c`: a point, midpoint or crossing becomes the newest tracked place.
pub fn track(tracked: &mut Vec<Tracked>, c: &Candidate) {
    if !c.kind.tracks() {
        return;
    }
    tracked.retain(|t| t.key != c.key);
    tracked.insert(0, Tracked { key: c.key.clone(), at: c.at });
    tracked.truncate(MAX_TRACKED);
}

/// What the viewport shows for the last snap.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shown {
    pub snap: Option<(Role, Vec2)>,
    pub guide: Option<GuideSnap>,
}

#[derive(Default)]
struct State {
    sketch: Option<u64>,
    /// The drawing tool these belong to, and its line's start (Line tool after its first click).
    tool: Option<&'static str>,
    anchor: Option<Vec2>,
    last: Option<String>,
    tracked: Vec<Tracked>,
    shown: Shown,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// Tools that place points on the sketch plane (Line, Rectangle, Circle, Arc, Spline, Point,
/// Text and the other point-placing ones).
pub fn drawing_tool(tool: &crate::tools::Tool) -> bool {
    match tool.kind {
        ToolKind::Draw(_) => tool.cmd != "sketch.arc3d",
        ToolKind::Ext => crate::sketch_tools::takes_points(tool.cmd),
        _ => false,
    }
}

/// The tracked places, newest first.
#[cfg(test)]
pub fn tracked() -> Vec<Tracked> {
    STATE.with(|s| s.borrow().tracked.clone())
}

/// What the last snap showed.
pub fn shown() -> Shown {
    STATE.with(|s| s.borrow().shown.clone())
}

fn reset() {
    STATE.with(|s| *s.borrow_mut() = State::default());
}

/// Whether this system places the active tool's points. A click takes the tool out of the app
/// while it runs, so the last hover's tool is kept until the next frame.
fn engaged(app: &SolveApp) -> bool {
    if !app.preferences.sketch_reference_snap || crate::tool_modifiers::target_midpoint() {
        return false;
    }
    let sketch = app.session.active_sketch;
    match app.tool.as_ref() {
        Some(tool) if drawing_tool(tool) => {
            let anchor = (tool.cmd == "sketch.line").then(|| tool.pts.last().map(|p| p.0)).flatten();
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                if s.tool != Some(tool.cmd) || s.sketch != sketch {
                    *s = State { tool: Some(tool.cmd), sketch, ..State::default() };
                }
                s.anchor = anchor;
            });
            true
        }
        Some(_) => false,
        None => STATE.with(|s| s.borrow().tool.is_some() && s.borrow().sketch == sketch),
    }
}

/// Snap the sketch point `lp` (under screen position `pos`) for a drawing tool: the point and
/// its point argument. None: this system does not place this tool's points (another tool, Shift
/// midpoint targeting, or the preference is off), and the classic snap does.
pub fn snap_at(
    app: &SolveApp,
    proj: &Proj,
    sk: &Sketch,
    plane: &solidcraft_engine::geom::Plane,
    pos: Pos2,
    lp: Vec2,
) -> Option<(Vec2, Option<String>)> {
    if !engaged(app) {
        return None;
    }
    crate::inference::clear();
    crate::sketch_tools::clear_snap_hint();
    // Ctrl/Cmd (the suppress-snap chord): no snap and no guide at all.
    if crate::drag_snap::suppressed() {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.last = None;
            s.shown = Shown::default();
        });
        return Some((lp, None));
    }
    let (a, b) = (proj.to_screen(plane.to_world(lp))?, proj.to_screen(plane.to_world(lp + Vec2::X))?);
    let ppu = f64::from(a.distance(b));
    let (anchor, last) = STATE.with(|s| (s.borrow().anchor, s.borrow().last.clone()));
    let mut chosen = choose(candidates(sk, lp, ppu, anchor), last.as_deref());
    // A woken centre (its circle's edge was hovered) reaches further when nothing else is near.
    if chosen.is_none()
        && let Some(sid) = app.session.active_sketch
        && let Some((_, id)) = app.viewport.woken_centre.as_ref().filter(|(k, _)| *k == sid)
        && let Some(p) = sk.points.iter().find(|p| &p.id == id)
        && p.pos.dist(lp) * ppu <= f64::from(crate::viewport::WOKEN_CENTRE_PX)
    {
        chosen = Some(Candidate {
            kind: SnapKind::Point,
            role: Role::Center,
            at: p.pos,
            px: p.pos.dist(lp) * ppu,
            key: format!("p:{}", p.id),
            arg: Some(p.id.clone()),
            relation: None,
        });
    }
    // A model vertex under the cursor is a real point too (projected into the sketch).
    if chosen.as_ref().is_none_or(|c| c.kind != SnapKind::Point) {
        for h in crate::viewport::pick(app, proj, pos) {
            if let crate::viewport::Hit::Vertex { point, .. } = h {
                let at = plane.to_local(point);
                STATE.with(|s| {
                    let mut s = s.borrow_mut();
                    s.last = None;
                    let c = Candidate {
                        kind: SnapKind::Point,
                        role: Role::Point,
                        at,
                        px: 0.0,
                        key: format!("v:{},{},{}", point.x, point.y, point.z),
                        arg: None,
                        relation: None,
                    };
                    track(&mut s.tracked, &c);
                    s.shown = Shown { snap: Some((Role::Point, at)), guide: None };
                });
                return Some((at, Some(format!("vertex:{},{},{}", point.x, point.y, point.z))));
            }
        }
    }
    if let Some(c) = chosen {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.last = Some(c.key.clone());
            track(&mut s.tracked, &c);
            s.shown = Shown { snap: Some((c.role, c.at)), guide: None };
        });
        if let (Some(relation), Some(anchor)) = (&c.relation, anchor) {
            let (kind, curve) = match relation {
                LineRelation::Tangent(id) => (crate::inference::Kind::Tangent, id),
                LineRelation::Perpendicular(id) => (crate::inference::Kind::Perpendicular, id),
            };
            crate::inference::hold(crate::inference::Inference {
                kind,
                curve: curve.clone(),
                at: c.at,
                dist: c.px / ppu,
                guide: (anchor, c.at),
                arg: None,
            });
        }
        return Some((c.at, c.arg));
    }
    let tracked: Vec<Vec2> = STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.last = None;
        s.shown = Shown::default();
        s.tracked.iter().map(|t| t.at).collect()
    });
    if let Some(mut g) = guide(&tracked, lp, ppu) {
        // Along one guide the free coordinate still follows the grid (Snap on).
        if let [(from, to)] = g.lines.as_mut_slice()
            && !sk.view.no_snap
        {
            let step = solidcraft_engine::view::grid_step(app.cam.half_height()).0 / 10.0;
            let round = |x: f64| (x / step).round() * step;
            let d = *to - *from;
            if d.y == 0.0 {
                to.x = round(to.x);
            } else if d.x == 0.0 {
                to.y = round(to.y);
            } else {
                // 45°: the grid takes x, and y follows it exactly along the diagonal.
                let x = round(to.x);
                let slope = if (d.x > 0.0) == (d.y > 0.0) { 1.0 } else { -1.0 };
                *to = Vec2::new(x, from.y + (x - from.x) * slope);
            }
            if to.is_finite() && to.dist(*from) > 0.0 {
                g.at = *to;
            } else {
                *to = g.at;
            }
        }
        let at = g.at;
        STATE.with(|s| s.borrow_mut().shown.guide = Some(g));
        return Some((at, None));
    }
    // Nothing here: the classic inferences of the line being drawn (through a centre,
    // horizontal or vertical from its start, parallel, on an extension), then the grid.
    if let Some(p) = crate::sketch_tools::refine_snap(app, proj, sk, plane, lp) {
        return Some(p);
    }
    if sk.view.no_snap {
        return Some((lp, None));
    }
    let step = solidcraft_engine::view::grid_step(app.cam.half_height()).0 / 10.0;
    Some((Vec2::new((lp.x / step).round() * step, (lp.y / step).round() * step), None))
}

/// Per frame: forget everything once the drawing tool changes or ends, then draw the snap's
/// glyph and the guides.
pub fn show(app: &SolveApp, painter: &egui::Painter, proj: &Proj) {
    let current = app.tool.as_ref().filter(|t| drawing_tool(t)).map(|t| t.cmd);
    let stale = STATE.with(|s| {
        let s = s.borrow();
        s.tool.is_some() && (s.tool != current || s.sketch != app.session.active_sketch)
    });
    if stale || !app.preferences.sketch_reference_snap {
        reset();
        crate::scenario::publish_count("sketch_snap_guides", 0.0);
        return;
    }
    if current.is_none() {
        return;
    }
    let shown = shown();
    crate::scenario::publish_count("sketch_snap_guides", shown.guide.as_ref().map_or(0, |g| g.lines.len()) as f64);
    let st = app.session.world_state();
    let Some(ss) = app.session.active_sketch.and_then(|s| st.sketch(s)) else { return };
    let to = |p: Vec2| proj.to_screen(ss.plane.to_world(p));
    let tk = Tokens::get();
    let colour = tk.form_success;
    if let Some(g) = &shown.guide {
        for (a, b) in &g.lines {
            if let (Some(a), Some(b)) = (to(*a), to(*b)) {
                painter.extend(crate::viewport::dashed_line(&[a, b], painter.clip_rect(), Stroke::new(1.2, colour), 5.0, 4.0));
                painter.circle_stroke(a, 3.0, Stroke::new(1.2, colour));
            }
        }
    }
    if let Some((role, at)) = shown.snap
        && let Some(p) = to(at)
    {
        glyph(painter, p, role, colour);
    }
}

/// The marker of a snap: its shape tells the kind (no text), in the snap colour.
pub fn glyph(painter: &egui::Painter, p: Pos2, role: Role, colour: egui::Color32) {
    use egui::vec2;
    let s = 6.0;
    let stroke = Stroke::new(1.6, colour);
    let line = |a: Pos2, b: Pos2| painter.line_segment([a, b], stroke);
    match role {
        Role::Endpoint | Role::Point | Role::Origin => {
            painter.rect_stroke(egui::Rect::from_center_size(p, vec2(2.0 * s, 2.0 * s)), 0.0, stroke, egui::StrokeKind::Middle);
            if role == Role::Origin {
                painter.circle_filled(p, 1.8, colour);
            }
        }
        Role::Center => {
            painter.circle_stroke(p, s, stroke);
            line(p - vec2(2.5, 0.0), p + vec2(2.5, 0.0));
            line(p - vec2(0.0, 2.5), p + vec2(0.0, 2.5));
        }
        Role::Midpoint => {
            let pts = vec![p + vec2(0.0, -s), p + vec2(s, s * 0.75), p + vec2(-s, s * 0.75)];
            painter.add(egui::Shape::closed_line(pts, stroke));
        }
        Role::Intersection => {
            line(p + vec2(-s, -s), p + vec2(s, s));
            line(p + vec2(-s, s), p + vec2(s, -s));
        }
        Role::Tangent => {
            painter.circle_stroke(p + vec2(0.0, 2.0), s * 0.7, stroke);
            line(p + vec2(-s, 2.0 - s * 0.7), p + vec2(s, 2.0 - s * 0.7));
        }
        Role::Perpendicular => {
            line(p + vec2(-s, -s), p + vec2(-s, s));
            line(p + vec2(-s, s), p + vec2(s, s));
            painter.rect_stroke(
                egui::Rect::from_min_size(p + vec2(-s, s - 4.0), vec2(4.0, 4.0)),
                0.0,
                Stroke::new(1.0, colour),
                egui::StrokeKind::Middle,
            );
        }
        Role::OnLine | Role::OnCurve => {
            let pts = vec![p + vec2(0.0, -s), p + vec2(s, 0.0), p + vec2(0.0, s), p + vec2(-s, 0.0)];
            painter.add(egui::Shape::closed_line(pts, stroke));
        }
    }
}

#[cfg(test)]
#[path = "sketch_snap_tests.rs"]
mod tests;
