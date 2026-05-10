use knot_core::KResult;
use knot_geom::{Point3, Vector3};
use knot_topo::BRep;

/// Triangle mesh output from tessellation.
#[derive(Clone, Debug)]
pub struct TriMesh {
    /// Vertex positions.
    pub positions: Vec<Point3>,
    /// Per-vertex normals.
    pub normals: Vec<Vector3>,
    /// Triangle indices (groups of 3).
    pub indices: Vec<u32>,
    /// Per-triangle source face index (maps back to BRep faces).
    pub face_ids: Vec<u32>,
}

impl TriMesh {
    pub fn vertex_count(&self) -> usize {
        self.positions.len()
    }

    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// Get positions as a flat f64 array [x0,y0,z0, x1,y1,z1, ...].
    pub fn positions_flat(&self) -> Vec<f64> {
        self.positions.iter().flat_map(|p| [p.x, p.y, p.z]).collect()
    }

    /// Get normals as a flat f64 array.
    pub fn normals_flat(&self) -> Vec<f64> {
        self.normals.iter().flat_map(|n| [n.x, n.y, n.z]).collect()
    }
}

/// Tessellation parameters.
#[derive(Clone, Copy, Debug)]
pub struct TessellateOptions {
    /// Maximum allowed normal deviation (radians) between adjacent triangles.
    pub normal_tolerance: f64,
    /// Maximum allowed edge length.
    pub max_edge_length: f64,
}

impl Default for TessellateOptions {
    fn default() -> Self {
        Self {
            normal_tolerance: 0.1,
            max_edge_length: f64::INFINITY,
        }
    }
}

/// Tessellate a BRep into a triangle mesh.
/// Each face is tessellated by triangulating its boundary polygon.
pub fn tessellate(brep: &BRep, _options: TessellateOptions) -> KResult<TriMesh> {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();
    let mut face_ids = Vec::new();

    let mut face_idx = 0u32;

    for solid in brep.solids() {
        tessellate_shell(
            solid.outer_shell(),
            &mut positions,
            &mut normals,
            &mut indices,
            &mut face_ids,
            &mut face_idx,
        )?;
        for void_shell in solid.void_shells() {
            tessellate_shell(
                void_shell,
                &mut positions,
                &mut normals,
                &mut indices,
                &mut face_ids,
                &mut face_idx,
            )?;
        }
    }

    Ok(TriMesh {
        positions,
        normals,
        indices,
        face_ids,
    })
}

fn tessellate_shell(
    shell: &knot_topo::Shell,
    positions: &mut Vec<Point3>,
    normals: &mut Vec<Vector3>,
    indices: &mut Vec<u32>,
    face_ids: &mut Vec<u32>,
    face_idx: &mut u32,
) -> KResult<()> {
    for face in shell.faces() {
        tessellate_face(face, positions, normals, indices, face_ids, *face_idx)?;
        *face_idx += 1;
    }
    Ok(())
}

fn tessellate_face(
    face: &knot_topo::Face,
    positions: &mut Vec<Point3>,
    normals: &mut Vec<Vector3>,
    indices: &mut Vec<u32>,
    face_ids: &mut Vec<u32>,
    face_idx: u32,
) -> KResult<()> {
    let outer_loop = face.outer_loop();
    let half_edges = outer_loop.half_edges();

    if half_edges.len() < 3 {
        return Ok(());
    }

    // Collect outer boundary vertices.
    let outer_verts: Vec<Point3> =
        half_edges.iter().map(|he| *he.start_vertex().point()).collect();

    // Compute face normal via Newell's method, respecting same_sense.
    let face_normal = {
        let n = compute_polygon_normal(&outer_verts);
        if face.same_sense() { n } else { -n }
    };

    // Collect inner loop vertices (holes).
    let inner_loops: Vec<Vec<Point3>> = face
        .inner_loops()
        .iter()
        .filter(|il| il.half_edges().len() >= 3)
        .map(|il| {
            il.half_edges()
                .iter()
                .map(|he| *he.start_vertex().point())
                .collect()
        })
        .collect();

    // Merge inner loops into outer polygon via bridge edges.
    let merged = if inner_loops.is_empty() {
        outer_verts
    } else {
        bridge_inner_loops(&outer_verts, &inner_loops, &face_normal)
    };

    let base_idx = positions.len() as u32;

    for v in &merged {
        positions.push(*v);
        normals.push(face_normal);
    }

    // Ear-clipping triangulation.
    let tris = ear_clip_triangulate(&merged, &face_normal);
    for [a, b, c] in tris {
        // Skip degenerate triangles produced by bridge slit edges.
        let pa = merged[a];
        let pb = merged[b];
        let pc = merged[c];
        let area = (pb - pa).cross(&(pc - pa)).norm();
        if area < 1e-20 {
            continue;
        }
        indices.push(base_idx + a as u32);
        indices.push(base_idx + b as u32);
        indices.push(base_idx + c as u32);
        face_ids.push(face_idx);
    }

    Ok(())
}

// ── inner loop bridging ──────────────────────────────────────────────────────

/// Bridge inner loops (holes) into the outer polygon by inserting slit
/// edges, producing a single simple polygon suitable for ear-clipping.
///
/// For each inner loop:
/// 1. Find its rightmost vertex (max x in the 2D projection).
/// 2. Find the nearest outer polygon vertex.
/// 3. Insert the inner loop at that point, duplicating the bridge vertices
///    to create a zero-width slit.
fn bridge_inner_loops(
    outer: &[Point3],
    inner_loops: &[Vec<Point3>],
    normal: &Vector3,
) -> Vec<Point3> {
    let proj = make_projection(normal);
    let mut merged = outer.to_vec();

    for inner in inner_loops {
        if inner.len() < 3 {
            continue;
        }

        // Find inner vertex with largest x in 2D projection.
        let inner_2d: Vec<[f64; 2]> = inner.iter().map(|p| proj(p)).collect();
        let rightmost_idx = inner_2d
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a[0].partial_cmp(&b[0]).unwrap())
            .unwrap()
            .0;

        // Find nearest vertex on the current merged polygon.
        let merged_2d: Vec<[f64; 2]> = merged.iter().map(|p| proj(p)).collect();
        let target = inner_2d[rightmost_idx];
        let bridge_idx = merged_2d
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                let da = (a[0] - target[0]).powi(2) + (a[1] - target[1]).powi(2);
                let db = (b[0] - target[0]).powi(2) + (b[1] - target[1]).powi(2);
                da.partial_cmp(&db).unwrap()
            })
            .unwrap()
            .0;

        // Build merged polygon:
        //   ..., outer[bridge], inner[right], inner[right+1], ..., inner[right],
        //   outer[bridge], outer[bridge+1], ...
        let p = inner.len();
        let mut new_merged = Vec::with_capacity(merged.len() + p + 2);
        new_merged.extend_from_slice(&merged[..=bridge_idx]);
        for k in 0..=p {
            new_merged.push(inner[(rightmost_idx + k) % p]);
        }
        new_merged.push(merged[bridge_idx]);
        if bridge_idx + 1 < merged.len() {
            new_merged.extend_from_slice(&merged[bridge_idx + 1..]);
        }

        merged = new_merged;
    }

    merged
}

/// Build a 2D projection function for the given face normal.
fn make_projection(normal: &Vector3) -> Box<dyn Fn(&Point3) -> [f64; 2]> {
    let ax = normal.x.abs();
    let ay = normal.y.abs();
    let az = normal.z.abs();
    if az >= ax && az >= ay {
        Box::new(|p: &Point3| [p.x, p.y])
    } else if ay >= ax {
        Box::new(|p: &Point3| [p.x, p.z])
    } else {
        Box::new(|p: &Point3| [p.y, p.z])
    }
}

// ── ear-clipping triangulation ───────────────────────────────────────────────

/// Triangulate a simple polygon via ear clipping.
///
/// Projects the 3D polygon onto its dominant 2D plane (determined by
/// `normal`), then iteratively clips "ear" vertices — convex vertices whose
/// triangle contains no other polygon vertex.
///
/// Returns triangle index triples referencing positions in `verts`.
fn ear_clip_triangulate(verts: &[Point3], normal: &Vector3) -> Vec<[usize; 3]> {
    let n = verts.len();
    if n < 3 {
        return vec![];
    }
    if n == 3 {
        return vec![[0, 1, 2]];
    }

    let proj = make_projection(normal);
    let pts: Vec<[f64; 2]> = verts.iter().map(|v| proj(v)).collect();

    // Determine winding of the projected polygon.
    let ccw = signed_area_2d(&pts) > 0.0;

    let mut remaining: Vec<usize> = (0..n).collect();
    let mut tris = Vec::with_capacity(n - 2);

    while remaining.len() > 3 {
        let len = remaining.len();
        let mut clipped = false;

        for i in 0..len {
            let pi = if i == 0 { len - 1 } else { i - 1 };
            let ni = (i + 1) % len;

            let a = &pts[remaining[pi]];
            let b = &pts[remaining[i]];
            let c = &pts[remaining[ni]];

            // Convex test: for CCW polygons, ears have positive cross product.
            let cross = cross_2d(a, b, c);
            if (ccw && cross <= 0.0) || (!ccw && cross >= 0.0) {
                continue; // reflex or degenerate — not an ear
            }

            // Containment test: no other remaining vertex may lie inside
            // (or on the boundary of) this triangle.
            let mut blocked = false;
            for j in 0..len {
                if j == pi || j == i || j == ni {
                    continue;
                }
                if point_in_triangle_2d(&pts[remaining[j]], a, b, c) {
                    blocked = true;
                    break;
                }
            }

            if !blocked {
                tris.push([remaining[pi], remaining[i], remaining[ni]]);
                remaining.remove(i);
                clipped = true;
                break;
            }
        }

        if !clipped {
            // No ear found — polygon is degenerate. Force-clip the first
            // vertex so the algorithm still terminates.
            let last = remaining.len() - 1;
            tris.push([remaining[last], remaining[0], remaining[1]]);
            remaining.remove(0);
        }
    }

    if remaining.len() == 3 {
        tris.push([remaining[0], remaining[1], remaining[2]]);
    }

    tris
}

/// Signed area of a 2D polygon (positive = CCW).
fn signed_area_2d(pts: &[[f64; 2]]) -> f64 {
    let n = pts.len();
    let mut area = 0.0;
    for i in 0..n {
        let j = (i + 1) % n;
        area += pts[i][0] * pts[j][1];
        area -= pts[j][0] * pts[i][1];
    }
    area * 0.5
}

/// 2D cross product of vectors (a→b) and (a→c).
fn cross_2d(a: &[f64; 2], b: &[f64; 2], c: &[f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

/// Test whether point `p` lies inside or on the boundary of triangle `abc`.
fn point_in_triangle_2d(p: &[f64; 2], a: &[f64; 2], b: &[f64; 2], c: &[f64; 2]) -> bool {
    let d1 = cross_2d(a, b, p);
    let d2 = cross_2d(b, c, p);
    let d3 = cross_2d(c, a, p);
    let has_neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let has_pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(has_neg && has_pos)
}

// ── polygon normal ───────────────────────────────────────────────────────────

/// Compute polygon normal via Newell's method.
fn compute_polygon_normal(verts: &[Point3]) -> Vector3 {
    let mut normal = Vector3::zeros();
    let n = verts.len();
    for i in 0..n {
        let curr = &verts[i];
        let next = &verts[(i + 1) % n];
        normal.x += (curr.y - next.y) * (curr.z + next.z);
        normal.y += (curr.z - next.z) * (curr.x + next.x);
        normal.z += (curr.x - next.x) * (curr.y + next.y);
    }
    let len = normal.norm();
    if len > 1e-30 {
        normal / len
    } else {
        Vector3::z()
    }
}
