use malachite_q::Rational;

/// Exact rational number for combinatorial predicates.
/// Wraps malachite's Rational to contain the dependency.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExactRational(pub Rational);

impl ExactRational {
    pub fn from_f64(v: f64) -> Self {
        Self(Rational::try_from(v).unwrap_or_else(|_| Rational::from(0)))
    }

    pub fn to_f64(&self) -> f64 {
        use malachite_base::num::conversion::traits::RoundingFrom;
        use malachite_base::rounding_modes::RoundingMode;
        let (val, _) = f64::rounding_from(&self.0, RoundingMode::Nearest);
        val
    }

    pub fn sign(&self) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        if self.0 > Rational::from(0) {
            Ordering::Greater
        } else if self.0 < Rational::from(0) {
            Ordering::Less
        } else {
            Ordering::Equal
        }
    }

    pub fn zero() -> Self {
        Self(Rational::from(0))
    }

    pub fn abs(&self) -> Self {
        if self.0 < Rational::from(0) {
            Self(-&self.0)
        } else {
            self.clone()
        }
    }
}

impl std::ops::Add for &ExactRational {
    type Output = ExactRational;
    fn add(self, rhs: &ExactRational) -> ExactRational {
        ExactRational(&self.0 + &rhs.0)
    }
}

impl std::ops::Sub for &ExactRational {
    type Output = ExactRational;
    fn sub(self, rhs: &ExactRational) -> ExactRational {
        ExactRational(&self.0 - &rhs.0)
    }
}

impl std::ops::Mul for &ExactRational {
    type Output = ExactRational;
    fn mul(self, rhs: &ExactRational) -> ExactRational {
        ExactRational(&self.0 * &rhs.0)
    }
}

/// Exact 3D point for predicate evaluation.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ExactPoint3 {
    pub x: ExactRational,
    pub y: ExactRational,
    pub z: ExactRational,
}

impl ExactPoint3 {
    pub fn new(x: ExactRational, y: ExactRational, z: ExactRational) -> Self {
        Self { x, y, z }
    }

    pub fn from_f64(x: f64, y: f64, z: f64) -> Self {
        Self {
            x: ExactRational::from_f64(x),
            y: ExactRational::from_f64(y),
            z: ExactRational::from_f64(z),
        }
    }

    /// Exact dot product with a vector.
    pub fn dot(&self, other: &ExactPoint3) -> ExactRational {
        let xx = &self.x * &other.x;
        let yy = &self.y * &other.y;
        let zz = &self.z * &other.z;
        &(&xx + &yy) + &zz
    }

    /// Exact subtraction (self - other).
    pub fn sub(&self, other: &ExactPoint3) -> ExactPoint3 {
        ExactPoint3 {
            x: &self.x - &other.x,
            y: &self.y - &other.y,
            z: &self.z - &other.z,
        }
    }

    /// Exact cross product.
    pub fn cross(&self, other: &ExactPoint3) -> ExactPoint3 {
        ExactPoint3 {
            x: &(&self.y * &other.z) - &(&self.z * &other.y),
            y: &(&self.z * &other.x) - &(&self.x * &other.z),
            z: &(&self.x * &other.y) - &(&self.y * &other.x),
        }
    }
}

/// Result of an orientation predicate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Orientation {
    Positive,
    Zero,
    Negative,
}

/// 3D orientation predicate: determines the orientation of point d
/// with respect to the plane defined by a, b, c.
pub fn orient3d(
    a: &ExactPoint3,
    b: &ExactPoint3,
    c: &ExactPoint3,
    d: &ExactPoint3,
) -> Orientation {
    let ax = &a.x.0 - &d.x.0;
    let ay = &a.y.0 - &d.y.0;
    let az = &a.z.0 - &d.z.0;
    let bx = &b.x.0 - &d.x.0;
    let by = &b.y.0 - &d.y.0;
    let bz = &b.z.0 - &d.z.0;
    let cx = &c.x.0 - &d.x.0;
    let cy = &c.y.0 - &d.y.0;
    let cz = &c.z.0 - &d.z.0;

    let det = &ax * (&by * &cz - &bz * &cy)
        - &ay * (&bx * &cz - &bz * &cx)
        + &az * (&bx * &cy - &by * &cx);

    let zero = Rational::from(0);
    if det > zero {
        Orientation::Positive
    } else if det < zero {
        Orientation::Negative
    } else {
        Orientation::Zero
    }
}

/// 2D orientation predicate.
pub fn orient2d(
    a: &[ExactRational; 2],
    b: &[ExactRational; 2],
    c: &[ExactRational; 2],
) -> Orientation {
    let det = (&a[0].0 - &c[0].0) * (&b[1].0 - &c[1].0)
        - (&a[1].0 - &c[1].0) * (&b[0].0 - &c[0].0);

    let zero = Rational::from(0);
    if det > zero {
        Orientation::Positive
    } else if det < zero {
        Orientation::Negative
    } else {
        Orientation::Zero
    }
}

/// Exact point-on-which-side-of-plane predicate.
/// Given a plane defined by (origin, normal) and a query point,
/// returns Positive if the point is on the normal side, Negative if opposite, Zero if on.
///
/// This is the fundamental predicate for face classification in booleans.
pub fn point_side_of_plane(
    plane_origin: &ExactPoint3,
    plane_normal: &ExactPoint3,
    query: &ExactPoint3,
) -> Orientation {
    let diff = query.sub(plane_origin);
    let dot = diff.dot(plane_normal);
    dot.sign().into()
}

impl From<std::cmp::Ordering> for Orientation {
    fn from(ord: std::cmp::Ordering) -> Self {
        match ord {
            std::cmp::Ordering::Greater => Orientation::Positive,
            std::cmp::Ordering::Less => Orientation::Negative,
            std::cmp::Ordering::Equal => Orientation::Zero,
        }
    }
}
