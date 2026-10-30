//! Shell (inward, open caps) of a body of revolution, worked out on its meridian.
//!
//! The face-by-face offset ([`crate::offset`]) only knows planes, cylinders, cones, spheres and
//! tori, so a body revolved from a spline profile (a vase) could not be shelled at all, and a
//! revolved body of arcs went through a slow boolean of two revolved sheets. Every face of a
//! body of revolution is swept from one curve of its meridian (the profile in a half-plane
//! through the axis), and every normal of such a face lies in that half-plane, so the shell's
//! inner wall is the meridian offset by the thickness along its own normal, revolved: a
//! constant-thickness wall, not a radial shrink. The material is then one closed region of the
//! half-plane (the kept walls, the rim on the open cap, the offset walls back, and the axis),
//! revolved by the same builder as a Revolve feature: no boolean.
//!
//! Scope: one closed shell whose faces are all swept about one axis (surfaces of revolution and
//! planes square to the axis), its meridian one chain between two points on the axis, the open
//! faces straight (flat or conical caps) at one end of that chain. Corners join sharp: offsets
//! that cross are cut where they cross, offsets that part run on along their ends to where they
//! meet. Anything else is left to the general shell (`None`). The body is checked to be what
//! its meridian says (its volume against the meridian's) before anything is built, and the shell
//! against the material region's after.

use std::collections::BTreeSet;

use solidcraft_geom::{Loop2, Plane, Region2, Seg2, Vec2, Vec3};
use truck_modeling::{self as mt, InnerSpace, Transform};

use crate::body::{Body, from_p3};
use crate::history::{FaceOrigin, FaceProv};
use crate::offset::Surf;
use crate::record;
use crate::{KernelError, Result};

/// A run of the meridian swept into one face: its segments in order, the faces it was swept
/// into (angular patches of one turn share it).
#[derive(Clone, Debug)]
struct Piece {
    segs: Vec<Seg2>,
    faces: Vec<usize>,
}

impl Piece {
    fn start(&self) -> Option<Vec2> {
        self.segs.first().map(Seg2::start)
    }
    fn end(&self) -> Option<Vec2> {
        self.segs.last().map(Seg2::end)
    }
    fn mid(&self) -> Option<Vec2> {
        let n = self.segs.len();
        let s = self.segs.get(n / 2)?;
        Some(if n % 2 == 1 { s.point_at(0.5) } else { s.start() })
    }
    fn reversed(&self) -> Piece {
        Piece { segs: self.segs.iter().rev().map(Seg2::reversed).collect(), faces: self.faces.clone() }
    }
}

/// The axis and the meridian frame: `x` radial in the meridian half-plane, `y` along the axis.
#[derive(Clone, Copy, Debug)]
struct Frame {
    o: Vec3,
    a: Vec3,
    u: Vec3,
}

impl Frame {
    /// A point's meridian coordinates (distance from the axis, height along it).
    fn meridian(&self, p: Vec3) -> Vec2 {
        let w = p - self.o;
        let h = w.dot(self.a);
        Vec2::new((w - self.a * h).len(), h)
    }
    fn plane(&self) -> Plane {
        Plane { origin: self.o, x: self.u, y: self.a }
    }
}

/// Not a body this shell takes: why, for the log.
fn skip<T>(why: &str) -> Option<T> {
    log::debug!("shell of revolution: left to the general shell: {why}");
    None
}

fn kind(f: &mt::Face) -> &'static str {
    match f.surface() {
        mt::Surface::Plane(_) => "a plane",
        mt::Surface::BSplineSurface(_) => "a B-spline surface",
        mt::Surface::NurbsSurface(_) => "a NURBS surface",
        mt::Surface::RevolutedCurve(_) => "a revolved curve",
    }
}

fn unsupported(m: &str) -> KernelError {
    KernelError::Failed(format!("not supported yet: shell of this body of revolution: {m}"))
}

/// The shell of `b` with the faces `opened` removed and walls `thickness` thick inside, when
/// `b` is a body of revolution in scope (see the module); `None` when it is not.
pub(crate) fn shell(b: &Body, opened: &[usize], thickness: f64) -> Result<Option<Body>> {
    if b.mesh.is_some() || b.solid.boundaries().len() != 1 || b.face_count() > 512 || opened.is_empty() {
        return Ok(skip("not one closed B-rep shell"));
    }
    if b.paint().is_some_and(|p| !p.faces.is_empty()) || b.split_lines().is_some_and(|s| !s.is_empty()) {
        return Ok(skip("painted or split faces"));
    }
    let size = b.size();
    let tol = (size * 1e-7).max(1e-9) * 100.0;
    let faces: Vec<mt::Face> = b.solid.face_iter().cloned().collect();
    let Some(frame) = axis(&faces, tol) else { return Ok(skip("no face of revolution to take the axis from")) };
    let mut pieces: Vec<Piece> = Vec::new();
    for (i, f) in faces.iter().enumerate() {
        let Some(segs) = meridian_of(f, &frame, tol) else {
            return Ok(skip(&format!("face {i} is not swept about the axis ({})", kind(f))));
        };
        let piece = Piece { segs, faces: vec![i] };
        let (Some(s), Some(e), Some(m)) = (piece.start(), piece.end(), piece.mid()) else { return Ok(None) };
        // Angular patches of one turn: the same run of the meridian.
        let same = pieces.iter_mut().find(|q| {
            let (Some(qs), Some(qe), Some(qm)) = (q.start(), q.end(), q.mid()) else { return false };
            qm.dist(m) < tol && ((qs.dist(s) < tol && qe.dist(e) < tol) || (qs.dist(e) < tol && qe.dist(s) < tol))
        });
        match same {
            Some(q) => q.faces.push(i),
            None => pieces.push(piece),
        }
    }
    let Some(chain) = chain_between_axis_points(pieces, tol) else { return Ok(skip("the meridian is not one run between two points on the axis")) };
    // The body must be the meridian turned once: its volume is the meridian region's by Pappus.
    let turned = pappus(&closed_outline(&chain)?, tol);
    let measured = crate::measure(b).map(|m| m.volume).unwrap_or(f64::NAN);
    if !((turned - measured).abs() <= measured.abs() * 2e-3) {
        return Ok(skip(&format!("the meridian turned once is {turned:.3} mm³, the body {measured:.3} mm³")));
    }
    // The open faces: a run of straight pieces at one end of the chain. Make it the far end.
    let open: Vec<bool> = chain.iter().map(|p| p.faces.iter().any(|f| opened.contains(f))).collect();
    let n = chain.len();
    let leading = open.iter().take_while(|o| **o).count();
    let trailing = open.iter().rev().take_while(|o| **o).count();
    let opened_count = open.iter().filter(|o| **o).count();
    let chain: Vec<Piece> = if trailing == opened_count && trailing > 0 && trailing < n {
        chain
    } else if leading == opened_count && leading > 0 && leading < n {
        chain.iter().rev().map(Piece::reversed).collect()
    } else {
        return Ok(skip("the open faces are not a run at one end of the meridian"));
    };
    let kept_count = n - opened_count;
    let (kept, cap) = chain.split_at(kept_count);
    let Some(cap_first) = cap.first() else { return Ok(None) };
    let closed = closed_outline(&chain)?;
    // The first open piece's first segment carries the rim: it has to be straight.
    let Some(&Seg2::Line { a: rim_from, b: rim_to }) = cap_first.segs.first() else { return Ok(skip("the open face is curved")) };
    // Which way is inside: the region is on the left of a counter-clockwise outline.
    let ccw = Loop2 { segs: closed.clone() }.signed_area() > 0.0;
    let side = if ccw { 1.0 } else { -1.0 };
    let walls: Vec<(Seg2, Vec<usize>)> = kept.iter().flat_map(|p| p.segs.iter().map(move |s| (*s, p.faces.clone()))).collect();
    let inner = offset_chain(&walls, side * thickness, Seg2::Line { a: rim_from, b: rim_to }, size, tol)?;
    // Material: the kept walls out to the rim, across the rim, the offset walls back down, and
    // up the axis to the start.
    let (Some(first), Some(e)) = (walls.first().map(|w| w.0.start()), walls.last().map(|w| w.0.end())) else { return Ok(None) };
    let (Some(e2), Some(a2)) = (inner.last().map(|s| s.0.end()), inner.first().map(|s| s.0.start())) else { return Ok(None) };
    // The rim stays on the open face: between the wall and the open face's far end.
    let along = (e2 - rim_from).dot(rim_to - rim_from) / (rim_to - rim_from).len2().max(1e-300);
    if !(along > 1e-9 && along < 1.0 - 1e-9) || e2.dist(e) < tol {
        return Err(KernelError::Invalid(format!(
            "the shell is too thick for the opening: {thickness:.4} mm walls would close the open face (it is {:.4} mm across here)",
            rim_from.dist(rim_to)
        )));
    }
    let mut loop_segs: Vec<(Seg2, Role)> = walls.iter().map(|(s, f)| (*s, Role::Outer(f.clone()))).collect();
    loop_segs.push((Seg2::Line { a: e, b: e2 }, Role::Rim(cap_first.faces.clone())));
    for (s, f) in inner.iter().rev() {
        loop_segs.push((s.reversed(), Role::Inner(f.clone())));
    }
    loop_segs.push((Seg2::Line { a: a2, b: first }, Role::Axis));
    snap_joints(&mut loop_segs);
    let segs: Vec<Seg2> = loop_segs.iter().map(|s| s.0).collect();
    if let Some(p) = crossing(&segs, (thickness * 1e-3).min(size * 1e-5).max(1e-6)) {
        return Err(KernelError::Invalid(format!(
            "the shell is too thick for this body: {thickness:.4} mm walls would cut through the wall near meridian point [{:.2}, {:.2}]",
            p.x, p.y
        )));
    }
    let mut outer = Loop2 { segs };
    if outer.signed_area() < 0.0 {
        outer = outer.reversed();
    }
    let region = Region2 { outer, holes: vec![] };
    let expect = pappus(&region.outer.segs, tol);
    let built = crate::build::revolve_whole(&frame.plane(), std::slice::from_ref(&region), Vec2::ZERO, Vec2::new(0.0, 1.0), std::f64::consts::TAU)?;
    let [body]: [Body; 1] = built.try_into().map_err(|_| unsupported("the material region turned into several bodies"))?;
    let bad = body.validity();
    let got = crate::measure(&body).map(|m| m.volume).unwrap_or(f64::NAN);
    if !bad.is_empty() || !((got - expect).abs() <= expect.abs() * 2e-3) {
        return Err(KernelError::Failed(format!(
            "shell of a body of revolution: the result did not check out ({bad:?}, volume {got:.3} against {expect:.3} mm³)"
        )));
    }
    let history = history(&body, &frame, &loop_segs, size);
    Ok(Some(body.with_checked_history(history, &record::counts(&[b]))))
}

/// What a segment of the material outline is: a kept wall (of these faces), the rim on an open
/// face, an offset wall (of these faces), or the axis.
#[derive(Clone, Debug)]
enum Role {
    Outer(Vec<usize>),
    Rim(Vec<usize>),
    Inner(Vec<usize>),
    Axis,
}

/// Each face of `body` (made from the material outline `outline`) in place of the faces of `b`
/// its outline segment comes from.
fn history(body: &Body, frame: &Frame, outline: &[(Seg2, Role)], size: f64) -> Option<crate::history::History> {
    let mesh = body.tessellate((size * 1e-3).max(1e-3)).ok()?;
    let n = body.face_count();
    // Each face's largest triangle's centre.
    let mut best: Vec<(f64, Option<Vec3>)> = vec![(-1.0, None); n];
    for (t, f) in mesh.triangles.iter().zip(&mesh.tri_face) {
        let Some([p, q, r]) = mesh.tri(t) else { continue };
        let area = (q - p).cross(r - p).len();
        if let Some(slot) = best.get_mut(*f as usize)
            && area > slot.0
        {
            *slot = (area, Some((p + q + r) * (1.0 / 3.0)));
        }
    }
    let mut regions: Vec<(BTreeSet<FaceOrigin>, bool)> = Vec::with_capacity(n);
    let mut inner: Vec<Option<Vec<usize>>> = Vec::with_capacity(n);
    for (_, c) in &best {
        let m = frame.meridian((*c)?);
        let (_, role) = outline
            .iter()
            .map(|(s, r)| (s.point_at(s.closest_param(m)).dist(m), r))
            .min_by(|x, y| x.0.total_cmp(&y.0))
            .map(|(d, r)| (d, r.clone()))?;
        let origins = |fs: &[usize]| fs.iter().map(|&face| FaceOrigin::Face { operand: 0, face }).collect::<BTreeSet<_>>();
        match role {
            Role::Outer(fs) => {
                regions.push((origins(&fs), true));
                inner.push(None);
            }
            Role::Rim(fs) => {
                regions.push((origins(&fs), false));
                inner.push(None);
            }
            Role::Inner(fs) => {
                regions.push((BTreeSet::new(), false));
                inner.push(Some(fs));
            }
            Role::Axis => return None,
        }
    }
    let mut faces: Vec<FaceProv> = record::regions(regions);
    for (f, walls) in faces.iter_mut().zip(inner) {
        if let Some(fs) = walls {
            *f = record::generated(fs.into_iter().map(|face| FaceOrigin::Offset { operand: 0, face }).collect());
        }
    }
    Some(record::from_faces(&body.solid, 1, faces))
}

/// The chain's outline closed along the axis.
fn closed_outline(chain: &[Piece]) -> Result<Vec<Seg2>> {
    let mut closed: Vec<Seg2> = chain.iter().flat_map(|p| p.segs.iter().copied()).collect();
    let (Some(first), Some(last)) = (closed.first().map(Seg2::start), closed.last().map(Seg2::end)) else {
        return Err(unsupported("an empty meridian"));
    };
    closed.push(Seg2::Line { a: last, b: first });
    Ok(closed)
}

/// Consecutive segments made to meet exactly (a line or a free curve moves its end onto its
/// neighbour's; arcs keep their own ends).
fn snap_joints(segs: &mut [(Seg2, Role)]) {
    let n = segs.len();
    for i in 0..n {
        let j = (i + 1) % n;
        let (Some(p), Some(q)) = (segs.get(i).map(|s| s.0), segs.get(j).map(|s| s.0)) else { continue };
        let x = match (p, q) {
            (Seg2::Arc { .. }, _) => p.end(),
            (_, Seg2::Arc { .. }) => q.start(),
            _ => (p.end() + q.start()) * 0.5,
        };
        if let Some(s) = segs.get_mut(i)
            && !matches!(s.0, Seg2::Arc { .. })
        {
            s.0 = with_end(&s.0, x);
        }
        if let Some(s) = segs.get_mut(j)
            && !matches!(s.0, Seg2::Arc { .. })
        {
            s.0 = with_start(&s.0, x);
        }
    }
}

fn with_end(s: &Seg2, x: Vec2) -> Seg2 {
    match *s {
        Seg2::Line { a, .. } => Seg2::Line { a, b: x },
        Seg2::Cubic { p0, p1, p2, .. } => Seg2::Cubic { p0, p1, p2, p3: x },
        Seg2::Conic { a, apex, w, .. } => Seg2::Conic { a, apex, b: x, w },
        Seg2::Bezier { n, p } => {
            let mut q = p;
            if let Some(last) = q.get_mut((n as usize).clamp(1, solidcraft_geom::MAX_BEZIER_DEGREE)) {
                *last = x;
            }
            Seg2::Bezier { n, p: q }
        }
        Seg2::Arc { .. } => *s,
    }
}

fn with_start(s: &Seg2, x: Vec2) -> Seg2 {
    match *s {
        Seg2::Line { b, .. } => Seg2::Line { a: x, b },
        Seg2::Cubic { p1, p2, p3, .. } => Seg2::Cubic { p0: x, p1, p2, p3 },
        Seg2::Conic { apex, b, w, .. } => Seg2::Conic { a: x, apex, b, w },
        Seg2::Bezier { n, p } => {
            let mut q = p;
            if let Some(first) = q.first_mut() {
                *first = x;
            }
            Seg2::Bezier { n, p: q }
        }
        Seg2::Arc { .. } => *s,
    }
}

/// The volume of the region bounded by `segs` turned once about the meridian's axis (x = 0),
/// by Pappus on a fine polyline: 2π × the region's first moment about the axis.
fn pappus(segs: &[Seg2], tol: f64) -> f64 {
    let pts: Vec<Vec2> = Loop2 { segs: segs.to_vec() }.polyline(tol.max(1e-9) * 0.1).into_iter().collect();
    let n = pts.len();
    let mut moment = 0.0;
    for i in 0..n {
        let (Some(p), Some(q)) = (pts.get(i), pts.get((i + 1) % n)) else { continue };
        moment += (p.x + q.x) * p.cross(*q) / 6.0;
    }
    2.0 * std::f64::consts::PI * moment.abs()
}

/// The axis every face is swept about: from the first face of revolution (a revolved curve, or a
/// cylinder, cone or torus); `None` without one.
fn axis(faces: &[mt::Face], tol: f64) -> Option<Frame> {
    use mt::{BoundedCurve, ParametricCurve};
    for f in faces {
        if let mt::Surface::RevolutedCurve(rc) = f.surface() {
            let m = *rc.transform();
            if !rigid(&m) {
                return None;
            }
            let o = from_p3(m.transform_point(rc.entity().origin()));
            let ax = m.transform_vector(rc.entity().axis());
            let a = Vec3::new(ax.x, ax.y, ax.z).normalized()?;
            // The meridian half-plane of the swept curve, where it is off the axis.
            let curve = mt::Transformed::transformed(rc.entity().entity_curve(), m);
            let (t0, t1) = curve.range_tuple();
            let u = (0..=8).find_map(|k| {
                let w = from_p3(curve.subs(t0 + (t1 - t0) * k as f64 / 8.0)) - o;
                let r = w - a * w.dot(a);
                (r.len() > tol).then(|| r.normalized()).flatten()
            })?;
            return Some(Frame { o, a, u });
        }
    }
    for f in faces {
        let Some(sf) = crate::offset::surf_of(f, tol) else { continue };
        let (o, a) = match sf {
            Surf::Cylinder { o, a, .. } | Surf::Torus { o, a, .. } => (o, a),
            Surf::Cone { v, a, .. } => (v, a),
            _ => continue,
        };
        let a = a.normalized()?;
        return Some(Frame { o, a, u: a.any_perp() });
    }
    // Free-form faces only (a body read from a STEP file): the axis of a circular edge.
    for f in faces {
        for e in f.edge_iter() {
            if let Some((o, a)) = circle_axis(&e.curve(), tol) {
                return Some(Frame { o, a, u: a.any_perp() });
            }
        }
    }
    None
}

/// The centre and normal of a curve that is a circle (or an arc of one).
fn circle_axis(c: &mt::Curve, tol: f64) -> Option<(Vec3, Vec3)> {
    use mt::{BoundedCurve, ParametricCurve};
    let (t0, t1) = c.range_tuple();
    let pts: Vec<Vec3> = (0..=8).map(|k| from_p3(c.subs(t0 + (t1 - t0) * k as f64 / 8.0))).collect();
    let (Some(&p), Some(&q), Some(&r)) = (pts.first(), pts.get(4), pts.last()) else { return None };
    let (b, c2) = (q - p, r - p);
    let n = b.cross(c2);
    if n.len() < tol * tol {
        return None;
    }
    let center = p + (c2 * b.len2() - b * c2.len2()).cross(n) * (1.0 / (2.0 * n.len2()));
    let radius = center.dist(p);
    let a = n.normalized()?;
    let on = pts.iter().all(|x| (x.dist(center) - radius).abs() < tol && (*x - center).dot(a).abs() < tol);
    (on && radius > tol).then_some((center, a))
}

fn rigid(m: &mt::Matrix4) -> bool {
    let (x, y, z) = (m.x.truncate(), m.y.truncate(), m.z.truncate());
    [x, y, z].iter().all(|v| (v.magnitude2() - 1.0).abs() <= 1e-9)
        && [x.dot(y), x.dot(z), y.dot(z)].iter().all(|v| v.abs() <= 1e-9)
        && m.x.w == 0.0
        && m.y.w == 0.0
        && m.z.w == 0.0
        && m.w.w == 1.0
}

/// The meridian points of a face's boundary (its vertices and the middles of its edges).
fn boundary_points(f: &mt::Face, frame: &Frame) -> Vec<Vec2> {
    use mt::{BoundedCurve, ParametricCurve};
    let mut out = Vec::new();
    for w in f.boundaries() {
        for e in w.edge_iter() {
            let c = e.curve();
            let (t0, t1) = c.range_tuple();
            for k in 0..4 {
                out.push(frame.meridian(from_p3(c.subs(t0 + (t1 - t0) * k as f64 / 4.0))));
            }
        }
    }
    out
}

/// The run of the meridian `f` is swept from, as segments in meridian coordinates; `None` when
/// the face is not swept about the frame's axis.
fn meridian_of(f: &mt::Face, frame: &Frame, tol: f64) -> Option<Vec<Seg2>> {
    let pts = boundary_points(f, frame);
    if pts.is_empty() {
        return None;
    }
    match f.surface() {
        mt::Surface::Plane(pl) => {
            let n = pl.normal();
            let n = Vec3::new(n.x, n.y, n.z).normalized()?;
            if n.dot(frame.a).abs() < 1.0 - 1e-9 {
                return None;
            }
            let h = pts.first()?.y;
            if pts.iter().any(|p| (p.y - h).abs() > tol) {
                return None;
            }
            let hi = pts.iter().map(|p| p.x).fold(f64::NEG_INFINITY, f64::max);
            // A whole disc is bounded by its rim alone: it reaches in to the axis.
            let disc = f.boundaries().len() == 1 && pts.iter().all(|p| (p.x - hi).abs() < tol);
            let lo = if disc { 0.0 } else { pts.iter().map(|p| p.x).fold(f64::INFINITY, f64::min) };
            (hi - lo > tol).then(|| vec![Seg2::Line { a: Vec2::new(lo.max(0.0), h), b: Vec2::new(hi, h) }])
        }
        mt::Surface::RevolutedCurve(rc) => {
            let m = *rc.transform();
            if !rigid(&m) {
                return None;
            }
            let o = from_p3(m.transform_point(rc.entity().origin()));
            let ax = m.transform_vector(rc.entity().axis());
            let a = Vec3::new(ax.x, ax.y, ax.z).normalized()?;
            if a.cross(frame.a).len() > 1e-9 || (o - frame.o).cross(frame.a).len() > tol {
                return None;
            }
            let curve = mt::Transformed::transformed(rc.entity().entity_curve(), m);
            let segs = curve_segments(&curve, frame, tol)?;
            trim_to(&segs, &pts, tol)
        }
        _ => match crate::offset::surf_of(f, tol) {
            // A face of another make (an extruded cylinder, an imported cone or torus).
            Some(sf) => analytic_meridian(sf, frame, &pts, tol),
            // A free-form face (a revolved spline read from a STEP file as a NURBS surface).
            None => swept_meridian(f, frame, &pts, tol),
        },
    }
}

/// The meridian of a free-form face that is a surface of revolution about the frame's axis (its
/// normals all in the half-plane through the axis): the parameter line that runs along the
/// meridian, followed through its middle and fitted with cubic pieces.
fn swept_meridian(f: &mt::Face, frame: &Frame, pts: &[Vec2], tol: f64) -> Option<Vec<Seg2>> {
    use mt::{BoundedCurve, ParametricCurve, ParametricSurface, ParametricSurface3D, SearchNearestParameter};
    let surf = f.surface();
    // Seeds for the nearest-parameter search over the surface's whole parameter box (a search
    // from one end of a closed surface can settle on its far side).
    let bounds = |r: (std::ops::Bound<f64>, std::ops::Bound<f64>)| match r {
        (std::ops::Bound::Included(a) | std::ops::Bound::Excluded(a), std::ops::Bound::Included(b) | std::ops::Bound::Excluded(b)) => Some((a, b)),
        _ => None,
    };
    let (ur, vr) = match &surf {
        mt::Surface::BSplineSurface(x) => x.parameter_range(),
        mt::Surface::NurbsSurface(x) => x.parameter_range(),
        _ => return None,
    };
    let ((ua, ub), (va, vb)) = (bounds(ur)?, bounds(vr)?);
    let seeds: Vec<(f64, f64)> =
        (0..=6).flat_map(|i| (0..=6).map(move |j| (ua + (ub - ua) * i as f64 / 6.0, va + (vb - va) * j as f64 / 6.0))).collect();
    let nearest = |q: mt::Point3| -> Option<(f64, f64)> {
        let seed =
            seeds.iter().min_by(|x, y| from_p3(surf.subs(x.0, x.1)).dist(from_p3(q)).total_cmp(&from_p3(surf.subs(y.0, y.1)).dist(from_p3(q))))?;
        let x = surf.search_nearest_parameter(q, Some(*seed), 100)?;
        (from_p3(surf.subs(x.0, x.1)).dist(from_p3(q)) <= tol).then_some(x)
    };
    // The face's parameter box, from its boundary.
    let mut uv: Vec<(f64, f64)> = Vec::new();
    for w in f.boundaries() {
        for e in w.edge_iter() {
            let c = e.curve();
            let (t0, t1) = c.range_tuple();
            for k in 0..=8 {
                uv.push(nearest(c.subs(t0 + (t1 - t0) * k as f64 / 8.0))?);
            }
        }
    }
    let (u0, u1) = uv.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |m, x| (m.0.min(x.0), m.1.max(x.0)));
    let (v0, v1) = uv.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |m, x| (m.0.min(x.1), m.1.max(x.1)));
    if !(u1 > u0 && v1 > v0) {
        return None;
    }
    let around = |p: Vec3| {
        let w = p - frame.o;
        let r = w - frame.a * w.dot(frame.a);
        (r.len() > tol).then(|| frame.a.cross(r).normalized()).flatten()
    };
    // A surface of revolution about the axis: every normal in its point's half-plane.
    for i in 0..=8 {
        for j in 0..=8 {
            let (u, v) = (u0 + (u1 - u0) * i as f64 / 8.0, v0 + (v1 - v0) * j as f64 / 8.0);
            let n = surf.normal(u, v);
            let Some(n) = Vec3::new(n.x, n.y, n.z).normalized() else { continue };
            // (A fitted surface, such as a round's, holds its normals to about 1e-5.)
            if around(from_p3(surf.subs(u, v))).is_some_and(|t| n.dot(t).abs() > 1e-4) {
                return None;
            }
        }
    }
    // Which parameter runs along the meridian: the one whose direction never turns about the axis.
    let (um, vm) = ((u0 + u1) * 0.5, (v0 + v1) * 0.5);
    let t = around(from_p3(surf.subs(um, vm)))?;
    let du = surf.uder(um, vm);
    let dv = surf.vder(um, vm);
    let along = |d: mt::Vector3| {
        let d = Vec3::new(d.x, d.y, d.z);
        d.normalized().map(|x| x.dot(t).abs())
    };
    let (au, av) = (along(du)?, along(dv)?);
    let mut out = Vec::new();
    let along_u = au < 1e-4 && av > 0.5;
    let meridian = |s: f64| frame.meridian(from_p3(if along_u { surf.subs(s, vm) } else { surf.subs(um, s) }));
    let (s0, s1) = if along_u { (u0, u1) } else { (v0, v1) };
    if !along_u && !(av < 1e-4 && au > 0.5) {
        return None;
    }
    // A round's meridian is a circle: kept as an arc, its offset collapses cleanly at its radius.
    let samples: Vec<Vec2> = (0..=32).map(|k| meridian(s0 + (s1 - s0) * k as f64 / 32.0)).collect();
    match arc_on(&samples, tol * 0.1) {
        Some(arc) => out.push(arc),
        None => fit(&|s| Some(meridian(s)), s0, s1, tol * 0.1, 0, &mut out)?,
    }
    // Every point of the face on the meridian turned round: the face is that surface of
    // revolution, not just close to one where it was sampled.
    let on = |m: Vec2| out.iter().map(|s| s.point_at(s.closest_param(m)).dist(m)).fold(f64::INFINITY, f64::min) <= tol;
    let swept =
        (0..=8).all(|i| (0..=8).all(|j| on(frame.meridian(from_p3(surf.subs(u0 + (u1 - u0) * i as f64 / 8.0, v0 + (v1 - v0) * j as f64 / 8.0))))));
    if !swept {
        return None;
    }
    trim_to(&out, pts, tol)
}

/// A swept curve in meridian coordinates, exactly where it can be (lines, arcs, Bézier and
/// conic spans), checked to lie in one half-plane through the axis.
fn curve_segments(curve: &mt::Curve, frame: &Frame, tol: f64) -> Option<Vec<Seg2>> {
    use mt::{BoundedCurve, ParametricCurve};
    // The curve's half-plane, and the map into it (linear there).
    let (t0, t1) = curve.range_tuple();
    let samples: Vec<Vec3> = (0..=16).map(|k| from_p3(curve.subs(t0 + (t1 - t0) * k as f64 / 16.0))).collect();
    let u = samples.iter().find_map(|p| {
        let w = *p - frame.o;
        let r = w - frame.a * w.dot(frame.a);
        (r.len() > tol).then(|| r.normalized()).flatten()
    })?;
    let to2 = |p: Vec3| {
        let w = p - frame.o;
        Vec2::new(w.dot(u), w.dot(frame.a))
    };
    let off_plane = |p: Vec3| {
        let w = p - frame.o;
        (w - u * w.dot(u) - frame.a * w.dot(frame.a)).len()
    };
    if samples.iter().any(|p| off_plane(*p) > tol || to2(*p).x < -tol) {
        return None;
    }
    let segs: Vec<Seg2> = match curve {
        mt::Curve::Line(l) => vec![Seg2::Line { a: to2(from_p3(l.0)), b: to2(from_p3(l.1)) }],
        mt::Curve::BSplineCurve(c) => c
            .bezier_decomposition()
            .into_iter()
            .map(|span| bezier_seg(&span.control_points().iter().map(|p| to2(from_p3(*p))).collect::<Vec<_>>()))
            .collect::<Option<_>>()?,
        mt::Curve::NurbsCurve(c) => c
            .non_rationalized()
            .bezier_decomposition()
            .into_iter()
            .map(|span| {
                let ctrl = span.control_points();
                let pts: Vec<(Vec2, f64)> = ctrl.iter().map(|h| (to2(Vec3::new(h.x / h.w, h.y / h.w, h.z / h.w)), h.w)).collect();
                rational_seg(&pts, tol)
            })
            .collect::<Option<_>>()?,
        mt::Curve::IntersectionCurve(_) => return None,
    };
    // Every control point mapped into the half-plane must sit on the curve's own samples' side.
    (!segs.is_empty() && segs.iter().all(|s| s.start().is_finite() && s.end().is_finite())).then_some(segs)
}

/// A Bézier span from its control points: a line, a cubic (a quadratic raised to one), or a
/// higher piece.
fn bezier_seg(c: &[Vec2]) -> Option<Seg2> {
    match c {
        [a, b] => Some(Seg2::Line { a: *a, b: *b }),
        [p0, p1, p2] => Some(Seg2::Cubic { p0: *p0, p1: *p0 + (*p1 - *p0) * (2.0 / 3.0), p2: *p2 + (*p1 - *p2) * (2.0 / 3.0), p3: *p2 }),
        [p0, p1, p2, p3] => Some(Seg2::Cubic { p0: *p0, p1: *p1, p2: *p2, p3: *p3 }),
        _ => solidcraft_geom::bezier(c),
    }
}

/// A rational span (control points with weights): a circular arc where it is one (so its offset
/// stays an arc), else a conic or a polynomial piece; `None` for others.
fn rational_seg(c: &[(Vec2, f64)], tol: f64) -> Option<Seg2> {
    let all_one = c.iter().all(|(_, w)| (w - 1.0).abs() < 1e-12);
    if all_one {
        return bezier_seg(&c.iter().map(|(p, _)| *p).collect::<Vec<_>>());
    }
    let [(a, wa), (apex, wm), (b, wb)] = c else { return None };
    if !(*wa > 0.0 && *wb > 0.0 && *wm > 0.0) {
        return None;
    }
    // End weights to 1: w = wm / √(wa·wb).
    let w = wm / (wa * wb).sqrt();
    let conic = Seg2::Conic { a: *a, apex: *apex, b: *b, w };
    let samples: Vec<Vec2> = (0..=16).map(|k| conic.point_at(k as f64 / 16.0)).collect();
    Some(arc_on(&samples, tol).unwrap_or(conic))
}

/// The circular arc from the first sample to the last through the middle one, when every
/// sample lies on it within `tol`.
fn arc_on(samples: &[Vec2], tol: f64) -> Option<Seg2> {
    let (p, m, q) = (*samples.first()?, *samples.get(samples.len() / 2)?, *samples.last()?);
    let (center, radius) = circle_through(p, m, q)?;
    if samples.iter().any(|x| (x.dist(center) - radius).abs() >= tol) {
        return None;
    }
    let start = (p - center).angle();
    let turn = (m - p).cross(q - m);
    let mut sweep = (q - center).angle() - start;
    let tau = std::f64::consts::TAU;
    if turn > 0.0 {
        while sweep <= 0.0 {
            sweep += tau;
        }
    } else {
        while sweep >= 0.0 {
            sweep -= tau;
        }
    }
    Some(Seg2::Arc { center, radius, start, sweep })
}

fn circle_through(p: Vec2, m: Vec2, q: Vec2) -> Option<(Vec2, f64)> {
    let (b, c) = (m - p, q - p);
    let d = 2.0 * b.cross(c);
    if d.abs() < 1e-14 {
        return None;
    }
    let (bb, cc) = (b.len2(), c.len2());
    let center = p + Vec2::new(c.y * bb - b.y * cc, b.x * cc - c.x * bb) * (1.0 / d);
    let r = center.dist(p);
    (center.is_finite() && r.is_finite()).then_some((center, r))
}

/// The part of the chain `segs` between the boundary points' first and last places along it
/// (a face that covers only part of its swept curve).
fn trim_to(segs: &[Seg2], pts: &[Vec2], tol: f64) -> Option<Vec<Seg2>> {
    // Each point's place: (segment, parameter), nearest segment.
    let place = |p: Vec2| -> Option<(usize, f64, f64)> {
        segs.iter()
            .enumerate()
            .map(|(i, s)| {
                let t = s.closest_param(p);
                (i, t, s.point_at(t).dist(p))
            })
            .min_by(|x, y| x.2.total_cmp(&y.2))
    };
    // Places along the chain as segment index plus parameter.
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for p in pts {
        let (i, t, d) = place(*p)?;
        if d > tol * 10.0 {
            return None;
        }
        let k = i as f64 + t;
        lo = lo.min(k);
        hi = hi.max(k);
    }
    let split = |k: f64| -> (usize, f64) {
        let i = (k.floor().max(0.0) as usize).min(segs.len().saturating_sub(1));
        (i, (k - i as f64).clamp(0.0, 1.0))
    };
    let ((i0, t0), (i1, t1)) = (split(lo), split(hi));
    let mut out = Vec::new();
    for (i, s) in segs.iter().enumerate() {
        if i < i0 || i > i1 {
            continue;
        }
        let from = if i == i0 { t0 } else { 0.0 };
        let to = if i == i1 { t1 } else { 1.0 };
        if to - from < 1e-9 {
            continue;
        }
        out.push(sub(s, from, to));
    }
    (!out.is_empty()).then_some(out)
}

/// The part of `s` from parameter `from` to `to`.
fn sub(s: &Seg2, from: f64, to: f64) -> Seg2 {
    let mut piece = *s;
    if to < 1.0 - 1e-12 {
        piece = piece.split_at(to).0;
    }
    if from > 1e-12 {
        let rel = if to > 1e-12 { from / to } else { 0.0 };
        piece = piece.split_at(rel).1;
    }
    piece
}

/// The meridian of an analytic face of revolution about the frame's axis from its boundary's
/// meridian points: a line (cylinder, cone) or an arc (sphere, torus).
fn analytic_meridian(sf: Surf, frame: &Frame, pts: &[Vec2], tol: f64) -> Option<Vec<Seg2>> {
    let coaxial = |o: Vec3, a: Vec3| a.cross(frame.a).len() < 1e-9 && (o - frame.o).cross(frame.a).len() < tol;
    let line = || {
        // The two points furthest apart along the run.
        let (p, q) = pts.iter().flat_map(|p| pts.iter().map(move |q| (*p, *q))).max_by(|x, y| x.0.dist(x.1).total_cmp(&y.0.dist(y.1)))?;
        let d = (q - p).normalized()?;
        pts.iter().all(|x| (*x - p).cross(d).abs() < tol).then(|| vec![Seg2::Line { a: p, b: q }])
    };
    let arc = |center: Vec2, radius: f64| {
        if pts.iter().any(|x| (x.dist(center) - radius).abs() > tol) {
            return None;
        }
        // The smallest arc holding every point: the complement of the widest gap between angles.
        let mut ang: Vec<f64> = pts.iter().map(|x| (*x - center).angle()).collect();
        ang.sort_by(f64::total_cmp);
        let n = ang.len();
        let tau = std::f64::consts::TAU;
        let (gap_at, _) = (0..n)
            .map(|i| {
                let (a, b) = (ang.get(i).copied().unwrap_or(0.0), ang.get((i + 1) % n).copied().unwrap_or(0.0));
                (i, if i + 1 == n { b + tau - a } else { b - a })
            })
            .max_by(|x, y| x.1.total_cmp(&y.1))?;
        let start = ang.get((gap_at + 1) % n).copied()?;
        let end = ang.get(gap_at).copied()?;
        let mut sweep = end - start;
        while sweep <= 0.0 {
            sweep += tau;
        }
        (sweep > 1e-9 && sweep < tau - 1e-9).then(|| vec![Seg2::Arc { center, radius, start, sweep }])
    };
    match sf {
        Surf::Cylinder { o, a, .. } | Surf::Cone { v: o, a, .. } if coaxial(o, a) => line(),
        Surf::Sphere { c, r, .. } if (c - frame.o).cross(frame.a).len() < tol => arc(Vec2::new(0.0, (c - frame.o).dot(frame.a)), r),
        Surf::Torus { o, a, big, h, r, .. } if coaxial(o, a) => {
            let sign = if a.dot(frame.a) > 0.0 { 1.0 } else { -1.0 };
            arc(Vec2::new(big, (o - frame.o).dot(frame.a) + h * sign), r)
        }
        Surf::Plane { .. } => None,
        _ => None,
    }
}

/// The pieces as one chain from a point on the axis to another, each in chain order; `None`
/// when they don't make one (a ring around the axis, a branch, a piece left over).
fn chain_between_axis_points(mut pieces: Vec<Piece>, tol: f64) -> Option<Vec<Piece>> {
    let on_axis = |p: Vec2| p.x.abs() < tol;
    // Start at a piece with one end on the axis.
    let start = pieces.iter().position(|p| p.start().is_some_and(on_axis) || p.end().is_some_and(on_axis))?;
    let mut first = pieces.swap_remove(start);
    if !first.start().is_some_and(on_axis) {
        first = first.reversed();
    }
    let mut chain = vec![first];
    while let Some(at) = chain.last().and_then(Piece::end) {
        if on_axis(at) {
            break;
        }
        let next = pieces.iter().position(|p| p.start().is_some_and(|s| s.dist(at) < tol) || p.end().is_some_and(|e| e.dist(at) < tol))?;
        let mut p = pieces.swap_remove(next);
        if !p.start().is_some_and(|s| s.dist(at) < tol) {
            p = p.reversed();
        }
        chain.push(p);
    }
    (pieces.is_empty() && chain.len() >= 2).then_some(chain)
}

/// An offset wall: its segment moved `d` to its left, as a function of the segment's parameter
/// running on past both ends (along a line or circle, or a free curve's end tangents).
#[derive(Clone, Copy, Debug)]
enum Moved {
    Line { a: Vec2, b: Vec2 },
    Arc { center: Vec2, radius: f64, start: f64, sweep: f64 },
    Free { seg: Seg2, d: f64 },
}

impl Moved {
    fn of(s: &Seg2, d: f64, tol: f64) -> Option<Moved> {
        Some(match *s {
            Seg2::Line { a, b } => {
                let n = (b - a).normalized()?.perp() * d;
                Moved::Line { a: a + n, b: b + n }
            }
            Seg2::Arc { center, radius, start, sweep } => {
                // Left of a counter-clockwise arc is its centre.
                let r = radius - d * sweep.signum();
                if r <= tol {
                    return None;
                }
                Moved::Arc { center, radius: r, start, sweep }
            }
            seg => Moved::Free { seg, d },
        })
    }
    fn free_at(seg: &Seg2, d: f64, t: f64) -> Option<Vec2> {
        Some(seg.point_at(t) + seg.derivative(t).normalized()?.perp() * d)
    }
    fn free_tangent(seg: &Seg2, d: f64, t: f64) -> Option<Vec2> {
        let h = 1e-6;
        let (a, b) = ((t - h).max(0.0), (t + h).min(1.0));
        Some((Moved::free_at(seg, d, b)? - Moved::free_at(seg, d, a)?) * (1.0 / (b - a)))
    }
    fn at(&self, t: f64) -> Option<Vec2> {
        match *self {
            Moved::Line { a, b } => Some(a.lerp(b, t)),
            Moved::Arc { center, radius, start, sweep } => Some(center + Vec2::from_angle(start + sweep * t) * radius),
            Moved::Free { seg, d } => {
                if t < 0.0 {
                    Some(Moved::free_at(&seg, d, 0.0)? + Moved::free_tangent(&seg, d, 0.0)? * t)
                } else if t > 1.0 {
                    Some(Moved::free_at(&seg, d, 1.0)? + Moved::free_tangent(&seg, d, 1.0)? * (t - 1.0))
                } else {
                    Moved::free_at(&seg, d, t)
                }
            }
        }
    }
    fn tangent(&self, t: f64) -> Option<Vec2> {
        let h = 1e-7;
        Some((self.at(t + h)? - self.at(t - h)?) * (0.5 / h))
    }
    /// Where the moved free curve folds over itself (it bends tighter than the move), as a
    /// parameter; `None` when it does not between `from` and `to`.
    fn folds(&self, from: f64, to: f64) -> Option<f64> {
        let Moved::Free { seg, d } = *self else { return None };
        let (a, b) = (from.max(0.0), to.min(1.0));
        (0..=64).map(|k| a + (b - a) * k as f64 / 64.0).find(|&t| {
            let (Some(q), p) = (Moved::free_tangent(&seg, d, t), seg.derivative(t)) else { return false };
            q.dot(p) <= 0.0
        })
    }
    /// The part from `t0` to `t1` as segments within `tol` of it.
    fn segments(&self, t0: f64, t1: f64, tol: f64) -> Option<Vec<Seg2>> {
        match *self {
            Moved::Line { .. } => Some(vec![Seg2::Line { a: self.at(t0)?, b: self.at(t1)? }]),
            Moved::Arc { center, radius, start, sweep } => {
                Some(vec![Seg2::Arc { center, radius, start: start + sweep * t0, sweep: sweep * (t1 - t0) }])
            }
            Moved::Free { .. } => {
                let mut out = Vec::new();
                // Run-on parts are straight; the curve's own part is fitted.
                if t0 < 0.0 {
                    out.push(Seg2::Line { a: self.at(t0)?, b: self.at(t1.min(0.0))? });
                }
                let (c0, c1) = (t0.max(0.0), t1.min(1.0));
                if c1 > c0 + 1e-12 {
                    fit(&|t| self.at(t), c0, c1, tol, 0, &mut out)?;
                }
                if t1 > 1.0 {
                    out.push(Seg2::Line { a: self.at(t0.max(1.0))?, b: self.at(t1)? });
                }
                Some(out)
            }
        }
    }
}

/// Cubic Hermite pieces through `f` from `t0` to `t1`, halved until each is within `tol` of it.
fn fit(f: &dyn Fn(f64) -> Option<Vec2>, t0: f64, t1: f64, tol: f64, depth: u32, out: &mut Vec<Seg2>) -> Option<()> {
    let h = (t1 - t0) * 1e-5;
    let d = |t: f64| -> Option<Vec2> {
        let (a, b) = ((t - h).max(t0), (t + h).min(t1));
        Some((f(b)? - f(a)?) * ((t1 - t0) / (b - a)))
    };
    let (p0, p3) = (f(t0)?, f(t1)?);
    let (d0, d3) = (d(t0)?, d(t1)?);
    let cubic = Seg2::Cubic { p0, p1: p0 + d0 * (1.0 / 3.0), p2: p3 - d3 * (1.0 / 3.0), p3 };
    let err = (1..8)
        .map(|k| k as f64 / 8.0)
        .map(|s| f(t0 + (t1 - t0) * s).map(|x| x.dist(cubic.point_at(s))))
        .try_fold(0.0f64, |m, e| e.map(|e| m.max(e)))?;
    if err <= tol || depth >= 14 {
        if err > tol * 10.0 {
            return None;
        }
        out.push(cubic);
        return Some(());
    }
    let m = (t0 + t1) * 0.5;
    fit(f, t0, m, tol, depth + 1, out)?;
    fit(f, m, t1, tol, depth + 1, out)
}

/// Where `p` (near parameter `s`) and `q` (near `t`) cross: their parameters, by Newton's method.
fn cross(p: &Moved, q: &Moved, mut s: f64, mut t: f64) -> Option<(f64, f64)> {
    for _ in 0..60 {
        let r = p.at(s)? - q.at(t)?;
        if r.len() < 1e-11 {
            return Some((s, t));
        }
        let (ds, dt) = (p.tangent(s)?, q.tangent(t)?);
        // r + ds·δs − dt·δt = 0
        let det = ds.cross(-dt);
        if det.abs() < 1e-14 {
            return None;
        }
        let step_s = (-r).cross(-dt) / det;
        let step_t = ds.cross(-r) / det;
        let k = 1.0f64.min(0.5 / step_s.abs().max(step_t.abs()).max(1e-300));
        s += step_s * k;
        t += step_t * k;
        if !s.is_finite() || !t.is_finite() || s.abs() > 50.0 || t.abs() > 50.0 {
            return None;
        }
    }
    let r = p.at(s)? - q.at(t)?;
    (r.len() < 1e-8).then_some((s, t))
}

/// Where `p` (near parameter `s`) meets the line through `a` along `dir`.
fn on_line(p: &Moved, a: Vec2, dir: Vec2, mut s: f64) -> Option<f64> {
    let n = dir.normalized()?.perp();
    for _ in 0..60 {
        let g = (p.at(s)? - a).dot(n);
        if g.abs() < 1e-11 {
            return Some(s);
        }
        let dg = p.tangent(s)?.dot(n);
        if dg.abs() < 1e-14 {
            return None;
        }
        let step = (-g / dg).clamp(-0.5, 0.5);
        s += step;
        if !s.is_finite() || s.abs() > 50.0 {
            return None;
        }
    }
    ((p.at(s)? - a).dot(n).abs() < 1e-8).then_some(s)
}

/// The walls (from the axis to the rim) moved `d` to their left, joined sharp, from the axis
/// to the open face's line `rim`: segments, each with the faces of the wall it lines.
fn offset_chain(walls: &[(Seg2, Vec<usize>)], d: f64, rim: Seg2, size: f64, tol: f64) -> Result<Vec<(Seg2, Vec<usize>)>> {
    let thickness = d.abs();
    let too_thick = |why: &str| KernelError::Invalid(format!("the shell is too thick for this body: {thickness:.4} mm walls {why}"));
    let mut moved: Vec<(usize, Moved)> = Vec::new();
    for (i, (s, _)) in walls.iter().enumerate() {
        // A round smaller than the thickness goes; its neighbours meet sharp.
        if let Some(m) = Moved::of(s, d, tol) {
            moved.push((i, m));
        }
    }
    let Seg2::Line { a: rim_a, b: rim_b } = rim else { return Err(unsupported("the open face is not flat")) };
    // Each round: the parameters each moved wall runs between; walls run past by their
    // neighbours (thinner than the shell) go, and the rest meet again.
    for _ in 0..walls.len() + 1 {
        if moved.is_empty() {
            return Err(too_thick("leave no cavity"));
        }
        let n = moved.len();
        let mut range: Vec<(f64, f64)> = vec![(0.0, 1.0); n];
        // The first wall starts on the axis (x = 0).
        let (_, first) = moved.first().copied().ok_or_else(|| too_thick("leave no cavity"))?;
        let s0 = on_line(&first, Vec2::ZERO, Vec2::new(0.0, 1.0), 0.0).ok_or_else(|| unsupported("the inner wall does not reach the axis"))?;
        if let Some(r) = range.first_mut() {
            r.0 = s0;
        }
        for k in 0..n.saturating_sub(1) {
            let (Some(&(i, p)), Some(&(j, q))) = (moved.get(k), moved.get(k + 1)) else { continue };
            let (Some(sp), Some(sq)) = (walls.get(i).map(|w| w.0), walls.get(j).map(|w| w.0)) else { continue };
            let smooth = j == i + 1 && {
                let (tin, tout) = (sp.derivative(1.0).normalized(), sq.derivative(0.0).normalized());
                // (Fitted faces, such as a round's, meet their neighbours tangent to about 1e-7.)
                tin.zip(tout).is_some_and(|(x, y)| x.cross(y).abs() < 1e-5 && x.dot(y) > 0.0)
            };
            let (s, t) = if smooth { (1.0, 0.0) } else { cross(&p, &q, 1.0, 0.0).ok_or_else(|| unsupported("two inner walls that do not meet"))? };
            if let Some(r) = range.get_mut(k) {
                r.1 = s;
            }
            if let Some(r) = range.get_mut(k + 1) {
                r.0 = t;
            }
        }
        // The last wall ends on the open face's line.
        let (_, last) = moved.last().copied().ok_or_else(|| too_thick("leave no cavity"))?;
        let s1 = on_line(&last, rim_a, rim_b - rim_a, 1.0).ok_or_else(|| unsupported("the inner wall does not reach the open face"))?;
        if let Some(r) = range.last_mut() {
            r.1 = s1;
        }
        // Walls turned round (their ends crossed): gone.
        let gone: Vec<usize> = range.iter().enumerate().filter(|(_, r)| r.1 - r.0 <= 1e-9).map(|(k, _)| k).collect();
        if gone.is_empty() {
            let mut out: Vec<(Seg2, Vec<usize>)> = Vec::new();
            for (&(i, m), &(t0, t1)) in moved.iter().zip(&range) {
                if let Some(at) = m.folds(t0, t1) {
                    let p = m.at(at).unwrap_or(Vec2::ZERO);
                    return Err(too_thick(&format!("are thicker than a wall's curve is round near meridian point [{:.2}, {:.2}]", p.x, p.y)));
                }
                let faces = walls.get(i).map(|w| w.1.clone()).unwrap_or_default();
                for s in m.segments(t0, t1, (size * 1e-6).max(1e-6)).ok_or_else(|| unsupported("an inner wall that could not be fitted"))? {
                    out.push((s, faces.clone()));
                }
            }
            check_inner(&out, tol).map_err(|w| too_thick(&w))?;
            return Ok(out);
        }
        moved = moved.into_iter().enumerate().filter(|(k, _)| !gone.contains(k)).map(|(_, m)| m).collect();
    }
    Err(too_thick("leave no cavity"))
}

/// The inner walls must keep to the axis's side.
fn check_inner(inner: &[(Seg2, Vec<usize>)], tol: f64) -> std::result::Result<(), String> {
    let pts = inner.iter().flat_map(|(s, _)| (0..=32).map(move |k| s.point_at(k as f64 / 32.0)));
    match pts.into_iter().find(|p| p.x < -tol) {
        Some(p) => Err(format!("would cross the axis (at height {:.3})", p.y)),
        None => Ok(()),
    }
}

/// Where the closed outline `segs` crosses itself (on a polyline within `tol`), if it does: the
/// inner walls cutting through the outer ones, or folding over themselves.
fn crossing(segs: &[Seg2], tol: f64) -> Option<Vec2> {
    let poly: Vec<Vec2> = Loop2 { segs: segs.to_vec() }.polyline(tol).into_iter().collect();
    let n = poly.len();
    for i in 0..n {
        for j in (i + 2)..n {
            if i == 0 && j + 1 == n {
                continue;
            }
            let (Some(a), Some(b), Some(c), Some(e)) = (poly.get(i), poly.get((i + 1) % n), poly.get(j), poly.get((j + 1) % n)) else { continue };
            let (r, q) = (*b - *a, *e - *c);
            let den = r.cross(q);
            if den.abs() < 1e-15 {
                continue;
            }
            let (u, v) = ((*c - *a).cross(q) / den, (*c - *a).cross(r) / den);
            if u > 1e-9 && u < 1.0 - 1e-9 && v > 1e-9 && v < 1.0 - 1e-9 {
                return Some(*a + r * u);
            }
        }
    }
    None
}
