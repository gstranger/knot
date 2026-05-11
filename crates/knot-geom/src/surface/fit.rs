//! NURBS surface fitting: grid interpolation and Coons patch construction.

use knot_core::{ErrorCode, KResult, KernelError};
use crate::point::Point3;
use crate::curve::{Curve, CurveParam};
use crate::curve::fit::{chord_length_params, averaging_knots, solve_interpolation_system};
use super::NurbsSurface;

/// Interpolate a NURBS surface through a rectangular grid of points.
///
/// Uses tensor-product interpolation: fits curves through each row (v-direction),
/// then fits through the resulting control points in the column direction (u-direction).
///
/// # Arguments
/// * `points` — Row-major grid: `points[row * cols + col]`.
///   Rows vary in the u-direction, columns in v.
/// * `rows` — Number of rows (u-direction count)
/// * `cols` — Number of columns (v-direction count)
/// * `degree_u` — Surface degree in u
/// * `degree_v` — Surface degree in v
pub fn interpolate_surface_grid(
    points: &[Point3],
    rows: usize,
    cols: usize,
    degree_u: u32,
    degree_v: u32,
) -> KResult<NurbsSurface> {
    if points.len() != rows * cols {
        return Err(KernelError::InvalidInput {
            code: ErrorCode::MalformedInput,
            detail: format!(
                "expected {} points ({}×{}), got {}",
                rows * cols,
                rows,
                cols,
                points.len()
            ),
        });
    }

    let pu = degree_u as usize;
    let pv = degree_v as usize;

    if rows < pu + 1 {
        return Err(KernelError::InvalidInput {
            code: ErrorCode::InsufficientControlPoints,
            detail: format!(
                "need at least {} rows for degree_u {}, got {}",
                pu + 1,
                degree_u,
                rows
            ),
        });
    }
    if cols < pv + 1 {
        return Err(KernelError::InvalidInput {
            code: ErrorCode::InsufficientControlPoints,
            detail: format!(
                "need at least {} cols for degree_v {}, got {}",
                pv + 1,
                degree_v,
                cols
            ),
        });
    }

    // Average chord-length params across rows → v-direction parameters
    let mut v_params = vec![0.0; cols];
    for row in 0..rows {
        let row_pts: Vec<Point3> = (0..cols).map(|c| points[row * cols + c]).collect();
        let p = chord_length_params(&row_pts);
        for c in 0..cols {
            v_params[c] += p[c];
        }
    }
    for v in &mut v_params {
        *v /= rows as f64;
    }
    v_params[0] = 0.0;
    v_params[cols - 1] = 1.0;

    // Average chord-length params across columns → u-direction parameters
    let mut u_params = vec![0.0; rows];
    for col in 0..cols {
        let col_pts: Vec<Point3> = (0..rows).map(|r| points[r * cols + col]).collect();
        let p = chord_length_params(&col_pts);
        for r in 0..rows {
            u_params[r] += p[r];
        }
    }
    for u in &mut u_params {
        *u /= cols as f64;
    }
    u_params[0] = 0.0;
    u_params[rows - 1] = 1.0;

    // Knot vectors
    let knots_v = averaging_knots(&v_params, degree_v);
    let knots_u = averaging_knots(&u_params, degree_u);

    // Pass 1: interpolate each row in v-direction
    let mut r_pts = vec![Point3::origin(); rows * cols];
    for row in 0..rows {
        let row_pts: Vec<Point3> = (0..cols).map(|c| points[row * cols + c]).collect();
        let cp = solve_interpolation_system(&row_pts, &v_params, &knots_v, degree_v)?;
        for c in 0..cols {
            r_pts[row * cols + c] = cp[c];
        }
    }

    // Pass 2: interpolate each column in u-direction
    let mut control_points = vec![Point3::origin(); rows * cols];
    for col in 0..cols {
        let col_pts: Vec<Point3> = (0..rows).map(|r| r_pts[r * cols + col]).collect();
        let cp = solve_interpolation_system(&col_pts, &u_params, &knots_u, degree_u)?;
        for r in 0..rows {
            control_points[r * cols + col] = cp[r];
        }
    }

    let weights = vec![1.0; rows * cols];

    NurbsSurface::new(
        control_points,
        weights,
        knots_u,
        knots_v,
        degree_u,
        degree_v,
        rows as u32,
        cols as u32,
    )
}

/// Construct a bilinear Coons patch from four boundary curves.
///
/// ```text
/// S(u,v) = (1−v)·bottom(u) + v·top(u)
///        + (1−u)·left(v)   + u·right(v)
///        − bilinear corner correction
/// ```
///
/// Corner compatibility is required:
/// `bottom(0) ≈ left(0)`, `bottom(1) ≈ right(0)`,
/// `top(0) ≈ left(1)`, `top(1) ≈ right(1)`.
///
/// The patch is sampled on a regular grid and then interpolated as a
/// NURBS surface with the requested degrees.
pub fn coons_patch(
    bottom: &Curve,
    top: &Curve,
    left: &Curve,
    right: &Curve,
    samples_u: usize,
    samples_v: usize,
    degree_u: u32,
    degree_v: u32,
) -> KResult<NurbsSurface> {
    if samples_u < (degree_u as usize) + 1 {
        return Err(KernelError::InvalidInput {
            code: ErrorCode::InsufficientControlPoints,
            detail: format!(
                "samples_u ({}) must be >= degree_u + 1 ({})",
                samples_u,
                degree_u + 1
            ),
        });
    }
    if samples_v < (degree_v as usize) + 1 {
        return Err(KernelError::InvalidInput {
            code: ErrorCode::InsufficientControlPoints,
            detail: format!(
                "samples_v ({}) must be >= degree_v + 1 ({})",
                samples_v,
                degree_v + 1
            ),
        });
    }

    let d_bot = bottom.domain();
    let d_top = top.domain();
    let d_left = left.domain();
    let d_right = right.domain();

    let p00 = bottom.point_at(CurveParam(d_bot.start));
    let p10 = bottom.point_at(CurveParam(d_bot.end));
    let p01 = top.point_at(CurveParam(d_top.start));
    let p11 = top.point_at(CurveParam(d_top.end));

    let mut grid = Vec::with_capacity(samples_u * samples_v);

    for i in 0..samples_u {
        let u = i as f64 / (samples_u - 1) as f64;
        let t_bot = d_bot.start + u * (d_bot.end - d_bot.start);
        let t_top = d_top.start + u * (d_top.end - d_top.start);

        for j in 0..samples_v {
            let v = j as f64 / (samples_v - 1) as f64;
            let t_left = d_left.start + v * (d_left.end - d_left.start);
            let t_right = d_right.start + v * (d_right.end - d_right.start);

            let bot = bottom.point_at(CurveParam(t_bot));
            let top_pt = top.point_at(CurveParam(t_top));
            let lft = left.point_at(CurveParam(t_left));
            let rgt = right.point_at(CurveParam(t_right));

            // Ruled surfaces along u and v
            let s1x = (1.0 - v) * bot.x + v * top_pt.x;
            let s1y = (1.0 - v) * bot.y + v * top_pt.y;
            let s1z = (1.0 - v) * bot.z + v * top_pt.z;

            let s2x = (1.0 - u) * lft.x + u * rgt.x;
            let s2y = (1.0 - u) * lft.y + u * rgt.y;
            let s2z = (1.0 - u) * lft.z + u * rgt.z;

            // Bilinear corner correction
            let cx = (1.0 - u) * (1.0 - v) * p00.x
                + u * (1.0 - v) * p10.x
                + (1.0 - u) * v * p01.x
                + u * v * p11.x;
            let cy = (1.0 - u) * (1.0 - v) * p00.y
                + u * (1.0 - v) * p10.y
                + (1.0 - u) * v * p01.y
                + u * v * p11.y;
            let cz = (1.0 - u) * (1.0 - v) * p00.z
                + u * (1.0 - v) * p10.z
                + (1.0 - u) * v * p01.z
                + u * v * p11.z;

            grid.push(Point3::new(s1x + s2x - cx, s1y + s2y - cy, s1z + s2z - cz));
        }
    }

    interpolate_surface_grid(&grid, samples_u, samples_v, degree_u, degree_v)
}
