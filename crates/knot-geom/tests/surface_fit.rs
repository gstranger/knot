use knot_geom::Point3;
use knot_geom::curve::{Curve, NurbsCurve};
use knot_geom::surface::fit::{interpolate_surface_grid, coons_patch};

fn make_grid(rows: usize, cols: usize, f: impl Fn(f64, f64) -> Point3) -> Vec<Point3> {
    let mut pts = Vec::with_capacity(rows * cols);
    for i in 0..rows {
        let u = i as f64 / (rows - 1) as f64;
        for j in 0..cols {
            let v = j as f64 / (cols - 1) as f64;
            pts.push(f(u, v));
        }
    }
    pts
}

#[test]
fn interpolate_flat_grid() {
    let rows = 4;
    let cols = 4;
    let grid = make_grid(rows, cols, |u, v| Point3::new(u, v, 0.0));

    let surf = interpolate_surface_grid(&grid, rows, cols, 3, 3).unwrap();

    // Every grid point should lie on the surface
    for i in 0..rows {
        let u = i as f64 / (rows - 1) as f64;
        for j in 0..cols {
            let v = j as f64 / (cols - 1) as f64;
            let p = surf.point_at(u, v);
            let expected = Point3::new(u, v, 0.0);
            let err = (p - expected).norm();
            assert!(err < 1e-8, "({i},{j}): error = {err}");
        }
    }
}

#[test]
fn interpolate_curved_grid() {
    let rows = 5;
    let cols = 5;
    let grid = make_grid(rows, cols, |u, v| {
        Point3::new(u, v, 0.2 * (u * std::f64::consts::PI).sin() * (v * std::f64::consts::PI).sin())
    });

    let surf = interpolate_surface_grid(&grid, rows, cols, 3, 3).unwrap();

    // Check corners
    let p00 = surf.point_at(0.0, 0.0);
    assert!((p00 - grid[0]).norm() < 1e-8);
    let p11 = surf.point_at(1.0, 1.0);
    assert!((p11 - grid[rows * cols - 1]).norm() < 1e-8);
}

#[test]
fn interpolate_bilinear_grid() {
    let rows = 3;
    let cols = 3;
    let grid = make_grid(rows, cols, |u, v| Point3::new(u, v, u + v));
    let surf = interpolate_surface_grid(&grid, rows, cols, 2, 2).unwrap();

    // Interior point should satisfy z = u + v
    let p = surf.point_at(0.5, 0.5);
    assert!((p.z - 1.0).abs() < 1e-8, "z = {} expected 1.0", p.z);
}

#[test]
fn interpolate_asymmetric_degrees() {
    let rows = 4;
    let cols = 6;
    let grid = make_grid(rows, cols, |u, v| Point3::new(u, v, u * v));
    let surf = interpolate_surface_grid(&grid, rows, cols, 2, 3).unwrap();

    let p = surf.point_at(0.0, 0.0);
    assert!((p - grid[0]).norm() < 1e-8);
}

#[test]
fn coons_patch_flat_boundaries() {
    // Four straight edges forming a unit square in z=0
    let bottom = Curve::Line(knot_geom::curve::LineSeg {
        start: Point3::new(0.0, 0.0, 0.0),
        end: Point3::new(1.0, 0.0, 0.0),
    });
    let top = Curve::Line(knot_geom::curve::LineSeg {
        start: Point3::new(0.0, 1.0, 0.0),
        end: Point3::new(1.0, 1.0, 0.0),
    });
    let left = Curve::Line(knot_geom::curve::LineSeg {
        start: Point3::new(0.0, 0.0, 0.0),
        end: Point3::new(0.0, 1.0, 0.0),
    });
    let right = Curve::Line(knot_geom::curve::LineSeg {
        start: Point3::new(1.0, 0.0, 0.0),
        end: Point3::new(1.0, 1.0, 0.0),
    });

    let surf = coons_patch(&bottom, &top, &left, &right, 6, 6, 3, 3).unwrap();

    // Center should be near (0.5, 0.5, 0.0)
    let mid = surf.point_at(0.5, 0.5);
    assert!((mid.x - 0.5).abs() < 1e-6, "x = {}", mid.x);
    assert!((mid.y - 0.5).abs() < 1e-6, "y = {}", mid.y);
    assert!(mid.z.abs() < 1e-6, "z = {}", mid.z);
}

#[test]
fn coons_patch_curved_boundary() {
    // Bottom is a NURBS arc (lifted), rest are straight lines
    let bottom_pts = vec![
        Point3::new(0.0, 0.0, 0.0),
        Point3::new(0.5, 0.0, 0.5),
        Point3::new(1.0, 0.0, 0.0),
    ];
    let bottom_curve = NurbsCurve::new(
        bottom_pts,
        vec![1.0, 1.0, 1.0],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        2,
    )
    .unwrap();
    let bottom = Curve::Nurbs(bottom_curve);

    let top = Curve::Line(knot_geom::curve::LineSeg {
        start: Point3::new(0.0, 1.0, 0.0),
        end: Point3::new(1.0, 1.0, 0.0),
    });
    let left = Curve::Line(knot_geom::curve::LineSeg {
        start: Point3::new(0.0, 0.0, 0.0),
        end: Point3::new(0.0, 1.0, 0.0),
    });
    let right = Curve::Line(knot_geom::curve::LineSeg {
        start: Point3::new(1.0, 0.0, 0.0),
        end: Point3::new(1.0, 1.0, 0.0),
    });

    let surf = coons_patch(&bottom, &top, &left, &right, 8, 8, 3, 3).unwrap();

    // Bottom edge midpoint should have z > 0 (lifted by the curved boundary)
    let p = surf.point_at(0.5, 0.0);
    assert!(p.z > 0.1, "bottom midpoint z = {} (expected > 0.1)", p.z);

    // Top edge should be flat
    let p_top = surf.point_at(0.5, 1.0);
    assert!(p_top.z.abs() < 1e-4, "top midpoint z = {}", p_top.z);
}

#[test]
fn grid_dimension_mismatch() {
    let grid = vec![Point3::origin(); 10];
    assert!(interpolate_surface_grid(&grid, 3, 4, 2, 2).is_err());
}

#[test]
fn grid_too_few_rows() {
    let grid = vec![Point3::origin(); 6];
    assert!(interpolate_surface_grid(&grid, 2, 3, 3, 2).is_err());
}
