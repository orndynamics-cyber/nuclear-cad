//! Move Edge: M on a straight edge between two planar faces moves the edge itself.
//!
//! An edge of a solid is where two faces meet, so moving it means rebuilding those faces, not
//! moving a line on its own. Each face that changes turns about its far side (the line across
//! the face from the edge, which stays where it is) until it passes through the edge's new
//! place, as a Draft about that side: one editable feature per face in the timeline, so
//! undo, redo and editing work as for any other feature. Which faces turn is the user's choice
//! (the [`MODES`]): both, or only one while the edge slides along the other face's plane.
//!
//! Only the well-defined case is offered: a straight edge with exactly two planar faces on it,
//! not lying in one plane. Anything else is refused with the reason, and the body is left as it
//! was.

use serde_json::{Value, json};
use solidcraft_engine::Sel;
use solidcraft_engine::geom::Vec3;

use crate::SolveApp;

/// Which way a positive Draft angle turns a face, as `solid.draft` measures it.
const DRAFT_SIGN: f64 = 1.0;

/// The Move Edge dialog's id (`crate::dialogs::Dialog::for_command`): no engine command has it.
pub(crate) const DIALOG: &str = "nuclear.move_edge";

/// How the edge moves: both faces turn (the edge moves out between them), or it slides along
/// the first or the second face, whose plane stays as it is.
pub(crate) const MODES: [&str; 3] = ["Both Faces", "Slide Along First Face", "Slide Along Second Face"];

/// A face on the edge, as the move needs it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Side {
    /// The face's outward normal.
    pub normal: Vec3,
    /// In the face's plane, square to the edge, pointing from the edge into the face.
    pub into: Vec3,
    /// How far the face reaches from the edge (to its far side).
    pub width: f64,
    /// A point well inside the face, near its far side: still on the face after the other
    /// face has turned.
    pub pick: Vec3,
}

/// The edge: a point on it, its direction, and the two faces on it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct EdgeFaces {
    pub at: Vec3,
    pub sides: [Side; 2],
}

/// The edge `sel` with its two planar faces, or why it cannot be moved.
pub(crate) fn faces_of(app: &SolveApp, sel: &Sel) -> Result<EdgeFaces, String> {
    let Sel::Edge { body, index, .. } = sel else { return Err(crate::i18n::tr("nuclear.move_edge.pick").into()) };
    let st = app.session.world_state();
    let b = st.body(body).ok_or_else(|| crate::i18n::tr("nuclear.move_edge.pick").to_string())?;
    let mesh = b.mesh();
    let points = mesh.edges.get(*index).ok_or_else(|| crate::i18n::tr("nuclear.move_edge.pick").to_string())?;
    let (Some(&a), Some(&z)) = (points.first(), points.last()) else { return Err(crate::i18n::tr("nuclear.move_edge.pick").into()) };
    let along = (z - a).normalized().ok_or_else(|| crate::i18n::tr("nuclear.move_edge.not_straight").to_string())?;
    let size = b.body.size().max(1e-6);
    let tol = (size * 1e-6).max(1e-7);
    if !points.iter().all(|p| (*p - a).cross(along).len() < tol * 10.0) {
        return Err(crate::i18n::tr("nuclear.move_edge.not_straight").into());
    }
    let faces = b.body.faces((size * 1e-3).max(1e-3)).map_err(|e| e.to_string())?;
    let on_edge: Vec<_> = faces
        .iter()
        .filter_map(|f| f.plane_normal.map(|n| (f, n)))
        .filter(|(f, n)| ((a - f.centroid).dot(*n)).abs() < tol * 100.0 && ((z - f.centroid).dot(*n)).abs() < tol * 100.0)
        .collect();
    let curved = faces.iter().filter(|f| f.plane_normal.is_none()).count();
    if on_edge.len() != 2 {
        return Err(
            crate::i18n::tr(if on_edge.len() < 2 && curved > 0 { "nuclear.move_edge.curved_face" } else { "nuclear.move_edge.two_faces" }).into()
        );
    }
    if on_edge[0].1.cross(on_edge[1].1).len() < 1e-6 {
        return Err(crate::i18n::tr("nuclear.move_edge.flat").into());
    }
    let mut sides = Vec::with_capacity(2);
    for (f, n) in &on_edge {
        let mut into = n.cross(along).normalized().ok_or_else(|| crate::i18n::tr("nuclear.move_edge.two_faces").to_string())?;
        if (f.centroid - a).dot(into) < 0.0 {
            into = into * -1.0;
        }
        // How far the face reaches: the farthest point of the edges lying in its plane.
        let width = mesh
            .edges
            .iter()
            .filter(|e| e.iter().all(|p| ((*p - f.centroid).dot(*n)).abs() < tol * 100.0))
            .flat_map(|e| e.iter())
            .map(|p| (*p - a).dot(into))
            .fold(0.0f64, f64::max);
        if width <= tol * 100.0 {
            return Err(crate::i18n::tr("nuclear.move_edge.two_faces").into());
        }
        let mid = a + (z - a) * 0.5;
        sides.push(Side { normal: *n, into, width, pick: mid + into * (width * 0.75) });
    }
    Ok(EdgeFaces { at: a, sides: [sides[0], sides[1]] })
}

/// The Draft commands that move the edge by `distance` (millimetres; positive is out of
/// the body) in `mode`.
pub(crate) fn commands(app: &SolveApp, sel: &Sel, distance: f64, mode: usize) -> Result<Vec<(String, Value)>, String> {
    if !distance.is_finite() {
        return Err(crate::i18n::tr("nuclear.move_edge.distance").into());
    }
    let Sel::Edge { body, .. } = sel else { return Err(crate::i18n::tr("nuclear.move_edge.pick").into()) };
    let e = faces_of(app, sel)?;
    // Where the edge goes, and which faces turn to meet it there.
    let (offset, turning): (Vec3, Vec<usize>) = match mode {
        1 | 2 => {
            // Slide along one face: out of the body within that face's plane.
            let kept = &e.sides[mode - 1];
            (kept.into * -distance, vec![2 - mode])
        }
        _ => {
            let out = (e.sides[0].normal + e.sides[1].normal).normalized().ok_or_else(|| crate::i18n::tr("nuclear.move_edge.flat").to_string())?;
            (out * distance, vec![0, 1])
        }
    };
    if offset.len() < 1e-9 {
        return Ok(Vec::new());
    }
    let to = e.at + offset;
    // The edge may not pass the far side of either face: that face would be gone.
    if e.sides.iter().any(|side| (to - e.at).dot(side.into) > side.width * 0.95) {
        return Err(crate::i18n::tr("nuclear.move_edge.too_far").into());
    }
    let mut out = Vec::new();
    for i in turning {
        let side = &e.sides[i];
        // The face turns about its far side (a Draft whose neutral plane holds that side): the
        // pull runs from the far side back to the edge, and the angle tilts the face until it
        // holds the edge's new place.
        let hinge = e.at + side.into * side.width;
        let pull = side.into * -1.0;
        let along_pull = (to - hinge).dot(pull);
        let out_of_face = (to - hinge).dot(side.normal);
        // A face turned past square to itself, or the edge pushed past the far side, would fold
        // over the body.
        if along_pull < side.width * 0.05 {
            return Err(crate::i18n::tr("nuclear.move_edge.too_far").into());
        }
        let angle = (-out_of_face).atan2(along_pull).to_degrees() * DRAFT_SIGN;
        if angle.abs() > 80.0 {
            return Err(crate::i18n::tr("nuclear.move_edge.too_far").into());
        }
        out.push((
            "solid.draft".to_string(),
            json!({
                "body": body,
                "faces": [[side.pick.x, side.pick.y, side.pick.z]],
                "neutral": {"origin": [hinge.x, hinge.y, hinge.z], "normal": [pull.x, pull.y, pull.z]},
                "pull": [pull.x, pull.y, pull.z],
                "angle": format!("{angle} deg"),
            }),
        ));
    }
    Ok(out)
}
