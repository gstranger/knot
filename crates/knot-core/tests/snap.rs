use knot_core::SnapGrid;
use nalgebra::Point3;

#[test]
fn snap_scalar() {
    let grid = SnapGrid::new(0.25);
    assert_eq!(grid.snap_scalar(0.3), 0.25);
    assert_eq!(grid.snap_scalar(0.4), 0.5);
    assert_eq!(grid.snap_scalar(1.0), 1.0);
    assert_eq!(grid.snap_scalar(-0.1), 0.0);
}

#[test]
fn snap_point() {
    let grid = SnapGrid::new(0.5);
    let p = Point3::new(0.3, 0.7, 1.1);
    let snapped = grid.snap(p);
    assert_eq!(snapped, Point3::new(0.5, 0.5, 1.0));
}

#[test]
fn snap_exact_grid_point_unchanged() {
    let grid = SnapGrid::new(1.0);
    let p = Point3::new(3.0, 4.0, 5.0);
    assert_eq!(grid.snap(p), p);
}

#[test]
#[should_panic]
fn zero_cell_size_panics() {
    SnapGrid::new(0.0);
}

#[test]
fn from_bbox_diagonal() {
    let grid = SnapGrid::from_bbox_diagonal(100.0, 1e-9);
    assert!((grid.cell_size - 1e-7).abs() < 1e-20);
}
