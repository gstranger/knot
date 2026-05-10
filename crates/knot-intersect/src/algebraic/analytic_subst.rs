//! Substitute a parametric surface (given as homogeneous-coordinate
//! polynomials X(u, v), Y(u, v), Z(u, v), W(u, v)) into the implicit
//! equation of an analytic surface, producing a bivariate polynomial
//! G(u, v) whose zero set is the intersection in the parametric
//! surface's domain.
//!
//! This is the algebraic-SSI heart of NURBS-vs-analytic intersection:
//! the analytic side already has a closed-form implicit, so no new
//! implicitization is needed. We just plug the parametric form into
//! it and reduce to a single bivariate polynomial.
//!
//! All arithmetic is in `BiPoly` (sparse exact rational coefficients).
//! For each surface type we homogenize the implicit so dividing by the
//! homogeneous denominator W^d (where d is the implicit's degree) is
//! avoided — instead we multiply everything through by W^d up front.
//! This keeps the result a polynomial.

use malachite_q::Rational;
use knot_geom::Point3;
use knot_geom::Vector3;
use knot_geom::surface::{Plane, Sphere, Cylinder, Cone, Torus};
use super::poly::BiPoly;

/// Homogeneous-coordinate input for a parametric surface. `x/w, y/w,
/// z/w` is the cartesian point at any (u, v) where `w(u, v) ≠ 0`.
pub struct HomogeneousSurface<'a> {
    pub x: &'a BiPoly,
    pub y: &'a BiPoly,
    pub z: &'a BiPoly,
    pub w: &'a BiPoly,
}

/// Substitute into a plane: `n · (P - origin) = 0`, homogenized as
/// `n_x · X + n_y · Y + n_z · Z - (n · origin) · W = 0`.
///
/// Result has the same bidegree as the input (no degree multiplication).
pub fn substitute_into_plane(s: HomogeneousSurface, plane: &Plane) -> BiPoly {
    let n = plane.normal;
    let o = plane.origin;
    let n_dot_o = n.x * o.x + n.y * o.y + n.z * o.z;

    let nx = s.x.scale(&rat(n.x));
    let ny = s.y.scale(&rat(n.y));
    let nz = s.z.scale(&rat(n.z));
    let bias = s.w.scale(&rat(-n_dot_o));

    nx.add(&ny).add(&nz).add(&bias)
}

/// Substitute into a sphere: `(P - center)² = r²`, homogenized as
/// `(X - cx·W)² + (Y - cy·W)² + (Z - cz·W)² - r²·W² = 0`.
///
/// Bidegree of result is 2× input bidegree.
pub fn substitute_into_sphere(s: HomogeneousSurface, sphere: &Sphere) -> BiPoly {
    let c = sphere.center;
    let r2 = sphere.radius * sphere.radius;

    let dx = s.x.sub(&s.w.scale(&rat(c.x)));
    let dy = s.y.sub(&s.w.scale(&rat(c.y)));
    let dz = s.z.sub(&s.w.scale(&rat(c.z)));
    let w2 = s.w.mul(s.w);

    let d2 = dx.mul(&dx).add(&dy.mul(&dy)).add(&dz.mul(&dz));
    let r2_w2 = w2.scale(&rat(r2));
    d2.sub(&r2_w2)
}

/// Substitute into a cylinder: in the cylinder's local frame (origin
/// at `cyl.origin`, z-axis = `cyl.axis`), the implicit is
/// `x² + y² = r²` (independent of z). Homogenized to
/// `(X' - 0)² + (Y' - 0)² - r²·W² = 0` where (X', Y', Z') are the
/// inputs transformed into the cylinder frame.
///
/// Bidegree of result is 2× input bidegree.
pub fn substitute_into_cylinder(s: HomogeneousSurface, cyl: &Cylinder) -> BiPoly {
    let frame = local_frame(cyl.origin, cyl.axis, cyl.ref_direction);
    let (lx, ly, _lz) = transform_xyz_to_local(&s, &frame);
    let w2 = s.w.mul(s.w);
    let r2 = cyl.radius * cyl.radius;
    let r2_w2 = w2.scale(&rat(r2));
    lx.mul(&lx).add(&ly.mul(&ly)).sub(&r2_w2)
}

/// Substitute into a cone (double cone, both halves): the implicit in
/// the cone's local frame is
/// `(P · axis)² · sin²(α) - (|P|² - (P · axis)²) · cos²(α) = 0`,
/// equivalently `(P · axis)² = (|P|²) · cos²(α)` after rearranging
/// (using sin² + cos² = 1). The homogenized form on
/// `P_local = local_xyz / W` is
/// `(local_z)² · cos²(α) - ((local_x)² + (local_y)²) · sin²(α)·W²·...`
///
/// Cleaner derivation: for a point at axial distance `v` from the
/// apex, the radial distance must equal `|v| · tan(α)`. So
/// `radial² = v² · tan²(α)`, i.e.
/// `(local_x² + local_y²) · cos²(α) = local_z² · sin²(α)`,
/// homogenized to
/// `(local_x² + local_y²) · cos²(α) - local_z² · sin²(α) = 0`
/// where local_x, local_y, local_z already absorbed W via the frame
/// transform (which is linear in W).
///
/// Bidegree of result is 2× input bidegree.
pub fn substitute_into_cone(s: HomogeneousSurface, cone: &Cone) -> BiPoly {
    let frame = local_frame(cone.apex, cone.axis, cone.ref_direction);
    let (lx, ly, lz) = transform_xyz_to_local(&s, &frame);
    let cos_a = cone.half_angle.cos();
    let sin_a = cone.half_angle.sin();
    let cos2 = cos_a * cos_a;
    let sin2 = sin_a * sin_a;

    let radial2 = lx.mul(&lx).add(&ly.mul(&ly));
    let axial2 = lz.mul(&lz);

    radial2.scale(&rat(cos2)).sub(&axial2.scale(&rat(sin2)))
}

/// Substitute into a torus: in the torus's local frame, the implicit
/// is `(|P|² + R² - r²)² - 4R²(P_x² + P_y²) = 0` where R is major
/// radius and r is minor. Homogenizing and using local coords:
///
///   ((LX² + LY² + LZ²) + (R² - r²) · W²)² - 4 R² (LX² + LY²) · W² = 0
///
/// Bidegree of result is 4× input bidegree.
pub fn substitute_into_torus(s: HomogeneousSurface, torus: &Torus) -> BiPoly {
    let frame = local_frame(torus.center, torus.axis, torus.ref_direction);
    let (lx, ly, lz) = transform_xyz_to_local(&s, &frame);
    let big_r = rat(torus.major_radius);
    let little_r = rat(torus.minor_radius);

    let lx2 = lx.mul(&lx);
    let ly2 = ly.mul(&ly);
    let lz2 = lz.mul(&lz);
    let w2 = s.w.mul(s.w);

    let sum_sq = lx2.add(&ly2).add(&lz2);
    let r_diff = &big_r * &big_r - &little_r * &little_r;
    let r_diff_poly = BiPoly::constant(r_diff);
    let inner = sum_sq.add(&r_diff_poly.mul(&w2));
    let inner_sq = inner.mul(&inner);

    let four_r2 = BiPoly::constant(&big_r * &big_r * Rational::from(4));
    let radial2_w2 = lx2.add(&ly2).mul(&w2);
    let rhs = four_r2.mul(&radial2_w2);

    inner_sq.sub(&rhs)
}

// ─────────────────────────────────────────────────────────────────────
// Local-frame transform.
//
// Each analytic surface lives in its own local frame: the implicit
// formulas above all assume the surface is at the origin with axis
// along z (or another canonical axis). To apply them to inputs in
// world coords we transform (X, Y, Z, W) through the frame as
// linear-in-(X, Y, Z, W) operations: the result is still polynomial.
//
// World point P_w corresponds to local P_l = R · (P_w - origin) where
// R is the orthonormal change-of-basis matrix [u | v | n] transposed.
// In homogeneous coords with X_w = X · 1 and origin O, the local X is
//
//   LX = u · (X_w - O)
//      = u_x · X - u_x · O_x · W + u_y · Y - u_y · O_y · W + ...
//
// i.e. a linear combination of (X, Y, Z, W) with rational coefficients.
// ─────────────────────────────────────────────────────────────────────

struct LocalFrame {
    origin: Point3,
    u: Vector3,
    v: Vector3,
    w: Vector3, // axis (third local basis vector)
}

fn local_frame(origin: Point3, axis: Vector3, ref_dir: Vector3) -> LocalFrame {
    let w = axis.normalize();
    let u_proj = ref_dir - w * ref_dir.dot(&w);
    let u = if u_proj.norm() > 1e-12 {
        u_proj.normalize()
    } else if w.x.abs() < 0.9 {
        Vector3::x().cross(&w).normalize()
    } else {
        Vector3::y().cross(&w).normalize()
    };
    let v = w.cross(&u);
    LocalFrame { origin, u, v, w }
}

/// Apply the frame transform to homogeneous polynomial inputs. Returns
/// (local_X, local_Y, local_Z) as `BiPoly` in the input's domain.
fn transform_xyz_to_local(s: &HomogeneousSurface, f: &LocalFrame) -> (BiPoly, BiPoly, BiPoly) {
    // local_x = u · (P_world - origin) where P_world = (X/W, Y/W, Z/W).
    // In homogeneous form we scale by W:
    //   LX = u_x · X + u_y · Y + u_z · Z - (u · origin) · W
    let dot_origin = |a: &Vector3| a.x * f.origin.x + a.y * f.origin.y + a.z * f.origin.z;
    let lx = combine(s, &f.u, dot_origin(&f.u));
    let ly = combine(s, &f.v, dot_origin(&f.v));
    let lz = combine(s, &f.w, dot_origin(&f.w));
    (lx, ly, lz)
}

fn combine(s: &HomogeneousSurface, axis: &Vector3, dot_origin: f64) -> BiPoly {
    let ax = s.x.scale(&rat(axis.x));
    let ay = s.y.scale(&rat(axis.y));
    let az = s.z.scale(&rat(axis.z));
    let bias = s.w.scale(&rat(-dot_origin));
    ax.add(&ay).add(&az).add(&bias)
}

fn rat(v: f64) -> Rational {
    Rational::try_from(v).unwrap_or(Rational::from(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a trivial "surface" whose homogeneous polynomials encode
    /// a single 3D point: X = px, Y = py, Z = pz, W = 1 (constants).
    /// Substituting evaluates the implicit at that point — useful for
    /// checking the homogenization manually.
    fn point_surface(p: Point3) -> (BiPoly, BiPoly, BiPoly, BiPoly) {
        let x = BiPoly::from_f64(p.x);
        let y = BiPoly::from_f64(p.y);
        let z = BiPoly::from_f64(p.z);
        let w = BiPoly::from_f64(1.0);
        (x, y, z, w)
    }

    #[test]
    fn plane_implicit_correct() {
        // Plane z = 0 (origin at 0, normal +z).
        let plane = Plane::new(Point3::origin(), Vector3::z());
        // Point (1, 2, 0) is on the plane → G should be 0.
        let (x, y, z, w) = point_surface(Point3::new(1.0, 2.0, 0.0));
        let g = substitute_into_plane(HomogeneousSurface { x: &x, y: &y, z: &z, w: &w }, &plane);
        assert!((g.eval_f64(0.0, 0.0)).abs() < 1e-12);
        // Point (1, 2, 3) is 3 above the plane → G should be 3.
        let (x, y, z, w) = point_surface(Point3::new(1.0, 2.0, 3.0));
        let g = substitute_into_plane(HomogeneousSurface { x: &x, y: &y, z: &z, w: &w }, &plane);
        assert!((g.eval_f64(0.0, 0.0) - 3.0).abs() < 1e-12);
    }

    #[test]
    fn sphere_implicit_correct() {
        let sphere = Sphere::new(Point3::origin(), 5.0);
        // (3, 4, 0) is at distance 5 → G(3,4,0) = 0
        let (x, y, z, w) = point_surface(Point3::new(3.0, 4.0, 0.0));
        let g = substitute_into_sphere(HomogeneousSurface { x: &x, y: &y, z: &z, w: &w }, &sphere);
        assert!((g.eval_f64(0.0, 0.0)).abs() < 1e-9);
        // (1, 0, 0) is inside (distance 1) → G = 1 - 25 = -24
        let (x, y, z, w) = point_surface(Point3::new(1.0, 0.0, 0.0));
        let g = substitute_into_sphere(HomogeneousSurface { x: &x, y: &y, z: &z, w: &w }, &sphere);
        assert!((g.eval_f64(0.0, 0.0) - (-24.0)).abs() < 1e-9);
    }

    #[test]
    fn cylinder_implicit_correct() {
        let cyl = Cylinder {
            origin: Point3::origin(),
            axis: Vector3::z(),
            radius: 3.0,
            ref_direction: Vector3::x(),
            v_min: -10.0, v_max: 10.0,
        };
        // (3, 0, 5) is on cylinder → G = 0
        let (x, y, z, w) = point_surface(Point3::new(3.0, 0.0, 5.0));
        let g = substitute_into_cylinder(HomogeneousSurface { x: &x, y: &y, z: &z, w: &w }, &cyl);
        assert!((g.eval_f64(0.0, 0.0)).abs() < 1e-9);
        // (1, 0, 0) is interior (radius 1) → G = 1 - 9 = -8
        let (x, y, z, w) = point_surface(Point3::new(1.0, 0.0, 0.0));
        let g = substitute_into_cylinder(HomogeneousSurface { x: &x, y: &y, z: &z, w: &w }, &cyl);
        assert!((g.eval_f64(0.0, 0.0) - (-8.0)).abs() < 1e-9);
    }

    #[test]
    fn cone_implicit_correct() {
        let cone = Cone {
            apex: Point3::origin(),
            axis: Vector3::z(),
            half_angle: std::f64::consts::FRAC_PI_4, // 45°
            ref_direction: Vector3::x(),
            v_min: 0.0, v_max: 10.0,
        };
        // 45° cone means radial = |z|. Point (3, 4, 5) has radial 5 = |5| → on cone → G = 0
        let (x, y, z, w) = point_surface(Point3::new(3.0, 4.0, 5.0));
        let g = substitute_into_cone(HomogeneousSurface { x: &x, y: &y, z: &z, w: &w }, &cone);
        assert!((g.eval_f64(0.0, 0.0)).abs() < 1e-9,
            "45° cone at (3,4,5) should be 0, got {}", g.eval_f64(0.0, 0.0));
        // Point (1, 0, 5): radial 1, axial 5. 1·cos(45)² - 25·sin(45)² = 0.5 - 12.5 = -12
        let (x, y, z, w) = point_surface(Point3::new(1.0, 0.0, 5.0));
        let g = substitute_into_cone(HomogeneousSurface { x: &x, y: &y, z: &z, w: &w }, &cone);
        assert!((g.eval_f64(0.0, 0.0) - (-12.0)).abs() < 1e-9);
    }

    #[test]
    fn torus_implicit_correct() {
        let torus = Torus {
            center: Point3::origin(),
            axis: Vector3::z(),
            major_radius: 3.0,
            minor_radius: 1.0,
            ref_direction: Vector3::x(),
        };
        // Point on torus: at radius R + r = 4 from axis, z = 0.
        let (x, y, z, w) = point_surface(Point3::new(4.0, 0.0, 0.0));
        let g = substitute_into_torus(HomogeneousSurface { x: &x, y: &y, z: &z, w: &w }, &torus);
        assert!((g.eval_f64(0.0, 0.0)).abs() < 1e-6,
            "torus at (4,0,0) should be ~0, got {}", g.eval_f64(0.0, 0.0));
        // Point inside torus tube: at radius R = 3, z = 0.
        // (|P|² + R² - r²)² - 4R²(P_x² + P_y²) = (9 + 9 - 1)² - 4·9·9 = 17² - 324 = 289-324 = -35
        let (x, y, z, w) = point_surface(Point3::new(3.0, 0.0, 0.0));
        let g = substitute_into_torus(HomogeneousSurface { x: &x, y: &y, z: &z, w: &w }, &torus);
        assert!((g.eval_f64(0.0, 0.0) - (-35.0)).abs() < 1e-6);
    }

    #[test]
    fn local_frame_offset_origin() {
        // Plane at z=10 with normal +z. Substituting (5, 5, 10) → G = 0.
        let plane = Plane::new(Point3::new(0.0, 0.0, 10.0), Vector3::z());
        let (x, y, z, w) = point_surface(Point3::new(5.0, 5.0, 10.0));
        let g = substitute_into_plane(HomogeneousSurface { x: &x, y: &y, z: &z, w: &w }, &plane);
        assert!((g.eval_f64(0.0, 0.0)).abs() < 1e-12);
        // (5, 5, 12) is 2 above → G = 2
        let (x, y, z, w) = point_surface(Point3::new(5.0, 5.0, 12.0));
        let g = substitute_into_plane(HomogeneousSurface { x: &x, y: &y, z: &z, w: &w }, &plane);
        assert!((g.eval_f64(0.0, 0.0) - 2.0).abs() < 1e-12);
    }
}
