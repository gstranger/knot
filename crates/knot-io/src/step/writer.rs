//! BRep to STEP AP203 writer.
//!
//! Walks the BRep topology tree, assigns sequential entity IDs, and emits
//! STEP entities in dependency order. Supports all analytical surface and
//! curve types the kernel represents.

use std::collections::HashMap;
use std::fmt::Write;
use std::sync::Arc;
use knot_core::{KResult, KernelError};
use knot_geom::Point3;
use knot_geom::Vector3;
use knot_geom::curve::{Curve, CircularArc, EllipticalArc};
use knot_geom::surface::{Surface, Plane, Cylinder, Sphere};
use knot_topo::*;

/// Write a BRep to a STEP AP203 string.
///
/// Before serializing, performs a trim-curve upgrade pass: for each
/// edge that lies between two analytical surfaces (plane/cylinder/
/// sphere combinations), if the edge is currently a polygonal `Line`
/// approximation of the analytical intersection, emit the analytical
/// curve (CIRCLE / ELLIPSE) instead. Downstream CAD tools then see
/// clean geometry instead of N-segment polylines.
///
/// The kernel keeps its internal polygonal representation (good for
/// boolean robustness); the upgrade is export-time only.
pub fn write_step(brep: &BRep) -> KResult<String> {
    let edge_surfaces = build_edge_surface_map(brep);
    let mut ctx = WriteContext::new(edge_surfaces);
    let solid_ids = ctx.collect_brep(brep)?;

    let mut out = String::with_capacity(4096);
    write_header(&mut out);
    writeln!(out, "DATA;").unwrap();
    ctx.emit_all(&mut out);
    for sid in &solid_ids {
        writeln!(out, "#{}=MANIFOLD_SOLID_BREP('',#{});", sid.solid_id, sid.shell_id).unwrap();
    }
    writeln!(out, "ENDSEC;").unwrap();
    writeln!(out, "END-ISO-10303-21;").unwrap();
    Ok(out)
}

fn write_header(out: &mut String) {
    writeln!(out, "ISO-10303-21;").unwrap();
    writeln!(out, "HEADER;").unwrap();
    writeln!(out, "FILE_DESCRIPTION(('knot-cad export'),'2;1');").unwrap();
    writeln!(out, "FILE_NAME('export.stp','2025-01-01',(''),(''),'knot','','');").unwrap();
    writeln!(out, "FILE_SCHEMA(('CONFIG_CONTROL_DESIGN'));").unwrap();
    writeln!(out, "ENDSEC;").unwrap();
}

// ═══════════════════════════════════════════════════════════════════
// Entity collection context
// ═══════════════════════════════════════════════════════════════════

struct SolidIds {
    solid_id: u64,
    shell_id: u64,
}

struct WriteContext {
    next_id: u64,
    /// Ordered list of entity lines to emit (id, line)
    entities: Vec<(u64, String)>,
    /// Dedup caches keyed on content to avoid duplicate entities
    point_cache: HashMap<PointKey, u64>,
    direction_cache: HashMap<PointKey, u64>,
    vertex_cache: HashMap<u64, u64>,   // vertex Id hash → STEP entity id
    edge_cache: HashMap<u64, u64>,     // edge Id hash → STEP EDGE_CURVE id
    surface_cache: HashMap<u64, u64>,  // surface pointer → STEP entity id
    /// sorted-vertex-pair-key → list of adjacent face surfaces. Used
    /// at edge-export time to upgrade polygonal trim curves to their
    /// analytical form (e.g. plane∩cylinder = circle).
    ///
    /// Key is `(min(vid_a, vid_b), max(vid_a, vid_b))` of the vertex
    /// content hashes, not the edge hash. Primitive constructors
    /// (`make_cylinder`, `make_box`, …) build separate `Edge` Arcs
    /// for adjacent faces sharing the same endpoint vertices; using
    /// the edge hash directly would leave each edge entry with only
    /// its own face's surface. Vertex content hashes are stable and
    /// shared as long as the underlying Point3 bit-equals.
    edge_surfaces: HashMap<(u64, u64), Vec<Arc<Surface>>>,
}

/// Hashable 3D point key (f64 bits).
#[derive(Clone, Copy, Hash, PartialEq, Eq)]
struct PointKey(u64, u64, u64);

impl PointKey {
    fn from_point(p: &Point3) -> Self {
        Self(p.x.to_bits(), p.y.to_bits(), p.z.to_bits())
    }
    fn from_vec(v: &Vector3) -> Self {
        Self(v.x.to_bits(), v.y.to_bits(), v.z.to_bits())
    }
}

impl WriteContext {
    fn new(edge_surfaces: HashMap<(u64, u64), Vec<Arc<Surface>>>) -> Self {
        Self {
            next_id: 1,
            entities: Vec::new(),
            point_cache: HashMap::new(),
            direction_cache: HashMap::new(),
            vertex_cache: HashMap::new(),
            edge_cache: HashMap::new(),
            surface_cache: HashMap::new(),
            edge_surfaces,
        }
    }

    fn alloc(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn push(&mut self, id: u64, line: String) {
        self.entities.push((id, line));
    }

    fn emit_all(&self, out: &mut String) {
        for (id, line) in &self.entities {
            writeln!(out, "#{}={}", id, line).unwrap();
        }
    }

    // ── Top-level BRep collection ──

    fn collect_brep(&mut self, brep: &BRep) -> KResult<Vec<SolidIds>> {
        let mut ids = Vec::new();
        for solid in brep.solids() {
            let shell_id = self.collect_shell(solid.outer_shell())?;
            let solid_id = self.alloc();
            ids.push(SolidIds { solid_id, shell_id });
        }
        Ok(ids)
    }

    fn collect_shell(&mut self, shell: &Shell) -> KResult<u64> {
        let mut face_ids = Vec::new();
        for face in shell.faces() {
            face_ids.push(self.collect_face(face)?);
        }
        let shell_id = self.alloc();
        let refs = id_list(&face_ids);
        self.push(shell_id, format!("CLOSED_SHELL('',({refs}));"));
        Ok(shell_id)
    }

    fn collect_face(&mut self, face: &Face) -> KResult<u64> {
        let surface_id = self.collect_surface(face.surface())?;
        let outer_bound_id = self.collect_loop(face.outer_loop(), true)?;

        let mut bound_ids = vec![outer_bound_id];
        for inner in face.inner_loops() {
            bound_ids.push(self.collect_loop(inner, false)?);
        }

        let sense = if face.same_sense() { ".T." } else { ".F." };
        let face_id = self.alloc();
        let refs = id_list(&bound_ids);
        self.push(face_id, format!("ADVANCED_FACE('',({refs}),#{surface_id},{sense});"));
        Ok(face_id)
    }

    fn collect_loop(&mut self, loop_: &Loop, is_outer: bool) -> KResult<u64> {
        let mut oe_ids = Vec::new();
        for he in loop_.half_edges() {
            oe_ids.push(self.collect_half_edge(he)?);
        }

        let loop_id = self.alloc();
        let refs = id_list(&oe_ids);
        self.push(loop_id, format!("EDGE_LOOP('',({refs}));"));

        let bound_type = if is_outer { "FACE_OUTER_BOUND" } else { "FACE_BOUND" };
        let bound_id = self.alloc();
        self.push(bound_id, format!("{bound_type}('',#{loop_id},.T.);"));
        Ok(bound_id)
    }

    fn collect_half_edge(&mut self, he: &HalfEdge) -> KResult<u64> {
        let edge_id = self.collect_edge(he.edge())?;
        let sense = if he.same_sense() { ".T." } else { ".F." };
        let oe_id = self.alloc();
        self.push(oe_id, format!("ORIENTED_EDGE('',*,*,#{edge_id},{sense});"));
        Ok(oe_id)
    }

    fn collect_edge(&mut self, edge: &Edge) -> KResult<u64> {
        let edge_hash = edge.id().hash_value();
        if let Some(&id) = self.edge_cache.get(&edge_hash) {
            return Ok(id);
        }

        let v_start = self.collect_vertex(edge.start())?;
        let v_end = self.collect_vertex(edge.end())?;
        // Export-time trim upgrade: if the edge is a polygonal Line
        // approximation but its adjacent surfaces have an analytical
        // intersection form (plane∩cylinder=circle, plane∩sphere=circle),
        // emit the analytical curve instead. Falls back to the original
        // curve when no upgrade applies.
        let upgraded = upgrade_curve_for_export(
            edge.curve(),
            edge.start().point(),
            edge.end().point(),
            self.edge_surfaces.get(&edge_vertex_key(edge)),
        );
        let curve_id = self.collect_curve(&upgraded, edge.start().point(), edge.end().point())?;

        let edge_id = self.alloc();
        self.push(edge_id, format!(
            "EDGE_CURVE('',#{v_start},#{v_end},#{curve_id},.T.);"
        ));
        self.edge_cache.insert(edge_hash, edge_id);
        Ok(edge_id)
    }

    fn collect_vertex(&mut self, vertex: &Vertex) -> KResult<u64> {
        let vh = vertex.id().hash_value();
        if let Some(&id) = self.vertex_cache.get(&vh) {
            return Ok(id);
        }

        let pt_id = self.collect_point(vertex.point())?;
        let vp_id = self.alloc();
        self.push(vp_id, format!("VERTEX_POINT('',#{pt_id});"));
        self.vertex_cache.insert(vh, vp_id);
        Ok(vp_id)
    }

    // ── Geometry entities ──

    fn collect_point(&mut self, p: &Point3) -> KResult<u64> {
        let key = PointKey::from_point(p);
        if let Some(&id) = self.point_cache.get(&key) {
            return Ok(id);
        }
        let id = self.alloc();
        self.push(id, format!("CARTESIAN_POINT('',({},{},{}));", fmt_f(p.x), fmt_f(p.y), fmt_f(p.z)));
        self.point_cache.insert(key, id);
        Ok(id)
    }

    fn collect_direction(&mut self, v: &Vector3) -> KResult<u64> {
        let key = PointKey::from_vec(v);
        if let Some(&id) = self.direction_cache.get(&key) {
            return Ok(id);
        }
        let id = self.alloc();
        self.push(id, format!("DIRECTION('',({},{},{}));", fmt_f(v.x), fmt_f(v.y), fmt_f(v.z)));
        self.direction_cache.insert(key, id);
        Ok(id)
    }

    fn collect_axis2(&mut self, origin: &Point3, axis: &Vector3, ref_dir: &Vector3) -> KResult<u64> {
        let pt_id = self.collect_point(origin)?;
        let ax_id = self.collect_direction(axis)?;
        let rd_id = self.collect_direction(ref_dir)?;
        let id = self.alloc();
        self.push(id, format!("AXIS2_PLACEMENT_3D('',#{pt_id},#{ax_id},#{rd_id});"));
        Ok(id)
    }

    fn collect_curve(&mut self, curve: &Curve, start: &Point3, end: &Point3) -> KResult<u64> {
        match curve {
            Curve::Line(_) => {
                let dir = end - start;
                let mag = dir.norm();
                let dir_n = if mag > 1e-15 { dir / mag } else { Vector3::x() };

                let pt_id = self.collect_point(start)?;
                let dir_id = self.collect_direction(&dir_n)?;

                let vec_id = self.alloc();
                self.push(vec_id, format!("VECTOR('',#{dir_id},{});", fmt_f(mag)));

                let line_id = self.alloc();
                self.push(line_id, format!("LINE('',#{pt_id},#{vec_id});"));
                Ok(line_id)
            }
            Curve::CircularArc(arc) => {
                let axis_id = self.collect_axis2(&arc.center, &arc.normal, &arc.ref_direction)?;
                let id = self.alloc();
                self.push(id, format!("CIRCLE('',#{axis_id},{});", fmt_f(arc.radius)));
                Ok(id)
            }
            Curve::EllipticalArc(arc) => {
                let axis_id = self.collect_axis2(&arc.center, &arc.normal, &arc.major_axis)?;
                let id = self.alloc();
                self.push(id, format!(
                    "ELLIPSE('',#{axis_id},{},{});",
                    fmt_f(arc.major_radius), fmt_f(arc.minor_radius)
                ));
                Ok(id)
            }
            Curve::Nurbs(nurbs) => {
                self.collect_bspline_curve(nurbs)
            }
        }
    }

    fn collect_bspline_curve(&mut self, nurbs: &knot_geom::curve::NurbsCurve) -> KResult<u64> {
        let degree = nurbs.degree();
        let cps = nurbs.control_points();
        let knots = nurbs.knots();

        // Collect control point entities
        let mut cp_ids = Vec::new();
        for cp in cps {
            cp_ids.push(self.collect_point(cp)?);
        }

        // Compress knots to (unique_values, multiplicities)
        let (knot_vals, mults) = compress_knots(knots);

        let cp_refs = id_list(&cp_ids);
        let mult_str = mults.iter().map(|m| m.to_string()).collect::<Vec<_>>().join(",");
        let knot_str = knot_vals.iter().map(|k| fmt_f(*k)).collect::<Vec<_>>().join(",");

        let id = self.alloc();
        self.push(id, format!(
            "B_SPLINE_CURVE_WITH_KNOTS('',{},({}),.UNSPECIFIED.,.U.,.U.,({},),({},),.UNSPECIFIED.);",
            degree, cp_refs, mult_str, knot_str
        ));
        Ok(id)
    }

    fn collect_surface(&mut self, surface: &Surface) -> KResult<u64> {
        // Use Arc pointer as cache key for surface dedup
        let ptr = std::sync::Arc::as_ptr(&std::sync::Arc::new(0u8)) as u64; // placeholder
        // Actually, we can't easily get the Arc pointer from &Surface.
        // Use a simple approach: don't cache surfaces (they're per-face anyway).
        // In practice, many faces share the same Arc<Surface>, but we'll emit
        // duplicates. This is valid STEP — just slightly larger files.

        match surface {
            Surface::Plane(p) => {
                // Compute a ref_direction for the plane
                let ref_dir = p.u_axis;
                let axis_id = self.collect_axis2(&p.origin, &p.normal, &ref_dir)?;
                let id = self.alloc();
                self.push(id, format!("PLANE('',#{axis_id});"));
                Ok(id)
            }
            Surface::Cylinder(c) => {
                let axis_id = self.collect_axis2(&c.origin, &c.axis, &c.ref_direction)?;
                let id = self.alloc();
                self.push(id, format!("CYLINDRICAL_SURFACE('',#{axis_id},{});", fmt_f(c.radius)));
                Ok(id)
            }
            Surface::Sphere(s) => {
                // Sphere needs an axis placement — use z-up
                let axis = Vector3::z();
                let ref_dir = Vector3::x();
                let axis_id = self.collect_axis2(&s.center, &axis, &ref_dir)?;
                let id = self.alloc();
                self.push(id, format!("SPHERICAL_SURFACE('',#{axis_id},{});", fmt_f(s.radius)));
                Ok(id)
            }
            Surface::Cone(c) => {
                let axis_id = self.collect_axis2(&c.apex, &c.axis, &c.ref_direction)?;
                let id = self.alloc();
                self.push(id, format!(
                    "CONICAL_SURFACE('',#{axis_id},{},{});",
                    fmt_f(0.0), fmt_f(c.half_angle) // radius at apex is 0
                ));
                Ok(id)
            }
            Surface::Torus(t) => {
                let axis_id = self.collect_axis2(&t.center, &t.axis, &t.ref_direction)?;
                let id = self.alloc();
                self.push(id, format!(
                    "TOROIDAL_SURFACE('',#{axis_id},{},{});",
                    fmt_f(t.major_radius), fmt_f(t.minor_radius)
                ));
                Ok(id)
            }
            Surface::Nurbs(nurbs) => {
                self.collect_bspline_surface(nurbs)
            }
        }
    }

    fn collect_bspline_surface(&mut self, nurbs: &knot_geom::surface::NurbsSurface) -> KResult<u64> {
        let u_deg = nurbs.degree_u();
        let v_deg = nurbs.degree_v();
        let cps = nurbs.control_points();
        let count_u = nurbs.count_u() as usize;
        let count_v = nurbs.count_v() as usize;
        let knots_u = nurbs.knots_u();
        let knots_v = nurbs.knots_v();

        // Collect control points as nested rows
        let mut cp_rows = Vec::new();
        for iu in 0..count_u {
            let mut row_ids = Vec::new();
            for iv in 0..count_v {
                let idx = iu * count_v + iv;
                row_ids.push(self.collect_point(&cps[idx])?);
            }
            cp_rows.push(format!("({})", id_list(&row_ids)));
        }
        let cp_grid = cp_rows.join(",");

        let (u_knot_vals, u_mults) = compress_knots(knots_u);
        let (v_knot_vals, v_mults) = compress_knots(knots_v);

        let u_mult_str = u_mults.iter().map(|m| m.to_string()).collect::<Vec<_>>().join(",");
        let v_mult_str = v_mults.iter().map(|m| m.to_string()).collect::<Vec<_>>().join(",");
        let u_knot_str = u_knot_vals.iter().map(|k| fmt_f(*k)).collect::<Vec<_>>().join(",");
        let v_knot_str = v_knot_vals.iter().map(|k| fmt_f(*k)).collect::<Vec<_>>().join(",");

        let id = self.alloc();
        self.push(id, format!(
            "B_SPLINE_SURFACE_WITH_KNOTS('',{},{},({}),.UNSPECIFIED.,.U.,.U.,.U.,({},),({},),({},),({},),.UNSPECIFIED.);",
            u_deg, v_deg, cp_grid,
            u_mult_str, v_mult_str, u_knot_str, v_knot_str
        ));
        Ok(id)
    }
}

// ═══════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════

/// Format a float for STEP output. Ensures a decimal point is always present.
fn fmt_f(v: f64) -> String {
    if v == 0.0 {
        "0.".to_string()
    } else if v.fract() == 0.0 {
        format!("{}.", v)
    } else {
        // Use enough precision to round-trip
        let s = format!("{:.15}", v);
        // Trim trailing zeros but keep at least one decimal digit
        let s = s.trim_end_matches('0');
        let s = if s.ends_with('.') { format!("{}0", s) } else { s.to_string() };
        s
    }
}

/// Format a list of entity IDs as "#1,#2,#3".
fn id_list(ids: &[u64]) -> String {
    ids.iter().map(|id| format!("#{id}")).collect::<Vec<_>>().join(",")
}

// ═══════════════════════════════════════════════════════════════════
// Trim-curve upgrade (export-time)
// ═══════════════════════════════════════════════════════════════════

/// Per-edge tolerance for accepting an analytical upgrade. The line
/// endpoints have to lie on the proposed curve within this distance,
/// otherwise we keep the polygonal form to stay correct.
const UPGRADE_TOL: f64 = 1e-6;

/// Walk the BRep once and accumulate, for each edge, the surfaces of
/// every face that uses that edge in its outer / inner loops. Used
/// by the writer to decide whether a polygonal trim curve can be
/// upgraded to its analytical form.
fn build_edge_surface_map(brep: &BRep) -> HashMap<(u64, u64), Vec<Arc<Surface>>> {
    let mut map: HashMap<(u64, u64), Vec<Arc<Surface>>> = HashMap::new();
    for solid in brep.solids() {
        for face in solid.outer_shell().faces() {
            let surface = face.surface().clone();
            let loops_iter = std::iter::once(face.outer_loop())
                .chain(face.inner_loops().iter());
            for loop_ in loops_iter {
                for he in loop_.half_edges() {
                    let key = edge_vertex_key(he.edge());
                    map.entry(key).or_default().push(surface.clone());
                }
            }
        }
    }
    map
}

fn edge_vertex_key(edge: &Edge) -> (u64, u64) {
    let a = edge.start().id().hash_value();
    let b = edge.end().id().hash_value();
    if a <= b { (a, b) } else { (b, a) }
}

/// If `curve` is a polygonal `Line` between two analytical surfaces
/// with a recognized intersection form, return the analytical curve.
/// Otherwise return `curve.clone()`. Validates endpoints lie on the
/// proposed curve within `UPGRADE_TOL` before accepting.
fn upgrade_curve_for_export(
    curve: &Curve,
    start: &Point3,
    end: &Point3,
    surfaces: Option<&Vec<Arc<Surface>>>,
) -> Curve {
    if !matches!(curve, Curve::Line(_)) {
        return curve.clone();
    }
    let surfaces = match surfaces {
        Some(s) if s.len() >= 2 => s,
        _ => return curve.clone(),
    };
    // Try every ordered pair of adjacent surfaces. The first match wins.
    for i in 0..surfaces.len() {
        for j in 0..surfaces.len() {
            if i == j { continue; }
            if let Some(c) = recognize_curve_form(
                surfaces[i].as_ref(),
                surfaces[j].as_ref(),
                start,
                end,
            ) {
                return c;
            }
        }
    }
    curve.clone()
}

/// Match the ordered surface pair against known analytical
/// intersection forms (plane∩cylinder = circle or ellipse,
/// plane∩sphere = circle). Returns the analytical curve when the
/// line endpoints lie on it within tolerance; otherwise `None`.
fn recognize_curve_form(s_a: &Surface, s_b: &Surface, start: &Point3, end: &Point3) -> Option<Curve> {
    match (s_a, s_b) {
        (Surface::Plane(p), Surface::Cylinder(c)) => plane_cylinder_form(p, c, start, end),
        (Surface::Plane(p), Surface::Sphere(s)) =>
            plane_sphere_arc(p, s, start, end).map(Curve::CircularArc),
        _ => None,
    }
}

/// Plane ∩ Cylinder is:
///   - a circle when the plane is perpendicular to the cylinder axis,
///   - an ellipse when oblique (axis crosses the plane non-perpendicularly),
///   - two parallel lines or a single tangent line when the plane
///     contains (or is parallel to) the axis — we leave those as
///     polylines.
///
/// The center is where the cylinder axis pierces the plane. For the
/// ellipse case: minor radius = cylinder radius, major radius =
/// `r / |a·n|`, major axis = axis projected into the plane, normalized.
fn plane_cylinder_form(p: &Plane, c: &Cylinder, start: &Point3, end: &Point3) -> Option<Curve> {
    let n = p.normal.normalize();
    let axis = c.axis.normalize();
    let an = axis.dot(&n);
    // Axis parallel to (or contained in) the plane: not a closed
    // intersection. Skip.
    if an.abs() < 1e-9 {
        return None;
    }
    // Center: solve (c.origin + t·axis - p.origin) · n = 0.
    let t = -(c.origin - p.origin).dot(&n) / an;
    let center = c.origin + axis * t;

    // Perpendicular case → circle.
    if (an.abs() - 1.0).abs() <= 1e-6 {
        let radius = c.radius;
        if !point_on_circle(start, &center, &axis, radius) { return None; }
        if !point_on_circle(end, &center, &axis, radius) { return None; }
        let (ref_dir, start_angle, end_angle) = arc_frame_and_angles(start, end, &center, &axis)?;
        return Some(Curve::CircularArc(CircularArc {
            center,
            normal: axis,
            radius,
            ref_direction: ref_dir,
            start_angle,
            end_angle,
        }));
    }

    // Oblique case → ellipse in the plane.
    let minor_radius = c.radius;
    let major_radius = c.radius / an.abs();
    // Major axis direction: project the cylinder axis onto the plane,
    // normalize. The projection's magnitude is sqrt(1 - (a·n)²) ≥
    // approx 1e-4 here because of the `|an| < 1 - 1e-6` branch.
    let axis_in_plane = axis - n * an;
    let axis_in_plane_len = axis_in_plane.norm();
    if axis_in_plane_len < 1e-9 {
        return None; // shouldn't happen given an check, but be defensive
    }
    let major_dir = axis_in_plane / axis_in_plane_len;
    // Validate endpoints lie on the ellipse within tolerance.
    if !point_on_ellipse(start, &center, &n, &major_dir, major_radius, minor_radius) {
        return None;
    }
    if !point_on_ellipse(end, &center, &n, &major_dir, major_radius, minor_radius) {
        return None;
    }
    let (start_angle, end_angle) =
        ellipse_angles(start, end, &center, &n, &major_dir, major_radius, minor_radius)?;
    Some(Curve::EllipticalArc(EllipticalArc {
        center,
        normal: n,
        major_axis: major_dir,
        major_radius,
        minor_radius,
        start_angle,
        end_angle,
    }))
}

/// Plane ∩ Sphere is always a circle (or empty); center is the
/// projection of the sphere center onto the plane, radius is
/// `sqrt(r² - d²)` where d is sphere-center-to-plane distance.
fn plane_sphere_arc(p: &Plane, s: &Sphere, start: &Point3, end: &Point3) -> Option<CircularArc> {
    let n = p.normal.normalize();
    let d = n.dot(&(s.center - p.origin));
    let r2 = s.radius * s.radius - d * d;
    if r2 < 1e-18 {
        return None; // plane tangent or missing the sphere
    }
    let radius = r2.sqrt();
    let center = s.center - n * d;
    if !point_on_circle(start, &center, &n, radius) { return None; }
    if !point_on_circle(end, &center, &n, radius) { return None; }
    let (ref_dir, start_angle, end_angle) = arc_frame_and_angles(start, end, &center, &n)?;
    Some(CircularArc {
        center,
        normal: n,
        radius,
        ref_direction: ref_dir,
        start_angle,
        end_angle,
    })
}

fn point_on_circle(p: &Point3, center: &Point3, normal: &Vector3, radius: f64) -> bool {
    let v = p - center;
    let along = v.dot(normal);
    if along.abs() > UPGRADE_TOL { return false; }
    let radial = (v - normal * along).norm();
    (radial - radius).abs() <= UPGRADE_TOL
}

/// Point lies on the ellipse defined by (center, normal, major_dir,
/// a, b) — i.e. in the plane (within tol) AND (u/a)² + (v/b)² ≈ 1
/// where (u, v) are the point's coords in the ellipse's local frame.
fn point_on_ellipse(
    p: &Point3,
    center: &Point3,
    normal: &Vector3,
    major_dir: &Vector3,
    a: f64,
    b: f64,
) -> bool {
    let v = p - center;
    let along = v.dot(normal);
    if along.abs() > UPGRADE_TOL { return false; }
    let minor_dir = normal.cross(major_dir);
    let u = v.dot(major_dir);
    let w = v.dot(&minor_dir);
    // Algebraic ellipse residual (u/a)² + (w/b)² - 1; scale-aware
    // tolerance so a 0.5e-7 absolute error on a unit ellipse passes.
    let residual = (u / a).powi(2) + (w / b).powi(2) - 1.0;
    residual.abs() <= UPGRADE_TOL * (1.0 + 1.0 / a.min(b))
}

/// Solve the ellipse parameter t for each of `start` and `end` from
/// the ellipse local frame: t such that point = center + a·cos(t)·M + b·sin(t)·m
/// where M = major_dir, m = normal × major_dir. Returns (start_angle,
/// end_angle), with end_angle normalized to lie in [start_angle,
/// start_angle + TAU).
fn ellipse_angles(
    start: &Point3,
    end: &Point3,
    center: &Point3,
    normal: &Vector3,
    major_dir: &Vector3,
    a: f64,
    b: f64,
) -> Option<(f64, f64)> {
    let minor_dir = normal.cross(major_dir);
    let angle_of = |p: &Point3| -> f64 {
        let v = p - center;
        let u = v.dot(major_dir) / a;
        let w = v.dot(&minor_dir) / b;
        w.atan2(u)
    };
    let s = angle_of(start);
    let mut e = angle_of(end);
    // Normalize so e ∈ (s, s + TAU].
    while e <= s { e += std::f64::consts::TAU; }
    while e > s + std::f64::consts::TAU { e -= std::f64::consts::TAU; }
    Some((s, e))
}

/// Build a (ref_direction, start_angle, end_angle) tuple for an arc
/// whose underlying circle is centered at `center` with the given
/// `normal`. `ref_direction` is chosen so that `start` lies at the
/// `start_angle` measured from it.
fn arc_frame_and_angles(
    start: &Point3,
    end: &Point3,
    center: &Point3,
    normal: &Vector3,
) -> Option<(Vector3, f64, f64)> {
    let n = normal.normalize();
    let v_start = start - center;
    let v_end = end - center;
    let r_start = v_start - n * v_start.dot(&n);
    let r_end = v_end - n * v_end.dot(&n);
    let r_len = r_start.norm();
    if r_len < UPGRADE_TOL { return None; }
    let ref_dir = r_start / r_len;
    let bi = n.cross(&ref_dir);
    let u_end = r_end.dot(&ref_dir);
    let v_end = r_end.dot(&bi);
    let mut end_angle = v_end.atan2(u_end);
    if end_angle < 0.0 {
        end_angle += std::f64::consts::TAU;
    }
    Some((ref_dir, 0.0, end_angle))
}

/// Compress an expanded knot vector into (unique_values, multiplicities).
fn compress_knots(knots: &[f64]) -> (Vec<f64>, Vec<u32>) {
    let mut vals = Vec::new();
    let mut mults = Vec::new();
    for &k in knots {
        if vals.last().map_or(true, |last: &f64| (k - last).abs() > 1e-12) {
            vals.push(k);
            mults.push(1);
        } else {
            *mults.last_mut().unwrap() += 1;
        }
    }
    (vals, mults)
}
