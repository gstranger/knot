pub mod nurbs;
pub mod plane;
pub mod sphere;
pub mod cylinder;
pub mod cone;
pub mod torus;
pub mod fit;

use crate::point::{Point3, Vector3};

pub use nurbs::NurbsSurface;
pub use plane::Plane;
pub use sphere::Sphere;
pub use cylinder::Cylinder;
pub use cone::Cone;
pub use torus::Torus;

/// Parameter pair on a surface.
#[derive(Clone, Copy, Debug)]
pub struct SurfaceParam {
    pub u: f64,
    pub v: f64,
}

/// Derivatives at a surface point.
#[derive(Clone, Debug)]
pub struct SurfaceDerivatives {
    pub point: Point3,
    pub du: Vector3,
    pub dv: Vector3,
    pub normal: Vector3,
}

/// Surface domain.
#[derive(Clone, Copy, Debug)]
pub struct SurfaceDomain {
    pub u_start: f64,
    pub u_end: f64,
    pub v_start: f64,
    pub v_end: f64,
}

/// Closest-point query result on a surface.
#[derive(Clone, Debug)]
pub struct SurfaceClosestPoint {
    pub param: SurfaceParam,
    pub point: Point3,
    pub distance: f64,
}

/// All surface types the kernel can represent.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum Surface {
    Nurbs(NurbsSurface),
    Plane(Plane),
    Sphere(Sphere),
    Cylinder(Cylinder),
    Cone(Cone),
    Torus(Torus),
}

impl Surface {
    /// Evaluate a point on the surface.
    pub fn point_at(&self, uv: SurfaceParam) -> Point3 {
        match self {
            Surface::Nurbs(s) => s.point_at(uv.u, uv.v),
            Surface::Plane(s) => s.point_at(uv.u, uv.v),
            Surface::Sphere(s) => s.point_at(uv.u, uv.v),
            Surface::Cylinder(s) => s.point_at(uv.u, uv.v),
            Surface::Cone(s) => s.point_at(uv.u, uv.v),
            Surface::Torus(s) => s.point_at(uv.u, uv.v),
        }
    }

    /// Get the surface normal at a parameter.
    pub fn normal_at(&self, uv: SurfaceParam) -> Vector3 {
        match self {
            Surface::Nurbs(s) => s.normal_at(uv.u, uv.v),
            Surface::Plane(s) => s.normal,
            Surface::Sphere(s) => s.normal_at(uv.u, uv.v),
            Surface::Cylinder(s) => s.normal_at(uv.u, uv.v),
            Surface::Cone(s) => s.normal_at(uv.u, uv.v),
            Surface::Torus(s) => s.normal_at(uv.u, uv.v),
        }
    }

    /// Compute surface derivatives (du, dv, normal) at a parameter.
    pub fn derivatives_at(&self, uv: SurfaceParam) -> SurfaceDerivatives {
        match self {
            Surface::Plane(s) => SurfaceDerivatives {
                point: s.point_at(uv.u, uv.v),
                du: s.u_axis,
                dv: s.v_axis,
                normal: s.normal,
            },
            Surface::Sphere(s) => {
                let (u, v) = (uv.u, uv.v);
                let cos_v = v.cos();
                let sin_v = v.sin();
                let cos_u = u.cos();
                let sin_u = u.sin();
                let r = s.radius;
                SurfaceDerivatives {
                    point: s.point_at(u, v),
                    du: Vector3::new(-r * cos_v * sin_u, r * cos_v * cos_u, 0.0),
                    dv: Vector3::new(-r * sin_v * cos_u, -r * sin_v * sin_u, r * cos_v),
                    normal: s.normal_at(u, v),
                }
            }
            Surface::Cylinder(c) => {
                let b = c.axis.cross(&c.ref_direction);
                let cos_u = uv.u.cos();
                let sin_u = uv.u.sin();
                let r = c.radius;
                SurfaceDerivatives {
                    point: c.point_at(uv.u, uv.v),
                    du: Vector3::new(
                        r * (-sin_u * c.ref_direction.x + cos_u * b.x),
                        r * (-sin_u * c.ref_direction.y + cos_u * b.y),
                        r * (-sin_u * c.ref_direction.z + cos_u * b.z),
                    ),
                    dv: c.axis,
                    normal: c.normal_at(uv.u, uv.v),
                }
            }
            // For cone, torus, and NURBS, use finite differences
            _ => {
                let h = 1e-7;
                let p = self.point_at(uv);
                let domain = self.domain();
                let pu = self.point_at(SurfaceParam {
                    u: (uv.u + h).min(domain.u_end),
                    v: uv.v,
                });
                let pv = self.point_at(SurfaceParam {
                    u: uv.u,
                    v: (uv.v + h).min(domain.v_end),
                });
                let du = (pu - p) / h;
                let dv = (pv - p) / h;
                let normal = du.cross(&dv);
                let len = normal.norm();
                SurfaceDerivatives {
                    point: p,
                    du,
                    dv,
                    normal: if len > 1e-30 { normal / len } else { Vector3::z() },
                }
            }
        }
    }

    /// Get the surface domain.
    pub fn domain(&self) -> SurfaceDomain {
        match self {
            Surface::Nurbs(s) => s.domain(),
            Surface::Plane(_) => SurfaceDomain {
                u_start: f64::NEG_INFINITY,
                u_end: f64::INFINITY,
                v_start: f64::NEG_INFINITY,
                v_end: f64::INFINITY,
            },
            Surface::Sphere(_) => SurfaceDomain {
                u_start: 0.0,
                u_end: std::f64::consts::TAU,
                v_start: -std::f64::consts::FRAC_PI_2,
                v_end: std::f64::consts::FRAC_PI_2,
            },
            Surface::Cylinder(c) => SurfaceDomain {
                u_start: 0.0,
                u_end: std::f64::consts::TAU,
                v_start: c.v_min,
                v_end: c.v_max,
            },
            Surface::Cone(c) => SurfaceDomain {
                u_start: 0.0,
                u_end: std::f64::consts::TAU,
                v_start: c.v_min,
                v_end: c.v_max,
            },
            Surface::Torus(_) => SurfaceDomain {
                u_start: 0.0,
                u_end: std::f64::consts::TAU,
                v_start: 0.0,
                v_end: std::f64::consts::TAU,
            },
        }
    }
}
