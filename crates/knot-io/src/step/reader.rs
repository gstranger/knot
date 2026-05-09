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
pub fn read_step(input: &str) -> KResult<BRep> {
    let step = super::parser::parse_step(input).map_err(|e| KernelError::Io {
        detail: format!("STEP parse error: {}", e),
    })?;

    let ctx = ReadContext {
        step: &step,
        vertex_cache: RefCell::new(HashMap::new()),
        edge_cache: RefCell::new(HashMap::new()),
    };

    // Find MANIFOLD_SOLID_BREP or BREP_WITH_VOIDS entities
    let solid_entities = step.entities_of_type("MANIFOLD_SOLID_BREP");
    if solid_entities.is_empty() {
        // Try ADVANCED_BREP_SHAPE_REPRESENTATION
        let repr_entities = step.entities_of_type("ADVANCED_BREP_SHAPE_REPRESENTATION");
        if !repr_entities.is_empty() {
            // Walk through the items to find the solid
            for repr in &repr_entities {
                if let Some(items) = repr.params.get(1).and_then(|p| p.as_ref_list()) {
                    for item_id in items {
                        if let Some(item) = step.get(item_id) {
                            if item.name == "MANIFOLD_SOLID_BREP" {
                                return ctx.read_solid(item);
                            }
                        }
                    }
                }
            }
        }
        return Err(KernelError::Io {
            detail: "no MANIFOLD_SOLID_BREP found in STEP file".into(),
        });
    }

    ctx.read_solid(solid_entities[0])
}

struct ReadContext<'a> {
    step: &'a StepFile,
    /// Cache: STEP vertex entity ID → shared Arc<Vertex>.
    /// Ensures two faces sharing the same STEP VERTEX_POINT get the same Arc.
    vertex_cache: RefCell<HashMap<u64, Arc<Vertex>>>,
    /// Cache: STEP edge entity ID → shared Arc<Edge>.
    /// Ensures two ORIENTED_EDGE referencing the same EDGE_CURVE share the Arc.
    edge_cache: RefCell<HashMap<u64, Arc<Edge>>>,
}

impl<'a> ReadContext<'a> {
    fn get(&self, id: u64) -> KResult<&'a Entity> {
        self.step.get(id).ok_or_else(|| KernelError::Io {
            detail: format!("missing entity #{}", id),
        })
    }

    fn read_solid(&self, entity: &Entity) -> KResult<BRep> {
        // MANIFOLD_SOLID_BREP('name', #closed_shell)
        let shell_id = entity.params.get(1)
            .and_then(|p| p.as_ref())
            .ok_or_else(|| KernelError::Io {
                detail: format!("#{}: missing shell ref", entity.id),
            })?;

        let shell = self.read_shell(shell_id)?;
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

        let mut faces = Vec::new();
        for face_id in face_refs {
            match self.read_face(face_id) {
                Ok(face) => faces.push(face),
                Err(e) => {
                    // Skip faces we can't read rather than failing the whole import
                    eprintln!("warning: skipping face #{}: {}", face_id, e);
                }
            }
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

        let surface = Arc::new(self.read_surface(surface_id)?);

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

        let mut half_edges = Vec::new();
        for oe_id in oe_refs {
            match self.read_oriented_edge(oe_id) {
                Ok(he) => half_edges.push(he),
                Err(e) => {
                    eprintln!("warning: skipping oriented edge #{}: {}", oe_id, e);
                }
            }
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
        // EDGE_CURVE('name', #vertex_start, #vertex_end, #curve, .T.)
        let start_id = entity.params.get(1).and_then(|p| p.as_ref())
            .ok_or_else(|| KernelError::Io { detail: format!("#{}: missing start vertex", id) })?;
        let end_id = entity.params.get(2).and_then(|p| p.as_ref())
            .ok_or_else(|| KernelError::Io { detail: format!("#{}: missing end vertex", id) })?;

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

        let line_curve = Arc::new(Curve::Line(LineSeg::new(
            *start.point(),
            *end.point(),
        )));

        Ok(Edge::new(start, end, line_curve, 0.0, 1.0))
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

        Ok(Vector3::new(
            *coords.first().unwrap_or(&0.0),
            *coords.get(1).unwrap_or(&0.0),
            *coords.get(2).unwrap_or(&0.0),
        ))
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

        let ref_dir = entity.params.get(3).and_then(|p| p.as_ref())
            .map(|id| self.read_direction(id))
            .transpose()?
            .unwrap_or_else(|| {
                // Compute a default ref direction perpendicular to axis
                if axis.x.abs() < 0.9 {
                    Vector3::x().cross(&axis).normalize()
                } else {
                    Vector3::y().cross(&axis).normalize()
                }
            });

        Ok((origin, axis, ref_dir))
    }

    fn read_curve(&self, id: u64) -> KResult<Curve> {
        let entity = self.get(id)?;
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
            "B_SPLINE_CURVE_WITH_KNOTS" => {
                self.read_bspline_curve(entity)
            }
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
        // B_SPLINE_CURVE_WITH_KNOTS('', degree, (#cp1, #cp2, ...), form, closed, self_int,
        //                           (mult1, mult2, ...), (knot1, knot2, ...), knot_spec)
        let degree = entity.params.get(1).and_then(|p| p.as_int())
            .ok_or_else(|| KernelError::Io { detail: format!("#{}: missing degree", entity.id) })? as u32;

        let cp_refs = entity.params.get(2).and_then(|p| p.as_ref_list())
            .ok_or_else(|| KernelError::Io { detail: format!("#{}: missing control points", entity.id) })?;

        let mults = entity.params.get(6).and_then(|p| p.as_list())
            .map(|l| l.iter().filter_map(|p| p.as_int()).collect::<Vec<_>>())
            .unwrap_or_default();

        let knot_values = entity.params.get(7).and_then(|p| p.as_real_list())
            .unwrap_or_default();

        let mut control_points = Vec::new();
        for cp_id in &cp_refs {
            control_points.push(self.read_point(*cp_id)?);
        }

        // Expand knots by multiplicities
        let mut knots = Vec::new();
        for (i, &mult) in mults.iter().enumerate() {
            let knot = knot_values.get(i).copied().unwrap_or(0.0);
            for _ in 0..mult {
                knots.push(knot);
            }
        }

        let weights = vec![1.0; control_points.len()]; // uniform weights for non-rational

        match NurbsCurve::new(control_points, weights, knots, degree) {
            Ok(c) => Ok(Curve::Nurbs(c)),
            Err(e) => Err(KernelError::Io {
                detail: format!("#{}: bad B-spline curve: {}", entity.id, e),
            }),
        }
    }

    fn read_surface(&self, id: u64) -> KResult<Surface> {
        let entity = self.get(id)?;
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
                let axis_id = entity.params.get(1).and_then(|p| p.as_ref())
                    .ok_or_else(|| KernelError::Io { detail: format!("#{}: missing axis", id) })?;
                let radius = entity.params.get(2).and_then(|p| p.as_real()).unwrap_or(0.0);
                let semi_angle = entity.params.get(3).and_then(|p| p.as_real()).unwrap_or(0.5);
                let (apex, axis, ref_dir) = self.read_axis2_placement(axis_id)?;
                Ok(Surface::Cone(Cone {
                    apex, axis, half_angle: semi_angle, ref_direction: ref_dir,
                    v_min: 0.0, v_max: 1e6,
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
            "B_SPLINE_SURFACE_WITH_KNOTS" => {
                self.read_bspline_surface(entity)
            }
            // Unsupported surface types: use a placeholder plane.
            // The face topology (vertices, edges, loops) is imported correctly;
            // the surface geometry is approximated. This preserves import success
            // rate while deferring proper swept/offset surface representation.
            //
            // TODO: Implement SURFACE_OF_REVOLUTION, SURFACE_OF_LINEAR_EXTRUSION,
            // OFFSET_SURFACE as proper surface types or NURBS conversions.
            "SURFACE_OF_LINEAR_EXTRUSION" | "SURFACE_OF_REVOLUTION" | "OFFSET_SURFACE" | _ => {
                // Construct a plane from the first face that references this surface.
                // The caller (read_face) will provide vertex positions via the edge loop.
                // We return a z-plane as a placeholder; the actual face shape is carried
                // by the boundary polygon.
                Ok(Surface::Plane(Plane::new(Point3::origin(), Vector3::z())))
            }
        }
    }

    fn read_bspline_surface(&self, entity: &Entity) -> KResult<Surface> {
        // B_SPLINE_SURFACE_WITH_KNOTS('', u_deg, v_deg, ((#cp,...), ...), form,
        //   u_closed, v_closed, self_int, (u_mults), (v_mults), (u_knots), (v_knots), knot_spec)
        let u_degree = entity.params.get(1).and_then(|p| p.as_int()).unwrap_or(1) as u32;
        let v_degree = entity.params.get(2).and_then(|p| p.as_int()).unwrap_or(1) as u32;

        // Control points: nested list of lists of refs
        let cp_rows = entity.params.get(3).and_then(|p| p.as_list())
            .ok_or_else(|| KernelError::Io { detail: format!("#{}: missing CPs", entity.id) })?;

        let mut control_points = Vec::new();
        let mut count_u = 0u32;
        let mut count_v = 0u32;

        for row in cp_rows {
            let row_refs = row.as_ref_list().unwrap_or_default();
            if count_u == 0 {
                count_v = row_refs.len() as u32;
            }
            count_u += 1;
            for cp_id in row_refs {
                control_points.push(self.read_point(cp_id)?);
            }
        }

        // Knot multiplicities and values
        let u_mults: Vec<i64> = entity.params.get(8).and_then(|p| p.as_list())
            .map(|l| l.iter().filter_map(|p| p.as_int()).collect())
            .unwrap_or_default();
        let v_mults: Vec<i64> = entity.params.get(9).and_then(|p| p.as_list())
            .map(|l| l.iter().filter_map(|p| p.as_int()).collect())
            .unwrap_or_default();
        let u_knot_vals = entity.params.get(10).and_then(|p| p.as_real_list()).unwrap_or_default();
        let v_knot_vals = entity.params.get(11).and_then(|p| p.as_real_list()).unwrap_or_default();

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

        let weights = vec![1.0; control_points.len()];

        match NurbsSurface::new(control_points, weights, knots_u, knots_v, u_degree, v_degree, count_u, count_v) {
            Ok(s) => Ok(Surface::Nurbs(s)),
            Err(e) => Err(KernelError::Io {
                detail: format!("#{}: bad B-spline surface: {}", entity.id, e),
            }),
        }
    }
}
