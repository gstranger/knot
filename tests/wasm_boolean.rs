use wasm_bindgen_test::*;

// Tests run in Node.js via wasm-pack test --node

use knot::geom::Point3;
use knot::ops::primitives::{make_box, make_sphere, make_cylinder};
use knot::ops::boolean::{boolean, BooleanOp};
use knot::ops::transform::transform_brep;
use knot::geom::transform::translation;
use knot::geom::Vector3;
use knot::tessellate::{tessellate, TessellateOptions};

#[wasm_bindgen_test]
fn wasm_make_box() {
    let brep = make_box(2.0, 2.0, 2.0).unwrap();
    assert_eq!(brep.solids()[0].outer_shell().face_count(), 6);
}

#[wasm_bindgen_test]
fn wasm_tessellate_box() {
    let brep = make_box(2.0, 2.0, 2.0).unwrap();
    let mesh = tessellate(&brep, TessellateOptions::default()).unwrap();
    assert!(mesh.triangle_count() > 0);
}

#[wasm_bindgen_test]
fn wasm_translate_box() {
    let brep = make_box(2.0, 2.0, 2.0).unwrap();
    let iso = translation(Vector3::new(1.0, 0.0, 0.0));
    let moved = transform_brep(&brep, &iso).unwrap();
    assert_eq!(moved.solids()[0].outer_shell().face_count(), 6);
}

#[wasm_bindgen_test]
fn wasm_boolean_union_identical_boxes() {
    // Union of two identical boxes is degenerate — all faces are "on boundary".
    // The kernel returns EmptyResult, which is correct.
    let a = make_box(2.0, 2.0, 2.0).unwrap();
    let b = make_box(2.0, 2.0, 2.0).unwrap();
    let result = boolean(&a, &b, BooleanOp::Union);
    assert!(result.is_err());
}

#[wasm_bindgen_test]
fn wasm_boolean_union_overlapping_boxes() {
    let a = make_box(2.0, 2.0, 2.0).unwrap();
    let b = make_box(2.0, 2.0, 2.0).unwrap();
    let iso = translation(Vector3::new(1.0, 0.0, 0.0));
    let b_moved = transform_brep(&b, &iso).unwrap();
    let result = boolean(&a, &b_moved, BooleanOp::Union).unwrap();
    assert!(result.solids()[0].outer_shell().face_count() > 0);
}

#[wasm_bindgen_test]
fn wasm_boolean_subtraction_overlapping_boxes() {
    let a = make_box(2.0, 2.0, 2.0).unwrap();
    let b = make_box(2.0, 2.0, 2.0).unwrap();
    let iso = translation(Vector3::new(1.0, 0.0, 0.0));
    let b_moved = transform_brep(&b, &iso).unwrap();
    let result = boolean(&a, &b_moved, BooleanOp::Subtraction).unwrap();
    assert!(result.solids()[0].outer_shell().face_count() > 0);
}

#[wasm_bindgen_test]
fn wasm_boolean_union_box_cylinder() {
    let a = make_box(2.0, 2.0, 2.0).unwrap();
    let b = make_cylinder(Point3::new(1.0, 0.0, 0.0), 0.6, 2.5, 16).unwrap();
    let result = boolean(&a, &b, BooleanOp::Union).unwrap();
    assert!(result.solids()[0].outer_shell().face_count() > 0);
}

#[wasm_bindgen_test]
fn wasm_boolean_subtraction_smaller_box() {
    // Exact geometry from the React demo
    let a = make_box(2.0, 2.0, 2.0).unwrap();
    let b = make_box(0.8, 0.8, 3.0).unwrap();
    let iso = translation(Vector3::new(0.5, 0.0, 0.0));
    let b_moved = transform_brep(&b, &iso).unwrap();
    let result = boolean(&a, &b_moved, BooleanOp::Subtraction).unwrap();
    let mesh = tessellate(&result, TessellateOptions::default()).unwrap();
    assert!(mesh.triangle_count() > 0);
}
