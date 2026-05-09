use wasm_bindgen::prelude::*;
use knot_geom::Point3;
use knot_geom::Vector3;
use knot_geom::curve::NurbsCurve;
use knot_geom::surface::{NurbsSurface, Sphere, Plane};
use knot_topo::*;

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
        .map_err(|e| JsError::new(&e.to_string()))?;

    Ok(JsCurve { inner: curve })
}

/// Opaque handle to a NURBS curve.
#[wasm_bindgen]
pub struct JsCurve {
    inner: NurbsCurve,
}

#[wasm_bindgen]
impl JsCurve {
    /// Evaluate a point at parameter t. Returns [x, y, z].
    pub fn point_at(&self, t: f64) -> Vec<f64> {
        let p = self.inner.point_at(t);
        vec![p.x, p.y, p.z]
    }

    /// Sample the curve at n evenly-spaced parameters.
    /// Returns flat array [x0,y0,z0, x1,y1,z1, ...] for efficient rendering.
    pub fn sample(&self, n: u32) -> Vec<f64> {
        let domain = self.inner.domain();
        let mut out = Vec::with_capacity(n as usize * 3);
        for i in 0..n {
            let t = domain.start + (domain.end - domain.start) * (i as f64 / (n - 1).max(1) as f64);
            let p = self.inner.point_at(t);
            out.push(p.x);
            out.push(p.y);
            out.push(p.z);
        }
        out
    }

    /// Get the curve domain [start, end].
    pub fn domain(&self) -> Vec<f64> {
        let d = self.inner.domain();
        vec![d.start, d.end]
    }

    /// Number of control points.
    pub fn control_point_count(&self) -> u32 {
        self.inner.control_points().len() as u32
    }

    /// Get control points as flat array [x0,y0,z0, ...].
    pub fn control_points(&self) -> Vec<f64> {
        self.inner.control_points().iter()
            .flat_map(|p| [p.x, p.y, p.z])
            .collect()
    }

    /// Insert a knot at parameter t, returning a new curve.
    pub fn insert_knot(&self, t: f64) -> JsCurve {
        JsCurve { inner: self.inner.insert_knot(t) }
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
    ).map_err(|e| JsError::new(&e.to_string()))?;

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

        JsSurfaceMesh { positions, normals, indices }
    }
}

/// Mesh data returned from surface sampling.
#[wasm_bindgen]
pub struct JsSurfaceMesh {
    positions: Vec<f64>,
    normals: Vec<f64>,
    indices: Vec<u32>,
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

    /// Tessellate the BRep into a triangle mesh.
    pub fn tessellate(&self) -> Result<JsSurfaceMesh, JsError> {
        let mesh = knot_tessellate::tessellate(&self.inner, knot_tessellate::TessellateOptions::default())
            .map_err(|e| JsError::new(&e.to_string()))?;
        Ok(JsSurfaceMesh {
            positions: mesh.positions_flat(),
            normals: mesh.normals_flat(),
            indices: mesh.indices,
        })
    }

    /// Export the BRep as binary STL bytes.
    pub fn to_stl(&self) -> Result<Vec<u8>, JsError> {
        let mesh = knot_tessellate::tessellate(&self.inner, knot_tessellate::TessellateOptions::default())
            .map_err(|e| JsError::new(&e.to_string()))?;
        knot_io::to_stl(&mesh).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Export the BRep as GLB (binary glTF 2.0) bytes.
    pub fn to_glb(&self) -> Result<Vec<u8>, JsError> {
        let mesh = knot_tessellate::tessellate(&self.inner, knot_tessellate::TessellateOptions::default())
            .map_err(|e| JsError::new(&e.to_string()))?;
        knot_io::to_glb(&mesh).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Translate (move) this BRep by (dx, dy, dz), returning a new BRep.
    pub fn translate(&self, dx: f64, dy: f64, dz: f64) -> Result<JsBrep, JsError> {
        let iso = knot_geom::transform::translation(knot_geom::Vector3::new(dx, dy, dz));
        let brep = knot_ops::transform_brep(&self.inner, &iso)
            .map_err(|e| JsError::new(&e.to_string()))?;
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
            .map_err(|e| JsError::new(&e.to_string()))?;
        Ok(JsBrep { inner: brep })
    }

    /// Scale this BRep by (sx, sy, sz), returning a new BRep.
    ///
    /// Uniform scaling (sx == sy == sz) works on all geometry types.
    /// Non-uniform scaling works on planes and NURBS surfaces but will
    /// error on analytical curved surfaces (spheres, cylinders, etc.).
    pub fn scale(&self, sx: f64, sy: f64, sz: f64) -> Result<JsBrep, JsError> {
        let brep = knot_ops::scale_brep(&self.inner, sx, sy, sz)
            .map_err(|e| JsError::new(&e.to_string()))?;
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
        .map_err(|e| JsError::new(&e.to_string()))?;
    let surface = std::sync::Arc::new(
        knot_geom::surface::Surface::Plane(
            knot_geom::surface::Plane::new(pts[0], normal)
        )
    );
    let face = Face::new(surface, loop_, vec![], true)
        .map_err(|e| JsError::new(&e.to_string()))?;
    let shell = Shell::new(vec![face], false)
        .map_err(|e| JsError::new(&e.to_string()))?;
    let solid = Solid::new(shell, vec![])
        .map_err(|e| JsError::new(&e.to_string()))?;
    let brep = knot_topo::BRep::new(vec![solid])
        .map_err(|e| JsError::new(&e.to_string()))?;
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
    ).map_err(|e| JsError::new(&e.to_string()))?;
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
    ).map_err(|e| JsError::new(&e.to_string()))?;
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
        .map_err(|e| JsError::new(&e.to_string()))?;
    Ok(JsBrep { inner: result })
}

/// Chamfer edges of a BRep with a constant distance.
///
/// `edge_points` layout is the same as `fillet_edges`.
#[wasm_bindgen]
pub fn chamfer_edges(brep: &JsBrep, edge_points: &[f64], distance: f64) -> Result<JsBrep, JsError> {
    let edges = parse_edge_pairs(edge_points)?;
    let result = knot_ops::chamfer(&brep.inner, &edges, distance)
        .map_err(|e| JsError::new(&e.to_string()))?;
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

/// Import a BRep from a STEP file string.
#[wasm_bindgen]
pub fn import_step(step_string: &str) -> Result<JsBrep, JsError> {
    let brep = knot_io::from_step(step_string)
        .map_err(|e| JsError::new(&e.to_string()))?;
    Ok(JsBrep { inner: brep })
}

/// Export a BRep as a STEP file string.
#[wasm_bindgen]
pub fn export_step(brep: &JsBrep) -> Result<String, JsError> {
    knot_io::to_step(&brep.inner)
        .map_err(|e| JsError::new(&e.to_string()))
}

/// Create a box BRep centered at the origin.
#[wasm_bindgen]
pub fn create_box(sx: f64, sy: f64, sz: f64) -> Result<JsBrep, JsError> {
    let brep = knot_ops::primitives::make_box(sx, sy, sz)
        .map_err(|e| JsError::new(&e.to_string()))?;
    Ok(JsBrep { inner: brep })
}

/// Create a sphere BRep.
#[wasm_bindgen]
pub fn create_sphere_brep(cx: f64, cy: f64, cz: f64, radius: f64, n_lon: u32, n_lat: u32) -> Result<JsBrep, JsError> {
    let brep = knot_ops::primitives::make_sphere(
        Point3::new(cx, cy, cz), radius, n_lon, n_lat,
    ).map_err(|e| JsError::new(&e.to_string()))?;
    Ok(JsBrep { inner: brep })
}

/// Create a cylinder BRep.
#[wasm_bindgen]
pub fn create_cylinder_brep(cx: f64, cy: f64, cz: f64, radius: f64, height: f64, n_sides: u32) -> Result<JsBrep, JsError> {
    let brep = knot_ops::primitives::make_cylinder(
        Point3::new(cx, cy, cz), radius, height, n_sides,
    ).map_err(|e| JsError::new(&e.to_string()))?;
    Ok(JsBrep { inner: brep })
}

/// Boolean union of two BReps.
#[wasm_bindgen]
pub fn boolean_union(a: &JsBrep, b: &JsBrep) -> Result<JsBrep, JsError> {
    let result = knot_ops::boolean::boolean(&a.inner, &b.inner, knot_ops::BooleanOp::Union)
        .map_err(|e| JsError::new(&e.to_string()))?;
    Ok(JsBrep { inner: result })
}

/// Boolean intersection of two BReps.
#[wasm_bindgen]
pub fn boolean_intersection(a: &JsBrep, b: &JsBrep) -> Result<JsBrep, JsError> {
    let result = knot_ops::boolean::boolean(&a.inner, &b.inner, knot_ops::BooleanOp::Intersection)
        .map_err(|e| JsError::new(&e.to_string()))?;
    Ok(JsBrep { inner: result })
}

/// Boolean subtraction (a minus b).
#[wasm_bindgen]
pub fn boolean_subtraction(a: &JsBrep, b: &JsBrep) -> Result<JsBrep, JsError> {
    let result = knot_ops::boolean::boolean(&a.inner, &b.inner, knot_ops::BooleanOp::Subtraction)
        .map_err(|e| JsError::new(&e.to_string()))?;
    Ok(JsBrep { inner: result })
}
