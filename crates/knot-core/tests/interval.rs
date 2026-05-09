use knot_core::Interval;

#[test]
fn point_interval_has_zero_width() {
    let i = Interval::point(3.0);
    assert_eq!(i.width(), 0.0);
    assert_eq!(i.midpoint(), 3.0);
}

#[test]
fn contains_and_overlap() {
    let a = Interval::new(1.0, 5.0);
    let b = Interval::new(3.0, 7.0);
    let c = Interval::new(6.0, 8.0);

    assert!(a.contains(3.0));
    assert!(!a.contains(6.0));
    assert!(a.overlaps(&b));
    assert!(!a.overlaps(&c));
}

#[test]
fn certainly_less_and_greater() {
    let a = Interval::new(1.0, 2.0);
    let b = Interval::new(3.0, 4.0);
    assert!(a.certainly_less_than(&b));
    assert!(b.certainly_greater_than(&a));
    assert!(!b.certainly_less_than(&a));
}

#[test]
fn addition() {
    let a = Interval::new(1.0, 2.0);
    let b = Interval::new(3.0, 5.0);
    let c = a + b;
    assert_eq!(c.lo, 4.0);
    assert_eq!(c.hi, 7.0);
}

#[test]
fn subtraction() {
    let a = Interval::new(5.0, 10.0);
    let b = Interval::new(1.0, 3.0);
    let c = a - b;
    assert_eq!(c.lo, 2.0);  // 5 - 3
    assert_eq!(c.hi, 9.0);  // 10 - 1
}

#[test]
fn multiplication_positive() {
    let a = Interval::new(2.0, 3.0);
    let b = Interval::new(4.0, 5.0);
    let c = a * b;
    assert_eq!(c.lo, 8.0);
    assert_eq!(c.hi, 15.0);
}

#[test]
fn multiplication_mixed_sign() {
    let a = Interval::new(-2.0, 3.0);
    let b = Interval::new(-1.0, 4.0);
    let c = a * b;
    assert_eq!(c.lo, -8.0);  // 3 * -1 or -2 * 4
    assert_eq!(c.hi, 12.0);  // 3 * 4
}

#[test]
fn division_nonzero() {
    let a = Interval::new(6.0, 12.0);
    let b = Interval::new(2.0, 3.0);
    let c = (a / b).unwrap();
    assert_eq!(c.lo, 2.0);   // 6 / 3
    assert_eq!(c.hi, 6.0);   // 12 / 2
}

#[test]
fn division_by_zero_returns_none() {
    let a = Interval::new(1.0, 2.0);
    let b = Interval::new(-1.0, 1.0);
    assert!((a / b).is_none());
}

#[test]
fn negation() {
    let a = Interval::new(2.0, 5.0);
    let neg = -a;
    assert_eq!(neg.lo, -5.0);
    assert_eq!(neg.hi, -2.0);
}

#[test]
fn union_and_intersection() {
    let a = Interval::new(1.0, 5.0);
    let b = Interval::new(3.0, 7.0);
    let u = a.union(&b);
    assert_eq!(u.lo, 1.0);
    assert_eq!(u.hi, 7.0);

    let i = a.intersection(&b).unwrap();
    assert_eq!(i.lo, 3.0);
    assert_eq!(i.hi, 5.0);
}

#[test]
fn intersection_disjoint_returns_none() {
    let a = Interval::new(1.0, 2.0);
    let b = Interval::new(3.0, 4.0);
    assert!(a.intersection(&b).is_none());
}

#[test]
fn contains_zero() {
    assert!(Interval::new(-1.0, 1.0).contains_zero());
    assert!(Interval::new(0.0, 1.0).contains_zero());
    assert!(!Interval::new(0.1, 1.0).contains_zero());
}
