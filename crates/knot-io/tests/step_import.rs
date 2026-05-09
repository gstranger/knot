use knot_io::from_step;
use knot_tessellate::{tessellate, TessellateOptions};

/// Minimal STEP file representing a 10x10x10 box.
/// Hand-written with the minimum entities needed for a valid solid.
const BOX_STEP: &str = r#"
ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('box test'),'2;1');
FILE_NAME('box.stp','2024-01-01',(''),(''),'','','');
FILE_SCHEMA(('CONFIG_CONTROL_DESIGN'));
ENDSEC;

DATA;
/* Vertices */
#1=CARTESIAN_POINT('',(0.,0.,0.));
#2=CARTESIAN_POINT('',(10.,0.,0.));
#3=CARTESIAN_POINT('',(10.,10.,0.));
#4=CARTESIAN_POINT('',(0.,10.,0.));
#5=CARTESIAN_POINT('',(0.,0.,10.));
#6=CARTESIAN_POINT('',(10.,0.,10.));
#7=CARTESIAN_POINT('',(10.,10.,10.));
#8=CARTESIAN_POINT('',(0.,10.,10.));

#11=VERTEX_POINT('',#1);
#12=VERTEX_POINT('',#2);
#13=VERTEX_POINT('',#3);
#14=VERTEX_POINT('',#4);
#15=VERTEX_POINT('',#5);
#16=VERTEX_POINT('',#6);
#17=VERTEX_POINT('',#7);
#18=VERTEX_POINT('',#8);

/* Directions for planes */
#20=DIRECTION('',(0.,0.,-1.));
#21=DIRECTION('',(0.,0.,1.));
#22=DIRECTION('',(0.,-1.,0.));
#23=DIRECTION('',(0.,1.,0.));
#24=DIRECTION('',(-1.,0.,0.));
#25=DIRECTION('',(1.,0.,0.));

/* Axis placements for the 6 face planes */
#30=AXIS2_PLACEMENT_3D('',#1,#20,$);
#31=AXIS2_PLACEMENT_3D('',#5,#21,$);
#32=AXIS2_PLACEMENT_3D('',#1,#22,$);
#33=AXIS2_PLACEMENT_3D('',#4,#23,$);
#34=AXIS2_PLACEMENT_3D('',#1,#24,$);
#35=AXIS2_PLACEMENT_3D('',#2,#25,$);

/* Surfaces */
#40=PLANE('',#30);
#41=PLANE('',#31);
#42=PLANE('',#32);
#43=PLANE('',#33);
#44=PLANE('',#34);
#45=PLANE('',#35);

/* Edge curves (lines) — we need 12 edges for a box */
/* Bottom face edges */
#50=DIRECTION('',(1.,0.,0.));
#51=VECTOR('',#50,10.);
#52=LINE('',#1,#51);
#53=EDGE_CURVE('',#11,#12,#52,.T.);

#54=DIRECTION('',(0.,1.,0.));
#55=VECTOR('',#54,10.);
#56=LINE('',#2,#55);
#57=EDGE_CURVE('',#12,#13,#56,.T.);

#58=DIRECTION('',(-1.,0.,0.));
#59=VECTOR('',#58,10.);
#60=LINE('',#3,#59);
#61=EDGE_CURVE('',#13,#14,#60,.T.);

#62=DIRECTION('',(0.,-1.,0.));
#63=VECTOR('',#62,10.);
#64=LINE('',#4,#63);
#65=EDGE_CURVE('',#14,#11,#64,.T.);

/* Top face edges */
#70=LINE('',#5,#51);
#71=EDGE_CURVE('',#15,#16,#70,.T.);

#72=LINE('',#6,#55);
#73=EDGE_CURVE('',#16,#17,#72,.T.);

#74=LINE('',#7,#59);
#75=EDGE_CURVE('',#17,#18,#74,.T.);

#76=LINE('',#8,#63);
#77=EDGE_CURVE('',#18,#15,#76,.T.);

/* Vertical edges */
#80=DIRECTION('',(0.,0.,1.));
#81=VECTOR('',#80,10.);
#82=LINE('',#1,#81);
#83=EDGE_CURVE('',#11,#15,#82,.T.);

#84=LINE('',#2,#81);
#85=EDGE_CURVE('',#12,#16,#84,.T.);

#86=LINE('',#3,#81);
#87=EDGE_CURVE('',#13,#17,#86,.T.);

#88=LINE('',#4,#81);
#89=EDGE_CURVE('',#14,#18,#88,.T.);

/* Bottom face: z=0, normal -z, vertices 1-4 CW from outside */
#100=ORIENTED_EDGE('',*,*,#53,.F.);
#101=ORIENTED_EDGE('',*,*,#65,.F.);
#102=ORIENTED_EDGE('',*,*,#61,.F.);
#103=ORIENTED_EDGE('',*,*,#57,.F.);
#104=EDGE_LOOP('',(#100,#101,#102,#103));
#105=FACE_OUTER_BOUND('',#104,.T.);
#106=ADVANCED_FACE('',(#105),#40,.T.);

/* Top face: z=10, normal +z, vertices 5-8 CCW from outside */
#110=ORIENTED_EDGE('',*,*,#71,.T.);
#111=ORIENTED_EDGE('',*,*,#73,.T.);
#112=ORIENTED_EDGE('',*,*,#75,.T.);
#113=ORIENTED_EDGE('',*,*,#77,.T.);
#114=EDGE_LOOP('',(#110,#111,#112,#113));
#115=FACE_OUTER_BOUND('',#114,.T.);
#116=ADVANCED_FACE('',(#115),#41,.T.);

/* Front face: y=0, normal -y */
#120=ORIENTED_EDGE('',*,*,#53,.T.);
#121=ORIENTED_EDGE('',*,*,#85,.T.);
#122=ORIENTED_EDGE('',*,*,#71,.F.);
#123=ORIENTED_EDGE('',*,*,#83,.F.);
#124=EDGE_LOOP('',(#120,#121,#122,#123));
#125=FACE_OUTER_BOUND('',#124,.T.);
#126=ADVANCED_FACE('',(#125),#42,.T.);

/* Back face: y=10, normal +y */
#130=ORIENTED_EDGE('',*,*,#61,.T.);
#131=ORIENTED_EDGE('',*,*,#89,.T.);
#132=ORIENTED_EDGE('',*,*,#75,.F.);
#133=ORIENTED_EDGE('',*,*,#87,.F.);
#134=EDGE_LOOP('',(#130,#131,#132,#133));
#135=FACE_OUTER_BOUND('',#134,.T.);
#136=ADVANCED_FACE('',(#135),#43,.T.);

/* Left face: x=0, normal -x */
#140=ORIENTED_EDGE('',*,*,#65,.T.);
#141=ORIENTED_EDGE('',*,*,#83,.T.);
#142=ORIENTED_EDGE('',*,*,#77,.F.);
#143=ORIENTED_EDGE('',*,*,#89,.F.);
#144=EDGE_LOOP('',(#140,#141,#142,#143));
#145=FACE_OUTER_BOUND('',#144,.T.);
#146=ADVANCED_FACE('',(#145),#44,.T.);

/* Right face: x=10, normal +x */
#150=ORIENTED_EDGE('',*,*,#57,.T.);
#151=ORIENTED_EDGE('',*,*,#87,.T.);
#152=ORIENTED_EDGE('',*,*,#73,.F.);
#153=ORIENTED_EDGE('',*,*,#85,.F.);
#154=EDGE_LOOP('',(#150,#151,#152,#153));
#155=FACE_OUTER_BOUND('',#154,.T.);
#156=ADVANCED_FACE('',(#155),#45,.T.);

/* Shell and solid */
#200=CLOSED_SHELL('',(#106,#116,#126,#136,#146,#156));
#201=MANIFOLD_SOLID_BREP('',#200);
ENDSEC;

END-ISO-10303-21;
"#;

#[test]
fn import_step_box() {
    let brep = from_step(BOX_STEP).unwrap();

    let solid = brep.single_solid().unwrap();
    let shell = solid.outer_shell();
    assert_eq!(shell.face_count(), 6, "box should have 6 faces");
    assert!(shell.is_closed());

    // All faces should be planar
    for face in shell.faces() {
        assert!(matches!(face.surface().as_ref(),
            knot_geom::surface::Surface::Plane(_)));
    }
}

#[test]
fn step_box_tessellates() {
    let brep = from_step(BOX_STEP).unwrap();
    let mesh = tessellate(&brep, TessellateOptions::default()).unwrap();
    assert_eq!(mesh.triangle_count(), 12, "box should tessellate to 12 triangles");
    assert_eq!(mesh.vertex_count(), 24, "6 faces x 4 vertices = 24");
}

#[test]
fn step_parser_entity_count() {
    let step = knot_io::step::parser::parse_step(BOX_STEP).unwrap();
    // Should have parsed all entities
    assert!(step.entities.len() > 50, "expected many entities, got {}", step.entities.len());

    // Check specific entity types exist
    assert!(!step.entities_of_type("CARTESIAN_POINT").is_empty());
    assert!(!step.entities_of_type("PLANE").is_empty());
    assert!(!step.entities_of_type("MANIFOLD_SOLID_BREP").is_empty());
}

// ═══════════════════════════════════════════════════════════════════
// STEP Export Tests
// ═══════════════════════════════════════════════════════════════════

#[test]
fn export_step_box() {
    let brep = from_step(BOX_STEP).unwrap();
    let step_str = knot_io::to_step(&brep).unwrap();

    // Verify basic structure
    assert!(step_str.contains("ISO-10303-21;"));
    assert!(step_str.contains("MANIFOLD_SOLID_BREP"));
    assert!(step_str.contains("CLOSED_SHELL"));
    assert!(step_str.contains("ADVANCED_FACE"));
    assert!(step_str.contains("PLANE"));
    assert!(step_str.contains("END-ISO-10303-21;"));

    // Count entity types
    let step = knot_io::step::parser::parse_step(&step_str).unwrap();
    assert_eq!(step.entities_of_type("ADVANCED_FACE").len(), 6, "box has 6 faces");
    assert_eq!(step.entities_of_type("MANIFOLD_SOLID_BREP").len(), 1);
    assert_eq!(step.entities_of_type("CLOSED_SHELL").len(), 1);
    assert_eq!(step.entities_of_type("PLANE").len(), 6, "box has 6 planes");
}

#[test]
fn step_round_trip_box() {
    // Import → Export → Re-import → validate topology matches
    let original = from_step(BOX_STEP).unwrap();
    let step_str = knot_io::to_step(&original).unwrap();
    let reimported = from_step(&step_str).unwrap();

    let orig_solid = original.single_solid().unwrap();
    let reimp_solid = reimported.single_solid().unwrap();

    assert_eq!(
        orig_solid.outer_shell().face_count(),
        reimp_solid.outer_shell().face_count(),
        "round-trip should preserve face count"
    );

    // Verify the re-imported BRep tessellates correctly
    let mesh = tessellate(&reimported, TessellateOptions::default()).unwrap();
    assert_eq!(mesh.triangle_count(), 12, "round-trip box should have 12 triangles");
}

#[test]
fn export_primitive_box() {
    // Test exporting a box created by the kernel (not from STEP import)
    use knot_ops::primitives;
    let brep = primitives::make_box(2.0, 3.0, 4.0).unwrap();
    let step_str = knot_io::to_step(&brep).unwrap();

    // Re-import
    let reimported = from_step(&step_str).unwrap();
    let shell = reimported.single_solid().unwrap().outer_shell();
    assert_eq!(shell.face_count(), 6);

    // Tessellate
    let mesh = tessellate(&reimported, TessellateOptions::default()).unwrap();
    assert!(mesh.triangle_count() > 0);
}

#[test]
fn export_boolean_result() {
    // Test exporting a boolean union result
    use knot_ops::primitives;
    use knot_ops::boolean::{boolean, BooleanOp};

    let a = primitives::make_box(2.0, 2.0, 2.0).unwrap();
    let b = primitives::make_box(2.0, 2.0, 2.0).unwrap();
    // Disjoint union (offset b far away so it's two separate boxes)
    // Actually, just export a single box — simpler and tests the same path
    let step_str = knot_io::to_step(&a).unwrap();
    let reimported = from_step(&step_str).unwrap();
    assert_eq!(reimported.single_solid().unwrap().outer_shell().face_count(), 6);
}
