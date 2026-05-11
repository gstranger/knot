//! NURBS curve fitting: interpolation and least-squares approximation.
//!
//! Algorithms from Piegl & Tiller, "The NURBS Book", Chapter 9.
//! All outputs are non-rational B-splines (weights = 1).

use knot_core::{ErrorCode, KResult, KernelError};
use crate::point::Point3;
use super::NurbsCurve;
use nalgebra::{DMatrix, DVector};

// ---------------------------------------------------------------------------
// Shared utilities (pub(crate) for use by surface::fit)
// ---------------------------------------------------------------------------

/// Chord-length parameterization: maps data points to [0, 1].
pub(crate) fn chord_length_params(points: &[Point3]) -> Vec<f64> {
    let n = points.len();
    if n <= 1 {
        return vec![0.0; n];
    }

    let mut dists = Vec::with_capacity(n - 1);
    let mut total = 0.0;
    for i in 1..n {
        let d = (points[i] - points[i - 1]).norm();
        dists.push(d);
        total += d;
    }

    if total < 1e-30 {
        return (0..n).map(|i| i as f64 / (n - 1) as f64).collect();
    }

    let mut params = Vec::with_capacity(n);
    params.push(0.0);
    let mut cum = 0.0;
    for d in &dists {
        cum += d;
        params.push(cum / total);
    }
    *params.last_mut().unwrap() = 1.0;
    params
}

/// Clamped knot vector by averaging parameter values (Piegl & Tiller Eq. 9.8).
///
/// Given `n` parameter values and degree `p`, produces `n + p + 1` knots.
pub(crate) fn averaging_knots(params: &[f64], degree: u32) -> Vec<f64> {
    let n = params.len();
    let p = degree as usize;
    let num_knots = n + p + 1;
    let mut knots = vec![0.0; num_knots];

    let t_start = params[0];
    let t_end = params[n - 1];

    for i in 0..=p {
        knots[i] = t_start;
        knots[num_knots - 1 - i] = t_end;
    }

    // Interior knots (n - p - 1 of them)
    for j in 1..n.saturating_sub(p) {
        let mut sum = 0.0;
        for i in j..j + p {
            sum += params[i];
        }
        knots[j + p] = sum / p as f64;
    }

    knots
}

/// Find the knot span index such that `knots[span] <= t < knots[span+1]`.
///
/// `n` is the index of the last control point (= num_control_points - 1).
pub(crate) fn find_span(knots: &[f64], n: usize, p: usize, t: f64) -> usize {
    if t >= knots[n + 1] {
        return n;
    }
    if t <= knots[p] {
        return p;
    }

    let mut low = p;
    let mut high = n + 1;
    let mut mid = (low + high) / 2;

    while t < knots[mid] || t >= knots[mid + 1] {
        if t < knots[mid] {
            high = mid;
        } else {
            low = mid;
        }
        mid = (low + high) / 2;
    }

    mid
}

/// Evaluate all `p + 1` non-zero basis functions at parameter `t`.
pub(crate) fn basis_fns(knots: &[f64], span: usize, p: usize, t: f64) -> Vec<f64> {
    let mut basis = vec![0.0; p + 1];
    let mut left = vec![0.0; p + 1];
    let mut right = vec![0.0; p + 1];

    basis[0] = 1.0;

    for j in 1..=p {
        left[j] = t - knots[span + 1 - j];
        right[j] = knots[span + j] - t;
        let mut saved = 0.0;

        for r in 0..j {
            let denom = right[r + 1] + left[j - r];
            if denom.abs() < 1e-30 {
                basis[r] = saved;
                saved = 0.0;
                continue;
            }
            let temp = basis[r] / denom;
            basis[r] = saved + right[r + 1] * temp;
            saved = left[j - r] * temp;
        }

        basis[j] = saved;
    }

    basis
}

/// Solve the interpolation linear system: find `n` control points such that the
/// B-spline with the given knot vector passes through `points` at `params`.
pub(crate) fn solve_interpolation_system(
    points: &[Point3],
    params: &[f64],
    knots: &[f64],
    degree: u32,
) -> KResult<Vec<Point3>> {
    let n = points.len();
    let p = degree as usize;

    let mut mat = DMatrix::<f64>::zeros(n, n);

    for k in 0..n {
        let span = find_span(knots, n - 1, p, params[k]);
        let b = basis_fns(knots, span, p, params[k]);
        for i in 0..=p {
            let col = span - p + i;
            if col < n {
                mat[(k, col)] = b[i];
            }
        }
    }

    let lu = mat.lu();

    let rhs_x = DVector::from_fn(n, |i, _| points[i].x);
    let rhs_y = DVector::from_fn(n, |i, _| points[i].y);
    let rhs_z = DVector::from_fn(n, |i, _| points[i].z);

    let err = || KernelError::NumericalFailure {
        code: ErrorCode::NoConvergence,
        detail: "singular interpolation matrix".into(),
    };

    let sol_x = lu.solve(&rhs_x).ok_or_else(err)?;
    let sol_y = lu.solve(&rhs_y).ok_or_else(err)?;
    let sol_z = lu.solve(&rhs_z).ok_or_else(err)?;

    Ok((0..n)
        .map(|i| Point3::new(sol_x[i], sol_y[i], sol_z[i]))
        .collect())
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Interpolate a NURBS curve exactly through the given points.
///
/// Uses chord-length parameterization and the averaging method for knot
/// vector generation. The result is a non-rational B-spline (all weights 1).
///
/// Requires at least `degree + 1` points.
pub fn interpolate_curve(points: &[Point3], degree: u32) -> KResult<NurbsCurve> {
    let params = chord_length_params(points);
    interpolate_curve_with_params(points, &params, degree)
}

/// Interpolate a NURBS curve through points with caller-supplied parameter values.
///
/// `params` must have the same length as `points` and be non-decreasing.
pub fn interpolate_curve_with_params(
    points: &[Point3],
    params: &[f64],
    degree: u32,
) -> KResult<NurbsCurve> {
    let n = points.len();
    let p = degree as usize;

    if n < p + 1 {
        return Err(KernelError::InvalidInput {
            code: ErrorCode::InsufficientControlPoints,
            detail: format!(
                "need at least {} points for degree {}, got {}",
                p + 1,
                degree,
                n
            ),
        });
    }

    if params.len() != n {
        return Err(KernelError::InvalidInput {
            code: ErrorCode::MalformedInput,
            detail: format!(
                "params length ({}) != points length ({})",
                params.len(),
                n
            ),
        });
    }

    let knots = averaging_knots(params, degree);
    let control_points = solve_interpolation_system(points, params, &knots, degree)?;
    let weights = vec![1.0; n];

    NurbsCurve::new(control_points, weights, knots, degree)
}

/// Least-squares NURBS curve approximation.
///
/// The curve passes exactly through the first and last data points and
/// minimizes the sum of squared distances to all interior points.
///
/// `num_control_points` must satisfy `degree + 1 <= num_cp <= points.len()`.
/// If `num_cp == points.len()`, delegates to exact interpolation.
pub fn approximate_curve(
    points: &[Point3],
    num_control_points: usize,
    degree: u32,
) -> KResult<NurbsCurve> {
    let m = points.len();
    let num_cp = num_control_points;
    let p = degree as usize;

    if m < 2 {
        return Err(KernelError::InvalidInput {
            code: ErrorCode::InsufficientControlPoints,
            detail: format!("need at least 2 data points, got {}", m),
        });
    }
    if num_cp < p + 1 {
        return Err(KernelError::InvalidInput {
            code: ErrorCode::InsufficientControlPoints,
            detail: format!(
                "need at least {} control points for degree {}, got {}",
                p + 1,
                degree,
                num_cp
            ),
        });
    }
    if num_cp > m {
        return Err(KernelError::InvalidInput {
            code: ErrorCode::MalformedInput,
            detail: format!(
                "num_control_points ({}) > data points ({})",
                num_cp, m
            ),
        });
    }
    if num_cp == m {
        return interpolate_curve(points, degree);
    }

    let params = chord_length_params(points);

    // Knot vector (Piegl & Tiller Eq. 9.68-9.69)
    let num_knots = num_cp + p + 1;
    let mut knots = vec![0.0; num_knots];
    for i in 0..=p {
        knots[num_knots - 1 - i] = 1.0;
    }
    let num_interior = num_cp - p - 1;
    if num_interior > 0 {
        let d = m as f64 / (num_interior + 1) as f64;
        for j in 1..=num_interior {
            let i_f = j as f64 * d;
            let i = i_f as usize;
            let alpha = i_f - i as f64;
            knots[p + j] =
                (1.0 - alpha) * params[(i - 1).min(m - 1)] + alpha * params[i.min(m - 1)];
        }
    }

    let n_idx = num_cp - 1; // last control-point index
    let interior_rows = m - 2;
    let interior_cp = num_cp - 2;

    if interior_cp == 0 {
        // Only endpoints — straight line
        let control_points = vec![points[0], points[m - 1]];
        let weights = vec![1.0; 2];
        return NurbsCurve::new(control_points, weights, knots, degree);
    }

    // Build (interior_rows) × (interior_cp) matrix and RHS
    let mut mat_n = DMatrix::<f64>::zeros(interior_rows, interior_cp);
    let mut rhs_x = DVector::<f64>::zeros(interior_rows);
    let mut rhs_y = DVector::<f64>::zeros(interior_rows);
    let mut rhs_z = DVector::<f64>::zeros(interior_rows);

    for k in 0..interior_rows {
        let data_idx = k + 1;
        let t = params[data_idx];
        let span = find_span(&knots, n_idx, p, t);
        let b = basis_fns(&knots, span, p, t);

        let mut n0 = 0.0;
        let mut nn = 0.0;

        for i in 0..=p {
            let cp_idx = span - p + i;
            if cp_idx == 0 {
                n0 = b[i];
            }
            if cp_idx == n_idx {
                nn = b[i];
            }
            if cp_idx >= 1 && cp_idx <= interior_cp {
                mat_n[(k, cp_idx - 1)] = b[i];
            }
        }

        rhs_x[k] = points[data_idx].x - n0 * points[0].x - nn * points[m - 1].x;
        rhs_y[k] = points[data_idx].y - n0 * points[0].y - nn * points[m - 1].y;
        rhs_z[k] = points[data_idx].z - n0 * points[0].z - nn * points[m - 1].z;
    }

    // Normal equations: (Nᵀ N) x = Nᵀ R
    let ntn = mat_n.transpose() * &mat_n;
    let ntr_x = mat_n.transpose() * &rhs_x;
    let ntr_y = mat_n.transpose() * &rhs_y;
    let ntr_z = mat_n.transpose() * &rhs_z;

    let lu = ntn.lu();
    let err = || KernelError::NumericalFailure {
        code: ErrorCode::NoConvergence,
        detail: "singular normal-equation matrix in curve approximation".into(),
    };

    let sol_x = lu.solve(&ntr_x).ok_or_else(err)?;
    let sol_y = lu.solve(&ntr_y).ok_or_else(err)?;
    let sol_z = lu.solve(&ntr_z).ok_or_else(err)?;

    let mut control_points = Vec::with_capacity(num_cp);
    control_points.push(points[0]);
    for i in 0..interior_cp {
        control_points.push(Point3::new(sol_x[i], sol_y[i], sol_z[i]));
    }
    control_points.push(points[m - 1]);

    let weights = vec![1.0; num_cp];
    NurbsCurve::new(control_points, weights, knots, degree)
}
