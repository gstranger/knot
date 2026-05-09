use knot_core::KResult;
use knot_geom::Point3;
use knot_geom::curve::{Curve, CurveParam, LineSeg};
use super::CurveCurveHit;

/// Compute intersections between two curves.
pub fn intersect_curves(a: &Curve, b: &Curve, tolerance: f64) -> KResult<Vec<CurveCurveHit>> {
    match (a, b) {
        (Curve::Line(la), Curve::Line(lb)) => line_line(la, lb, tolerance),
        _ => general_intersect(a, b, tolerance),
    }
}

/// Line-line intersection in 3D.
fn line_line(a: &LineSeg, b: &LineSeg, tolerance: f64) -> KResult<Vec<CurveCurveHit>> {
    let da = a.direction();
    let db = b.direction();
    let w = a.start - b.start;

    let a_dot_a = da.dot(&da);
    let a_dot_b = da.dot(&db);
    let b_dot_b = db.dot(&db);
    let a_dot_w = da.dot(&w);
    let b_dot_w = db.dot(&w);

    let denom = a_dot_a * b_dot_b - a_dot_b * a_dot_b;

    if denom.abs() < 1e-30 {
        return Ok(Vec::new());
    }

    let ta = (a_dot_b * b_dot_w - b_dot_b * a_dot_w) / denom;
    let tb = (a_dot_a * b_dot_w - a_dot_b * a_dot_w) / denom;

    if ta < -tolerance || ta > 1.0 + tolerance || tb < -tolerance || tb > 1.0 + tolerance {
        return Ok(Vec::new());
    }

    let ta = ta.clamp(0.0, 1.0);
    let tb = tb.clamp(0.0, 1.0);

    let pa = a.point_at(ta);
    let pb = b.point_at(tb);
    let dist = (pa - pb).norm();

    if dist > tolerance {
        return Ok(Vec::new());
    }

    Ok(vec![CurveCurveHit {
        point: Point3::new((pa.x + pb.x) / 2.0, (pa.y + pb.y) / 2.0, (pa.z + pb.z) / 2.0),
        param_a: CurveParam(ta),
        param_b: CurveParam(tb),
    }])
}

/// General curve-curve intersection using bounding-box subdivision
/// followed by Newton refinement on candidate pairs.
fn general_intersect(a: &Curve, b: &Curve, tolerance: f64) -> KResult<Vec<CurveCurveHit>> {
    let a_domain = a.domain();
    let b_domain = b.domain();

    // Phase 1: Find candidate parameter pairs via subdivision
    let mut candidates = Vec::new();
    find_candidates(
        a, a_domain.start, a_domain.end,
        b, b_domain.start, b_domain.end,
        tolerance, &mut candidates, 0,
    );

    // Phase 2: Newton-refine each candidate
    let mut hits = Vec::new();
    for (ta_init, tb_init) in &candidates {
        if let Some(hit) = newton_refine(a, b, *ta_init, *tb_init, tolerance) {
            hits.push(hit);
        }
    }

    // Phase 3: Deduplicate
    deduplicate_hits(&mut hits, tolerance * 10.0);

    Ok(hits)
}

/// Recursively subdivide to find candidate parameter pairs where curves might intersect.
fn find_candidates(
    a: &Curve, a_lo: f64, a_hi: f64,
    b: &Curve, b_lo: f64, b_hi: f64,
    tolerance: f64,
    candidates: &mut Vec<(f64, f64)>,
    depth: usize,
) {
    const MAX_DEPTH: usize = 30;
    const MIN_INTERVAL: f64 = 1e-4;

    let a_bb = sample_bbox(a, a_lo, a_hi, 4).expand(tolerance);
    let b_bb = sample_bbox(b, b_lo, b_hi, 4);

    if !a_bb.intersects(&b_bb) {
        return;
    }

    let a_size = a_hi - a_lo;
    let b_size = b_hi - b_lo;

    if depth >= MAX_DEPTH || (a_size < MIN_INTERVAL && b_size < MIN_INTERVAL) {
        candidates.push(((a_lo + a_hi) / 2.0, (b_lo + b_hi) / 2.0));
        return;
    }

    // Subdivide the larger interval
    if a_size >= b_size {
        let a_mid = (a_lo + a_hi) / 2.0;
        find_candidates(a, a_lo, a_mid, b, b_lo, b_hi, tolerance, candidates, depth + 1);
        find_candidates(a, a_mid, a_hi, b, b_lo, b_hi, tolerance, candidates, depth + 1);
    } else {
        let b_mid = (b_lo + b_hi) / 2.0;
        find_candidates(a, a_lo, a_hi, b, b_lo, b_mid, tolerance, candidates, depth + 1);
        find_candidates(a, a_lo, a_hi, b, b_mid, b_hi, tolerance, candidates, depth + 1);
    }
}

/// Newton iteration to refine intersection: minimize |C_a(ta) - C_b(tb)|^2.
/// We solve the system:
///   f1 = (C_a - C_b) . C_a' = 0
///   f2 = (C_a - C_b) . C_b' = 0  (note: negative because moving tb reduces distance)
fn newton_refine(
    a: &Curve, b: &Curve,
    mut ta: f64, mut tb: f64,
    tolerance: f64,
) -> Option<CurveCurveHit> {
    let a_domain = a.domain();
    let b_domain = b.domain();

    for _ in 0..20 {
        let da = a.derivatives_at(CurveParam(ta));
        let db = b.derivatives_at(CurveParam(tb));
        let diff = da.point - db.point;

        let f1 = diff.dot(&da.d1);
        let f2 = -diff.dot(&db.d1);

        // Jacobian
        let j11 = da.d1.norm_squared() + diff.dot(&da.d2.unwrap_or_default());
        let j12 = -da.d1.dot(&db.d1);
        let j21 = -da.d1.dot(&db.d1);
        let j22 = db.d1.norm_squared() - diff.dot(&db.d2.unwrap_or_default());

        let det = j11 * j22 - j12 * j21;
        if det.abs() < 1e-30 {
            break;
        }

        let dta = (j22 * f1 - j12 * f2) / det;
        let dtb = (-j21 * f1 + j11 * f2) / det;

        ta -= dta;
        tb -= dtb;
        ta = ta.clamp(a_domain.start, a_domain.end);
        tb = tb.clamp(b_domain.start, b_domain.end);

        if dta.abs() < 1e-14 && dtb.abs() < 1e-14 {
            break;
        }
    }

    let pa = a.point_at(CurveParam(ta));
    let pb = b.point_at(CurveParam(tb));
    let dist = (pa - pb).norm();

    if dist < tolerance {
        Some(CurveCurveHit {
            point: Point3::new((pa.x+pb.x)/2.0, (pa.y+pb.y)/2.0, (pa.z+pb.z)/2.0),
            param_a: CurveParam(ta),
            param_b: CurveParam(tb),
        })
    } else {
        None
    }
}

fn sample_bbox(curve: &Curve, lo: f64, hi: f64, n: usize) -> knot_core::Aabb3 {
    let mut pts = Vec::with_capacity(n + 1);
    for i in 0..=n {
        let t = lo + (hi - lo) * i as f64 / n as f64;
        pts.push(curve.point_at(CurveParam(t)));
    }
    knot_core::Aabb3::from_points(&pts).unwrap()
}

fn deduplicate_hits(hits: &mut Vec<CurveCurveHit>, min_param_dist: f64) {
    if hits.len() <= 1 {
        return;
    }
    hits.sort_by(|a, b| a.param_a.0.partial_cmp(&b.param_a.0).unwrap());
    let mut deduped = vec![hits[0].clone()];
    for hit in &hits[1..] {
        let last = deduped.last().unwrap();
        if (hit.param_a.0 - last.param_a.0).abs() > min_param_dist {
            deduped.push(hit.clone());
        }
    }
    *hits = deduped;
}
