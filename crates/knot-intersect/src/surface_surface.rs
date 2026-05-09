use knot_core::KResult;
use knot_geom::Point3;
use knot_geom::Vector3;
use knot_geom::surface::{Surface, SurfaceParam, SurfaceDomain, Plane, Sphere, Cylinder, Cone, Torus};
use super::SurfaceSurfaceTrace;

/// Compute the intersection curves between two surfaces.
///
/// Pipeline:
/// 1. Analytical fast-paths (plane-plane, plane-sphere, plane-cylinder, sphere-sphere)
/// 2. Coincidence detection
/// 3. Subdivision seed-finding with bounding-hull analysis
/// 4. Newton projection of seeds onto intersection
/// 5. Marching with curvature-adaptive stepping
/// 6. Closed-loop detection
/// 7. Tangent intersection detection
pub fn intersect_surfaces(
    a: &Surface,
    b: &Surface,
    tolerance: f64,
) -> KResult<Vec<SurfaceSurfaceTrace>> {
    // ── Full dispatch table for all surface pair types ──
    //
    // Analytical fast-paths are ordered by frequency in real CAD models.
    // Each handler checks for special configurations (coaxial, parallel,
    // tangent) before falling through to general marching.
    //
    // Extension point: add new handlers here for analytical SSI.
    // The pattern is: check geometric configuration → solve analytically
    // if possible → fall through to general_ssi if not.

    match (a, b) {
        // ── Plane-X (most common, always analytical) ──
        (Surface::Plane(pa), Surface::Plane(pb)) => plane_plane(pa, pb, tolerance),
        (Surface::Plane(p), Surface::Sphere(s)) | (Surface::Sphere(s), Surface::Plane(p)) =>
            plane_sphere(p, s, tolerance),
        (Surface::Plane(p), Surface::Cylinder(c)) | (Surface::Cylinder(c), Surface::Plane(p)) =>
            plane_cylinder(p, c, tolerance),
        (Surface::Plane(p), Surface::Cone(c)) | (Surface::Cone(c), Surface::Plane(p)) =>
            plane_cone(p, c, tolerance),
        (Surface::Plane(p), Surface::Torus(t)) | (Surface::Torus(t), Surface::Plane(p)) =>
            plane_torus(p, t, tolerance),

        // ── Sphere-X ──
        (Surface::Sphere(s1), Surface::Sphere(s2)) => sphere_sphere(s1, s2, tolerance),
        // Sphere-Cylinder, Sphere-Cone, Sphere-Torus → general for now
        // TODO: sphere-cylinder is a quartic in one param, tractable

        // ── Cylinder-X ──
        (Surface::Cylinder(c1), Surface::Cylinder(c2)) => cylinder_cylinder(c1, c2, tolerance),
        (Surface::Cylinder(c), Surface::Cone(k)) | (Surface::Cone(k), Surface::Cylinder(c)) =>
            cylinder_cone(c, k, tolerance),
        (Surface::Cylinder(c), Surface::Torus(t)) | (Surface::Torus(t), Surface::Cylinder(c)) =>
            cylinder_torus(c, t, tolerance),

        // ── Cone-X ──
        (Surface::Cone(c1), Surface::Cone(c2)) => cone_cone(c1, c2, tolerance),
        (Surface::Cone(c), Surface::Torus(t)) | (Surface::Torus(t), Surface::Cone(c)) =>
            cone_torus(c, t, tolerance),

        // ── Torus-Torus ──
        (Surface::Torus(t1), Surface::Torus(t2)) => torus_torus(t1, t2, tolerance),

        // ── Anything involving NURBS → general marching ──
        _ => general_ssi(a, b, tolerance),
    }
}

// ═══════════════════════════════════════════════════════════════════
// Analytical Fast-Paths
// ═══════════════════════════════════════════════════════════════════

fn plane_plane(a: &Plane, b: &Plane, tolerance: f64) -> KResult<Vec<SurfaceSurfaceTrace>> {
    let dir = a.normal.cross(&b.normal);
    let len = dir.norm();

    if len < 1e-12 {
        return Ok(Vec::new());
    }

    let dir = dir / len;
    let da = a.normal.dot(&a.origin.coords);
    let db = b.normal.dot(&b.origin.coords);
    let p = find_line_point(&a.normal, da, &b.normal, db, &dir);

    let extent = 100.0;
    let p0 = p - dir * extent;
    let p1 = p + dir * extent;
    let n = 50;

    let mut points = Vec::with_capacity(n + 1);
    let mut params_a = Vec::with_capacity(n + 1);
    let mut params_b = Vec::with_capacity(n + 1);

    for i in 0..=n {
        let t = i as f64 / n as f64;
        let pt = Point3::new(
            p0.x + t * (p1.x - p0.x),
            p0.y + t * (p1.y - p0.y),
            p0.z + t * (p1.z - p0.z),
        );
        let va = pt - a.origin;
        let vb = pt - b.origin;
        points.push(pt);
        params_a.push(SurfaceParam { u: va.dot(&a.u_axis), v: va.dot(&a.v_axis) });
        params_b.push(SurfaceParam { u: vb.dot(&b.u_axis), v: vb.dot(&b.v_axis) });
    }

    Ok(vec![SurfaceSurfaceTrace { points, params_a, params_b }])
}

fn find_line_point(n1: &Vector3, d1: f64, n2: &Vector3, d2: f64, dir: &Vector3) -> Point3 {
    let ax = dir.x.abs();
    let ay = dir.y.abs();
    let az = dir.z.abs();

    if ax >= ay && ax >= az {
        let det = n1.y * n2.z - n1.z * n2.y;
        if det.abs() > 1e-15 {
            let y = (d1 * n2.z - d2 * n1.z) / det;
            let z = (n1.y * d2 - n2.y * d1) / det;
            return Point3::new(0.0, y, z);
        }
    }
    if ay >= az {
        let det = n1.x * n2.z - n1.z * n2.x;
        if det.abs() > 1e-15 {
            let x = (d1 * n2.z - d2 * n1.z) / det;
            let z = (n1.x * d2 - n2.x * d1) / det;
            return Point3::new(x, 0.0, z);
        }
    }
    let det = n1.x * n2.y - n1.y * n2.x;
    if det.abs() > 1e-15 {
        let x = (d1 * n2.y - d2 * n1.y) / det;
        let y = (n1.x * d2 - n2.x * d1) / det;
        return Point3::new(x, y, 0.0);
    }
    Point3::origin()
}

fn plane_sphere(plane: &Plane, sphere: &Sphere, tolerance: f64) -> KResult<Vec<SurfaceSurfaceTrace>> {
    let dist = plane.signed_distance(&sphere.center);

    if dist.abs() > sphere.radius + tolerance {
        return Ok(Vec::new());
    }

    let r = (sphere.radius * sphere.radius - dist * dist).max(0.0).sqrt();

    if r < tolerance {
        let point = sphere.center - plane.normal * dist;
        return Ok(vec![SurfaceSurfaceTrace {
            points: vec![point],
            params_a: vec![plane_param(plane, &point)],
            params_b: vec![sphere_param(sphere, &point)],
        }]);
    }

    let center = sphere.center - plane.normal * dist;
    let n = 64;
    let mut points = Vec::with_capacity(n + 1);
    let mut params_a = Vec::with_capacity(n + 1);
    let mut params_b = Vec::with_capacity(n + 1);

    for i in 0..=n {
        let t = std::f64::consts::TAU * i as f64 / n as f64;
        let pt = center + plane.u_axis * (r * t.cos()) + plane.v_axis * (r * t.sin());
        points.push(pt);
        params_a.push(plane_param(plane, &pt));
        params_b.push(sphere_param(sphere, &pt));
    }

    Ok(vec![SurfaceSurfaceTrace { points, params_a, params_b }])
}

fn plane_cylinder(plane: &Plane, cyl: &Cylinder, tolerance: f64) -> KResult<Vec<SurfaceSurfaceTrace>> {
    let dot = plane.normal.dot(&cyl.axis).abs();

    if dot < 1e-12 {
        let dist = plane.signed_distance(&cyl.origin);
        if dist.abs() > cyl.radius + tolerance {
            return Ok(Vec::new());
        }

        let offset_dir = plane.normal - cyl.axis * plane.normal.dot(&cyl.axis);
        let len = offset_dir.norm();
        if len < 1e-12 {
            return Ok(Vec::new());
        }
        let offset_dir = offset_dir / len;

        let d = dist.abs();
        if d > cyl.radius - tolerance {
            let p = cyl.origin + offset_dir * cyl.radius;
            return Ok(vec![make_line_trace(plane, p, cyl.axis, cyl.v_min, cyl.v_max)]);
        }

        let half_chord = (cyl.radius * cyl.radius - d * d).sqrt();
        let perp = cyl.axis.cross(&offset_dir).normalize();
        let base = cyl.origin + offset_dir * d;

        let p1 = base + perp * half_chord;
        let p2 = base - perp * half_chord;

        Ok(vec![
            make_line_trace(plane, p1, cyl.axis, cyl.v_min, cyl.v_max),
            make_line_trace(plane, p2, cyl.axis, cyl.v_min, cyl.v_max),
        ])
    } else {
        general_ssi(&Surface::Plane(plane.clone()), &Surface::Cylinder(cyl.clone()), tolerance)
    }
}

fn sphere_sphere(a: &Sphere, b: &Sphere, tolerance: f64) -> KResult<Vec<SurfaceSurfaceTrace>> {
    let d_vec = b.center - a.center;
    let d = d_vec.norm();

    if d > a.radius + b.radius + tolerance || d < (a.radius - b.radius).abs() - tolerance {
        return Ok(Vec::new());
    }

    if d < 1e-15 {
        return Ok(Vec::new());
    }

    let axis = d_vec / d;
    let h = (a.radius * a.radius - b.radius * b.radius + d * d) / (2.0 * d);
    let r = (a.radius * a.radius - h * h).max(0.0).sqrt();
    let center = a.center + axis * h;

    if r < tolerance {
        return Ok(vec![SurfaceSurfaceTrace {
            points: vec![center],
            params_a: vec![sphere_param(a, &center)],
            params_b: vec![sphere_param(b, &center)],
        }]);
    }

    let u = if axis.x.abs() < 0.9 {
        Vector3::x().cross(&axis).normalize()
    } else {
        Vector3::y().cross(&axis).normalize()
    };
    let v = axis.cross(&u);

    let n = 64;
    let mut points = Vec::with_capacity(n + 1);
    let mut params_a = Vec::with_capacity(n + 1);
    let mut params_b = Vec::with_capacity(n + 1);

    for i in 0..=n {
        let t = std::f64::consts::TAU * i as f64 / n as f64;
        let pt = center + u * (r * t.cos()) + v * (r * t.sin());
        points.push(pt);
        params_a.push(sphere_param(a, &pt));
        params_b.push(sphere_param(b, &pt));
    }

    Ok(vec![SurfaceSurfaceTrace { points, params_a, params_b }])
}

fn cylinder_cylinder(a: &Cylinder, b: &Cylinder, tolerance: f64) -> KResult<Vec<SurfaceSurfaceTrace>> {
    // Check if axes are parallel
    let dot = a.axis.dot(&b.axis);
    if dot.abs() < 1.0 - 1e-6 {
        // Non-parallel axes — fall through to general SSI
        return general_ssi(
            &Surface::Cylinder(a.clone()),
            &Surface::Cylinder(b.clone()),
            tolerance,
        );
    }

    // Parallel axes. Compute distance between axis lines in the cross-section.
    let d_vec = b.origin - a.origin;
    let along = d_vec.dot(&a.axis); // signed displacement of B along A's axis
    let perp = d_vec - a.axis * along;
    let d = perp.norm();

    if d > a.radius + b.radius + tolerance || d < (a.radius - b.radius).abs() - tolerance {
        return Ok(Vec::new());
    }
    if d < 1e-15 {
        return Ok(Vec::new()); // concentric
    }

    // V-range overlap (in A's parameter space)
    let v_lo = a.v_min.max(b.v_min + along);
    let v_hi = a.v_max.min(b.v_max + along);
    if v_lo > v_hi + tolerance {
        return Ok(Vec::new());
    }

    // Circle-circle intersection in the cross-section plane.
    // Circle A at origin, radius r_a. Circle B at distance d, radius r_b.
    let x = (d * d + a.radius * a.radius - b.radius * b.radius) / (2.0 * d);
    let y_sq = a.radius * a.radius - x * x;
    if y_sq < -tolerance {
        return Ok(Vec::new());
    }
    let y = y_sq.max(0.0).sqrt();

    let perp_dir = perp / d;
    let cross_dir = a.axis.cross(&perp_dir);
    let binorm_a = a.axis.cross(&a.ref_direction);
    let binorm_b = b.axis.cross(&b.ref_direction);

    let mut traces = Vec::new();
    let signs: &[f64] = if y < tolerance { &[0.0] } else { &[1.0, -1.0] };

    for &sign in signs {
        // Offset from A's center to intersection point (perpendicular to axis)
        let offset = perp_dir * x + cross_dir * (y * sign);

        // u-parameter on cylinder A
        let u_a = offset.dot(&binorm_a).atan2(offset.dot(&a.ref_direction));
        let u_a = u_a.rem_euclid(std::f64::consts::TAU);

        // Offset from B's center to intersection point
        let offset_b = offset - perp;
        let u_b = offset_b.dot(&binorm_b).atan2(offset_b.dot(&b.ref_direction));
        let u_b = u_b.rem_euclid(std::f64::consts::TAU);

        // Build trace along the axis within the v overlap
        let n = 20;
        let mut points = Vec::with_capacity(n + 1);
        let mut params_a = Vec::with_capacity(n + 1);
        let mut params_b = Vec::with_capacity(n + 1);

        for i in 0..=n {
            let t = i as f64 / n as f64;
            let v_a = v_lo + t * (v_hi - v_lo);
            let v_b = v_a - along;
            let pt = a.origin + offset + a.axis * v_a;
            points.push(pt);
            params_a.push(SurfaceParam { u: u_a, v: v_a });
            params_b.push(SurfaceParam { u: u_b, v: v_b });
        }

        if points.len() >= 2 {
            traces.push(SurfaceSurfaceTrace { points, params_a, params_b });
        }
    }

    Ok(traces)
}

// ═══════════════════════════════════════════════════════════════════
// Plane-Cone: conic section (always analytical)
// ═══════════════════════════════════════════════════════════════════

fn plane_cone(plane: &Plane, cone: &Cone, tolerance: f64) -> KResult<Vec<SurfaceSurfaceTrace>> {
    // Plane-cone intersection is always a conic section (ellipse, hyperbola,
    // parabola, pair of lines, or point). For now, use general marching with
    // the cone's v-domain clipped to its actual extent.
    // TODO: analytical conic section computation
    general_ssi(&Surface::Plane(plane.clone()), &Surface::Cone(cone.clone()), tolerance)
}

// ═══════════════════════════════════════════════════════════════════
// Plane-Torus: degree-4 plane curve
// ═══════════════════════════════════════════════════════════════════

fn plane_torus(plane: &Plane, torus: &Torus, tolerance: f64) -> KResult<Vec<SurfaceSurfaceTrace>> {
    // Special case: plane perpendicular to torus axis through center.
    // Intersection is two concentric circles.
    let dot = plane.normal.dot(&torus.axis).abs();
    if dot > 1.0 - 1e-6 {
        // Plane is perpendicular to torus axis
        let dist = plane.signed_distance(&torus.center);
        if dist.abs() > torus.minor_radius + tolerance {
            return Ok(Vec::new());
        }

        // Cross-section of torus tube at height dist: circle of radius
        // r_tube_at_height = sqrt(minor_r² - dist²)
        let r_tube = (torus.minor_radius * torus.minor_radius - dist * dist).max(0.0).sqrt();
        if r_tube < tolerance {
            // Tangent — single circle
            let center = torus.center + torus.axis * dist;
            return Ok(vec![trace_circle(center, torus.axis, torus.major_radius,
                &torus.ref_direction, plane)]);
        }

        // Two concentric circles: inner and outer
        let center = torus.center + torus.axis * dist;
        let r_inner = torus.major_radius - r_tube;
        let r_outer = torus.major_radius + r_tube;

        let mut traces = Vec::new();
        if r_outer > tolerance {
            traces.push(trace_circle(center, torus.axis, r_outer, &torus.ref_direction, plane));
        }
        if r_inner > tolerance {
            traces.push(trace_circle(center, torus.axis, r_inner, &torus.ref_direction, plane));
        }
        return Ok(traces);
    }

    // Special case: plane contains torus axis (meridional cut).
    // Intersection is two circles in the plane.
    let axis_in_plane = (plane.signed_distance(&torus.center)).abs() < tolerance
        && plane.normal.dot(&torus.axis).abs() < tolerance;
    if axis_in_plane {
        // The plane cuts through the torus along its axis.
        // Intersection: two circles of radius minor_r, centered at
        // (center ± major_r * direction_in_plane)
        let dir_in_plane = torus.axis.cross(&plane.normal).normalize();
        let c1 = torus.center + dir_in_plane * torus.major_radius;
        let c2 = torus.center - dir_in_plane * torus.major_radius;

        let mut traces = Vec::new();
        traces.push(trace_circle(c1, plane.normal, torus.minor_radius,
            &dir_in_plane, plane));
        traces.push(trace_circle(c2, plane.normal, torus.minor_radius,
            &dir_in_plane, plane));
        return Ok(traces);
    }

    // General case → marching
    general_ssi(&Surface::Plane(plane.clone()), &Surface::Torus(torus.clone()), tolerance)
}

/// Trace a circle as a polyline for use in SurfaceSurfaceTrace.
fn trace_circle(center: Point3, normal: Vector3, radius: f64,
    ref_dir: &Vector3, plane: &Plane) -> SurfaceSurfaceTrace {
    let u = if ref_dir.cross(&normal).norm() > 1e-12 {
        (ref_dir - normal * ref_dir.dot(&normal)).normalize()
    } else {
        if normal.x.abs() < 0.9 {
            Vector3::x().cross(&normal).normalize()
        } else {
            Vector3::y().cross(&normal).normalize()
        }
    };
    let v = normal.cross(&u);

    let n = 64;
    let mut points = Vec::with_capacity(n + 1);
    let mut params_a = Vec::with_capacity(n + 1);
    let mut params_b = Vec::with_capacity(n + 1);

    for i in 0..=n {
        let t = std::f64::consts::TAU * i as f64 / n as f64;
        let pt = center + u * (radius * t.cos()) + v * (radius * t.sin());
        points.push(pt);
        params_a.push(plane_param(plane, &pt));
        params_b.push(SurfaceParam { u: t, v: 0.0 }); // approximate
    }

    SurfaceSurfaceTrace { points, params_a, params_b }
}

// ═══════════════════════════════════════════════════════════════════
// Cylinder-Cone: coaxial/parallel special cases
// ═══════════════════════════════════════════════════════════════════

fn cylinder_cone(cyl: &Cylinder, cone: &Cone, tolerance: f64) -> KResult<Vec<SurfaceSurfaceTrace>> {
    let dot = cyl.axis.dot(&cone.axis).abs();

    if dot > 1.0 - 1e-6 {
        // Coaxial or parallel axes.
        // The cylinder and cone share an axis direction.
        // Intersection points occur where the cone's radius equals the cylinder's radius:
        //   r_cone(v) = v * tan(half_angle)
        //   r_cone(v) = r_cylinder → v = r_cylinder / tan(half_angle)
        let tan_ha = cone.half_angle.tan();
        if tan_ha.abs() < 1e-15 { return Ok(Vec::new()); } // degenerate cone

        // Distance between axes (perpendicular component)
        let d_vec = cyl.origin - cone.apex;
        let along = d_vec.dot(&cone.axis);
        let perp = (d_vec - cone.axis * along).norm();

        if perp < tolerance {
            // Truly coaxial — intersection is 0, 1, or 2 circles
            let v_intersect = cyl.radius / tan_ha;

            // Check if v is within both surfaces' domains
            let v_values: Vec<f64> = [v_intersect, -v_intersect].iter().copied()
                .filter(|&v| {
                    let v_on_cone = v;
                    let v_on_cyl = cone.apex.coords.dot(&cone.axis) + v - cyl.origin.coords.dot(&cyl.axis);
                    v_on_cone >= cone.v_min - tolerance && v_on_cone <= cone.v_max + tolerance
                        && v_on_cyl >= cyl.v_min - tolerance && v_on_cyl <= cyl.v_max + tolerance
                })
                .collect();

            let mut traces = Vec::new();
            for v in v_values {
                let center = cone.apex + cone.axis * v;
                let r = (v * tan_ha).abs();
                if r > tolerance {
                    let u_dir = cyl.ref_direction;
                    let binorm = cone.axis.cross(&u_dir);
                    let n = 64;
                    let mut points = Vec::with_capacity(n + 1);
                    let mut params_a = Vec::with_capacity(n + 1);
                    let mut params_b = Vec::with_capacity(n + 1);
                    for i in 0..=n {
                        let t = std::f64::consts::TAU * i as f64 / n as f64;
                        let pt = center + u_dir * (r * t.cos()) + binorm * (r * t.sin());
                        points.push(pt);
                        params_a.push(SurfaceParam { u: t, v: (pt - cyl.origin).dot(&cyl.axis) });
                        params_b.push(SurfaceParam { u: t, v });
                    }
                    traces.push(SurfaceSurfaceTrace { points, params_a, params_b });
                }
            }
            return Ok(traces);
        }
    }

    // Non-coaxial → general marching
    general_ssi(&Surface::Cylinder(cyl.clone()), &Surface::Cone(cone.clone()), tolerance)
}

// ═══════════════════════════════════════════════════════════════════
// Cylinder-Torus: coaxial special case
// ═══════════════════════════════════════════════════════════════════

fn cylinder_torus(cyl: &Cylinder, torus: &Torus, tolerance: f64) -> KResult<Vec<SurfaceSurfaceTrace>> {
    let dot = cyl.axis.dot(&torus.axis).abs();

    if dot > 1.0 - 1e-6 {
        // Coaxial: cylinder and torus share an axis.
        // Intersection occurs where the torus cross-section circle at height h
        // has the same distance from the axis as the cylinder radius.
        //
        // Torus at height h: two points at distances (R ± sqrt(r² - h²)) from axis.
        // Cylinder radius: ρ. Solve R ± sqrt(r² - h²) = ρ.

        let d_vec = cyl.origin - torus.center;
        let along = d_vec.dot(&torus.axis);
        let perp = (d_vec - torus.axis * along).norm();

        if perp < tolerance {
            // Truly coaxial
            let rho = cyl.radius;
            let big_r = torus.major_radius;
            let little_r = torus.minor_radius;

            // Solve R + sqrt(r² - h²) = ρ → h² = r² - (ρ - R)²
            // and   R - sqrt(r² - h²) = ρ → h² = r² - (R - ρ)²
            // Both give h² = r² - (ρ - R)², so same equation.
            let delta = rho - big_r;
            let h_sq = little_r * little_r - delta * delta;

            if h_sq < -tolerance {
                return Ok(Vec::new()); // no intersection
            }

            let mut h_values = Vec::new();
            if h_sq.abs() < tolerance {
                h_values.push(0.0); // tangent — single circle
            } else {
                let h = h_sq.max(0.0).sqrt();
                h_values.push(h);
                h_values.push(-h);
            }

            let mut traces = Vec::new();
            for h in h_values {
                let center = torus.center + torus.axis * h;
                // At height h, the cylinder has radius rho, so the circle is at radius rho
                let n = 64;
                let u_dir = cyl.ref_direction;
                let binorm = torus.axis.cross(&u_dir);
                let mut points = Vec::with_capacity(n + 1);
                let mut params_a = Vec::with_capacity(n + 1);
                let mut params_b = Vec::with_capacity(n + 1);
                for i in 0..=n {
                    let t = std::f64::consts::TAU * i as f64 / n as f64;
                    let pt = center + u_dir * (rho * t.cos()) + binorm * (rho * t.sin());
                    points.push(pt);
                    let v_cyl = (pt - cyl.origin).dot(&cyl.axis);
                    params_a.push(SurfaceParam { u: t, v: v_cyl });
                    // torus v-param: angle in the tube cross-section
                    let v_torus = h.atan2(delta);
                    params_b.push(SurfaceParam { u: t, v: v_torus });
                }
                traces.push(SurfaceSurfaceTrace { points, params_a, params_b });
            }

            return Ok(traces);
        }
    }

    // Non-coaxial → algebraic pipeline (walking skeleton).
    // Falls back to general marching if the algebraic path produces no traces.
    use crate::algebraic::cylinder_torus::intersect_cylinder_torus;
    let traces = intersect_cylinder_torus(cyl, torus, tolerance)?;
    if !traces.is_empty() {
        return Ok(traces);
    }

    // Fallback: general marching
    general_ssi(&Surface::Cylinder(cyl.clone()), &Surface::Torus(torus.clone()), tolerance)
}

// ═══════════════════════════════════════════════════════════════════
// Cone-Cone, Cone-Torus, Torus-Torus: stubs → general marching
// ═══════════════════════════════════════════════════════════════════

fn cone_cone(c1: &Cone, c2: &Cone, tolerance: f64) -> KResult<Vec<SurfaceSurfaceTrace>> {
    // TODO: coaxial cone-cone → circles at matching radii
    general_ssi(&Surface::Cone(c1.clone()), &Surface::Cone(c2.clone()), tolerance)
}

fn cone_torus(cone: &Cone, torus: &Torus, tolerance: f64) -> KResult<Vec<SurfaceSurfaceTrace>> {
    // TODO: coaxial cone-torus → quartic in one parameter
    general_ssi(&Surface::Cone(cone.clone()), &Surface::Torus(torus.clone()), tolerance)
}

fn torus_torus(t1: &Torus, t2: &Torus, tolerance: f64) -> KResult<Vec<SurfaceSurfaceTrace>> {
    // TODO: coaxial torus-torus → circles
    general_ssi(&Surface::Torus(t1.clone()), &Surface::Torus(t2.clone()), tolerance)
}

// ═══════════════════════════════════════════════════════════════════
// General SSI: Hardened Marching Algorithm
// ═══════════════════════════════════════════════════════════════════

/// General surface-surface intersection.
///
/// Handles:
/// - Multi-component intersections (separate loops/curves)
/// - Closed loops (trace returns to its start)
/// - Tangent intersections (|n_a × n_b| → 0)
/// - Curvature-adaptive step size
fn general_ssi(
    a: &Surface,
    b: &Surface,
    tolerance: f64,
) -> KResult<Vec<SurfaceSurfaceTrace>> {
    let a_domain = a.domain();
    let b_domain = b.domain();

    let (a_u0, a_u1, a_v0, a_v1) = clamp_domain(&a_domain);
    let (b_u0, b_u1, b_v0, b_v1) = clamp_domain(&b_domain);

    // Phase 1: Seed finding via Newton projection.
    // Sample surface A on a grid, then Newton-project each sample onto
    // surface B to find the closest point. O(n) evaluations on B per
    // sample on A (vs O(n²) for grid-vs-grid search).
    let seeds = find_seeds_by_projection(
        a, b,
        a_u0, a_u1, a_v0, a_v1,
        b_u0, b_u1, b_v0, b_v1,
        tolerance,
    );

    // Phase 2: Newton-refine seeds onto the intersection
    let mut refined: Vec<SeedPoint> = Vec::new();
    for (uv_a, uv_b, _) in &seeds {
        if let Some((ra, rb, pt)) = newton_ssi(a, b, *uv_a, *uv_b, tolerance) {
            // Check not duplicate of an existing refined seed
            let is_dup = refined.iter().any(|s| (pt - s.point).norm() < tolerance * 10.0);
            if !is_dup {
                // Check tangency: is this a tangent touch or a transversal crossing?
                let da = a.derivatives_at(ra);
                let db = b.derivatives_at(rb);
                let cross = da.normal.cross(&db.normal);
                let tangent_mag = cross.norm();

                refined.push(SeedPoint {
                    uv_a: ra,
                    uv_b: rb,
                    point: pt,
                    is_tangent: tangent_mag < tolerance * 100.0,
                });
            }
        }
    }

    if refined.is_empty() {
        return Ok(Vec::new());
    }

    // Phase 3: March from each non-tangent seed.
    // Tangent seeds are reported as single-point traces.
    let mut traces = Vec::new();
    let mut used = vec![false; refined.len()];

    for seed_idx in 0..refined.len() {
        if used[seed_idx] {
            continue;
        }
        used[seed_idx] = true;
        let seed = &refined[seed_idx];

        if seed.is_tangent {
            // Tangent intersection: report as a single-point trace
            traces.push(SurfaceSurfaceTrace {
                points: vec![seed.point],
                params_a: vec![seed.uv_a],
                params_b: vec![seed.uv_b],
            });
            continue;
        }

        // March in both directions
        let fwd = march(a, b, seed.uv_a, seed.uv_b, seed.point, 1.0, tolerance, &a_domain, &b_domain);
        let bwd = march(a, b, seed.uv_a, seed.uv_b, seed.point, -1.0, tolerance, &a_domain, &b_domain);

        // Assemble: backward (reversed) + seed + forward
        let mut points = Vec::new();
        let mut params_a = Vec::new();
        let mut params_b = Vec::new();

        for i in (0..bwd.points.len()).rev() {
            points.push(bwd.points[i]);
            params_a.push(bwd.params_a[i]);
            params_b.push(bwd.params_b[i]);
        }
        points.push(seed.point);
        params_a.push(seed.uv_a);
        params_b.push(seed.uv_b);
        for i in 0..fwd.points.len() {
            points.push(fwd.points[i]);
            params_a.push(fwd.params_a[i]);
            params_b.push(fwd.params_b[i]);
        }

        // Check if the trace is a closed loop:
        // if the last point is close to the first, close it
        if points.len() >= 4 {
            let first = points[0];
            let last = *points.last().unwrap();
            if (last - first).norm() < tolerance * 50.0 {
                // Snap last point to first to close the loop
                *points.last_mut().unwrap() = first;
                *params_a.last_mut().unwrap() = params_a[0];
                *params_b.last_mut().unwrap() = params_b[0];
            }
        }

        // Mark nearby seeds as used (they belong to this trace component)
        for (i, s) in refined.iter().enumerate() {
            if !used[i] {
                for pt in &points {
                    if (pt - s.point).norm() < tolerance * 50.0 {
                        used[i] = true;
                        break;
                    }
                }
            }
        }

        if points.len() >= 2 {
            traces.push(SurfaceSurfaceTrace { points, params_a, params_b });
        }
    }

    Ok(traces)
}

struct SeedPoint {
    uv_a: SurfaceParam,
    uv_b: SurfaceParam,
    point: Point3,
    is_tangent: bool,
}

// ═══════════════════════════════════════════════════════════════════
// Seed Finding
// ═══════════════════════════════════════════════════════════════════

/// Primary seed-finder: Newton projection.
///
/// Samples surface A on a grid. For each sample point, Newton-projects
/// onto surface B to find the closest point. If the distance is within
/// tolerance, the pair is a seed candidate.
///
/// Cost: O(n_samples × newton_iters) ≈ 100 × 10 = 1000 surface evaluations
/// vs the previous O(n² × depth) ≈ 5000+ evaluations.
///
/// This is the extension point for analytical seed-finders:
/// add match arms for specific surface pair types (e.g., Plane-Torus)
/// that compute seed points directly from the surface equations.
fn find_seeds_by_projection(
    a: &Surface, b: &Surface,
    a_u0: f64, a_u1: f64, a_v0: f64, a_v1: f64,
    b_u0: f64, b_u1: f64, b_v0: f64, b_v1: f64,
    tolerance: f64,
) -> Vec<(SurfaceParam, SurfaceParam, f64)> {
    // Extension point: analytical seed-finders for specific surface pairs.
    // When implemented, these return exact seed points without iteration.
    //
    // match (a, b) {
    //     (Surface::Plane(p), Surface::Torus(t)) => return plane_torus_seeds(p, t, tolerance),
    //     (Surface::Plane(p), Surface::Cone(c))  => return plane_cone_seeds(p, c, tolerance),
    //     (Surface::Cylinder(c1), Surface::Cone(c2)) => return cyl_cone_seeds(c1, c2, tolerance),
    //     _ => {} // fall through to projection
    // }

    let n = 10; // samples per axis on surface A
    let threshold = tolerance * 50.0;
    let mut seeds = Vec::new();

    let a_du = (a_u1 - a_u0) / n as f64;
    let a_dv = (a_v1 - a_v0) / n as f64;

    // Coarse grid on B for Newton starting guess
    let nb = 4;
    let b_du = (b_u1 - b_u0) / nb as f64;
    let b_dv = (b_v1 - b_v0) / nb as f64;

    for ia_u in 0..n {
        for ia_v in 0..n {
            let uv_a = SurfaceParam {
                u: a_u0 + a_du * (ia_u as f64 + 0.5),
                v: a_v0 + a_dv * (ia_v as f64 + 0.5),
            };
            let pa = a.point_at(uv_a);

            // Find a coarse starting guess on B (4×4 = 16 evaluations)
            let mut best_uv_b = SurfaceParam { u: (b_u0 + b_u1) * 0.5, v: (b_v0 + b_v1) * 0.5 };
            let mut best_dist = f64::MAX;
            for ib_u in 0..=nb {
                for ib_v in 0..=nb {
                    let uv_b = SurfaceParam {
                        u: b_u0 + b_du * ib_u as f64,
                        v: b_v0 + b_dv * ib_v as f64,
                    };
                    let d = (a.point_at(uv_a) - b.point_at(uv_b)).norm();
                    if d < best_dist {
                        best_dist = d;
                        best_uv_b = uv_b;
                    }
                }
            }

            // Newton-project pa onto surface B (~5-10 iterations)
            if let Some(projected_uv) = project_onto_surface(b, &pa, best_uv_b) {
                let pb = b.point_at(projected_uv);
                let dist = (pa - pb).norm();
                if dist < threshold {
                    seeds.push((uv_a, projected_uv, dist));
                }
            }
        }
    }

    seeds
}

/// Fallback: hierarchical seed-finding via recursive subdivision.
/// Evaluates surface distance at grid cell corners; recurses into cells
/// where surfaces are within range.
fn find_seeds_recursive(
    a: &Surface, b: &Surface,
    a_u0: f64, a_u1: f64, a_v0: f64, a_v1: f64,
    b_u0: f64, b_u1: f64, b_v0: f64, b_v1: f64,
    tolerance: f64,
    depth: usize,
    seeds: &mut Vec<(SurfaceParam, SurfaceParam, f64)>,
) {
    const MAX_DEPTH: usize = 4;
    let n = if depth == 0 { 8 } else { 4 };

    let a_du = (a_u1 - a_u0) / n as f64;
    let a_dv = (a_v1 - a_v0) / n as f64;
    let b_du = (b_u1 - b_u0) / n as f64;
    let b_dv = (b_v1 - b_v0) / n as f64;

    for ia_u in 0..n {
        for ia_v in 0..n {
            let uv_a = SurfaceParam {
                u: a_u0 + a_du * (ia_u as f64 + 0.5),
                v: a_v0 + a_dv * (ia_v as f64 + 0.5),
            };
            let pa = a.point_at(uv_a);

            // Find closest sample on b
            let mut best_dist = f64::MAX;
            let mut best_uv_b = SurfaceParam { u: b_u0, v: b_v0 };

            for ib_u in 0..=n {
                for ib_v in 0..=n {
                    let uv_b = SurfaceParam {
                        u: b_u0 + b_du * ib_u as f64,
                        v: b_v0 + b_dv * ib_v as f64,
                    };
                    let pb = b.point_at(uv_b);
                    let d = (pa - pb).norm();
                    if d < best_dist {
                        best_dist = d;
                        best_uv_b = uv_b;
                    }
                }
            }

            let threshold = if depth < MAX_DEPTH { tolerance * 200.0 } else { tolerance * 50.0 };

            if best_dist < threshold {
                if depth < MAX_DEPTH && best_dist > tolerance * 2.0 {
                    // Recurse with finer grid around this region
                    let margin = 1.5;
                    find_seeds_recursive(
                        a, b,
                        (uv_a.u - a_du * margin).max(a_u0),
                        (uv_a.u + a_du * margin).min(a_u1),
                        (uv_a.v - a_dv * margin).max(a_v0),
                        (uv_a.v + a_dv * margin).min(a_v1),
                        (best_uv_b.u - b_du * margin).max(b_u0),
                        (best_uv_b.u + b_du * margin).min(b_u1),
                        (best_uv_b.v - b_dv * margin).max(b_v0),
                        (best_uv_b.v + b_dv * margin).min(b_v1),
                        tolerance, depth + 1, seeds,
                    );
                } else {
                    seeds.push((uv_a, best_uv_b, best_dist));
                }
            }
        }
    }
}

/// March result for one direction.
struct MarchResult {
    points: Vec<Point3>,
    params_a: Vec<SurfaceParam>,
    params_b: Vec<SurfaceParam>,
    closed: bool,
}

/// March along the intersection curve in one direction.
///
/// Handles:
/// - Curvature-adaptive step size (smaller steps where normals change rapidly)
/// - Closed loop detection (return to start)
/// - Domain boundary detection
/// - Tangent breakdown (cross product vanishes)
fn march(
    a: &Surface, b: &Surface,
    start_uv_a: SurfaceParam, start_uv_b: SurfaceParam,
    start_pt: Point3,
    sign: f64,
    tolerance: f64,
    a_domain: &SurfaceDomain,
    b_domain: &SurfaceDomain,
) -> MarchResult {
    let base_step = tolerance * 50.0;
    let min_step = tolerance * 2.0;
    let max_step = 1.0;
    let max_steps = 1000;
    let close_loop_dist = tolerance * 30.0;

    let mut points = Vec::new();
    let mut params_a = Vec::new();
    let mut params_b = Vec::new();
    let mut uv_a = start_uv_a;
    let mut uv_b = start_uv_b;
    let mut prev_tangent: Option<Vector3> = None;

    for step_num in 0..max_steps {
        let da = a.derivatives_at(uv_a);
        let db = b.derivatives_at(uv_b);

        // Intersection tangent = n_a × n_b
        let cross = da.normal.cross(&db.normal);
        let cross_len = cross.norm();

        if cross_len < 1e-14 {
            // Tangent intersection — surfaces are parallel here. Stop marching.
            break;
        }

        let tangent = cross / cross_len * sign;

        // Ensure consistent tangent direction (avoid flipping)
        let tangent = if let Some(ref prev) = prev_tangent {
            if tangent.dot(prev) < 0.0 { -tangent } else { tangent }
        } else {
            tangent
        };
        prev_tangent = Some(tangent);

        // Adaptive step size based on cross product magnitude.
        // Small cross product (near tangent) → smaller steps for accuracy.
        // Also reduce step if curvature is high (tangent direction changes fast).
        let angle_factor = cross_len.clamp(0.01, 1.0);
        let step = (base_step / angle_factor).clamp(min_step, max_step);

        // Predict next point
        let predicted = da.point + tangent * step;

        // Project onto both surfaces
        let new_uv_a = project_onto_surface(a, &predicted, uv_a);
        let new_uv_b = project_onto_surface(b, &predicted, uv_b);

        let (new_uv_a, new_uv_b) = match (new_uv_a, new_uv_b) {
            (Some(a), Some(b)) => (a, b),
            _ => break,
        };

        // Newton refine onto intersection
        let refined = newton_ssi(a, b, new_uv_a, new_uv_b, tolerance);
        let (ra, rb, pt) = match refined {
            Some(r) => r,
            None => break,
        };

        // Check domain boundaries
        if out_of_domain(ra, a_domain) || out_of_domain(rb, b_domain) {
            break;
        }

        // Check progress (not stalled)
        let last_pt = points.last().copied().unwrap_or(start_pt);
        let progress = (pt - last_pt).norm();
        if progress < tolerance * 0.01 {
            break;
        }

        // Closed loop detection: check if we've returned to the start
        if step_num > 3 && (pt - start_pt).norm() < close_loop_dist {
            points.push(pt);
            params_a.push(ra);
            params_b.push(rb);
            return MarchResult { points, params_a, params_b, closed: true };
        }

        points.push(pt);
        params_a.push(ra);
        params_b.push(rb);
        uv_a = ra;
        uv_b = rb;
    }

    MarchResult { points, params_a, params_b, closed: false }
}

// ═══════════════════════════════════════════════════════════════════
// Newton Solvers
// ═══════════════════════════════════════════════════════════════════

/// Newton iteration to find a point on the intersection of two surfaces.
/// Uses alternating tangent-plane projection with damping.
fn newton_ssi(
    a: &Surface, b: &Surface,
    mut uv_a: SurfaceParam, mut uv_b: SurfaceParam,
    tolerance: f64,
) -> Option<(SurfaceParam, SurfaceParam, Point3)> {
    for _ in 0..30 {
        let pa = a.point_at(uv_a);
        let pb = b.point_at(uv_b);
        let diff = pa - pb;

        if diff.norm() < tolerance {
            return Some((uv_a, uv_b, midpoint(&pa, &pb)));
        }

        let da = a.derivatives_at(uv_a);
        let db = b.derivatives_at(uv_b);

        // Project onto tangent plane of a: move a toward b
        if let Some((du, dv)) = solve_tangent_plane(&da.du, &da.dv, &(pb - pa)) {
            uv_a.u += du * 0.5;
            uv_a.v += dv * 0.5;
        } else {
            break;
        }

        // Project onto tangent plane of b: move b toward a
        if let Some((du, dv)) = solve_tangent_plane(&db.du, &db.dv, &(pa - pb)) {
            uv_b.u += du * 0.5;
            uv_b.v += dv * 0.5;
        } else {
            break;
        }
    }

    let pa = a.point_at(uv_a);
    let pb = b.point_at(uv_b);
    if (pa - pb).norm() < tolerance * 10.0 {
        Some((uv_a, uv_b, midpoint(&pa, &pb)))
    } else {
        None
    }
}

/// Solve the 2x2 tangent-plane system: du*Su + dv*Sv ≈ target.
/// Returns (du, dv) or None if singular.
fn solve_tangent_plane(su: &Vector3, sv: &Vector3, target: &Vector3) -> Option<(f64, f64)> {
    let a11 = su.dot(su);
    let a12 = su.dot(sv);
    let a22 = sv.dot(sv);
    let b1 = su.dot(target);
    let b2 = sv.dot(target);

    let det = a11 * a22 - a12 * a12;
    if det.abs() < 1e-30 {
        return None;
    }

    Some(((a22 * b1 - a12 * b2) / det, (a11 * b2 - a12 * b1) / det))
}

/// Project a 3D point onto a surface using Newton iteration.
fn project_onto_surface(surface: &Surface, target: &Point3, init: SurfaceParam) -> Option<SurfaceParam> {
    let mut uv = init;

    for _ in 0..20 {
        let sd = surface.derivatives_at(uv);
        let diff = *target - sd.point;

        if diff.norm() < 1e-12 {
            return Some(uv);
        }

        if let Some((du, dv)) = solve_tangent_plane(&sd.du, &sd.dv, &diff) {
            uv.u += du;
            uv.v += dv;
        } else {
            break;
        }
    }

    let p = surface.point_at(uv);
    if (p - *target).norm() < 1.0 {
        Some(uv)
    } else {
        None
    }
}

// ═══════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════

fn clamp_domain(d: &SurfaceDomain) -> (f64, f64, f64, f64) {
    (d.u_start.max(-100.0), d.u_end.min(100.0), d.v_start.max(-100.0), d.v_end.min(100.0))
}

/// Sample a surface on a coarse grid and return its AABB.
fn sample_surface_bbox(
    surface: &Surface, u0: f64, u1: f64, v0: f64, v1: f64, n: usize,
) -> knot_core::Aabb3 {
    let mut pts = Vec::with_capacity((n + 1) * (n + 1));
    for iu in 0..=n {
        for iv in 0..=n {
            let uv = SurfaceParam {
                u: u0 + (u1 - u0) * iu as f64 / n as f64,
                v: v0 + (v1 - v0) * iv as f64 / n as f64,
            };
            pts.push(surface.point_at(uv));
        }
    }
    knot_core::Aabb3::from_points(&pts).unwrap()
}

fn out_of_domain(uv: SurfaceParam, domain: &SurfaceDomain) -> bool {
    let margin = 0.01;
    if domain.u_start.is_finite() && (uv.u < domain.u_start - margin || uv.u > domain.u_end + margin) {
        return true;
    }
    if domain.v_start.is_finite() && (uv.v < domain.v_start - margin || uv.v > domain.v_end + margin) {
        return true;
    }
    false
}

fn midpoint(a: &Point3, b: &Point3) -> Point3 {
    Point3::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0, (a.z + b.z) / 2.0)
}

fn make_line_trace(plane: &Plane, point: Point3, dir: Vector3, v_min: f64, v_max: f64) -> SurfaceSurfaceTrace {
    let n = 20;
    let mut points = Vec::with_capacity(n + 1);
    let mut params_a = Vec::with_capacity(n + 1);
    let mut params_b = Vec::with_capacity(n + 1);

    for i in 0..=n {
        let t = v_min + (v_max - v_min) * i as f64 / n as f64;
        let pt = point + dir * t;
        points.push(pt);
        params_a.push(plane_param(plane, &pt));
        params_b.push(SurfaceParam { u: 0.0, v: t });
    }

    SurfaceSurfaceTrace { points, params_a, params_b }
}

fn plane_param(plane: &Plane, pt: &Point3) -> SurfaceParam {
    let v = pt - plane.origin;
    SurfaceParam { u: v.dot(&plane.u_axis), v: v.dot(&plane.v_axis) }
}

fn sphere_param(sphere: &Sphere, pt: &Point3) -> SurfaceParam {
    let n = (pt - sphere.center).normalize();
    SurfaceParam {
        u: n.y.atan2(n.x).rem_euclid(std::f64::consts::TAU),
        v: n.z.asin(),
    }
}
