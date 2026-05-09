use std::ops::{Add, Div, Mul, Neg, Sub};

/// Interval arithmetic for validated floating-point bounds.
/// Guarantees the true value lies within [lo, hi].
#[derive(Clone, Copy, Debug)]
pub struct Interval {
    pub lo: f64,
    pub hi: f64,
}

impl Interval {
    pub fn new(lo: f64, hi: f64) -> Self {
        debug_assert!(lo <= hi, "Interval: lo ({lo}) > hi ({hi})");
        Self { lo, hi }
    }

    pub fn point(v: f64) -> Self {
        Self { lo: v, hi: v }
    }

    pub fn width(&self) -> f64 {
        self.hi - self.lo
    }

    pub fn midpoint(&self) -> f64 {
        0.5 * (self.lo + self.hi)
    }

    pub fn contains(&self, v: f64) -> bool {
        self.lo <= v && v <= self.hi
    }

    pub fn overlaps(&self, other: &Interval) -> bool {
        self.lo <= other.hi && other.lo <= self.hi
    }

    pub fn certainly_less_than(&self, other: &Interval) -> bool {
        self.hi < other.lo
    }

    pub fn certainly_greater_than(&self, other: &Interval) -> bool {
        self.lo > other.hi
    }

    pub fn contains_zero(&self) -> bool {
        self.lo <= 0.0 && self.hi >= 0.0
    }

    pub fn union(&self, other: &Interval) -> Interval {
        Interval {
            lo: self.lo.min(other.lo),
            hi: self.hi.max(other.hi),
        }
    }

    pub fn intersection(&self, other: &Interval) -> Option<Interval> {
        let lo = self.lo.max(other.lo);
        let hi = self.hi.min(other.hi);
        if lo <= hi {
            Some(Interval { lo, hi })
        } else {
            None
        }
    }
}

impl Add for Interval {
    type Output = Interval;
    fn add(self, rhs: Interval) -> Interval {
        Interval {
            lo: self.lo + rhs.lo,
            hi: self.hi + rhs.hi,
        }
    }
}

impl Sub for Interval {
    type Output = Interval;
    fn sub(self, rhs: Interval) -> Interval {
        Interval {
            lo: self.lo - rhs.hi,
            hi: self.hi - rhs.lo,
        }
    }
}

impl Mul for Interval {
    type Output = Interval;
    fn mul(self, rhs: Interval) -> Interval {
        let products = [
            self.lo * rhs.lo,
            self.lo * rhs.hi,
            self.hi * rhs.lo,
            self.hi * rhs.hi,
        ];
        Interval {
            lo: products.iter().copied().fold(f64::INFINITY, f64::min),
            hi: products.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        }
    }
}

impl Div for Interval {
    type Output = Option<Interval>;
    fn div(self, rhs: Interval) -> Option<Interval> {
        if rhs.contains_zero() {
            None
        } else {
            let inv = Interval {
                lo: 1.0 / rhs.hi,
                hi: 1.0 / rhs.lo,
            };
            Some(self * inv)
        }
    }
}

impl Neg for Interval {
    type Output = Interval;
    fn neg(self) -> Interval {
        Interval {
            lo: -self.hi,
            hi: -self.lo,
        }
    }
}
