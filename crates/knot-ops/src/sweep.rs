use knot_core::KResult;
use knot_geom::curve::Curve;
use knot_topo::BRep;

/// Sweep a profile along a single rail curve.
pub fn sweep_1rail(_profile: &BRep, _rail: &Curve) -> KResult<BRep> {
    Err(knot_core::KernelError::OperationFailed {
        code: knot_core::ErrorCode::UnsupportedConfiguration,
        detail: "sweep not yet implemented".into(),
    })
}

/// Sweep a profile along two rail curves.
pub fn sweep_2rail(_profile: &BRep, _rail_a: &Curve, _rail_b: &Curve) -> KResult<BRep> {
    Err(knot_core::KernelError::OperationFailed {
        code: knot_core::ErrorCode::UnsupportedConfiguration,
        detail: "sweep not yet implemented".into(),
    })
}
