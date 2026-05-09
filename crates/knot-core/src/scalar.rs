/// Absolute geometric tolerance for coincidence checks.
/// Used only for geometric approximation — never for topological decisions.
pub const TOLERANCE: f64 = 1e-10;

/// Relative tolerance for normalized comparisons.
pub const REL_TOLERANCE: f64 = 1e-12;

/// Default snap grid resolution (fraction of bounding box diagonal).
pub const DEFAULT_SNAP_RESOLUTION: f64 = 1e-9;
