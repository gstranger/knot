//! Algebraic SSI framework.
//!
//! Provides exact-arithmetic polynomial algebra for computing surface-surface
//! intersections via parametric substitution. The architecture:
//!
//! 1. **Bivariate polynomial layer** — sparse representation with exact rational
//!    coefficients. Supports multiplication, substitution, partial derivatives.
//!
//! 2. **Parametric substitution** — substitute one surface's rational
//!    parameterization into the other's implicit equation, producing a
//!    bivariate polynomial F(s, v) = 0 whose zero set is the intersection.
//!
//! 3. **Bernstein subdivision** — isolate roots of univariate polynomials
//!    (discriminants, resultants) and find zero-cells of bivariate polynomials.
//!
//! 4. **Quartic solver** — Ferrari's method for tracing branches once topology
//!    is determined.
//!
//! Each surface pair specialization is ~50 lines on top of this framework.

pub mod poly;
pub mod bernstein;
pub mod quartic;
pub mod univariate;
pub mod discriminant;
pub mod branch_topology;
pub mod nurbs_bridge;
pub mod analytic_subst;
pub mod nurbs_analytic;
pub mod nurbs_nurbs;
pub mod cylinder_torus;
pub mod cone_torus;
