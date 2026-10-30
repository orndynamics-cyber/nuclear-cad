#![allow(clippy::unwrap_used)]
use super::*;
use crate::scenario::Harness;
use serde_json::{Value, json};

fn v(x: f64, y: f64) -> Vec2 {
    Vec2::new(x, y)
}

/// Ten screen pixels per sketch unit: 8 px is 0.8 units.
const PPU: f64 = 10.0;

fn lines() -> Sketch {
    let mut s = Sketch::new();
    s.add_line(v(0.0, 0.0), v(20.0, 0.0), None, None, Some("h")).unwrap();
    s.add_line(v(10.0, -10.0), v(10.0, 10.0), None, None, Some("v")).unwrap();
    s.add_circle(v(40.0, 0.0), 5.0, None, Some("c")).unwrap();
    s
}

fn best(sk: &Sketch, at: Vec2, anchor: Option<Vec2>) -> Option<Candidate> {
    choose(candidates(sk, at, PPU, anchor), None)
}

#[test]
fn a_real_point_beats_every_virtual_place_and_names_its_role() {
    let sk = lines();
    let c = best(&sk, v(20.3, 0.2), None).unwrap();
    assert_eq!((c.kind, c.role), (SnapKind::Point, Role::Endpoint));
    assert_eq!(c.at, v(20.0, 0.0));
    let c = best(&sk, v(40.2, 0.1), None).unwrap();
    assert_eq!(c.role, Role::Center);
    let c = best(&sk, v(0.1, 0.1), None).unwrap();
    assert_eq!(c.kind, SnapKind::Point, "the origin or the line's end: a real point either way");
}

#[test]
fn midpoint_before_intersection_before_the_curve_and_the_radius_is_in_pixels() {
    let sk = lines();
    // The midpoint of `h` (10, 0) is also the crossing with `v`: an existing point? No, so the
    // midpoint (better kind) wins over the crossing at the same place.
    let c = best(&sk, v(10.3, 0.3), None).unwrap();
    assert_eq!(c.kind, SnapKind::Midpoint);
    assert_eq!(c.arg.as_deref(), Some("mid:h"));
    // Past 8 px of the midpoint but on the line: on the line, at the foot of the perpendicular.
    let c = best(&sk, v(14.0, 0.5), None).unwrap();
    assert_eq!((c.kind, c.role), (SnapKind::OnCurve, Role::OnLine));
    assert!(c.at.dist(v(14.0, 0.0)) < 1e-12);
    assert!(c.arg.unwrap().starts_with("on:h:"));
    // The same sketch distance is 2.5 px when zoomed out: still on the line; 16 px zoomed in: nothing.
    assert!(choose(candidates(&sk, v(14.0, 0.5), 5.0, None), None).is_some());
    assert!(choose(candidates(&sk, v(14.0, 0.5), 32.0, None), None).is_none());
}

#[test]
fn a_crossing_snaps_with_both_curves_and_an_existing_point_wins_over_a_virtual_one() {
    let mut sk = Sketch::new();
    sk.add_line(v(0.0, 0.0), v(30.0, 0.0), None, None, Some("a")).unwrap();
    sk.add_line(v(7.0, -10.0), v(7.0, 20.0), None, None, Some("b")).unwrap();
    let c = best(&sk, v(7.3, 0.4), None).unwrap();
    assert_eq!(c.kind, SnapKind::Intersection);
    assert_eq!(c.arg.as_deref(), Some("x:a+b:7,0"));
    // A point already at the crossing: only the point is offered there.
    sk.add_point(v(7.0, 0.0), Some("p")).unwrap();
    let all = candidates(&sk, v(7.3, 0.4), PPU, None);
    assert!(all.iter().all(|c| c.kind != SnapKind::Intersection), "{all:?}");
    assert_eq!(choose(all, None).unwrap().arg.as_deref(), Some("p"));
}

#[test]
fn an_arc_offers_its_midpoint() {
    let mut sk = Sketch::new();
    sk.add_arc(v(0.0, 0.0), v(10.0, 0.0), v(0.0, 10.0), [None; 3], Some("arc")).unwrap();
    let m = v(10.0, 10.0) * std::f64::consts::FRAC_1_SQRT_2;
    let c = best(&sk, m + v(0.2, -0.3), None).unwrap();
    assert_eq!(c.kind, SnapKind::Midpoint);
    assert!(c.at.dist(m) < 1e-9);
    assert!(c.arg.unwrap().starts_with("on:arc:"));
}

#[test]
fn tangents_have_two_branches_and_need_the_anchor_outside() {
    let sk = lines();
    let anchor = v(40.0, 20.0);
    let half = (5.0f64 / 20.0).acos();
    let o = v(40.0, 0.0);
    for s in [-1.0, 1.0] {
        let t = o + Vec2::from_angle(std::f64::consts::FRAC_PI_2 + s * half) * 5.0;
        let c = best(&sk, t + v(0.1, 0.1), Some(anchor)).unwrap();
        assert_eq!(c.kind, SnapKind::Tangent);
        assert!(c.at.dist(t) < 1e-9);
        assert_eq!(c.relation, Some(LineRelation::Tangent("c".into())));
        assert!((c.at - o).dot(c.at - anchor).abs() < 1e-9, "the radius meets the line at a right angle");
    }
    // Inside the circle there is no tangent.
    let near = o + Vec2::from_angle(1.0) * 5.0;
    assert!(candidates(&sk, near, PPU, Some(v(41.0, 0.0))).iter().all(|c| c.kind != SnapKind::Tangent));
}

#[test]
fn a_perpendicular_foot_lies_inside_the_segment_and_away_from_the_anchor() {
    let sk = lines();
    let c = best(&sk, v(5.2, 0.3), Some(v(5.0, 10.0))).unwrap();
    assert_eq!(c.kind, SnapKind::Perpendicular);
    assert_eq!(c.at, v(5.0, 0.0));
    assert_eq!(c.relation, Some(LineRelation::Perpendicular("h".into())));
    // The foot past the end of the segment: no perpendicular (only the line beyond is near).
    assert!(candidates(&sk, v(-0.6, 0.3), PPU, Some(v(-0.6, 10.0))).iter().all(|c| c.kind != SnapKind::Perpendicular));
    // The anchor on the line itself: the foot is the anchor.
    assert!(candidates(&sk, v(5.0, 0.3), PPU, Some(v(5.0, 0.0))).iter().all(|c| c.kind != SnapKind::Perpendicular));
}

#[test]
fn the_shown_snap_is_kept_until_it_leaves_or_a_better_kind_comes_within_reach() {
    let mut sk = Sketch::new();
    sk.add_point(v(0.0, 0.0), Some("a")).unwrap();
    sk.add_point(v(1.0, 0.0), Some("b")).unwrap();
    let first = best(&sk, v(0.4, 0.0), None).unwrap();
    assert_eq!(first.key, "p:a", "a tie at the same distance goes to the stable id");
    // Nearer to b now, but a is still within 8 px: it stays.
    let kept = choose(candidates(&sk, v(0.6, 0.0), PPU, None), Some(&first.key)).unwrap();
    assert_eq!(kept.key, "p:a");
    // Past 8 px from a: b takes over.
    let next = choose(candidates(&sk, v(0.85, 0.0), PPU, None), Some(&kept.key)).unwrap();
    assert_eq!(next.key, "p:b");
    // A curve shown before yields to a point that comes within reach.
    let mut sk = lines();
    sk.add_point(v(15.0, 0.0), Some("q")).unwrap();
    let on = choose(candidates(&sk, v(16.0, 0.2), PPU, None), None).unwrap();
    assert_eq!(on.kind, SnapKind::OnCurve);
    let p = choose(candidates(&sk, v(15.5, 0.2), PPU, None), Some(&on.key)).unwrap();
    assert_eq!(p.arg.as_deref(), Some("q"));
}

#[test]
fn a_stale_key_shows_nothing_of_its_own() {
    let sk = lines();
    let c = choose(candidates(&sk, v(30.0, 30.0), PPU, None), Some("o:gone"));
    assert!(c.is_none());
}

#[test]
fn guides_run_from_the_tracked_point_in_eight_directions_as_rays() {
    let p = v(3.0, 7.0);
    // Almost horizontal: the cursor goes onto P's level exactly.
    let g = guide(&[p], v(30.0, 7.4), PPU).unwrap();
    assert_eq!(g.at, v(30.0, 7.0));
    assert_eq!(g.lines, vec![(p, v(30.0, 7.0))]);
    let g = guide(&[p], v(-30.0, 6.7), PPU).unwrap();
    assert_eq!(g.at.y, 7.0, "180° too");
    let g = guide(&[p], v(3.3, -20.0), PPU).unwrap();
    assert_eq!(g.at.x, 3.0, "270°");
    let g = guide(&[p], p + v(10.0, 10.3), PPU).unwrap();
    assert!((g.at - p).cross(v(1.0, 1.0)).abs() < 1e-9, "45°");
    let g = guide(&[p], p + v(-10.0, 9.7), PPU).unwrap();
    assert!((g.at - p).cross(v(-1.0, 1.0)).abs() < 1e-9, "135°");
    // Too far off any ray, or still on the point itself.
    assert!(guide(&[p], v(30.0, 8.0), PPU).is_none());
    assert!(guide(&[p], p + v(0.5, 0.1), PPU).is_none());
}

#[test]
fn two_tracked_points_put_the_cursor_on_the_crossing_of_their_guides() {
    let (a, b) = (v(0.0, 0.0), v(20.0, 15.0));
    // Horizontal from a and vertical from b cross at (20, 0).
    let g = guide(&[b, a], v(20.6, 0.7), PPU).unwrap();
    assert_eq!(g.at, v(20.0, 0.0));
    assert_eq!(g.lines.len(), 2);
    // Far from any crossing: the nearer single ray.
    let g = guide(&[b, a], v(8.0, 0.3), PPU).unwrap();
    assert_eq!(g.at, v(8.0, 0.0));
    assert_eq!(g.lines, vec![(a, v(8.0, 0.0))]);
}

#[test]
fn tracking_keeps_two_distinct_places_newest_first() {
    let sk = lines();
    let mut tracked = Vec::new();
    for at in [v(20.0, 0.0), v(10.0, 0.4), v(20.0, 0.0), v(40.0, 0.0)] {
        track(&mut tracked, &best(&sk, at, None).unwrap());
    }
    let keys: Vec<&str> = tracked.iter().map(|t| t.key.as_str()).collect();
    assert_eq!(keys.len(), 2);
    assert_eq!(keys[1], tracked[1].key);
    assert!(keys[0].starts_with("p:"), "{keys:?}");
    // On a curve: not tracked.
    let before = tracked.clone();
    track(&mut tracked, &best(&sk, v(14.0, 0.2), None).unwrap());
    assert_eq!(tracked, before);
}

#[test]
fn hostile_input_finds_nothing_and_never_panics() {
    let sk = lines();
    for (at, ppu) in [(v(f64::NAN, 0.0), PPU), (v(1.0, 1.0), 0.0), (v(1.0, 1.0), f64::INFINITY), (v(1e308, -1e308), PPU)] {
        assert!(choose(candidates(&sk, at, ppu, Some(v(f64::NAN, 1.0))), None).is_none());
        assert!(guide(&[v(0.0, 0.0), v(1e308, 1e308)], at, ppu).is_none_or(|g| g.at.is_finite()));
    }
}

// ---- in the app ----

fn fixture() -> Harness {
    let mut h = Harness::new();
    h.app.home.open = false;
    h.app.preferences.look_at_anim = false;
    for (command, params) in [
        ("sketch.create", json!({"plane":"XY"})),
        ("sketch.line", json!({"points":[[0,0],[20,0]]})),
        ("sketch.circle.center", json!({"center":[40,20],"radius":5})),
        ("sketch.point", json!({"point":[-15,12],"id":"P"})),
    ] {
        h.app.run(command, params).unwrap();
    }
    h.call("ui.view", json!({"view":"top"}));
    h.call("ui.camera", json!({"target":[10,5,0],"distance":150}));
    h.frames(3);
    h
}

fn step(h: &mut Harness, value: Value) {
    h.step(&value).unwrap();
}

fn sketch(h: &Harness) -> Sketch {
    h.app.session.world_state().sketch(1).unwrap().sketch.clone()
}

fn constraint_names(h: &Harness) -> Vec<&'static str> {
    sketch(h).constraints.iter().map(|c| c.kind.name()).collect()
}

#[test]
fn a_line_ends_level_with_a_point_the_cursor_rested_on() {
    let mut h = fixture();
    h.app.start("sketch.line");
    step(&mut h, json!({"click":{"sketch":[0,30]}}));
    // Rest on P, then move away nearly horizontally.
    step(&mut h, json!({"move":{"sketch":[-15,12]}}));
    assert_eq!(tracked().first().map(|t| t.at), Some(v(-15.0, 12.0)));
    step(&mut h, json!({"move":{"sketch":[25,12.3]}}));
    let shown = shown();
    assert_eq!(shown.guide.as_ref().map(|g| g.lines.len()), Some(1), "{shown:?}");
    let before = sketch(&h).constraints.len();
    step(&mut h, json!({"click":{"sketch":[25,12.3]}}));
    let sk = sketch(&h);
    let end = sk.points.iter().rev().find(|p| p.pos.dist(v(25.0, 12.0)) < 1e-6);
    assert!(end.is_some(), "the end is on P's level: {:?}", sk.points);
    assert_eq!(sk.constraints.len(), before, "a guide adds no constraint");
}

#[test]
fn clicks_keep_the_midpoint_tangent_and_perpendicular_relations() {
    let mut h = fixture();
    h.app.start("sketch.line");
    // The line's midpoint, then perpendicular back onto it from above.
    step(&mut h, json!({"move":{"sketch":[10.2,0.2]}}));
    assert_eq!(shown().snap.map(|s| s.0), Some(Role::Midpoint));
    step(&mut h, json!({"click":{"sketch":[10.2,0.2]}}));
    // The relation is kept once the line exists, at its second click.
    step(&mut h, json!({"click":{"sketch":[12,-30]}}));
    assert!(constraint_names(&h).contains(&"MidPoint"), "{:?}", constraint_names(&h));
    step(&mut h, json!({"key":"Escape"}));
    h.frames(2);
    h.app.start("sketch.line");
    step(&mut h, json!({"click":{"sketch":[5,25]}}));
    step(&mut h, json!({"move":{"sketch":[5.2,0.3]}}));
    assert_eq!(shown().snap.map(|s| s.0), Some(Role::Perpendicular));
    step(&mut h, json!({"click":{"sketch":[5.2,0.3]}}));
    assert!(constraint_names(&h).contains(&"Perpendicular"), "{:?}", constraint_names(&h));
    step(&mut h, json!({"key":"Escape"}));
    h.frames(2);
    // A tangent to the circle from (40, 32).
    h.app.start("sketch.line");
    step(&mut h, json!({"click":{"sketch":[40,32]}}));
    let half = (5.0f64 / 12.0).acos();
    let t = v(40.0, 20.0) + Vec2::from_angle(std::f64::consts::FRAC_PI_2 + half) * 5.0;
    step(&mut h, json!({"move":{"sketch":[t.x + 0.1, t.y + 0.1]}}));
    assert_eq!(shown().snap.map(|s| s.0), Some(Role::Tangent));
    step(&mut h, json!({"click":{"sketch":[t.x + 0.1, t.y + 0.1]}}));
    assert!(constraint_names(&h).contains(&"Tangent"), "{:?}", constraint_names(&h));
}

#[test]
fn ctrl_frees_the_cursor_and_a_new_tool_forgets_tracked_points() {
    let mut h = fixture();
    h.app.start("sketch.line");
    step(&mut h, json!({"move":{"sketch":[-15,12]}}));
    assert_eq!(tracked().len(), 1);
    step(&mut h, json!({"move":{"sketch":[-14.9,12.1]},"ctrl":true}));
    assert_eq!(shown(), Shown::default(), "no snap and no guide with Ctrl");
    step(&mut h, json!({"key":"Escape"}));
    h.frames(2);
    assert!(tracked().is_empty(), "leaving the tool clears tracked points");
    h.app.start("sketch.circle.center");
    step(&mut h, json!({"move":{"sketch":[20,0]}}));
    assert_eq!(tracked().first().map(|t| t.at), Some(v(20.0, 0.0)));
    step(&mut h, json!({"move":{"sketch":[0,30]}}));
    h.app.start("sketch.rectangle.two_point");
    h.frames(2);
    assert!(tracked().is_empty(), "another tool clears them");
}

#[test]
fn the_preference_off_keeps_the_classic_snap() {
    let mut h = fixture();
    h.call("ui.prefs", json!({"sketch_reference_snap":false}));
    h.app.start("sketch.line");
    step(&mut h, json!({"move":{"sketch":[-15,12]}}));
    step(&mut h, json!({"move":{"sketch":[25,12.3]}}));
    assert_eq!(shown(), Shown::default());
    assert!(tracked().is_empty());
}
