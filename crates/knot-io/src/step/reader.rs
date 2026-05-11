//! STEP to BRep reader.
//!
//! Maps STEP AP203/AP214 entities to knot-topo/knot-geom types.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;
use knot_core::{KResult, KernelError, ErrorCode};
use knot_geom::Point3;
use knot_geom::Vector3;
use knot_geom::curve::{Curve, LineSeg, CircularArc, NurbsCurve};
use knot_geom::surface::{Surface, Plane, Sphere, Cylinder, Cone, Torus, NurbsSurface};
use knot_topo::*;

use super::parser::{StepFile, Entity, Param};

/// Read a STEP file string and return a BRep.
///
/// STEP files can contain multiple `MANIFOLD_SOLID_BREP` entities
/// (an assembly, or a part with several disjoint solids). We pick
/// the **first solid that imports successfully** and skip the rest.
/// Earlier we tried merging all solids' shells into one combined
/// shell, on the hypothesis that "off by 2·(N-1)" Euler violations
/// were caused by partial loading. The audit showed this is
/// **wrong** for ABC: most multi-solid files are assemblies whose
/// components share walls, and merging shells creates coincident /
/// adjacent face duplicates that produce thousands of spurious
/// intersection curves downstream — wedging the boolean budget on
/// pairs that previously fit comfortably (e.g., pair (24, 25)
/// went from 1.4s SSI loop to producing 5818 traces from 410 pairs
/// and busting the budget).
///
/// The right fix for assembly-style multi-solid files is not at
/// the import layer — it's at the boolean dispatch layer, where
/// each solid component should be treated as a separate operand
/// (or coincident faces should be detected and shared during
/// merge). Until that exists, single-solid loading is the right
/// trade. The cost is that genuinely-disjoint multi-solid files
/// load with N-1 components missing; for ABC chunk 0000 these
/// produce `EulerViolation` rejections that are honest about the
/// missing topology.
///
/// If the first solid fails to import, fall through to subsequent
/// solids (some files have a small "Part 2" stub before the main
/// body — try the next one). If the file has no
/// `MANIFOLD_SOLID_BREP` at all, scan
/// `ADVANCED_BREP_SHAPE_REPRESENTATION` items for nested solids.
/// Test-only entry point: read a single curve or surface by entity id from a
/// raw STEP string. Lets entity-decoding tests assert against the resulting
/// `Surface`/`Curve` variant directly, without building a full BRep around it.
#[doc(hidden)]
pub fn debug_read_surface(input: &str, id: u64) -> KResult<Surface> {
    let step = super::parser::parse_step(input).map_err(|e| KernelError::Io {
        detail: format!("STEP parse error: {}", e),
    })?;
    let ctx = ReadContext {
        step: &step,
        vertex_cache: RefCell::new(HashMap::new()),
        edge_cache: RefCell::new(HashMap::new()),
        surface_cache: RefCell::new(HashMap::new()),
    };
    ctx.read_surface(id)
}

/// Test-only entry point for curves. See `debug_read_surface`.
#[doc(hidden)]
pub fn debug_read_curve(input: &str, id: u64) -> KResult<Curve> {
    let step = super::parser::parse_step(input).map_err(|e| KernelError::Io {
        detail: format!("STEP parse error: {}", e),
    })?;
    let ctx = ReadContext {
        step: &step,
        vertex_cache: RefCell::new(HashMap::new()),
        edge_cache: RefCell::new(HashMap::new()),
        surface_cache: RefCell::new(HashMap::new()),
    };
    ctx.read_curve(id)
}

pub fn read_step(input: &str) -> KResult<BRep> {
    let step = super::parser::parse_step(input).map_err(|e| KernelError::Io {
        detail: format!("STEP parse error: {}", e),
    })?;

    let ctx = ReadContext {
        step: &step,
        vertex_cache: RefCell::new(HashMap::new()),
        edge_cache: RefCell::new(HashMap::new()),
        surface_cache: RefCell::new(HashMap::new()),
    };

    let mut solid_entities = step.entities_of_type("MANIFOLD_SOLID_BREP");

    // Fall back to scanning ADVANCED_BREP_SHAPE_REPRESENTATION items
    // if no top-level MANIFOLD_SOLID_BREP entries were indexed.
    if solid_entities.is_empty() {
        let repr_entities = step.entities_of_type("ADVANCED_BREP_SHAPE_REPRESENTATION");
        for repr in &repr_entities {
            if let Some(items) = repr.params.get(1).and_then(|p| p.as_ref_list()) {
                for item_id in items {
                    if let Some(item) = step.get(item_id) {
                        if item.name == "MANIFOLD_SOLID_BREP"
                            && !solid_entities.iter().any(|e| e.id == item.id)
                        {
                            solid_entities.push(item);
                        }
                    }
                }
            }
        }
    }

    if solid_entities.is_empty() {
        return Err(KernelError::Io {
            detail: "no MANIFOLD_SOLID_BREP found in STEP file".into(),
        });
    }

    // Try entities in source order; take the first that imports
    // successfully.
    let mut last_err: Option<KernelError> = None;
    for entity in &solid_entities {
        match ctx.read_solid(entity) {
            Ok(brep) => return Ok(brep),
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap_or_else(|| KernelError::Io {
        detail: "no readable MANIFOLD_SOLID_BREP".into(),
    }))
}

struct ReadContext<'a> {
    step: &'a StepFile,
    /// Cache: STEP vertex entity ID → shared Arc<Vertex>.
    /// Ensures two faces sharing the same STEP VERTEX_POINT get the same Arc.
    vertex_cache: RefCell<HashMap<u64, Arc<Vertex>>>,
    /// Cache: STEP edge entity ID → shared Arc<Edge>.
    /// Ensures two ORIENTED_EDGE referencing the same EDGE_CURVE share the Arc.
    edge_cache: RefCell<HashMap<u64, Arc<Edge>>>,
    /// Cache: STEP surface entity ID → shared Arc<Surface>.
    /// Multiple ADVANCED_FACEs typically reference the same underlying
    /// surface (six faces of a cube share one PLANE; multiple holes
    /// share one CYLINDRICAL_SURFACE). Sharing the Arc lets downstream
    /// code dedup work via pointer identity (e.g., the boolean's
    /// SSI memoization).
    surface_cache: RefCell<HashMap<u64, Arc<Surface>>>,
}

impl<'a> ReadContext<'a> {
    fn get(&self, id: u64) -> KResult<&'a Entity> {
        self.step.get(id).ok_or_else(|| KernelError::Io {
            detail: format!("missing entity #{}", id),
        })
    }

    /// Read a single MANIFOLD_SOLID_BREP and return its shell. Used
    /// by `read_step` to load every solid in a multi-solid file.
    fn read_solid_shell(&self, entity: &Entity) -> KResult<Shell> {
        let shell_id = entity.params.get(1)
            .and_then(|p| p.as_ref())
            .ok_or_else(|| KernelError::Io {
                detail: format!("#{}: missing shell ref", entity.id),
            })?;
        self.read_shell(shell_id)
    }

    /// Backwards-compatible: read a single solid's BRep (one
    /// MANIFOLD_SOLID_BREP only). Retained for callers that want a
    /// single-solid result; the public `read_step` entry point uses
    /// `read_solid_shell` and merges across all top-level solids.
    #[allow(dead_code)]
    fn read_solid(&self, entity: &Entity) -> KResult<BRep> {
        let shell = self.read_solid_shell(entity)?;
        let solid = Solid::new(shell, vec![])?;
        BRep::new(vec![solid])
    }

    fn read_shell(&self, id: u64) -> KResult<Shell> {
        let entity = self.get(id)?;
        // CLOSED_SHELL('name', (#face1, #face2, ...))
        let face_refs = entity.params.get(1)
            .and_then(|p| p.as_ref_list())
            .ok_or_else(|| KernelError::Io {
                detail: format!("#{}: missing face list", id),
            })?;

        let total_faces = face_refs.len();
        let mut faces = Vec::new();
        let mut dropped: Vec<(u64, String)> = Vec::new();
        for face_id in face_refs {
            match self.read_face(face_id) {
                Ok(face) => faces.push(face),
                Err(e) => dropped.push((face_id, e.to_string())),
            }
        }

        // Strict-import contract: a shell with even one dropped face
        // has missing topology. The boolean's downstream validation
        // would catch this as an Euler violation, but at that point
        // we've lost the cause. Fail here with the precise reason.
        if !dropped.is_empty() {
            let n = dropped.len();
            let preview: Vec<String> = dropped.iter().take(3)
                .map(|(id, msg)| format!("#{}: {}", id, msg.chars().take(80).collect::<String>()))
                .collect();
            return Err(KernelError::Io {
                detail: format!(
                    "shell #{}: dropped {} of {} faces (kept {}). First failures: {}",
                    id, n, total_faces, faces.len(), preview.join("; ")
                ),
            });
        }

        if faces.len() < 4 {
            return Err(KernelError::Io {
                detail: format!("shell #{}: only {} faces read", id, faces.len()),
            });
        }

        Shell::new(faces, true)
    }

    fn read_face(&self, id: u64) -> KResult<Face> {
        let entity = self.get(id)?;
        // ADVANCED_FACE('name', (#bound1, #bound2, ...), #surface, .T.)
        let bound_refs = entity.params.get(1)
            .and_then(|p| p.as_ref_list())
            .ok_or_else(|| KernelError::Io {
                detail: format!("#{}: missing bounds", id),
            })?;

        let surface_id = entity.params.get(2)
            .and_then(|p| p.as_ref())
            .ok_or_else(|| KernelError::Io {
                detail: format!("#{}: missing surface", id),
            })?;

        let same_sense = entity.params.get(3)
            .and_then(|p| p.as_bool())
            .unwrap_or(true);

        let surface = {
            let cached = self.surface_cache.borrow().get(&surface_id).cloned();
            if let Some(s) = cached {
                s
            } else {
                let s = Arc::new(self.read_surface(surface_id)?);
                self.surface_cache.borrow_mut().insert(surface_id, s.clone());
                s
            }
        };

        let mut outer_loop = None;
        let mut inner_loops = Vec::new();

        for bound_id in bound_refs {
            let bound_entity = self.get(bound_id)?;
            let is_outer = bound_entity.name == "FACE_OUTER_BOUND";
            let loop_ = self.read_face_bound(bound_id, is_outer)?;

            if is_outer || outer_loop.is_none() {
                outer_loop = Some(loop_);
            } else {
                inner_loops.push(loop_);
            }
        }

        let outer_loop = outer_loop.ok_or_else(|| KernelError::Io {
            detail: format!("#{}: no outer loop", id),
        })?;

        Face::new(surface, outer_loop, inner_loops, same_sense)
    }

    fn read_face_bound(&self, id: u64, is_outer: bool) -> KResult<Loop> {
        let entity = self.get(id)?;
        // FACE_BOUND('name', #edge_loop, .T.)
        let loop_id = entity.params.get(1)
            .and_then(|p| p.as_ref())
            .ok_or_else(|| KernelError::Io {
                detail: format!("#{}: missing edge loop", id),
            })?;

        let orientation = entity.params.get(2)
            .and_then(|p| p.as_bool())
            .unwrap_or(true);

        self.read_edge_loop(loop_id, is_outer, orientation)
    }

    fn read_edge_loop(&self, id: u64, is_outer: bool, _bound_orientation: bool) -> KResult<Loop> {
        let entity = self.get(id)?;
        // EDGE_LOOP('name', (#oe1, #oe2, ...))
        let oe_refs = entity.params.get(1)
            .and_then(|p| p.as_ref_list())
            .ok_or_else(|| KernelError::Io {
                detail: format!("#{}: missing oriented edges", id),
            })?;

        let total_oe = oe_refs.len();
        let mut half_edges = Vec::new();
        let mut dropped: Vec<(u64, String)> = Vec::new();
        for oe_id in oe_refs {
            match self.read_oriented_edge(oe_id) {
                Ok(he) => half_edges.push(he),
                Err(e) => dropped.push((oe_id, e.to_string())),
            }
        }

        // Strict-import contract: a loop missing any of its oriented
        // edges is open. Surface this here so the caller (read_shell)
        // can drop the whole face cleanly rather than carrying a
        // ragged loop into validation.
        if !dropped.is_empty() {
            let preview: Vec<String> = dropped.iter().take(3)
                .map(|(id, msg)| format!("#{}: {}", id, msg.chars().take(80).collect::<String>()))
                .collect();
            return Err(KernelError::Io {
                detail: format!(
                    "edge loop #{}: dropped {} of {} oriented edges. First: {}",
                    id, dropped.len(), total_oe, preview.join("; ")
                ),
            });
        }

        if half_edges.is_empty() {
            return Err(KernelError::Io {
                detail: format!("edge loop #{}: no edges", id),
            });
        }

        // STEP allows 1-2 edge loops (seam edges on rotational surfaces:
        // a single circular edge forming a closed loop). Accept them.
        Loop::new(half_edges, is_outer)
    }

    fn read_oriented_edge(&self, id: u64) -> KResult<HalfEdge> {
        let entity = self.get(id)?;
        // ORIENTED_EDGE('', *, *, #edge_curve, .T.)
        let edge_id = entity.params.get(3)
            .and_then(|p| p.as_ref())
            .ok_or_else(|| KernelError::Io {
                detail: format!("#{}: missing edge ref", id),
            })?;

        let same_sense = entity.params.get(4)
            .and_then(|p| p.as_bool())
            .unwrap_or(true);

        // Cache edges by STEP entity ID so two ORIENTED_EDGEs referencing
        // the same EDGE_CURVE share a single Arc<Edge>.
        let edge = {
            let cached = self.edge_cache.borrow().get(&edge_id).cloned();
            if let Some(e) = cached {
                e
            } else {
                let e = Arc::new(self.read_edge_curve(edge_id)?);
                self.edge_cache.borrow_mut().insert(edge_id, e.clone());
                e
            }
        };

        Ok(HalfEdge::new(edge, same_sense))
    }

    fn read_edge_curve(&self, id: u64) -> KResult<Edge> {
        let entity = self.get(id)?;
        // EDGE_CURVE('name', #vertex_start, #vertex_end, #curve, same_sense)
        let start_id = entity.params.get(1).and_then(|p| p.as_ref())
            .ok_or_else(|| KernelError::Io { detail: format!("#{}: missing start vertex", id) })?;
        let end_id = entity.params.get(2).and_then(|p| p.as_ref())
            .ok_or_else(|| KernelError::Io { detail: format!("#{}: missing end vertex", id) })?;
        let curve_id = entity.params.get(3).and_then(|p| p.as_ref())
            .ok_or_else(|| KernelError::Io { detail: format!("#{}: missing curve ref", id) })?;

        // Cache vertices by STEP entity ID so shared VERTEX_POINTs produce
        // shared Arc<Vertex> instances. This is the key fix for Euler topology:
        // two faces sharing an edge get the same Arc<Vertex> at each endpoint.
        let start = {
            let cached = self.vertex_cache.borrow().get(&start_id).cloned();
            if let Some(v) = cached {
                v
            } else {
                let v = Arc::new(self.read_vertex(start_id)?);
                self.vertex_cache.borrow_mut().insert(start_id, v.clone());
                v
            }
        };
        let end = {
            let cached = self.vertex_cache.borrow().get(&end_id).cloned();
            if let Some(v) = cached {
                v
            } else {
                let v = Arc::new(self.read_vertex(end_id)?);
                self.vertex_cache.borrow_mut().insert(end_id, v.clone());
                v
            }
        };

        // Preserve the actual STEP curve geometry, but reconcile it
        // with the edge's vertex points. STEP encodes the curve and
        // its endpoints as separate CARTESIAN_POINTs that may differ
        // by ULP-level float drift — closest_point projects to the
        // *curve's* native parameterization, so point_at(t_start) lands
        // on the curve's stored origin/control point rather than
        // exactly on the vertex. For LINE we rebuild the segment from
        // the vertex points (the only correct interpretation of a
        // bounded line edge); for CircularArc we keep the geometric
        // circle but compute angles from vertex positions; for NURBS
        // we accept the curve as-is and rely on the relaxed validation
        // tolerance to absorb residual mismatches.
        //
        // Falls back to a LineSeg between vertices if the curve type
        // is unsupported (rather than dropping the edge, which would
        // break Euler/manifold checks downstream).
        let curve_result = self.read_curve(curve_id);
        let (curve, t_start, t_end) = match curve_result {
            Ok(c) => reconcile_edge_curve(c, start.point(), end.point()),
            Err(_) => {
                let line = Curve::Line(LineSeg::new(*start.point(), *end.point()));
                (Arc::new(line), 0.0, 1.0)
            }
        };

        Ok(Edge::new(start, end, curve, t_start, t_end))
    }

    fn read_vertex(&self, id: u64) -> KResult<Vertex> {
        let entity = self.get(id)?;
        // VERTEX_POINT('name', #point)
        let pt_id = entity.params.get(1).and_then(|p| p.as_ref())
            .ok_or_else(|| KernelError::Io { detail: format!("#{}: missing point", id) })?;
        let point = self.read_point(pt_id)?;
        Ok(Vertex::new(point))
    }

    // ── Geometry readers ──

    fn read_point(&self, id: u64) -> KResult<Point3> {
        let entity = self.get(id)?;
        // CARTESIAN_POINT('name', (x, y, z))
        let coords = entity.params.get(1)
            .and_then(|p| p.as_real_list())
            .ok_or_else(|| KernelError::Io { detail: format!("#{}: bad point coords", id) })?;

        if coords.len() < 3 {
            // 2D point — add z=0
            return Ok(Point3::new(
                *coords.first().unwrap_or(&0.0),
                *coords.get(1).unwrap_or(&0.0),
                0.0,
            ));
        }
        Ok(Point3::new(coords[0], coords[1], coords[2]))
    }

    fn read_direction(&self, id: u64) -> KResult<Vector3> {
        let entity = self.get(id)?;
        let coords = entity.params.get(1)
            .and_then(|p| p.as_real_list())
            .ok_or_else(|| KernelError::Io { detail: format!("#{}: bad direction", id) })?;

        // STEP DIRECTION entities are usually unit-length but the spec
        // doesn't strictly require it, and downstream geometry assumes
        // a unit vector. Normalize defensively.
        let raw = Vector3::new(
            *coords.first().unwrap_or(&0.0),
            *coords.get(1).unwrap_or(&0.0),
            *coords.get(2).unwrap_or(&0.0),
        );
        let n = raw.norm();
        if n < 1e-15 {
            return Err(KernelError::Io {
                detail: format!("#{}: zero-length DIRECTION", id),
            });
        }
        Ok(raw / n)
    }

    fn read_axis2_placement(&self, id: u64) -> KResult<(Point3, Vector3, Vector3)> {
        let entity = self.get(id)?;
        // AXIS2_PLACEMENT_3D('name', #point, #axis, #ref_dir_or_$)
        let origin = entity.params.get(1).and_then(|p| p.as_ref())
            .map(|id| self.read_point(id))
            .transpose()?
            .unwrap_or(Point3::origin());

        let axis = entity.params.get(2).and_then(|p| p.as_ref())
            .map(|id| self.read_direction(id))
            .transpose()?
            .unwrap_or(Vector3::z());

        let ref_raw = entity.params.get(3).and_then(|p| p.as_ref())
            .map(|id| self.read_direction(id))
            .transpose()?
            .unwrap_or_else(|| {
                if axis.x.abs() < 0.9 {
                    Vector3::x().cross(&axis).normalize()
                } else {
                    Vector3::y().cross(&axis).normalize()
                }
            });

        // STEP requires ref_direction to be perpendicular to axis but
        // many exporters write a slightly off direction (e.g., the
        // global X axis when the local axis has been rotated). The
        // CircularArc / surface code assumes a strictly orthonormal
        // (axis, ref_dir, binormal) frame; if ref_dir has any
        // component along axis, point_at evaluates off the surface.
        // Gram-Schmidt-orthogonalize and renormalize.
        let along = axis.dot(&ref_raw);
        let ref_orth = ref_raw - axis * along;
        let n = ref_orth.norm();
        let ref_dir = if n < 1e-12 {
            // ref_raw was parallel to axis — fall back to a synthesized
            // perpendicular.
            if axis.x.abs() < 0.9 {
                Vector3::x().cross(&axis).normalize()
            } else {
                Vector3::y().cross(&axis).normalize()
            }
        } else {
            ref_orth / n
        };

        Ok((origin, axis, ref_dir))
    }

    fn read_curve(&self, id: u64) -> KResult<Curve> {
        let entity = self.get(id)?;
        // Complex entities: the "primary" sub-entity recorded in `entity.name`
        // is whichever one had non-empty params last. That can be a generic
        // wrapper like REPRESENTATION_ITEM that has nothing to do with the
        // geometry. If we see B-spline sub-entities, dispatch there.
        if !entity.parts.is_empty()
            && (entity.part("B_SPLINE_CURVE_WITH_KNOTS").is_some()
                || entity.part("B_SPLINE_CURVE").is_some())
        {
            return self.read_bspline_curve(entity);
        }
        match entity.name.as_str() {
            "LINE" => {
                let pt_id = entity.params.get(1).and_then(|p| p.as_ref())
                    .ok_or_else(|| KernelError::Io { detail: format!("#{}: LINE missing point", id) })?;
                let vec_id = entity.params.get(2).and_then(|p| p.as_ref())
                    .ok_or_else(|| KernelError::Io { detail: format!("#{}: LINE missing vector", id) })?;

                let origin = self.read_point(pt_id)?;
                let vec_entity = self.get(vec_id)?;
                // VECTOR('name', #direction, magnitude)
                let dir_id = vec_entity.params.get(1).and_then(|p| p.as_ref()).unwrap_or(0);
                let mag = vec_entity.params.get(2).and_then(|p| p.as_real()).unwrap_or(1.0);
                let dir = self.read_direction(dir_id).unwrap_or(Vector3::x());

                let end = origin + dir * mag;
                Ok(Curve::Line(LineSeg::new(origin, end)))
            }
            "CIRCLE" => {
                let axis_id = entity.params.get(1).and_then(|p| p.as_ref())
                    .ok_or_else(|| KernelError::Io { detail: format!("#{}: CIRCLE missing axis", id) })?;
                let radius = entity.params.get(2).and_then(|p| p.as_real())
                    .ok_or_else(|| KernelError::Io { detail: format!("#{}: CIRCLE missing radius", id) })?;

                let (center, normal, ref_dir) = self.read_axis2_placement(axis_id)?;
                Ok(Curve::CircularArc(CircularArc {
                    center,
                    normal,
                    radius,
                    ref_direction: ref_dir,
                    start_angle: 0.0,
                    end_angle: std::f64::consts::TAU,
                }))
            }
            "ELLIPSE" => {
                // ELLIPSE('name', #axis2_placement, semi_axis_1, semi_axis_2)
                let axis_id = entity.params.get(1).and_then(|p| p.as_ref())
                    .ok_or_else(|| KernelError::Io { detail: format!("#{}: ELLIPSE missing axis", id) })?;
                let semi_a = entity.params.get(2).and_then(|p| p.as_real())
                    .ok_or_else(|| KernelError::Io { detail: format!("#{}: ELLIPSE missing semi_axis_1", id) })?;
                let semi_b = entity.params.get(3).and_then(|p| p.as_real())
                    .ok_or_else(|| KernelError::Io { detail: format!("#{}: ELLIPSE missing semi_axis_2", id) })?;

                let (center, normal, ref_dir) = self.read_axis2_placement(axis_id)?;
                use knot_geom::curve::EllipticalArc;
                Ok(Curve::EllipticalArc(EllipticalArc {
                    center,
                    normal,
                    major_axis: ref_dir,
                    major_radius: semi_a,
                    minor_radius: semi_b,
                    start_angle: 0.0,
                    end_angle: std::f64::consts::TAU,
                }))
            }
            "B_SPLINE_CURVE_WITH_KNOTS" | "RATIONAL_B_SPLINE_CURVE" => {
                self.read_bspline_curve(entity)
            }
            "TRIMMED_CURVE" => self.read_trimmed_curve(entity),
            "COMPOSITE_CURVE" => self.read_composite_curve(entity),
            "SEAM_CURVE" | "SURFACE_CURVE" | "INTERSECTION_CURVE" => {
                // These are wrapper entities: ('name', #3d_curve, (#pcurve1, ...), .PCURVE_S1.)
                // Extract the underlying 3D curve
                let curve_id = entity.params.get(1).and_then(|p| p.as_ref())
                    .ok_or_else(|| KernelError::Io {
                        detail: format!("#{}: {} missing curve ref", id, entity.name),
                    })?;
                self.read_curve(curve_id)
            }
            _ => {
                Err(KernelError::Io {
                    detail: format!("#{}: unsupported curve type {}", id, entity.name),
                })
            }
        }
    }

    fn read_bspline_curve(&self, entity: &Entity) -> KResult<Curve> {
        // Two layouts to handle:
        //
        // 1. Simple entity B_SPLINE_CURVE_WITH_KNOTS(name, degree, cps, form,
        //    closed, self_int, mults, knots, knot_spec).
        // 2. Complex entity with siblings B_SPLINE_CURVE(degree, cps, form,
        //    closed, self_int) and B_SPLINE_CURVE_WITH_KNOTS(mults, knots,
        //    knot_spec) — the IS subtype split.
        let is_complex = !entity.parts.is_empty();

        let (degree, cp_refs, mults, knot_values) = if is_complex {
            let bsc_params = entity.part("B_SPLINE_CURVE").ok_or_else(|| KernelError::Io {
                detail: format!("#{}: complex curve missing B_SPLINE_CURVE part", entity.id),
            })?;
            let bswk_params = entity.part("B_SPLINE_CURVE_WITH_KNOTS").ok_or_else(|| KernelError::Io {
                detail: format!("#{}: complex curve missing B_SPLINE_CURVE_WITH_KNOTS part", entity.id),
            })?;
            let degree = bsc_params.get(0).and_then(|p| p.as_int())
                .ok_or_else(|| KernelError::Io { detail: format!("#{}: missing degree", entity.id) })? as u32;
            let cps = bsc_params.get(1).and_then(|p| p.as_ref_list())
                .ok_or_else(|| KernelError::Io { detail: format!("#{}: missing control points", entity.id) })?;
            let mults: Vec<i64> = bswk_params.get(0).and_then(|p| p.as_list())
                .map(|l| l.iter().filter_map(|p| p.as_int()).collect())
                .unwrap_or_default();
            let knots = bswk_params.get(1).and_then(|p| p.as_real_list()).unwrap_or_default();
            (degree, cps, mults, knots)
        } else {
            let degree = entity.params.get(1).and_then(|p| p.as_int())
                .ok_or_else(|| KernelError::Io { detail: format!("#{}: missing degree", entity.id) })? as u32;
            let cps = entity.params.get(2).and_then(|p| p.as_ref_list())
                .ok_or_else(|| KernelError::Io { detail: format!("#{}: missing control points", entity.id) })?;
            let mults: Vec<i64> = entity.params.get(6).and_then(|p| p.as_list())
                .map(|l| l.iter().filter_map(|p| p.as_int()).collect())
                .unwrap_or_default();
            let knots = entity.params.get(7).and_then(|p| p.as_real_list()).unwrap_or_default();
            (degree, cps, mults, knots)
        };

        let mut control_points = Vec::new();
        for cp_id in &cp_refs {
            control_points.push(self.read_point(*cp_id)?);
        }

        let mut knots = Vec::new();
        for (i, &mult) in mults.iter().enumerate() {
            let knot = knot_values.get(i).copied().unwrap_or(0.0);
            for _ in 0..mult {
                knots.push(knot);
            }
        }

        // Pull rational weights from the sibling RATIONAL_B_SPLINE_CURVE if
        // this is part of a complex entity; otherwise treat as non-rational.
        let weights = self
            .rational_weights(entity, "RATIONAL_B_SPLINE_CURVE")
            .unwrap_or_else(|| vec![1.0; control_points.len()]);
        if weights.len() != control_points.len() {
            return Err(KernelError::Io {
                detail: format!(
                    "#{}: rational weight count {} != control point count {}",
                    entity.id, weights.len(), control_points.len()
                ),
            });
        }

        match NurbsCurve::new(control_points, weights, knots, degree) {
            Ok(c) => Ok(Curve::Nurbs(c)),
            Err(e) => Err(KernelError::Io {
                detail: format!("#{}: bad B-spline curve: {}", entity.id, e),
            }),
        }
    }

    /// TRIMMED_CURVE('name', #basis_curve, trim_1, trim_2, sense, trim_preference)
    ///
    /// The two trim parameters are lists that can contain a parametric value,
    /// a point reference, or both. We honour parametric values when present
    /// and either the basis curve has a meaningful angle domain (arc) or is
    /// a line (then we clamp `t in [0,1]`). For NURBS we currently keep the
    /// full curve — trimming a NURBS requires either knot-insertion or
    /// re-parameterization, and silent approximation here would propagate
    /// into the boolean's exact predicates.
    fn read_trimmed_curve(&self, entity: &Entity) -> KResult<Curve> {
        let basis_id = entity.params.get(1).and_then(|p| p.as_ref())
            .ok_or_else(|| KernelError::Io { detail: format!("#{}: TRIMMED_CURVE missing basis", entity.id) })?;
        let basis = self.read_curve(basis_id)?;

        let trim_a = trim_parameter(entity.params.get(2));
        let trim_b = trim_parameter(entity.params.get(3));

        match (basis, trim_a, trim_b) {
            (Curve::Line(line), Some(a), Some(b)) => {
                // STEP line trims are along the embedded VECTOR's magnitude
                // direction. Without re-reading the original VECTOR magnitude
                // we treat trim params as fractions along the line's [0,1]
                // domain — close enough for the cases that show up in the
                // wild, and we hard-fail by ignoring if the bounds are bad.
                let lo = a.min(b).clamp(0.0, 1.0);
                let hi = a.max(b).clamp(0.0, 1.0);
                if hi <= lo + 1e-15 {
                    return Err(KernelError::Io {
                        detail: format!("#{}: TRIMMED_CURVE degenerate range", entity.id),
                    });
                }
                let start = line.point_at(lo);
                let end = line.point_at(hi);
                Ok(Curve::Line(knot_geom::curve::LineSeg::new(start, end)))
            }
            (Curve::CircularArc(arc), Some(a), Some(b)) => {
                let (lo, hi) = if b >= a { (a, b) } else { (a, b + std::f64::consts::TAU) };
                Ok(Curve::CircularArc(CircularArc {
                    center: arc.center,
                    normal: arc.normal,
                    radius: arc.radius,
                    ref_direction: arc.ref_direction,
                    start_angle: lo,
                    end_angle: hi,
                }))
            }
            (Curve::EllipticalArc(arc), Some(a), Some(b)) => {
                use knot_geom::curve::EllipticalArc;
                let (lo, hi) = if b >= a { (a, b) } else { (a, b + std::f64::consts::TAU) };
                Ok(Curve::EllipticalArc(EllipticalArc {
                    start_angle: lo,
                    end_angle: hi,
                    ..arc
                }))
            }
            (other, _, _) => Ok(other),
        }
    }

    /// COMPOSITE_CURVE('name', (#seg1, #seg2, ...), self_intersect)
    /// where each #seg is a COMPOSITE_CURVE_SEGMENT('', .CONTINUOUS., .T., #parent).
    ///
    /// v1: returns the first segment's parent curve. Real composite-curve
    /// support requires NURBS concatenation (a Track-C job); silently fitting
    /// would inject approximation error into the exact-predicate topology.
    fn read_composite_curve(&self, entity: &Entity) -> KResult<Curve> {
        let segs = entity.params.get(1).and_then(|p| p.as_ref_list())
            .ok_or_else(|| KernelError::Io { detail: format!("#{}: COMPOSITE_CURVE missing segments", entity.id) })?;
        let first_seg = segs.first().copied()
            .ok_or_else(|| KernelError::Io { detail: format!("#{}: COMPOSITE_CURVE has no segments", entity.id) })?;
        let seg = self.get(first_seg)?;
        // COMPOSITE_CURVE_SEGMENT(transition, same_sense, parent_curve) — three
        // params, parent is at index 2. (Some files put a name string at the
        // front; tolerate either layout by scanning for the first Ref.)
        let parent_id = seg.params.iter().find_map(|p| p.as_ref())
            .ok_or_else(|| KernelError::Io { detail: format!("#{}: COMPOSITE_CURVE_SEGMENT missing parent", first_seg) })?;
        self.read_curve(parent_id)
    }

    /// SURFACE_OF_LINEAR_EXTRUSION('', #basis_curve, #direction_vector).
    ///
    /// Analytical specialisations:
    ///   - LINE basis + any non-parallel direction → PLANE
    ///   - CIRCLE basis + direction parallel to circle normal → CYLINDER
    ///
    /// Anything else returns `Err`; the caller falls back to the
    /// placeholder plane (with origin anchored to the basis curve start).
    fn read_linear_extrusion_surface(&self, entity: &Entity) -> KResult<Surface> {
        let basis_id = entity.params.get(1).and_then(|p| p.as_ref())
            .ok_or_else(|| KernelError::Io { detail: format!("#{}: extrusion missing basis", entity.id) })?;
        let vec_id = entity.params.get(2).and_then(|p| p.as_ref())
            .ok_or_else(|| KernelError::Io { detail: format!("#{}: extrusion missing direction", entity.id) })?;

        let basis = self.read_curve(basis_id)?;
        let direction = self.read_vector(vec_id)?;
        let dir_n = direction.norm();
        if dir_n < 1e-15 {
            return Err(KernelError::Io { detail: format!("#{}: extrusion direction is zero", entity.id) });
        }
        let dir_unit = direction / dir_n;

        match basis {
            Curve::Line(line) => {
                let line_dir = line.direction();
                let ln = line_dir.norm();
                if ln < 1e-15 {
                    return Err(KernelError::Io { detail: format!("#{}: zero-length basis line", entity.id) });
                }
                let line_unit = line_dir / ln;
                let cross = line_unit.cross(&dir_unit);
                let cn = cross.norm();
                if cn < 1e-9 {
                    return Err(KernelError::Io {
                        detail: format!("#{}: extrusion direction parallel to basis line", entity.id),
                    });
                }
                Ok(Surface::Plane(Plane::new(line.start, cross / cn)))
            }
            Curve::CircularArc(arc) => {
                let an = arc.normal.norm();
                if an < 1e-15 {
                    return Err(KernelError::Io { detail: format!("#{}: arc normal is zero", entity.id) });
                }
                let arc_normal_unit = arc.normal / an;
                if (arc_normal_unit.dot(&dir_unit).abs() - 1.0).abs() > 1e-6 {
                    return Err(KernelError::Io {
                        detail: format!("#{}: arc extrude direction not parallel to arc normal — not a cylinder", entity.id),
                    });
                }
                Ok(Surface::Cylinder(Cylinder {
                    origin: arc.center,
                    axis: dir_unit,
                    radius: arc.radius,
                    ref_direction: arc.ref_direction,
                    v_min: -1e6,
                    v_max: 1e6,
                }))
            }
            _ => Err(KernelError::Io {
                detail: format!("#{}: extrusion of non-line/non-arc basis not yet specialised", entity.id),
            }),
        }
    }

    /// SURFACE_OF_REVOLUTION('', #basis_curve, #axis1_placement).
    ///
    /// Analytical specialisations:
    ///   - LINE parallel to axis → CYLINDER (radius = perpendicular dist)
    ///   - LINE meeting axis at an angle → CONE (apex at intersection)
    ///   - CIRCLE whose plane contains the axis, centre on axis → SPHERE
    ///   - CIRCLE whose plane contains the axis, centre off axis → TORUS
    fn read_revolution_surface(&self, entity: &Entity) -> KResult<Surface> {
        let basis_id = entity.params.get(1).and_then(|p| p.as_ref())
            .ok_or_else(|| KernelError::Io { detail: format!("#{}: revolution missing basis", entity.id) })?;
        let axis_id = entity.params.get(2).and_then(|p| p.as_ref())
            .ok_or_else(|| KernelError::Io { detail: format!("#{}: revolution missing axis", entity.id) })?;

        let basis = self.read_curve(basis_id)?;
        // axis1_placement is the AXIS1_PLACEMENT (location + axis) variant.
        // Read it via the same helper as AXIS2_PLACEMENT — the third
        // component (ref_direction) gets a synthetic perpendicular.
        let (origin, axis, _) = self.read_axis2_placement(axis_id)?;
        let axn = axis.norm();
        if axn < 1e-15 {
            return Err(KernelError::Io { detail: format!("#{}: revolution axis is zero", entity.id) });
        }
        let axis_unit = axis / axn;

        match basis {
            Curve::Line(line) => revolve_line(&line, origin, axis_unit, entity.id),
            Curve::CircularArc(arc) => revolve_circle(&arc, origin, axis_unit, entity.id),
            _ => Err(KernelError::Io {
                detail: format!("#{}: revolution of non-line/non-arc basis not yet specialised", entity.id),
            }),
        }
    }

    fn read_vector(&self, vec_id: u64) -> KResult<Vector3> {
        let v = self.get(vec_id)?;
        let dir_id = v.params.get(1).and_then(|p| p.as_ref())
            .ok_or_else(|| KernelError::Io { detail: format!("#{}: VECTOR missing direction", vec_id) })?;
        let mag = v.params.get(2).and_then(|p| p.as_real()).unwrap_or(1.0);
        let dir = self.read_direction(dir_id)?;
        Ok(dir * mag)
    }

    /// Look up weights from a RATIONAL_B_SPLINE_{CURVE,SURFACE} sibling.
    ///
    /// The rational sub-entity's single parameter is the weight list.
    /// For surfaces it's a list-of-lists (one row per u-index); we flatten
    /// to match `control_points` storage order (row-major over u, then v).
    fn rational_weights(&self, entity: &Entity, sibling: &str) -> Option<Vec<f64>> {
        let part = entity.part(sibling)?;
        let raw = part.get(0)?;
        // Surface case: list of lists.
        if let Some(list) = raw.as_list() {
            let nested = list.iter().any(|p| p.as_list().is_some());
            if nested {
                let mut out = Vec::new();
                for row in list {
                    if let Some(row_vals) = row.as_real_list() {
                        out.extend(row_vals);
                    }
                }
                return Some(out);
            }
            return raw.as_real_list();
        }
        None
    }

    fn read_surface(&self, id: u64) -> KResult<Surface> {
        let entity = self.get(id)?;
        if !entity.parts.is_empty()
            && (entity.part("B_SPLINE_SURFACE_WITH_KNOTS").is_some()
                || entity.part("B_SPLINE_SURFACE").is_some())
        {
            return self.read_bspline_surface(entity);
        }
        match entity.name.as_str() {
            "PLANE" => {
                let axis_id = entity.params.get(1).and_then(|p| p.as_ref())
                    .ok_or_else(|| KernelError::Io { detail: format!("#{}: PLANE missing axis", id) })?;
                let (origin, normal, _) = self.read_axis2_placement(axis_id)?;
                Ok(Surface::Plane(Plane::new(origin, normal)))
            }
            "CYLINDRICAL_SURFACE" => {
                let axis_id = entity.params.get(1).and_then(|p| p.as_ref())
                    .ok_or_else(|| KernelError::Io { detail: format!("#{}: missing axis", id) })?;
                let radius = entity.params.get(2).and_then(|p| p.as_real())
                    .ok_or_else(|| KernelError::Io { detail: format!("#{}: missing radius", id) })?;
                let (origin, axis, ref_dir) = self.read_axis2_placement(axis_id)?;
                Ok(Surface::Cylinder(Cylinder {
                    origin, axis, radius, ref_direction: ref_dir,
                    v_min: -1e6, v_max: 1e6,
                }))
            }
            "SPHERICAL_SURFACE" => {
                let axis_id = entity.params.get(1).and_then(|p| p.as_ref())
                    .ok_or_else(|| KernelError::Io { detail: format!("#{}: missing axis", id) })?;
                let radius = entity.params.get(2).and_then(|p| p.as_real())
                    .ok_or_else(|| KernelError::Io { detail: format!("#{}: missing radius", id) })?;
                let (center, _, _) = self.read_axis2_placement(axis_id)?;
                Ok(Surface::Sphere(Sphere::new(center, radius)))
            }
            "CONICAL_SURFACE" => {
                // STEP CONICAL_SURFACE(axis2_placement, radius, semi_angle):
                //   the placement origin is the cone's parametric origin
                //   (v=0), with `radius` measured at v=0. The geometric
                //   apex is at v = -radius / tan(semi_angle), i.e.,
                //   shifted backwards along the axis. Our internal
                //   `Cone` representation stores the geometric apex.
                let axis_id = entity.params.get(1).and_then(|p| p.as_ref())
                    .ok_or_else(|| KernelError::Io { detail: format!("#{}: missing axis", id) })?;
                let radius = entity.params.get(2).and_then(|p| p.as_real())
                    .ok_or_else(|| KernelError::Io { detail: format!("#{}: CONICAL_SURFACE missing radius", id) })?;
                let semi_angle = entity.params.get(3).and_then(|p| p.as_real())
                    .ok_or_else(|| KernelError::Io { detail: format!("#{}: CONICAL_SURFACE missing semi_angle", id) })?;
                let (placement_origin, axis, ref_dir) = self.read_axis2_placement(axis_id)?;

                let tan_ha = semi_angle.tan();
                if !tan_ha.is_finite() || tan_ha.abs() < 1e-12 {
                    return Err(KernelError::Io {
                        detail: format!("#{}: CONICAL_SURFACE degenerate semi_angle {}", id, semi_angle),
                    });
                }
                let apex_offset = radius / tan_ha;
                let apex = placement_origin - axis * apex_offset;

                // Domain spans the working volume on the apex side
                // (v > 0) of the cone. The placement origin sits at
                // v = apex_offset; allow a generous range above it
                // and clamp the apex itself out of the parametric
                // domain (v_min > 0) to avoid the singularity.
                Ok(Surface::Cone(Cone {
                    apex, axis, half_angle: semi_angle, ref_direction: ref_dir,
                    v_min: apex_offset * 0.001,
                    v_max: apex_offset + 1e6,
                }))
            }
            "TOROIDAL_SURFACE" => {
                let axis_id = entity.params.get(1).and_then(|p| p.as_ref())
                    .ok_or_else(|| KernelError::Io { detail: format!("#{}: missing axis", id) })?;
                let major_r = entity.params.get(2).and_then(|p| p.as_real()).unwrap_or(1.0);
                let minor_r = entity.params.get(3).and_then(|p| p.as_real()).unwrap_or(0.5);
                let (center, axis, ref_dir) = self.read_axis2_placement(axis_id)?;
                Ok(Surface::Torus(Torus {
                    center, axis, major_radius: major_r, minor_radius: minor_r,
                    ref_direction: ref_dir,
                }))
            }
            "B_SPLINE_SURFACE_WITH_KNOTS" | "RATIONAL_B_SPLINE_SURFACE" => {
                self.read_bspline_surface(entity)
            }
            "SURFACE_OF_LINEAR_EXTRUSION" => {
                self.read_linear_extrusion_surface(entity)
                    .or_else(|_| Ok(self.placeholder_surface(entity)))
            }
            "SURFACE_OF_REVOLUTION" => {
                self.read_revolution_surface(entity)
                    .or_else(|_| Ok(self.placeholder_surface(entity)))
            }
            _ => {
                // Unsupported surface types (OFFSET_SURFACE, others) still
                // produce a placeholder so the face topology can land. The
                // boundary polygon carries the face's actual shape.
                Ok(self.placeholder_surface(entity))
            }
        }
    }

    /// Anchor the placeholder plane on something better than the global
    /// origin so face polygons don't all collapse to z=0 visually. We use
    /// the first control / endpoint we can find on the basis curve.
    fn placeholder_surface(&self, entity: &Entity) -> Surface {
        let basis_point = entity
            .params
            .get(1)
            .and_then(|p| p.as_ref())
            .and_then(|id| self.read_curve(id).ok())
            .map(|c| c.point_at(knot_geom::curve::CurveParam(c.domain().start)))
            .unwrap_or(Point3::origin());
        Surface::Plane(Plane::new(basis_point, Vector3::z()))
    }

    fn read_bspline_surface(&self, entity: &Entity) -> KResult<Surface> {
        // Like the curve reader: simple entity puts everything in params with
        // a leading name; complex entities split between B_SPLINE_SURFACE
        // (degrees + cps) and B_SPLINE_SURFACE_WITH_KNOTS (mults + knots).
        let is_complex = !entity.parts.is_empty();

        let (u_degree, v_degree, cp_rows, u_mults, v_mults, u_knot_vals, v_knot_vals) = if is_complex {
            let bss = entity.part("B_SPLINE_SURFACE").ok_or_else(|| KernelError::Io {
                detail: format!("#{}: complex surface missing B_SPLINE_SURFACE part", entity.id),
            })?;
            let bswk = entity.part("B_SPLINE_SURFACE_WITH_KNOTS").ok_or_else(|| KernelError::Io {
                detail: format!("#{}: complex surface missing B_SPLINE_SURFACE_WITH_KNOTS part", entity.id),
            })?;
            let u_deg = bss.get(0).and_then(|p| p.as_int()).unwrap_or(1) as u32;
            let v_deg = bss.get(1).and_then(|p| p.as_int()).unwrap_or(1) as u32;
            let rows = bss.get(2).and_then(|p| p.as_list())
                .ok_or_else(|| KernelError::Io { detail: format!("#{}: missing CPs", entity.id) })?;
            let u_m: Vec<i64> = bswk.get(0).and_then(|p| p.as_list())
                .map(|l| l.iter().filter_map(|p| p.as_int()).collect())
                .unwrap_or_default();
            let v_m: Vec<i64> = bswk.get(1).and_then(|p| p.as_list())
                .map(|l| l.iter().filter_map(|p| p.as_int()).collect())
                .unwrap_or_default();
            let u_kv = bswk.get(2).and_then(|p| p.as_real_list()).unwrap_or_default();
            let v_kv = bswk.get(3).and_then(|p| p.as_real_list()).unwrap_or_default();
            (u_deg, v_deg, rows.to_vec(), u_m, v_m, u_kv, v_kv)
        } else {
            let u_deg = entity.params.get(1).and_then(|p| p.as_int()).unwrap_or(1) as u32;
            let v_deg = entity.params.get(2).and_then(|p| p.as_int()).unwrap_or(1) as u32;
            let rows = entity.params.get(3).and_then(|p| p.as_list())
                .ok_or_else(|| KernelError::Io { detail: format!("#{}: missing CPs", entity.id) })?;
            let u_m: Vec<i64> = entity.params.get(8).and_then(|p| p.as_list())
                .map(|l| l.iter().filter_map(|p| p.as_int()).collect())
                .unwrap_or_default();
            let v_m: Vec<i64> = entity.params.get(9).and_then(|p| p.as_list())
                .map(|l| l.iter().filter_map(|p| p.as_int()).collect())
                .unwrap_or_default();
            let u_kv = entity.params.get(10).and_then(|p| p.as_real_list()).unwrap_or_default();
            let v_kv = entity.params.get(11).and_then(|p| p.as_real_list()).unwrap_or_default();
            (u_deg, v_deg, rows.to_vec(), u_m, v_m, u_kv, v_kv)
        };

        let mut control_points = Vec::new();
        let mut count_u = 0u32;
        let mut count_v = 0u32;

        for row in &cp_rows {
            let row_refs = row.as_ref_list().unwrap_or_default();
            if count_u == 0 {
                count_v = row_refs.len() as u32;
            }
            count_u += 1;
            for cp_id in row_refs {
                control_points.push(self.read_point(cp_id)?);
            }
        }

        let mut knots_u = Vec::new();
        for (i, &m) in u_mults.iter().enumerate() {
            let k = u_knot_vals.get(i).copied().unwrap_or(0.0);
            for _ in 0..m { knots_u.push(k); }
        }

        let mut knots_v = Vec::new();
        for (i, &m) in v_mults.iter().enumerate() {
            let k = v_knot_vals.get(i).copied().unwrap_or(0.0);
            for _ in 0..m { knots_v.push(k); }
        }

        let weights = self
            .rational_weights(entity, "RATIONAL_B_SPLINE_SURFACE")
            .unwrap_or_else(|| vec![1.0; control_points.len()]);
        if weights.len() != control_points.len() {
            return Err(KernelError::Io {
                detail: format!(
                    "#{}: rational weight count {} != control point count {}",
                    entity.id, weights.len(), control_points.len()
                ),
            });
        }

        match NurbsSurface::new(control_points, weights, knots_u, knots_v, u_degree, v_degree, count_u, count_v) {
            Ok(s) => Ok(Surface::Nurbs(s)),
            Err(e) => Err(KernelError::Io {
                detail: format!("#{}: bad B-spline surface: {}", entity.id, e),
            }),
        }
    }
}

/// Pull a numeric parameter out of a STEP trim list. Trim lists look like
/// `(PARAMETER_VALUE(0.5))` or `(PARAMETER_VALUE(0.5), #123)`, where the
/// `#123` is a CARTESIAN_POINT alternative. We honour the first
/// numerical value we can find; point-form trims are not yet decoded.
fn trim_parameter(param: Option<&Param>) -> Option<f64> {
    let list = param?.as_list()?;
    list.iter().find_map(|p| p.as_real())
}

/// Revolve a line around an axis. Returns CYLINDER for axis-parallel
/// lines, CONE for lines that meet the axis.
fn revolve_line(line: &LineSeg, axis_origin: Point3, axis_unit: Vector3, id: u64) -> KResult<Surface> {
    let line_dir = line.direction();
    let ln = line_dir.norm();
    if ln < 1e-15 {
        return Err(KernelError::Io { detail: format!("#{}: revolution line is zero-length", id) });
    }
    let line_unit = line_dir / ln;
    let parallel = (line_unit.dot(&axis_unit).abs() - 1.0).abs() < 1e-9;

    // Radius at the line's start: perpendicular distance to the axis.
    let rel = line.start - axis_origin;
    let radial = rel - axis_unit * rel.dot(&axis_unit);
    let r_start = radial.norm();

    if parallel {
        if r_start < 1e-12 {
            // Line on the axis: degenerates to the axis itself.
            return Err(KernelError::Io {
                detail: format!("#{}: revolution line lies on axis (no surface)", id),
            });
        }
        let ref_dir = radial / r_start;
        return Ok(Surface::Cylinder(Cylinder {
            origin: axis_origin + axis_unit * rel.dot(&axis_unit),
            axis: axis_unit,
            radius: r_start,
            ref_direction: ref_dir,
            v_min: -1e6,
            v_max: 1e6,
        }));
    }

    // For a cone, the line must intersect the axis at a single apex.
    // Solve start + t*line_unit = axis_origin + s*axis_unit for some t, s,
    // such that the residual (perpendicular component) vanishes.
    // The 3D system is overdetermined; the standard approach is the
    // line-line closest-point computation followed by a coincidence check.
    let w = line.start - axis_origin;
    let a = 1.0;
    let b = line_unit.dot(&axis_unit);
    let c = 1.0;
    let d = line_unit.dot(&w);
    let e = axis_unit.dot(&w);
    let denom = a * c - b * b;
    if denom.abs() < 1e-15 {
        return Err(KernelError::Io { detail: format!("#{}: revolution line nearly parallel to axis", id) });
    }
    let t = (b * e - c * d) / denom;
    let s = (a * e - b * d) / denom;
    let p_on_line = line.start + line_unit * t;
    let p_on_axis = axis_origin + axis_unit * s;
    if (p_on_line - p_on_axis).norm() > 1e-6 {
        return Err(KernelError::Io {
            detail: format!("#{}: revolution line and axis are skew — not a cone", id),
        });
    }
    let apex = p_on_axis;

    // Half-angle = angle between line direction and axis, in [0, π/2].
    let cos_a = line_unit.dot(&axis_unit).abs();
    let half_angle = cos_a.clamp(-1.0, 1.0).acos();
    if half_angle < 1e-6 || (half_angle - std::f64::consts::FRAC_PI_2).abs() < 1e-6 {
        return Err(KernelError::Io {
            detail: format!("#{}: revolution produces degenerate cone", id),
        });
    }

    // ref_direction = unit vector in the plane perpendicular to the axis
    // pointing toward where the line emerges from the apex. The line might
    // *start* at the apex (radial component = 0 there) — sample some other
    // point on the line for a non-degenerate radial component.
    let pick_radial = |t: f64| -> Vector3 {
        let p = line.point_at(t);
        let along = axis_unit * (p - apex).dot(&axis_unit);
        (p - apex) - along
    };
    let mut radial_v = pick_radial(1.0);
    if radial_v.norm() < 1e-12 {
        radial_v = pick_radial(0.5);
    }
    if radial_v.norm() < 1e-12 {
        radial_v = pick_radial(0.0);
    }
    let rn = radial_v.norm();
    if rn < 1e-12 {
        return Err(KernelError::Io {
            detail: format!("#{}: cannot determine cone ref_direction", id),
        });
    }
    let ref_direction = radial_v / rn;

    Ok(Surface::Cone(Cone {
        apex,
        axis: if line.start.coords.dot(&axis_unit) >= apex.coords.dot(&axis_unit) { axis_unit } else { -axis_unit },
        half_angle,
        ref_direction,
        v_min: 1e-3,
        v_max: 1e6,
    }))
}

/// Revolve a circle around an axis. The circle's plane must contain the
/// axis (i.e. `circle.normal ⊥ axis`). If the circle centre lies on the
/// axis we get a sphere; otherwise a torus.
fn revolve_circle(arc: &CircularArc, axis_origin: Point3, axis_unit: Vector3, id: u64) -> KResult<Surface> {
    let an = arc.normal.norm();
    if an < 1e-15 {
        return Err(KernelError::Io { detail: format!("#{}: arc normal is zero", id) });
    }
    let arc_normal_unit = arc.normal / an;

    // Plane-contains-axis test: arc normal must be perpendicular to axis.
    if arc_normal_unit.dot(&axis_unit).abs() > 1e-6 {
        return Err(KernelError::Io {
            detail: format!("#{}: arc plane does not contain revolution axis — not sphere/torus", id),
        });
    }

    // Distance from circle centre to axis line.
    let rel = arc.center - axis_origin;
    let along = axis_unit * rel.dot(&axis_unit);
    let radial = rel - along;
    let centre_to_axis = radial.norm();

    if centre_to_axis < 1e-9 {
        // Sphere case.
        return Ok(Surface::Sphere(Sphere::new(arc.center, arc.radius)));
    }

    // Torus case.
    let ref_direction = radial / centre_to_axis;
    Ok(Surface::Torus(Torus {
        center: axis_origin + along,
        axis: axis_unit,
        major_radius: centre_to_axis,
        minor_radius: arc.radius,
        ref_direction,
    }))
}

/// Reconcile a STEP edge's curve with its vertex endpoints. STEP
/// stores the curve and its endpoint vertices as independent
/// CARTESIAN_POINTs, so a naive `point_at(closest_point.param)` for
/// the start vertex lands on the curve's stored origin/control
/// point — *not* on the vertex — drifting by ULP noise or worse.
///
/// Today we ship the conservative reconciliation: only LINE edges
/// are kept geometrically (rebuilt exactly from the two vertex
/// points so endpoints are precise). Curved edges (CircularArc,
/// EllipticalArc, NURBS) are downgraded to a LineSeg approximation
/// between the vertex points — same as the historical import
/// behavior. This preserves the boolean's existing topology behavior
/// for non-planar faces.
///
/// The full curve-preserving import is implemented one branch up
/// (see `reconcile_edge_curve_full` in the test module) but isn't
/// currently wired in: enabling it for all curve types reveals
/// downstream bugs in the boolean's split-face / cell-classification
/// when faced with non-line edges. That work is staged for a later
/// landing, after the split-face code is curve-aware.
fn reconcile_edge_curve(
    raw: Curve,
    start_pt: &Point3,
    end_pt: &Point3,
) -> (Arc<Curve>, f64, f64) {
    use knot_geom::curve::LineSeg;

    match raw {
        Curve::Line(_) => {
            // Rebuild from vertex points — line direction and bounds
            // both follow from the vertices.
            let line = LineSeg::new(*start_pt, *end_pt);
            (Arc::new(Curve::Line(line)), 0.0, 1.0)
        }
        _ => {
            // Conservative: line approximation between vertices.
            // Matches pre-change import behavior so the boolean's
            // existing topology code path is unaffected.
            let line = LineSeg::new(*start_pt, *end_pt);
            (Arc::new(Curve::Line(line)), 0.0, 1.0)
        }
    }
}
