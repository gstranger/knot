use wasm_bindgen::prelude::*;
use knot_geom::Point3;
use knot_geom::Vector3;
use knot_geom::curve::NurbsCurve;
use knot_geom::surface::{NurbsSurface, Sphere, Plane};
use knot_topo::*;

/// Convert a `KernelError` into a `JsError` with a stable, parseable
/// message format: `"E<code>:<kind>:<detail>"`. Consumers on the JS
/// side use the matching `KnotError` / `parseKnotError` helpers in
/// `js/src/error.ts` to discriminate failure modes without parsing
/// free-form English.
fn kernel_err_to_js(e: knot_core::KernelError) -> JsError {
    JsError::new(&format_kernel_error(&e))
}

fn format_kernel_error(e: &knot_core::KernelError) -> String {
    use knot_core::KernelError::*;
    let (code, kind, detail) = match e {
        InvalidGeometry { code, detail } => (Some(*code), "InvalidGeometry", detail.as_str()),
        TopoInconsistency { code, detail } => (Some(*code), "TopoInconsistency", detail.as_str()),
        IntersectionFailure { code, detail } => (Some(*code), "IntersectionFailure", detail.as_str()),
        OperationFailed { code, detail } => (Some(*code), "OperationFailed", detail.as_str()),
        InvalidInput { code, detail } => (Some(*code), "InvalidInput", detail.as_str()),
        Degenerate { code, detail } => (Some(*code), "Degenerate", detail.as_str()),
        NumericalFailure { code, detail } => (Some(*code), "NumericalFailure", detail.as_str()),
        // Io has no ErrorCode field; use E000 as a placeholder so the
        // format is uniform on the JS side.
        Io { detail } => (None, "Io", detail.as_str()),
    };
    match code {
        Some(c) => format!("{}:{}:{}", c, kind, detail),
        None => format!("E000:{}:{}", kind, detail),
    }
}

/// Returns the kernel version string.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

// ── Curve API ──

/// Create a NURBS curve and return an opaque handle.
/// control_points: flat [x,y,z, x,y,z, ...], weights: [...], knots: [...], degree: u32
#[wasm_bindgen]
pub fn create_nurbs_curve(
    control_points: &[f64],
    weights: &[f64],
    knots: &[f64],
    degree: u32,
) -> Result<JsCurve, JsError> {
    if control_points.len() % 3 != 0 {
        return Err(JsError::new("control_points length must be a multiple of 3"));
    }
    let pts: Vec<Point3> = control_points
        .chunks_exact(3)
        .map(|c| Point3::new(c[0], c[1], c[2]))
        .collect();

    let curve = NurbsCurve::new(pts, weights.to_vec(), knots.to_vec(), degree)
        .map_err(kernel_err_to_js)?;

    Ok(JsCurve { inner: knot_geom::curve::Curve::Nurbs(curve) })
}

/// Create a line segment curve from two endpoints.
#[wasm_bindgen]
pub fn create_line(
    x0: f64, y0: f64, z0: f64,
    x1: f64, y1: f64, z1: f64,
) -> JsCurve {
    JsCurve {
        inner: knot_geom::curve::Curve::Line(
            knot_geom::curve::LineSeg::new(
                Point3::new(x0, y0, z0),
                Point3::new(x1, y1, z1),
            )
        ),
    }
}

/// Create a circular arc curve.
///
/// - `cx,cy,cz`: centre
/// - `nx,ny,nz`: arc plane normal
/// - `radius`: arc radius
/// - `rx,ry,rz`: reference direction (angle = 0)
/// - `start_angle`, `end_angle`: in radians
#[wasm_bindgen]
pub fn create_arc(
    cx: f64, cy: f64, cz: f64,
    nx: f64, ny: f64, nz: f64,
    radius: f64,
    rx: f64, ry: f64, rz: f64,
    start_angle: f64, end_angle: f64,
) -> JsCurve {
    JsCurve {
        inner: knot_geom::curve::Curve::CircularArc(
            knot_geom::curve::CircularArc {
                center: Point3::new(cx, cy, cz),
                normal: Vector3::new(nx, ny, nz).normalize(),
                radius,
                ref_direction: Vector3::new(rx, ry, rz).normalize(),
                start_angle,
                end_angle,
            }
        ),
    }
}

/// Opaque handle to a curve (line, arc, or NURBS).
#[wasm_bindgen]
pub struct JsCurve {
    pub(crate) inner: knot_geom::curve::Curve,
}

#[wasm_bindgen]
impl JsCurve {
    /// Evaluate a point at parameter t. Returns [x, y, z].
    pub fn point_at(&self, t: f64) -> Vec<f64> {
        let p = self.inner.point_at(knot_geom::curve::CurveParam(t));
        vec![p.x, p.y, p.z]
    }

    /// Tangent vector at parameter t. Returns [dx, dy, dz].
    pub fn tangent_at(&self, t: f64) -> Vec<f64> {
        let d = self.inner.derivatives_at(knot_geom::curve::CurveParam(t));
        vec![d.d1.x, d.d1.y, d.d1.z]
    }

    /// Sample the curve at n evenly-spaced parameters.
    /// Returns flat array [x0,y0,z0, x1,y1,z1, ...].
    pub fn sample(&self, n: u32) -> Vec<f64> {
        let domain = self.inner.domain();
        let mut out = Vec::with_capacity(n as usize * 3);
        for i in 0..n {
            let t = domain.start + (domain.end - domain.start) * (i as f64 / (n - 1).max(1) as f64);
            let p = self.inner.point_at(knot_geom::curve::CurveParam(t));
            out.push(p.x);
            out.push(p.y);
            out.push(p.z);
        }
        out
    }

    /// Divide the curve into `n` equal-parameter segments.
    /// Returns `n + 1` parameter values.
    pub fn divide(&self, n: u32) -> Vec<f64> {
        let domain = self.inner.domain();
        (0..=n)
            .map(|i| domain.start + (domain.end - domain.start) * (i as f64 / n.max(1) as f64))
            .collect()
    }

    /// Get the curve domain [start, end].
    pub fn domain(&self) -> Vec<f64> {
        let d = self.inner.domain();
        vec![d.start, d.end]
    }

    /// Closest point on the curve to query (qx, qy, qz).
    /// Returns [param, px, py, pz, distance].
    pub fn closest_point(&self, qx: f64, qy: f64, qz: f64) -> Vec<f64> {
        let q = Point3::new(qx, qy, qz);
        let cp = self.inner.closest_point(&q);
        vec![cp.param.0, cp.point.x, cp.point.y, cp.point.z, cp.distance]
    }

    /// Bounding box. Returns [min_x, min_y, min_z, max_x, max_y, max_z].
    pub fn bounding_box(&self) -> Vec<f64> {
        let bb = self.inner.bounding_box();
        vec![bb.min.x, bb.min.y, bb.min.z, bb.max.x, bb.max.y, bb.max.z]
    }

    /// Offset this curve by `distance` in the plane with the given normal.
    ///
    /// Exact for lines and circular arcs only. Returns an error for NURBS
    /// and elliptical arcs (their exact offset is not the same curve type).
    pub fn offset(&self, distance: f64, nx: f64, ny: f64, nz: f64) -> Result<JsCurve, JsError> {
        let curve = knot_geom::curve::offset::offset(
            &self.inner,
            distance,
            Vector3::new(nx, ny, nz),
        )
        .map_err(kernel_err_to_js)?;
        Ok(JsCurve { inner: curve })
    }

    /// Curve type as a string: "line", "arc", "elliptical_arc", or "nurbs".
    pub fn curve_type(&self) -> String {
        match &self.inner {
            knot_geom::curve::Curve::Line(_) => "line".into(),
            knot_geom::curve::Curve::CircularArc(_) => "arc".into(),
            knot_geom::curve::Curve::EllipticalArc(_) => "elliptical_arc".into(),
            knot_geom::curve::Curve::Nurbs(_) => "nurbs".into(),
        }
    }

    // ── NURBS-specific methods ───────────────────────────────────────

    /// Number of control points (NURBS only, returns 0 for other types).
    pub fn control_point_count(&self) -> u32 {
        match &self.inner {
            knot_geom::curve::Curve::Nurbs(n) => n.control_points().len() as u32,
            _ => 0,
        }
    }

    /// Get control points as flat array [x0,y0,z0, ...] (NURBS only).
    pub fn control_points(&self) -> Vec<f64> {
        match &self.inner {
            knot_geom::curve::Curve::Nurbs(n) => {
                n.control_points().iter().flat_map(|p| [p.x, p.y, p.z]).collect()
            }
            _ => vec![],
        }
    }

    /// Insert a knot at parameter t, returning a new curve (NURBS only).
    pub fn insert_knot(&self, t: f64) -> Result<JsCurve, JsError> {
        match &self.inner {
            knot_geom::curve::Curve::Nurbs(n) => {
                Ok(JsCurve { inner: knot_geom::curve::Curve::Nurbs(n.insert_knot(t)) })
            }
            _ => Err(JsError::new("insert_knot is only available for NURBS curves")),
        }
    }

    /// Full derivatives at parameter t.
    /// Returns flat [px, py, pz, d1x, d1y, d1z, d2x, d2y, d2z]. When the
    /// second derivative is unavailable, the last 3 entries are NaN.
    pub fn derivatives_at(&self, t: f64) -> Vec<f64> {
        let d = self.inner.derivatives_at(knot_geom::curve::CurveParam(t));
        let (d2x, d2y, d2z) = match d.d2 {
            Some(v) => (v.x, v.y, v.z),
            None => (f64::NAN, f64::NAN, f64::NAN),
        };
        vec![d.point.x, d.point.y, d.point.z, d.d1.x, d.d1.y, d.d1.z, d2x, d2y, d2z]
    }

    /// Arc length of the curve. `tolerance` is the requested relative
    /// accuracy; `1e-6` is comfortable for CAD-scale geometry.
    pub fn length(&self, tolerance: f64) -> f64 {
        self.inner.length(tolerance)
    }

    /// Split this curve at parameter `t`. Errors if `t` sits at or
    /// outside the curve's domain endpoints. Returns two new curves;
    /// the input is unchanged.
    pub fn split_at(&self, t: f64) -> Result<JsCurveSplit, JsError> {
        let (a, b) = self.inner
            .split_at(knot_geom::curve::CurveParam(t))
            .map_err(JsError::new)?;
        Ok(JsCurveSplit {
            left: Some(JsCurve { inner: a }),
            right: Some(JsCurve { inner: b }),
        })
    }

    /// Reversed-orientation copy of the curve. Same 3D point set,
    /// opposite parameterization.
    pub fn reverse(&self) -> JsCurve {
        JsCurve { inner: self.inner.reverse() }
    }

    /// Divide the curve into `n` equal-arc-length segments. Returns
    /// `n + 1` parameter values (including both domain endpoints).
    /// Unlike `divide`, this is arc-length-uniform, not
    /// parameter-uniform — meaningful for non-linearly-parameterized
    /// curves (NURBS in particular).
    pub fn divide_by_length(&self, n: u32, tolerance: f64) -> Vec<f64> {
        self.inner
            .divide_by_length(n, tolerance)
            .into_iter()
            .map(|p| p.0)
            .collect()
    }

    /// Intersect this curve with `other`. Returns a flat array of hits:
    /// `[t_a, t_b, x, y, z, t_a, t_b, x, y, z, ...]`. Empty when there
    /// are no intersections within `tolerance`.
    pub fn intersect(&self, other: &JsCurve, tolerance: f64) -> Result<Vec<f64>, JsError> {
        let hits = knot_intersect::curve_curve::intersect_curves(
            &self.inner,
            &other.inner,
            tolerance,
        )
        .map_err(kernel_err_to_js)?;
        let mut out = Vec::with_capacity(hits.len() * 5);
        for h in hits {
            out.push(h.param_a.0);
            out.push(h.param_b.0);
            out.push(h.point.x);
            out.push(h.point.y);
            out.push(h.point.z);
        }
        Ok(out)
    }
}

/// Two-curve split result. Used because `wasm-bindgen` does not yet
/// permit returning a tuple of opaque handles directly.
#[wasm_bindgen]
pub struct JsCurveSplit {
    left: Option<JsCurve>,
    right: Option<JsCurve>,
}

#[wasm_bindgen]
impl JsCurveSplit {
    pub fn left(&mut self) -> Option<JsCurve> {
        self.left.take()
    }
    pub fn right(&mut self) -> Option<JsCurve> {
        self.right.take()
    }
}

// ── Surface API ──

/// Create a NURBS surface.
/// control_points: flat [x,y,z,...], row-major order (count_u rows, count_v cols).
#[wasm_bindgen]
pub fn create_nurbs_surface(
    control_points: &[f64],
    weights: &[f64],
    knots_u: &[f64],
    knots_v: &[f64],
    degree_u: u32,
    degree_v: u32,
    count_u: u32,
    count_v: u32,
) -> Result<JsSurface, JsError> {
    if control_points.len() % 3 != 0 {
        return Err(JsError::new("control_points length must be a multiple of 3"));
    }
    let pts: Vec<Point3> = control_points
        .chunks_exact(3)
        .map(|c| Point3::new(c[0], c[1], c[2]))
        .collect();

    let surface = NurbsSurface::new(
        pts, weights.to_vec(),
        knots_u.to_vec(), knots_v.to_vec(),
        degree_u, degree_v, count_u, count_v,
    ).map_err(kernel_err_to_js)?;

    Ok(JsSurface { inner: SurfaceKind::Nurbs(surface) })
}

/// Create a sphere surface.
#[wasm_bindgen]
pub fn create_sphere(cx: f64, cy: f64, cz: f64, radius: f64) -> JsSurface {
    JsSurface {
        inner: SurfaceKind::Sphere(Sphere::new(Point3::new(cx, cy, cz), radius)),
    }
}

enum SurfaceKind {
    Nurbs(NurbsSurface),
    Sphere(Sphere),
}

/// Opaque handle to a surface.
#[wasm_bindgen]
pub struct JsSurface {
    inner: SurfaceKind,
}

#[wasm_bindgen]
impl JsSurface {
    /// Evaluate a point at (u, v). Returns [x, y, z].
    pub fn point_at(&self, u: f64, v: f64) -> Vec<f64> {
        let p = match &self.inner {
            SurfaceKind::Nurbs(s) => s.point_at(u, v),
            SurfaceKind::Sphere(s) => s.point_at(u, v),
        };
        vec![p.x, p.y, p.z]
    }

    /// Sample the surface on an n_u x n_v grid.
    /// Returns flat array of positions [x,y,z,...] and flat array of triangle indices.
    pub fn sample_grid(&self, n_u: u32, n_v: u32) -> JsSurfaceMesh {
        let (u_start, u_end, v_start, v_end) = match &self.inner {
            SurfaceKind::Nurbs(s) => {
                let d = s.domain();
                (d.u_start, d.u_end, d.v_start, d.v_end)
            }
            SurfaceKind::Sphere(_) => {
                (0.0, std::f64::consts::TAU, -std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2)
            }
        };

        let mut positions = Vec::with_capacity((n_u * n_v * 3) as usize);
        let mut normals = Vec::with_capacity((n_u * n_v * 3) as usize);

        for i in 0..n_u {
            let u = u_start + (u_end - u_start) * (i as f64 / (n_u - 1).max(1) as f64);
            for j in 0..n_v {
                let v = v_start + (v_end - v_start) * (j as f64 / (n_v - 1).max(1) as f64);
                let p = match &self.inner {
                    SurfaceKind::Nurbs(s) => s.point_at(u, v),
                    SurfaceKind::Sphere(s) => s.point_at(u, v),
                };
                positions.push(p.x);
                positions.push(p.y);
                positions.push(p.z);

                let n = match &self.inner {
                    SurfaceKind::Nurbs(s) => s.normal_at(u, v),
                    SurfaceKind::Sphere(s) => s.normal_at(u, v),
                };
                normals.push(n.x);
                normals.push(n.y);
                normals.push(n.z);
            }
        }

        // Build triangle indices for the grid
        let mut indices = Vec::with_capacity(((n_u - 1) * (n_v - 1) * 6) as usize);
        for i in 0..n_u - 1 {
            for j in 0..n_v - 1 {
                let idx = |r: u32, c: u32| r * n_v + c;
                let a = idx(i, j);
                let b = idx(i + 1, j);
                let c = idx(i + 1, j + 1);
                let d = idx(i, j + 1);
                indices.push(a);
                indices.push(b);
                indices.push(c);
                indices.push(a);
                indices.push(c);
                indices.push(d);
            }
        }

        let tri_count = indices.len() / 3;
        JsSurfaceMesh { positions, normals, indices, face_ids: vec![0; tri_count] }
    }
}

/// Mesh data returned from surface sampling.
#[wasm_bindgen]
pub struct JsSurfaceMesh {
    positions: Vec<f64>,
    normals: Vec<f64>,
    indices: Vec<u32>,
    face_ids: Vec<u32>,
}

#[wasm_bindgen]
impl JsSurfaceMesh {
    /// Flat array of vertex positions [x0,y0,z0, x1,y1,z1, ...].
    pub fn positions(&self) -> Vec<f64> {
        self.positions.clone()
    }

    /// Flat array of vertex normals [nx0,ny0,nz0, ...].
    pub fn normals(&self) -> Vec<f64> {
        self.normals.clone()
    }

    /// Triangle indices (groups of 3).
    pub fn indices(&self) -> Vec<u32> {
        self.indices.clone()
    }

    /// Per-triangle source face index (maps each triangle back to its BRep face).
    pub fn face_ids(&self) -> Vec<u32> {
        self.face_ids.clone()
    }

    pub fn vertex_count(&self) -> u32 {
        (self.positions.len() / 3) as u32
    }

    pub fn triangle_count(&self) -> u32 {
        (self.indices.len() / 3) as u32
    }
}

// ── BRep Primitive API ──

/// Opaque handle to a BRep solid.
#[wasm_bindgen]
pub struct JsBrep {
    inner: knot_topo::BRep,
}

#[wasm_bindgen]
impl JsBrep {
    /// Number of faces in the solid.
    pub fn face_count(&self) -> u32 {
        self.inner.solids().iter()
            .map(|s| s.outer_shell().face_count() as u32)
            .sum()
    }

    /// Tessellate the BRep into a triangle mesh using default options.
    pub fn tessellate(&self) -> Result<JsSurfaceMesh, JsError> {
        self.tessellate_with(0.1, f64::INFINITY)
    }

    /// Tessellate with custom quality parameters.
    ///
    /// - `normal_tolerance`: max normal deviation in radians (smaller = finer mesh).
    /// - `max_edge_length`: max triangle edge length (smaller = finer mesh).
    pub fn tessellate_with(&self, normal_tolerance: f64, max_edge_length: f64) -> Result<JsSurfaceMesh, JsError> {
        let opts = knot_tessellate::TessellateOptions {
            normal_tolerance,
            max_edge_length,
        };
        let mesh = knot_tessellate::tessellate(&self.inner, opts)
            .map_err(kernel_err_to_js)?;
        Ok(JsSurfaceMesh {
            positions: mesh.positions_flat(),
            normals: mesh.normals_flat(),
            face_ids: mesh.face_ids,
            indices: mesh.indices,
        })
    }

    /// Validate the BRep topology. Throws on error.
    pub fn validate(&self) -> Result<(), JsError> {
        self.inner.validate().map_err(kernel_err_to_js)
    }

    /// Axis-aligned bounding box: returns [min_x, min_y, min_z, max_x, max_y, max_z].
    pub fn bounding_box(&self) -> Result<Vec<f64>, JsError> {
        let mut all_pts: Vec<knot_geom::Point3> = Vec::new();
        for solid in self.inner.solids() {
            for face in solid.outer_shell().faces() {
                for he in face.outer_loop().half_edges() {
                    all_pts.push(*he.start_vertex().point());
                }
            }
        }
        let bbox = knot_core::Aabb3::from_points(&all_pts)
            .ok_or_else(|| JsError::new("empty BRep has no bounding box"))?;
        Ok(vec![bbox.min.x, bbox.min.y, bbox.min.z, bbox.max.x, bbox.max.y, bbox.max.z])
    }

    /// Enumerate unique edges as a flat float buffer:
    /// `[x0a,y0a,z0a, x0b,y0b,z0b, x1a,y1a,z1a, ...]` — six floats
    /// per edge (start XYZ, end XYZ). Same layout the `fillet_edges`
    /// / `chamfer_edges` entry points consume, so the typical
    /// graph-side pipeline `brep → edges → fillet` is shape-compatible.
    ///
    /// Deduplicates by sorted-vertex-id-pair so each edge appears
    /// exactly once regardless of how many faces it bounds.
    pub fn edges(&self) -> Vec<f64> {
        use std::collections::HashSet;
        let mut seen: HashSet<(u64, u64)> = HashSet::new();
        let mut out: Vec<f64> = Vec::new();
        for solid in self.inner.solids() {
            for face in solid.outer_shell().faces() {
                let loops_iter = std::iter::once(face.outer_loop())
                    .chain(face.inner_loops().iter());
                for loop_ in loops_iter {
                    for he in loop_.half_edges() {
                        let edge = he.edge();
                        let a = edge.start().id().hash_value();
                        let b = edge.end().id().hash_value();
                        let key = if a <= b { (a, b) } else { (b, a) };
                        if !seen.insert(key) { continue; }
                        let sp = edge.start().point();
                        let ep = edge.end().point();
                        out.extend_from_slice(&[sp.x, sp.y, sp.z, ep.x, ep.y, ep.z]);
                    }
                }
            }
        }
        out
    }

    /// Serialize to CBOR bytes for persistence.
    pub fn to_cbor(&self) -> Result<Vec<u8>, JsError> {
        knot_io::to_cbor(&self.inner).map_err(kernel_err_to_js)
    }

    /// Export the BRep as binary STL bytes.
    pub fn to_stl(&self) -> Result<Vec<u8>, JsError> {
        let mesh = knot_tessellate::tessellate(&self.inner, knot_tessellate::TessellateOptions::default())
            .map_err(kernel_err_to_js)?;
        knot_io::to_stl(&mesh).map_err(kernel_err_to_js)
    }

    /// Export the BRep as GLB (binary glTF 2.0) bytes.
    pub fn to_glb(&self) -> Result<Vec<u8>, JsError> {
        let mesh = knot_tessellate::tessellate(&self.inner, knot_tessellate::TessellateOptions::default())
            .map_err(kernel_err_to_js)?;
        knot_io::to_glb(&mesh).map_err(kernel_err_to_js)
    }

    /// Translate (move) this BRep by (dx, dy, dz), returning a new BRep.
    pub fn translate(&self, dx: f64, dy: f64, dz: f64) -> Result<JsBrep, JsError> {
        let iso = knot_geom::transform::translation(knot_geom::Vector3::new(dx, dy, dz));
        let brep = knot_ops::transform_brep(&self.inner, &iso)
            .map_err(kernel_err_to_js)?;
        Ok(JsBrep { inner: brep })
    }

    /// Rotate this BRep around an axis through the origin, returning a new BRep.
    /// axis: (ax, ay, az) — will be normalized. angle: radians.
    pub fn rotate(&self, ax: f64, ay: f64, az: f64, angle: f64) -> Result<JsBrep, JsError> {
        let axis = knot_geom::Vector3::new(ax, ay, az);
        let len = axis.norm();
        if len < 1e-30 {
            return Err(JsError::new("rotation axis cannot be zero-length"));
        }
        let iso = knot_geom::transform::rotation(axis / len, angle);
        let brep = knot_ops::transform_brep(&self.inner, &iso)
            .map_err(kernel_err_to_js)?;
        Ok(JsBrep { inner: brep })
    }

    /// Scale this BRep by (sx, sy, sz), returning a new BRep.
    ///
    /// Uniform scaling (sx == sy == sz) works on all geometry types.
    /// Non-uniform scaling works on planes and NURBS surfaces but will
    /// error on analytical curved surfaces (spheres, cylinders, etc.).
    pub fn scale(&self, sx: f64, sy: f64, sz: f64) -> Result<JsBrep, JsError> {
        let brep = knot_ops::scale_brep(&self.inner, sx, sy, sz)
            .map_err(kernel_err_to_js)?;
        Ok(JsBrep { inner: brep })
    }
}

/// Create a planar profile BRep from a flat array of 2D or 3D points.
///
/// For 2D points (stride=2): `points` is [x0,y0, x1,y1, ...], placed in the
/// z=0 plane with normal +z.
///
/// For 3D points (stride=3): `points` is [x0,y0,z0, x1,y1,z1, ...], the
/// normal is computed from the polygon via Newell's method.
///
/// The result is a single-face open BRep suitable for `extrude` and `revolve`.
#[wasm_bindgen]
pub fn create_profile(points: &[f64], stride: u32) -> Result<JsBrep, JsError> {
    let stride = stride as usize;
    if stride != 2 && stride != 3 {
        return Err(JsError::new("stride must be 2 or 3"));
    }
    if points.len() % stride != 0 {
        return Err(JsError::new("points length must be a multiple of stride"));
    }
    let n = points.len() / stride;
    if n < 3 {
        return Err(JsError::new("profile must have at least 3 vertices"));
    }

    let pts: Vec<Point3> = points.chunks_exact(stride)
        .map(|c| Point3::new(c[0], c[1], if stride == 3 { c[2] } else { 0.0 }))
        .collect();

    let normal = newell_normal_js(&pts);

    let verts: Vec<std::sync::Arc<Vertex>> = pts.iter()
        .map(|p| std::sync::Arc::new(Vertex::new(*p)))
        .collect();
    let mut edges = Vec::with_capacity(n);
    for i in 0..n {
        let j = (i + 1) % n;
        use knot_geom::curve::{Curve, LineSeg};
        let curve = std::sync::Arc::new(Curve::Line(LineSeg::new(pts[i], pts[j])));
        let edge = std::sync::Arc::new(Edge::new(
            verts[i].clone(), verts[j].clone(), curve, 0.0, 1.0,
        ));
        edges.push(HalfEdge::new(edge, true));
    }
    let loop_ = Loop::new(edges, true)
        .map_err(kernel_err_to_js)?;
    let surface = std::sync::Arc::new(
        knot_geom::surface::Surface::Plane(
            knot_geom::surface::Plane::new(pts[0], normal)
        )
    );
    let face = Face::new(surface, loop_, vec![], true)
        .map_err(kernel_err_to_js)?;
    let shell = Shell::new(vec![face], false)
        .map_err(kernel_err_to_js)?;
    let solid = Solid::new(shell, vec![])
        .map_err(kernel_err_to_js)?;
    let brep = knot_topo::BRep::new(vec![solid])
        .map_err(kernel_err_to_js)?;
    Ok(JsBrep { inner: brep })
}

/// Extrude a profile BRep along a direction vector.
///
/// `profile` should be a planar face BRep (e.g. from `create_profile`).
/// Returns a closed solid.
#[wasm_bindgen]
pub fn extrude(profile: &JsBrep, dx: f64, dy: f64, dz: f64, distance: f64) -> Result<JsBrep, JsError> {
    let brep = knot_ops::extrude_linear(
        &profile.inner,
        Vector3::new(dx, dy, dz),
        distance,
    ).map_err(kernel_err_to_js)?;
    Ok(JsBrep { inner: brep })
}

/// Revolve a profile BRep around an axis.
///
/// `profile` should be a planar face BRep (e.g. from `create_profile`).
/// `ox,oy,oz` is a point on the axis, `ax,ay,az` is the axis direction.
/// `angle` is in radians (use 2*PI for a full revolution).
/// Returns a closed solid.
#[wasm_bindgen]
pub fn revolve_brep(
    profile: &JsBrep,
    ox: f64, oy: f64, oz: f64,
    ax: f64, ay: f64, az: f64,
    angle: f64,
) -> Result<JsBrep, JsError> {
    let brep = knot_ops::revolve(
        &profile.inner,
        Point3::new(ox, oy, oz),
        Vector3::new(ax, ay, az),
        angle,
    ).map_err(kernel_err_to_js)?;
    Ok(JsBrep { inner: brep })
}

/// Sweep a profile BRep along a rail curve.
///
/// `profile` should be a planar face BRep (e.g. from `create_profile`).
/// `rail` is any curve (line, arc, or NURBS).
/// Returns a closed solid.  If the rail is closed, no cap faces are added.
#[wasm_bindgen]
pub fn sweep(profile: &JsBrep, rail: &JsCurve) -> Result<JsBrep, JsError> {
    let brep = knot_ops::sweep_1rail(&profile.inner, &rail.inner)
        .map_err(kernel_err_to_js)?;
    Ok(JsBrep { inner: brep })
}

/// Loft between two planar profile BReps.
///
/// Profiles must share outer-loop vertex count; vertex `i` of `a` is
/// connected to vertex `i` of `b`. Returns a closed solid with two
/// planar caps and one ruled strip of side faces.
#[wasm_bindgen]
pub fn loft2(a: &JsBrep, b: &JsBrep) -> Result<JsBrep, JsError> {
    let brep = knot_ops::loft(&[a.inner.clone(), b.inner.clone()])
        .map_err(kernel_err_to_js)?;
    Ok(JsBrep { inner: brep })
}

/// Loft through three planar profile BReps.
///
/// Same constraints as `loft2`. Useful for waist-shaped solids without
/// needing list-typed graph ports. Once data trees land, this collapses
/// into a single variadic `loft` taking a list of profiles.
#[wasm_bindgen]
pub fn loft3(a: &JsBrep, b: &JsBrep, c: &JsBrep) -> Result<JsBrep, JsError> {
    let brep = knot_ops::loft(&[a.inner.clone(), b.inner.clone(), c.inner.clone()])
        .map_err(kernel_err_to_js)?;
    Ok(JsBrep { inner: brep })
}

fn newell_normal_js(pts: &[Point3]) -> Vector3 {
    let n = pts.len();
    let (mut nx, mut ny, mut nz) = (0.0, 0.0, 0.0);
    for i in 0..n {
        let c = pts[i];
        let next = pts[(i + 1) % n];
        nx += (c.y - next.y) * (c.z + next.z);
        ny += (c.z - next.z) * (c.x + next.x);
        nz += (c.x - next.x) * (c.y + next.y);
    }
    let v = Vector3::new(nx, ny, nz);
    let len = v.norm();
    if len > 1e-30 { v / len } else { Vector3::z() }
}

/// Fillet edges of a BRep with a constant radius.
///
/// `edge_points` is a flat array of vertex-pair coordinates identifying
/// which edges to fillet: [x0a,y0a,z0a, x0b,y0b,z0b, x1a,y1a,z1a, x1b,y1b,z1b, ...]
/// (6 floats per edge: two 3D endpoints).
///
/// Both adjacent faces must be planar. The edge must be a straight line.
#[wasm_bindgen]
pub fn fillet_edges(brep: &JsBrep, edge_points: &[f64], radius: f64) -> Result<JsBrep, JsError> {
    let edges = parse_edge_pairs(edge_points)?;
    let result = knot_ops::fillet(&brep.inner, &edges, radius)
        .map_err(kernel_err_to_js)?;
    Ok(JsBrep { inner: result })
}

/// Chamfer edges of a BRep with a constant distance.
///
/// `edge_points` layout is the same as `fillet_edges`.
#[wasm_bindgen]
pub fn chamfer_edges(brep: &JsBrep, edge_points: &[f64], distance: f64) -> Result<JsBrep, JsError> {
    let edges = parse_edge_pairs(edge_points)?;
    let result = knot_ops::chamfer(&brep.inner, &edges, distance)
        .map_err(kernel_err_to_js)?;
    Ok(JsBrep { inner: result })
}

fn parse_edge_pairs(flat: &[f64]) -> Result<Vec<(Point3, Point3)>, JsError> {
    if flat.len() % 6 != 0 {
        return Err(JsError::new("edge_points length must be a multiple of 6 (two 3D points per edge)"));
    }
    Ok(flat.chunks_exact(6)
        .map(|c| (
            Point3::new(c[0], c[1], c[2]),
            Point3::new(c[3], c[4], c[5]),
        ))
        .collect())
}

/// Deserialize a BRep from CBOR bytes (produced by `JsBrep.to_cbor()`).
#[wasm_bindgen]
pub fn from_cbor(data: &[u8]) -> Result<JsBrep, JsError> {
    let brep = knot_io::from_cbor(data)
        .map_err(kernel_err_to_js)?;
    Ok(JsBrep { inner: brep })
}

/// Import a BRep from a STEP file string.
#[wasm_bindgen]
pub fn import_step(step_string: &str) -> Result<JsBrep, JsError> {
    let brep = knot_io::from_step(step_string)
        .map_err(kernel_err_to_js)?;
    Ok(JsBrep { inner: brep })
}

/// Export a BRep as a STEP file string.
#[wasm_bindgen]
pub fn export_step(brep: &JsBrep) -> Result<String, JsError> {
    knot_io::to_step(&brep.inner)
        .map_err(kernel_err_to_js)
}

/// Create a box BRep centered at the origin.
#[wasm_bindgen]
pub fn create_box(sx: f64, sy: f64, sz: f64) -> Result<JsBrep, JsError> {
    let brep = knot_ops::primitives::make_box(sx, sy, sz)
        .map_err(kernel_err_to_js)?;
    Ok(JsBrep { inner: brep })
}

/// Create a sphere BRep.
#[wasm_bindgen]
pub fn create_sphere_brep(cx: f64, cy: f64, cz: f64, radius: f64, n_lon: u32, n_lat: u32) -> Result<JsBrep, JsError> {
    let brep = knot_ops::primitives::make_sphere(
        Point3::new(cx, cy, cz), radius, n_lon, n_lat,
    ).map_err(kernel_err_to_js)?;
    Ok(JsBrep { inner: brep })
}

/// Create a cylinder BRep.
#[wasm_bindgen]
pub fn create_cylinder_brep(cx: f64, cy: f64, cz: f64, radius: f64, height: f64, n_sides: u32) -> Result<JsBrep, JsError> {
    let brep = knot_ops::primitives::make_cylinder(
        Point3::new(cx, cy, cz), radius, height, n_sides,
    ).map_err(kernel_err_to_js)?;
    Ok(JsBrep { inner: brep })
}

/// Boolean union of two BReps.
#[wasm_bindgen]
pub fn boolean_union(a: &JsBrep, b: &JsBrep) -> Result<JsBrep, JsError> {
    let result = knot_ops::boolean::boolean(&a.inner, &b.inner, knot_ops::BooleanOp::Union)
        .map_err(kernel_err_to_js)?;
    Ok(JsBrep { inner: result })
}

/// Boolean intersection of two BReps.
#[wasm_bindgen]
pub fn boolean_intersection(a: &JsBrep, b: &JsBrep) -> Result<JsBrep, JsError> {
    let result = knot_ops::boolean::boolean(&a.inner, &b.inner, knot_ops::BooleanOp::Intersection)
        .map_err(kernel_err_to_js)?;
    Ok(JsBrep { inner: result })
}

/// Boolean subtraction (a minus b).
#[wasm_bindgen]
pub fn boolean_subtraction(a: &JsBrep, b: &JsBrep) -> Result<JsBrep, JsError> {
    let result = knot_ops::boolean::boolean(&a.inner, &b.inner, knot_ops::BooleanOp::Subtraction)
        .map_err(kernel_err_to_js)?;
    Ok(JsBrep { inner: result })
}
