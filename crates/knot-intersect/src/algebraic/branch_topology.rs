//! Topology-aware branch connector for quartic-in-v polynomials F(s,v)=0.
//!
//! The discriminant of F (treated as a quartic in v with coefficients in s)
//! partitions the s-axis into stable intervals where the count of real
//! v-roots is constant and their sort order is preserved. Within an
//! interval, the k-th sorted root is a continuous function of s — a
//! "slot." This module traces those slots and stitches them together
//! across critical s-values into curve chains.
//!
//! The connection rule at a critical point: two adjacent (in sort order)
//! roots merge into a double root and either turn back (the U-turn case,
//! count drops by 2) or pass through as a tangent (no count change,
//! handled implicitly by the stable-interval treatment). When count
//! changes by 2k, k disjoint adjacent pairs of roots on the larger side
//! form U-turns; the remaining roots match the smaller side in sort
//! order. The U-turn pairs are chosen by minimum mismatch with the
//! smaller-side values (or by minimum gap when no smaller-side data).
//!
//! Walking the resulting graph gives polylines in (s, v) space. Open
//! ends fall at the s-window boundary; closed loops fall out naturally.

use super::poly::BiPoly;
use super::quartic::solve_univariate;
use std::collections::HashMap;

/// Quartic root sample at one s value. Roots are sorted ascending so
/// that "slot k" identifies a unique continuous branch within a stable
/// interval.
#[derive(Debug, Clone)]
struct Sample {
    s: f64,
    roots: Vec<f64>,
}

/// A run of samples between two consecutive critical s-values where
/// the real-root count is constant. Within an interval, slot k is a
/// continuous function of s, so tracing reduces to a per-slot polyline.
#[derive(Debug, Clone)]
struct StableInterval {
    s_lo: f64,
    s_hi: f64,
    /// Samples sorted by s ascending. All have `slot_count` roots.
    /// Empty if the interval has no real roots.
    samples: Vec<Sample>,
    /// Number of real roots, constant within the interval. May be 0.
    slot_count: usize,
}

/// One slot's trajectory through one stable interval, recorded as
/// (s, v) points in s-ascending order.
#[derive(Debug, Clone)]
struct Segment {
    points: Vec<(f64, f64)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Side {
    /// First point in the segment (smallest s).
    Start,
    /// Last point in the segment (largest s).
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct EndpointId {
    segment: usize,
    side: Side,
}

/// One unit in a boundary matching: each pair tells us how endpoints at
/// the boundary connect. Slot indices are in their respective intervals'
/// sorted-ascending order.
#[derive(Debug, Clone, Copy)]
enum MatchPair {
    /// Left interval's slot connects through the boundary into right
    /// interval's slot. The intersection curve passes through s_crit.
    PassThrough { left: usize, right: usize },
    /// Two slots in the LEFT interval form a U-turn at the boundary —
    /// their right ends meet at s_crit and the curve turns back.
    /// (Real-root count drops crossing the boundary.)
    LeftUTurn { a: usize, b: usize },
    /// Two slots in the RIGHT interval form a U-turn at the boundary —
    /// they emerge together from a single double root at s_crit.
    /// (Real-root count rises crossing the boundary.)
    RightUTurn { a: usize, b: usize },
}

/// Number of interior samples per stable interval. 40 is comfortable
/// for typical CAD intersections (curves of low-order curvature) and
/// keeps Ferrari calls bounded.
const INTERIOR_SAMPLES: usize = 40;

/// Fraction of interval length used as inset δ when sampling near
/// critical points. The first and last interior samples sit at
/// s_lo + δ and s_hi - δ, well clear of the merger.
const BOUNDARY_INSET_FRAC: f64 = 0.005;

/// Lower bound on δ in absolute units. Prevents the inset from
/// collapsing to zero when intervals are extremely short.
const BOUNDARY_INSET_MIN: f64 = 1e-6;

/// Maximum allowed gap between consecutive slot values within an
/// interval. Beyond this, the sort order has likely been disrupted by
/// a missed critical event; the segment is split to avoid mis-matching
/// continuous branches across the discontinuity.
const SLOT_JUMP_GUARD: f64 = 1.0;

/// Trace branches of F(s,v)=0 across all stable intervals, threading
/// connections through critical s-values, and clip output to the
/// requested v-window. Returns one (s,v) polyline per intersection
/// curve. Closed loops are emitted as polylines whose first and last
/// points coincide; open chains are not closed.
pub fn trace_branches_topology(
    v_coeffs: &[(u32, BiPoly)],
    s_range: f64,
    v_min: f64,
    v_max: f64,
    tolerance: f64,
) -> Vec<Vec<(f64, f64)>> {
    if v_coeffs.is_empty() {
        return Vec::new();
    }

    let critical = super::discriminant::find_critical_s_values(v_coeffs, s_range);
    let bounds = build_interval_bounds(s_range, &critical);
    let intervals = build_intervals(v_coeffs, &bounds);
    let (segments, partner) = build_topology_graph(&intervals);
    let raw_chains = extract_chains(&segments, &partner);

    let mut clipped = Vec::new();
    for chain in raw_chains {
        for piece in clip_chain_to_v_range(&chain, v_min, v_max, tolerance) {
            if piece.len() >= 3 {
                clipped.push(piece);
            }
        }
    }
    clipped
}

fn build_interval_bounds(s_range: f64, critical: &[f64]) -> Vec<f64> {
    let mut bounds = vec![-s_range];
    for &c in critical {
        if c > -s_range && c < s_range {
            bounds.push(c);
        }
    }
    bounds.push(s_range);
    bounds.sort_by(|a, b| a.partial_cmp(b).unwrap());
    bounds.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
    bounds
}

fn build_intervals(v_coeffs: &[(u32, BiPoly)], bounds: &[f64]) -> Vec<StableInterval> {
    let mut intervals = Vec::with_capacity(bounds.len().saturating_sub(1));
    for win in bounds.windows(2) {
        let s_lo = win[0];
        let s_hi = win[1];
        let len = s_hi - s_lo;
        if len <= 0.0 {
            intervals.push(StableInterval { s_lo, s_hi, samples: Vec::new(), slot_count: 0 });
            continue;
        }
        let inset = (len * BOUNDARY_INSET_FRAC).max(BOUNDARY_INSET_MIN).min(len * 0.45);
        let interior_lo = s_lo + inset;
        let interior_hi = s_hi - inset;

        let mut samples = Vec::with_capacity(INTERIOR_SAMPLES);
        for i in 0..INTERIOR_SAMPLES {
            let t = if INTERIOR_SAMPLES == 1 {
                0.5
            } else {
                i as f64 / (INTERIOR_SAMPLES - 1) as f64
            };
            let s = interior_lo + t * (interior_hi - interior_lo);
            let coeffs = eval_v_coeffs_at_s(v_coeffs, s);
            let mut roots = solve_univariate(&coeffs);
            roots.sort_by(|a, b| a.partial_cmp(b).unwrap());
            samples.push(Sample { s, roots });
        }

        let stable_count = mode_root_count(&samples);
        let samples: Vec<Sample> = samples
            .into_iter()
            .filter(|sm| sm.roots.len() == stable_count)
            .collect();

        let slot_count = if samples.is_empty() { 0 } else { stable_count };
        intervals.push(StableInterval { s_lo, s_hi, samples, slot_count });
    }
    intervals
}

fn mode_root_count(samples: &[Sample]) -> usize {
    let mut counts: HashMap<usize, usize> = HashMap::new();
    for s in samples {
        *counts.entry(s.roots.len()).or_insert(0) += 1;
    }
    counts.into_iter().max_by_key(|&(_, c)| c).map(|(k, _)| k).unwrap_or(0)
}

/// Build per-slot segments and the endpoint partner map that captures
/// boundary matchings (pass-through and U-turn pairings).
fn build_topology_graph(
    intervals: &[StableInterval],
) -> (Vec<Segment>, HashMap<EndpointId, EndpointId>) {
    let mut segments: Vec<Segment> = Vec::new();
    // For each interval, the slot→segment_id mapping. Inner Vec is keyed
    // by slot index. May contain multiple split sub-segments for one
    // slot if the SLOT_JUMP_GUARD trips; in that case slot_count entries
    // each contain the LAST sub-segment created (the boundary one). Sub-
    // segments before/after splits are still in `segments` but only
    // their endpoints near the interval boundaries can be matched.
    let mut interval_to_segments: Vec<Vec<usize>> = Vec::with_capacity(intervals.len());

    for iv in intervals {
        if iv.samples.is_empty() || iv.slot_count == 0 {
            interval_to_segments.push(Vec::new());
            continue;
        }
        let mut slot_segs = Vec::with_capacity(iv.slot_count);
        for slot in 0..iv.slot_count {
            // Each slot becomes one segment within this interval.
            let pts: Vec<(f64, f64)> =
                iv.samples.iter().map(|sm| (sm.s, sm.roots[slot])).collect();

            // Defensive split: if a single jump within the slot exceeds
            // SLOT_JUMP_GUARD, sort order was disrupted (probably by a
            // missed critical event). Keep only the sub-segment that
            // touches s_hi — that's the one whose right endpoint must
            // match the next boundary correctly. Earlier sub-segments
            // become standalone open arcs.
            let split = split_on_jumps(&pts, SLOT_JUMP_GUARD);
            let mut last_id = usize::MAX;
            for sub in split {
                if sub.len() >= 2 {
                    last_id = segments.len();
                    segments.push(Segment { points: sub });
                }
            }
            // If splitting produced nothing, fall back to the raw
            // points so the slot still has one segment to match.
            if last_id == usize::MAX {
                last_id = segments.len();
                segments.push(Segment { points: pts });
            }
            slot_segs.push(last_id);
        }
        interval_to_segments.push(slot_segs);
    }

    let mut partner: HashMap<EndpointId, EndpointId> = HashMap::new();

    for i in 0..intervals.len().saturating_sub(1) {
        connect_at_boundary(
            &intervals[i],
            &intervals[i + 1],
            &interval_to_segments[i],
            &interval_to_segments[i + 1],
            &segments,
            &mut partner,
        );
    }

    (segments, partner)
}

/// Build the matching across one critical-point boundary and write the
/// resulting endpoint pairings into `partner`.
fn connect_at_boundary(
    left_iv: &StableInterval,
    right_iv: &StableInterval,
    left_segs: &[usize],
    right_segs: &[usize],
    segments: &[Segment],
    partner: &mut HashMap<EndpointId, EndpointId>,
) {
    let l_count = left_iv.slot_count;
    let r_count = right_iv.slot_count;

    if l_count == 0 && r_count == 0 {
        return;
    }
    if l_count == 0 {
        // Right-side U-turns only — pair adjacent right slots greedily.
        let pairs = pair_all_adjacent(r_count);
        for (a, b) in pairs {
            insert_pair(
                partner,
                EndpointId { segment: right_segs[a], side: Side::Start },
                EndpointId { segment: right_segs[b], side: Side::Start },
            );
        }
        return;
    }
    if r_count == 0 {
        let pairs = pair_all_adjacent(l_count);
        for (a, b) in pairs {
            insert_pair(
                partner,
                EndpointId { segment: left_segs[a], side: Side::End },
                EndpointId { segment: left_segs[b], side: Side::End },
            );
        }
        return;
    }

    // Use values at the boundary-facing ends of each interval's segments.
    let left_vals: Vec<f64> = left_segs
        .iter()
        .map(|&seg| segments[seg].points.last().unwrap().1)
        .collect();
    let right_vals: Vec<f64> = right_segs
        .iter()
        .map(|&seg| segments[seg].points.first().unwrap().1)
        .collect();

    let matching = build_boundary_matching(&left_vals, &right_vals);

    for pair in matching {
        match pair {
            MatchPair::PassThrough { left, right } => {
                insert_pair(
                    partner,
                    EndpointId { segment: left_segs[left], side: Side::End },
                    EndpointId { segment: right_segs[right], side: Side::Start },
                );
            }
            MatchPair::LeftUTurn { a, b } => {
                insert_pair(
                    partner,
                    EndpointId { segment: left_segs[a], side: Side::End },
                    EndpointId { segment: left_segs[b], side: Side::End },
                );
            }
            MatchPair::RightUTurn { a, b } => {
                insert_pair(
                    partner,
                    EndpointId { segment: right_segs[a], side: Side::Start },
                    EndpointId { segment: right_segs[b], side: Side::Start },
                );
            }
        }
    }
}

fn insert_pair(
    partner: &mut HashMap<EndpointId, EndpointId>,
    a: EndpointId,
    b: EndpointId,
) {
    if a == b { return; }
    partner.insert(a, b);
    partner.insert(b, a);
}

/// Pick the boundary matching that best aligns the larger side's
/// remaining roots with the smaller side's values, after subtracting
/// adjacent U-turn pairs. Same-count case is pure pass-through by
/// sorted order. Difference must be even (single critical event with
/// generic codimension 1).
fn build_boundary_matching(left: &[f64], right: &[f64]) -> Vec<MatchPair> {
    let l = left.len();
    let r = right.len();
    if l == r {
        return (0..l)
            .map(|i| MatchPair::PassThrough { left: i, right: i })
            .collect();
    }

    let (big, small, big_is_left) = if l > r {
        (left, right, true)
    } else {
        (right, left, false)
    };
    let extras = big.len() - small.len();
    debug_assert!(extras % 2 == 0, "odd root-count change at boundary");
    let n_pairs = extras / 2;

    let u_pairs = find_uturn_pairs(big, small, n_pairs);

    let used: std::collections::HashSet<usize> =
        u_pairs.iter().flat_map(|&(a, b)| [a, b]).collect();
    let remaining: Vec<usize> = (0..big.len()).filter(|i| !used.contains(i)).collect();

    let mut result = Vec::with_capacity(u_pairs.len() + remaining.len().min(small.len()));

    for &(a, b) in &u_pairs {
        if big_is_left {
            result.push(MatchPair::LeftUTurn { a, b });
        } else {
            result.push(MatchPair::RightUTurn { a, b });
        }
    }

    let n_pass = remaining.len().min(small.len());
    for i in 0..n_pass {
        let big_slot = remaining[i];
        let small_slot = i;
        if big_is_left {
            result.push(MatchPair::PassThrough { left: big_slot, right: small_slot });
        } else {
            result.push(MatchPair::PassThrough { left: small_slot, right: big_slot });
        }
    }
    result
}

/// Choose `n_pairs` disjoint adjacent index pairs from `big` such that
/// the un-paired remainder matches `small` in sort order with minimum
/// total absolute error. When `small` is empty, fall back to greedy
/// minimum-gap pairing on `big`.
fn find_uturn_pairs(big: &[f64], small: &[f64], n_pairs: usize) -> Vec<(usize, usize)> {
    if n_pairs == 0 {
        return Vec::new();
    }
    let n = big.len();
    if n < 2 * n_pairs {
        return Vec::new();
    }

    if small.is_empty() {
        return greedy_min_gap_pairs(big, n_pairs);
    }

    let candidates = generate_disjoint_adjacent_pair_sets(n, n_pairs);
    let mut best: Option<(f64, Vec<(usize, usize)>)> = None;
    for cand in candidates {
        let used: std::collections::HashSet<usize> =
            cand.iter().flat_map(|&(a, b)| [a, b]).collect();
        let remaining: Vec<f64> =
            (0..n).filter(|i| !used.contains(i)).map(|i| big[i]).collect();
        if remaining.len() != small.len() {
            continue;
        }
        let dist: f64 = remaining
            .iter()
            .zip(small.iter())
            .map(|(a, b)| (a - b).abs())
            .sum();
        match &best {
            Some((d, _)) if *d <= dist => {}
            _ => best = Some((dist, cand)),
        }
    }
    best.map(|(_, c)| c).unwrap_or_else(|| greedy_min_gap_pairs(big, n_pairs))
}

fn greedy_min_gap_pairs(big: &[f64], n_pairs: usize) -> Vec<(usize, usize)> {
    let n = big.len();
    if n < 2 {
        return Vec::new();
    }
    let mut gaps: Vec<(f64, usize)> =
        (0..n - 1).map(|i| ((big[i + 1] - big[i]).abs(), i)).collect();
    gaps.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let mut chosen = Vec::with_capacity(n_pairs);
    let mut used = vec![false; n];
    for (_, i) in gaps {
        if used[i] || used[i + 1] {
            continue;
        }
        chosen.push((i, i + 1));
        used[i] = true;
        used[i + 1] = true;
        if chosen.len() == n_pairs {
            break;
        }
    }
    chosen
}

/// Enumerate all sets of `k` disjoint adjacent-index pairs (i, i+1)
/// drawn from {0,...,n-1}. For typical quartic intersections n ≤ 4
/// and k ≤ 2, so the candidate count is small.
fn generate_disjoint_adjacent_pair_sets(n: usize, k: usize) -> Vec<Vec<(usize, usize)>> {
    fn helper(
        n: usize,
        k: usize,
        start: usize,
        current: &mut Vec<(usize, usize)>,
        result: &mut Vec<Vec<(usize, usize)>>,
    ) {
        if k == 0 {
            result.push(current.clone());
            return;
        }
        let mut i = start;
        while i + 1 < n {
            current.push((i, i + 1));
            helper(n, k - 1, i + 2, current, result);
            current.pop();
            i += 1;
        }
    }
    let mut result = Vec::new();
    let mut current = Vec::new();
    helper(n, k, 0, &mut current, &mut result);
    result
}

fn pair_all_adjacent(n: usize) -> Vec<(usize, usize)> {
    let mut pairs = Vec::with_capacity(n / 2);
    let mut i = 0;
    while i + 1 < n {
        pairs.push((i, i + 1));
        i += 2;
    }
    pairs
}

/// Split a slot's per-sample point list into sub-runs whenever |Δv|
/// between consecutive samples exceeds `max_jump`. Used as a
/// defensive guard against missed critical points causing a sort-
/// order swap mid-interval.
fn split_on_jumps(pts: &[(f64, f64)], max_jump: f64) -> Vec<Vec<(f64, f64)>> {
    if pts.is_empty() {
        return Vec::new();
    }
    let mut result = Vec::new();
    let mut current = vec![pts[0]];
    for w in pts.windows(2) {
        let (_, v_prev) = w[0];
        let (s, v) = w[1];
        if (v - v_prev).abs() > max_jump {
            if current.len() >= 1 {
                result.push(std::mem::take(&mut current));
            }
        }
        current.push((s, v));
    }
    if !current.is_empty() {
        result.push(current);
    }
    result
}

/// Walk the segment graph: emit a polyline for every connected chain.
/// Open chains start at unmatched endpoints (the s-window edges or
/// orphans from odd-count empty-side boundaries). Closed loops fall
/// out by visiting any remaining unvisited segment.
fn extract_chains(
    segments: &[Segment],
    partner: &HashMap<EndpointId, EndpointId>,
) -> Vec<Vec<(f64, f64)>> {
    let mut visited = vec![false; segments.len()];
    let mut chains = Vec::new();

    // Pass 1: chains rooted at open endpoints. We scan Start sides first
    // so that open-on-the-left chains get traced left-to-right.
    for seg_id in 0..segments.len() {
        if visited[seg_id] {
            continue;
        }
        let start_open =
            !partner.contains_key(&EndpointId { segment: seg_id, side: Side::Start });
        if start_open {
            let chain = walk_chain(seg_id, true, segments, partner, &mut visited);
            if !chain.is_empty() {
                chains.push(chain);
            }
        }
    }
    for seg_id in 0..segments.len() {
        if visited[seg_id] {
            continue;
        }
        let end_open =
            !partner.contains_key(&EndpointId { segment: seg_id, side: Side::End });
        if end_open {
            let chain = walk_chain(seg_id, false, segments, partner, &mut visited);
            if !chain.is_empty() {
                chains.push(chain);
            }
        }
    }

    // Pass 2: closed loops — anything left.
    for seg_id in 0..segments.len() {
        if visited[seg_id] {
            continue;
        }
        let chain = walk_chain(seg_id, true, segments, partner, &mut visited);
        if !chain.is_empty() {
            chains.push(chain);
        }
    }

    chains
}

fn walk_chain(
    start_seg: usize,
    forward: bool,
    segments: &[Segment],
    partner: &HashMap<EndpointId, EndpointId>,
    visited: &mut [bool],
) -> Vec<(f64, f64)> {
    let mut chain: Vec<(f64, f64)> = Vec::new();
    let mut cur = start_seg;
    let mut current_forward = forward;
    let start_segment = start_seg;
    let mut hit_loop = false;

    loop {
        if visited[cur] {
            // Closed loop: re-visiting the start. Mark closed by ensuring
            // the chain end equals the chain start (caller may re-snap).
            if cur == start_segment && !chain.is_empty() {
                hit_loop = true;
            }
            break;
        }
        visited[cur] = true;
        let pts = &segments[cur].points;

        // Append the segment's points in the chosen direction. Skip the
        // first point if it duplicates the chain's current tail (avoids
        // duplicate vertices at boundary U-turns and pass-throughs).
        if current_forward {
            for (i, &p) in pts.iter().enumerate() {
                if i == 0 && !chain.is_empty() && approx_eq_pt(*chain.last().unwrap(), p) {
                    continue;
                }
                chain.push(p);
            }
        } else {
            for (i, p) in pts.iter().rev().enumerate() {
                if i == 0 && !chain.is_empty() && approx_eq_pt(*chain.last().unwrap(), *p) {
                    continue;
                }
                chain.push(*p);
            }
        }

        // Find the "exit" endpoint and follow its partner.
        let exit_side = if current_forward { Side::End } else { Side::Start };
        let exit = EndpointId { segment: cur, side: exit_side };
        let next = match partner.get(&exit) {
            Some(p) => *p,
            None => break, // open end — chain terminates
        };
        cur = next.segment;
        current_forward = matches!(next.side, Side::Start);
    }

    // Close the polyline if we returned to the start (closed loop).
    if hit_loop && chain.len() >= 2 {
        let first = chain[0];
        let last = *chain.last().unwrap();
        if !approx_eq_pt(first, last) {
            chain.push(first);
        }
    }
    chain
}

fn approx_eq_pt(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < 1e-10 && (a.1 - b.1).abs() < 1e-9
}

/// Trim a chain to the [v_min, v_max] window, splitting at the
/// crossings. The tolerance widens the in-range test (so points
/// slightly outside aren't dropped over numerical noise), but the
/// crossing point itself snaps to the nominal v_min / v_max so the
/// result respects the surface's parametric domain exactly.
fn clip_chain_to_v_range(
    chain: &[(f64, f64)],
    v_min: f64,
    v_max: f64,
    tolerance: f64,
) -> Vec<Vec<(f64, f64)>> {
    if chain.is_empty() {
        return Vec::new();
    }
    let lo = v_min - tolerance;
    let hi = v_max + tolerance;
    let in_range = |v: f64| v >= lo && v <= hi;

    let mut result: Vec<Vec<(f64, f64)>> = Vec::new();
    let mut current: Vec<(f64, f64)> = Vec::new();

    for i in 0..chain.len() {
        let (s, v) = chain[i];
        if in_range(v) {
            if current.is_empty() && i > 0 {
                let (sp, vp) = chain[i - 1];
                if !in_range(vp) {
                    if let Some(cross) = interp_cross(sp, vp, s, v, v_min, v_max) {
                        current.push(cross);
                    }
                }
            }
            current.push((s, v));
        } else if !current.is_empty() {
            let (sp, vp) = chain[i - 1];
            if let Some(cross) = interp_cross(sp, vp, s, v, v_min, v_max) {
                current.push(cross);
            }
            result.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        result.push(current);
    }
    result
}

fn interp_cross(
    s0: f64,
    v0: f64,
    s1: f64,
    v1: f64,
    v_min: f64,
    v_max: f64,
) -> Option<(f64, f64)> {
    if (v1 - v0).abs() < 1e-15 {
        return None;
    }
    // Choose the nominal boundary that the segment actually crosses.
    let target = if v0 < v_min || v1 < v_min {
        v_min
    } else if v0 > v_max || v1 > v_max {
        v_max
    } else {
        return None;
    };
    let t = (target - v0) / (v1 - v0);
    if !(0.0..=1.0).contains(&t) {
        return None;
    }
    Some((s0 + t * (s1 - s0), target))
}

/// Evaluate F(s, v) at fixed s, returning the resulting univariate
/// polynomial in v as a coefficient vector (ascending power order).
/// Length is `max_v_degree + 1`. Used by the topology tracer to feed
/// the per-sample root finder.
fn eval_v_coeffs_at_s(v_coeffs: &[(u32, BiPoly)], s: f64) -> Vec<f64> {
    let max_deg = v_coeffs
        .iter()
        .map(|(d, _)| *d as usize)
        .max()
        .unwrap_or(0);
    let mut result = vec![0.0f64; max_deg + 1];
    for (deg, poly) in v_coeffs {
        result[*deg as usize] = poly.eval_f64(s, 0.0);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use malachite_q::Rational;

    /// F(s,v) = v² - s.
    /// On s > 0 the quartic (which is a quadratic here) has 2 real
    /// roots ±√s; on s < 0 it has zero. The branch should be a
    /// parabola opening to the right that turns at s=0.
    #[test]
    fn parabola_uturn_at_origin() {
        let v_coeffs = vec![
            (0u32, BiPoly::x().scale(&Rational::from(-1))),
            (1, BiPoly::zero()),
            (2, BiPoly::from_f64(1.0)),
        ];
        let chains = trace_branches_topology(&v_coeffs, 5.0, -10.0, 10.0, 1e-6);
        assert!(!chains.is_empty(), "should produce at least one chain");
        // Total point count should be substantial (full parabola arc).
        let total: usize = chains.iter().map(|c| c.len()).sum();
        assert!(total >= 30, "parabola arc should have many points, got {}", total);
        // Every point must satisfy v² ≈ s (the polynomial)
        for chain in &chains {
            for &(s, v) in chain {
                let resid = v * v - s;
                assert!(resid.abs() < 1e-6, "point ({}, {}) off curve: {}", s, v, resid);
            }
        }
    }

    /// F(s,v) = v² - 1: two horizontal lines at v = ±1, no critical
    /// points. Expect two open chains spanning the full s window.
    #[test]
    fn two_parallel_branches_no_critical_points() {
        let v_coeffs = vec![
            (0u32, BiPoly::from_f64(-1.0)),
            (1, BiPoly::zero()),
            (2, BiPoly::from_f64(1.0)),
        ];
        let chains = trace_branches_topology(&v_coeffs, 5.0, -10.0, 10.0, 1e-6);
        assert_eq!(chains.len(), 2, "expected 2 chains, got {}", chains.len());
        // Each chain is roughly horizontal at v=±1.
        for chain in &chains {
            let v0 = chain[0].1;
            assert!((v0.abs() - 1.0).abs() < 1e-6, "chain v not ≈ ±1: {}", v0);
            for &(_, v) in chain {
                assert!((v - v0).abs() < 1e-6, "chain not horizontal");
            }
        }
    }

    /// F(s,v) = (v² - 1)(v² - s²): four branches v = ±1 and v = ±s.
    /// At s = ±1 two pairs of branches cross. The crossings are
    /// transversal (not mergers), so the discriminant is zero there
    /// but the count of real roots stays the same. The traced chains
    /// should preserve continuity through the crossings.
    #[test]
    fn lines_with_transversal_crossings() {
        // F = v⁴ - (1 + s²) v² + s²
        let one = BiPoly::from_f64(1.0);
        let s_sq = &BiPoly::x() * &BiPoly::x();
        let one_plus_s2 = &one + &s_sq;
        let v_coeffs = vec![
            (0u32, s_sq.clone()),
            (1, BiPoly::zero()),
            (2, one_plus_s2.scale(&Rational::from(-1))),
            (3, BiPoly::zero()),
            (4, BiPoly::from_f64(1.0)),
        ];
        let chains = trace_branches_topology(&v_coeffs, 3.0, -10.0, 10.0, 1e-6);
        assert!(!chains.is_empty());
        // Every output point must satisfy F(s,v) = 0.
        let f_full = {
            let v = BiPoly::y();
            let v2 = &v * &v;
            let v4 = &v2 * &v2;
            &(&v4 - &(&one_plus_s2 * &v2)) + &s_sq
        };
        for chain in &chains {
            for &(s, v) in chain {
                let r = f_full.eval_f64(s, v);
                assert!(r.abs() < 1e-4, "off-curve point ({}, {}) F={}", s, v, r);
            }
        }
    }

    #[test]
    fn boundary_matching_pure_passthrough() {
        let m = build_boundary_matching(&[1.0, 2.0, 3.0], &[1.1, 2.05, 2.95]);
        assert_eq!(m.len(), 3);
        for p in m {
            match p {
                MatchPair::PassThrough { left, right } => assert_eq!(left, right),
                _ => panic!("expected pass-through"),
            }
        }
    }

    #[test]
    fn boundary_matching_left_uturn_at_top() {
        // Left has 4, right has 2. Top pair (slots 2,3) is the U-turn —
        // remaining (slots 0,1 with values 0.0, 1.0) match right
        // (0.05, 1.05).
        let m = build_boundary_matching(&[0.0, 1.0, 4.5, 5.0], &[0.05, 1.05]);
        let has_uturn_2_3 = m.iter().any(|p| matches!(p, MatchPair::LeftUTurn { a: 2, b: 3 }));
        assert!(has_uturn_2_3, "expected LeftUTurn(2,3): {:?}", m);
        // Pass-throughs: 0→0, 1→1
        let pt0 = m.iter().any(|p| matches!(p, MatchPair::PassThrough { left: 0, right: 0 }));
        let pt1 = m.iter().any(|p| matches!(p, MatchPair::PassThrough { left: 1, right: 1 }));
        assert!(pt0 && pt1, "missing pass-throughs: {:?}", m);
    }

    #[test]
    fn boundary_matching_left_uturn_in_middle() {
        // Left 4, right 2. The middle pair (1,2) merged.
        let m = build_boundary_matching(&[0.0, 4.0, 4.1, 8.0], &[0.05, 8.05]);
        let has_uturn_1_2 = m.iter().any(|p| matches!(p, MatchPair::LeftUTurn { a: 1, b: 2 }));
        assert!(has_uturn_1_2, "expected LeftUTurn(1,2): {:?}", m);
        let pt00 = m.iter().any(|p| matches!(p, MatchPair::PassThrough { left: 0, right: 0 }));
        let pt31 = m.iter().any(|p| matches!(p, MatchPair::PassThrough { left: 3, right: 1 }));
        assert!(pt00 && pt31, "expected pass-throughs 0→0 and 3→1: {:?}", m);
    }

    #[test]
    fn boundary_matching_right_uturn_born() {
        // Left 0, right 2 — both right slots are part of one U-turn.
        // (Tested as a corner case via 2→4 transition.)
        let m = build_boundary_matching(&[0.0, 5.0], &[0.0, 2.4, 2.6, 5.0]);
        let has_right_uturn = m.iter().any(|p| matches!(p, MatchPair::RightUTurn { a: 1, b: 2 }));
        assert!(has_right_uturn, "expected RightUTurn(1,2): {:?}", m);
    }

    #[test]
    fn enumerate_pair_sets_n4_k1() {
        let sets = generate_disjoint_adjacent_pair_sets(4, 1);
        assert_eq!(sets.len(), 3);
        assert!(sets.contains(&vec![(0, 1)]));
        assert!(sets.contains(&vec![(1, 2)]));
        assert!(sets.contains(&vec![(2, 3)]));
    }

    #[test]
    fn enumerate_pair_sets_n4_k2() {
        let sets = generate_disjoint_adjacent_pair_sets(4, 2);
        assert_eq!(sets.len(), 1);
        assert_eq!(sets[0], vec![(0, 1), (2, 3)]);
    }

    #[test]
    fn enumerate_pair_sets_n2_k1() {
        let sets = generate_disjoint_adjacent_pair_sets(2, 1);
        assert_eq!(sets, vec![vec![(0, 1)]]);
    }

    #[test]
    fn clip_chain_strictly_inside() {
        let chain = vec![(0.0, 1.0), (1.0, 1.5), (2.0, 2.0)];
        let pieces = clip_chain_to_v_range(&chain, 0.0, 5.0, 1e-9);
        assert_eq!(pieces.len(), 1);
        assert_eq!(pieces[0].len(), 3);
    }

    #[test]
    fn clip_chain_exits_top() {
        let chain = vec![(0.0, 0.5), (1.0, 1.5), (2.0, 2.5)];
        let pieces = clip_chain_to_v_range(&chain, 0.0, 1.0, 1e-9);
        assert_eq!(pieces.len(), 1);
        // Last point should be the interpolated crossing at v=1.
        assert!((pieces[0].last().unwrap().1 - 1.0).abs() < 1e-9);
    }

    #[test]
    fn clip_chain_dipped_below_then_back() {
        let chain = vec![(0.0, 0.5), (1.0, -0.5), (2.0, 0.5)];
        let pieces = clip_chain_to_v_range(&chain, 0.0, 5.0, 1e-9);
        // Should split into two pieces — leaves and re-enters the range.
        assert_eq!(pieces.len(), 2);
    }

    #[test]
    fn split_on_jumps_basic() {
        let pts = vec![(0.0, 0.0), (1.0, 0.1), (2.0, 5.0), (3.0, 5.1)];
        let runs = split_on_jumps(&pts, 1.0);
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].len(), 2);
        assert_eq!(runs[1].len(), 2);
    }
}
