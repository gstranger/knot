pub mod algebraic;
pub mod curve_curve;
pub mod curve_surface;
pub mod surface_surface;

use knot_geom::Point3;
use knot_geom::curve::CurveParam;
use knot_geom::surface::SurfaceParam;

/// Result of a curve-curve intersection.
#[derive(Clone, Debug)]
pub struct CurveCurveHit {
    pub point: Point3,
    pub param_a: CurveParam,
    pub param_b: CurveParam,
}

/// Result of a curve-surface intersection.
#[derive(Clone, Debug)]
pub struct CurveSurfaceHit {
    pub point: Point3,
    pub curve_param: CurveParam,
    pub surface_param: SurfaceParam,
}

/// Result of a surface-surface intersection: a traced intersection curve.
#[derive(Clone, Debug)]
pub struct SurfaceSurfaceTrace {
    pub points: Vec<Point3>,
    pub params_a: Vec<SurfaceParam>,
    pub params_b: Vec<SurfaceParam>,
}
