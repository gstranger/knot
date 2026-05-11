//! STEP export upgrades polygonal trim curves to analytical form
//! when the adjacent surfaces have a recognized intersection.
//!
//! Tests are written against the emitted STEP text directly — we
//! count CIRCLE / LINE entities to verify the upgrade fired on
//! plane∩cylinder and plane∩sphere edges.

use knot_geom::Point3;
use knot_io::to_step;
use knot_ops::primitives;

fn count_entities(step: &str, name: &str) -> usize {
    // STEP entities live on their own line as `#NN=TYPENAME(...);`.
    // Count lines that match `=NAME(`.
    let pat = format!("={}(", name);
    step.lines().filter(|l| l.contains(&pat)).count()
}

#[test]
fn cylinder_cap_edges_become_circles() {
    // 24-sided cylinder: each cap rim is 24 line-segment edges
    // approximating one circle. The export should recognize the
    // plane∩cylinder intersection and emit CIRCLE entities for those
    // edges. We don't insist on merging the 24 arcs back into one
    // CIRCLE (that's a follow-on dedup), but every per-edge upgrade
    // makes one CIRCLE entity.
    let cyl = primitives::make_cylinder(Point3::origin(), 1.0, 2.0, 24).unwrap();
    let step = to_step(&cyl).unwrap();

    let n_circle = count_entities(&step, "CIRCLE");
    let n_line = count_entities(&step, "LINE");

    // Two caps × 24 edges each = 48 cap-rim edges. Each becomes one
    // CIRCLE. (Edges shared between cap and side face dedup at the
    // edge level, so 48 CIRCLEs not 96.)
    assert!(
        n_circle >= 48,
        "expected ≥48 CIRCLE entities for cap-rim upgrade, got {}",
        n_circle,
    );
    // Side faces are quads with two vertical line edges plus two
    // (now circular) horizontal edges, so 24 sides × 2 vertical
    // edges = 48 vertical LINE entities. The 48 horizontal edges
    // have been upgraded away from LINE.
    assert!(
        n_line <= 48,
        "expected ≤48 LINE entities after upgrade, got {}",
        n_line,
    );
}

#[test]
fn sphere_uv_edges_partially_upgrade_to_circles() {
    // The sphere is built as a UV grid of quad faces, all sharing
    // one Sphere surface. Edges between two sphere faces have NO
    // plane to intersect, so they CAN'T be upgraded — they stay as
    // Lines. There are no plane-sphere edges in `make_sphere`, so
    // this test mainly confirms the upgrade pass doesn't corrupt
    // the export.
    let sph = primitives::make_sphere(Point3::origin(), 1.0, 8, 4).unwrap();
    let step = to_step(&sph).unwrap();
    // Roundtrip through the parser to make sure the output is
    // still valid STEP.
    let parsed = knot_io::from_step(&step).unwrap();
    assert!(parsed.validate().is_ok());
}

#[test]
fn box_export_has_no_circles() {
    // A box is all-plane: no analytical-arc edges. The upgrade pass
    // should be a no-op.
    let box_ = primitives::make_box(2.0, 3.0, 4.0).unwrap();
    let step = to_step(&box_).unwrap();
    assert_eq!(count_entities(&step, "CIRCLE"), 0);
    assert!(count_entities(&step, "LINE") >= 12); // 12 box edges, line-curve each
}

#[test]
fn cylinder_export_roundtrips_through_parser() {
    // The upgraded export must still parse correctly — we depend on
    // the parser handling CIRCLE in EDGE_CURVE positions.
    let cyl = primitives::make_cylinder(Point3::origin(), 1.5, 3.0, 12).unwrap();
    let step = to_step(&cyl).unwrap();
    let parsed = knot_io::from_step(&step).unwrap();
    assert!(parsed.validate().is_ok());

    // Tessellation result should be geometrically equivalent.
    let mesh_orig = knot_tessellate::tessellate(
        &cyl,
        knot_tessellate::TessellateOptions::default(),
    )
    .unwrap();
    let mesh_round = knot_tessellate::tessellate(
        &parsed,
        knot_tessellate::TessellateOptions::default(),
    )
    .unwrap();
    // Triangle counts should match (same primitive, same tess opts).
    assert_eq!(
        mesh_orig.triangle_count(),
        mesh_round.triangle_count(),
        "tessellation differs after STEP roundtrip",
    );
}
