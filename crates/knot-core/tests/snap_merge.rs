use knot_core::SnapGrid;
use nalgebra::Point3;

#[test]
fn snap_and_merge_collapses_nearby() {
    let grid = SnapGrid::new(0.1);
    let points = vec![
        Point3::new(0.0, 0.0, 0.0),
        Point3::new(0.03, 0.02, 0.01),  // should merge with first
        Point3::new(1.0, 0.0, 0.0),
        Point3::new(1.01, 0.0, 0.0),    // should merge with third
    ];
    let (unique, map) = grid.snap_and_merge(&points);
    assert_eq!(unique.len(), 2, "should collapse to 2 unique points, got {}", unique.len());
    assert_eq!(map[0], map[1], "first two should map to same");
    assert_eq!(map[2], map[3], "last two should map to same");
    assert_ne!(map[0], map[2], "different clusters should differ");
}

#[test]
fn snap_and_merge_preserves_distinct() {
    let grid = SnapGrid::new(0.01);
    let points = vec![
        Point3::new(0.0, 0.0, 0.0),
        Point3::new(0.5, 0.0, 0.0),
        Point3::new(1.0, 0.0, 0.0),
    ];
    let (unique, map) = grid.snap_and_merge(&points);
    assert_eq!(unique.len(), 3);
    assert_eq!(map, vec![0, 1, 2]);
}

#[test]
fn coincident_on_grid() {
    let grid = SnapGrid::new(0.5);
    let a = Point3::new(0.1, 0.1, 0.1);
    let b = Point3::new(0.2, 0.2, 0.2);
    assert!(grid.coincident(a, b), "should be coincident on coarse grid");

    let c = Point3::new(0.1, 0.1, 0.1);
    let d = Point3::new(0.6, 0.1, 0.1);
    assert!(!grid.coincident(c, d), "should not be coincident");
}

#[test]
fn snap_deterministic() {
    let grid = SnapGrid::new(0.25);
    let p = Point3::new(0.37, 0.62, 0.99);
    let s1 = grid.snap(p);
    let s2 = grid.snap(p);
    assert_eq!(s1, s2, "snapping should be deterministic");
}
