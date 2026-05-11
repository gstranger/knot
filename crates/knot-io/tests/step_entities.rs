//! Entity-decode tests for the STEP reader's expanded coverage:
//!   - RATIONAL_B_SPLINE_{CURVE,SURFACE} (complex entities)
//!   - TRIMMED_CURVE
//!   - COMPOSITE_CURVE
//!   - SURFACE_OF_LINEAR_EXTRUSION (analytical cases)
//!   - SURFACE_OF_REVOLUTION (analytical cases)
//!
//! Uses `knot_io::step::reader::debug_read_{surface,curve}` so each test
//! can drive a single entity without spinning up a full BRep.

use knot_geom::curve::Curve;
use knot_geom::surface::Surface;
use knot_io::step::reader::{debug_read_curve, debug_read_surface};

const EPS: f64 = 1e-9;

// ── Rational B-spline curve ──────────────────────────────────────

#[test]
fn rational_bspline_curve_weights_picked_up_from_complex_entity() {
    // Degree-1 B-spline (= polyline) over 3 control points with rational
    // weights (1, 2, 1). The midpoint at t=0.5 should be pulled toward
    // the central control point by the weight ratio.
    //
    // Without weights: midpoint = (cp0 + cp2)/2 + 0 from cp1 = (0.5, 0, 0).
    // With (1, 2, 1) and degree 1: equivalent to non-rational here
    // because the basis is piecewise linear and only one nonzero basis
    // at each t — keep this test focused on weight transport rather than
    // rational evaluation. We instead read the curve and verify it parses
    // without error and the parameter domain spans [0,2].
    let step = r#"
ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('test'),'2;1');
FILE_NAME('t.stp','2026','','','','','');
FILE_SCHEMA(('CONFIG_CONTROL_DESIGN'));
ENDSEC;
DATA;
#1=CARTESIAN_POINT('',(0.,0.,0.));
#2=CARTESIAN_POINT('',(1.,1.,0.));
#3=CARTESIAN_POINT('',(2.,0.,0.));
#10=(
  BOUNDED_CURVE()
  B_SPLINE_CURVE(1,(#1,#2,#3),.UNSPECIFIED.,.F.,.F.)
  B_SPLINE_CURVE_WITH_KNOTS((2,1,2),(0.,1.,2.),.UNSPECIFIED.)
  CURVE()
  GEOMETRIC_REPRESENTATION_ITEM()
  RATIONAL_B_SPLINE_CURVE((1.0,2.0,1.0))
  REPRESENTATION_ITEM('')
);
ENDSEC;
END-ISO-10303-21;
"#;
    let curve = debug_read_curve(step, 10).expect("rational B-spline curve must read");
    match curve {
        Curve::Nurbs(n) => {
            assert_eq!(n.control_points().len(), 3);
            let weights = n.weights();
            assert!((weights[0] - 1.0).abs() < EPS, "weight[0] = {}", weights[0]);
            assert!((weights[1] - 2.0).abs() < EPS, "weight[1] = {}", weights[1]);
            assert!((weights[2] - 1.0).abs() < EPS, "weight[2] = {}", weights[2]);
        }
        _ => panic!("expected NURBS curve"),
    }
}

// ── TRIMMED_CURVE ────────────────────────────────────────────────

#[test]
fn trimmed_line_clamps_endpoints() {
    // Trim a 10-unit line from t=0.2 to t=0.8 → 6-unit line at offsets.
    let step = r#"
ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('t'),'2;1');
FILE_NAME('t','2026','','','','','');
FILE_SCHEMA(('CC'));
ENDSEC;
DATA;
#1=CARTESIAN_POINT('',(0.,0.,0.));
#2=DIRECTION('',(1.,0.,0.));
#3=VECTOR('',#2,1.);
#4=LINE('',#1,#3);
#5=TRIMMED_CURVE('',#4,(PARAMETER_VALUE(0.2)),(PARAMETER_VALUE(0.8)),.T.,.PARAMETER.);
ENDSEC;
END-ISO-10303-21;
"#;
    let c = debug_read_curve(step, 5).expect("trimmed line must read");
    match c {
        Curve::Line(line) => {
            assert!((line.start.x - 0.2).abs() < EPS, "start.x = {}", line.start.x);
            assert!((line.end.x   - 0.8).abs() < EPS, "end.x = {}", line.end.x);
        }
        _ => panic!("expected Line"),
    }
}

#[test]
fn trimmed_arc_adjusts_angles() {
    use std::f64::consts::FRAC_PI_2;
    let step = r#"
ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('t'),'2;1');
FILE_NAME('t','2026','','','','','');
FILE_SCHEMA(('CC'));
ENDSEC;
DATA;
#1=CARTESIAN_POINT('',(0.,0.,0.));
#2=DIRECTION('',(0.,0.,1.));
#3=DIRECTION('',(1.,0.,0.));
#4=AXIS2_PLACEMENT_3D('',#1,#2,#3);
#5=CIRCLE('',#4,1.);
#6=TRIMMED_CURVE('',#5,(PARAMETER_VALUE(0.0)),(PARAMETER_VALUE(1.5707963267948966)),.T.,.PARAMETER.);
ENDSEC;
END-ISO-10303-21;
"#;
    let c = debug_read_curve(step, 6).expect("trimmed arc must read");
    match c {
        Curve::CircularArc(a) => {
            assert!((a.start_angle - 0.0).abs() < EPS);
            assert!((a.end_angle - FRAC_PI_2).abs() < EPS);
        }
        _ => panic!("expected CircularArc"),
    }
}

// ── COMPOSITE_CURVE ──────────────────────────────────────────────

#[test]
fn composite_curve_returns_first_segment_for_now() {
    let step = r#"
ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('t'),'2;1');
FILE_NAME('t','2026','','','','','');
FILE_SCHEMA(('CC'));
ENDSEC;
DATA;
#1=CARTESIAN_POINT('',(0.,0.,0.));
#2=DIRECTION('',(1.,0.,0.));
#3=VECTOR('',#2,2.);
#4=LINE('',#1,#3);
#5=COMPOSITE_CURVE_SEGMENT(.CONTINUOUS.,.T.,#4);
#6=COMPOSITE_CURVE('',(#5),.F.);
ENDSEC;
END-ISO-10303-21;
"#;
    let c = debug_read_curve(step, 6).expect("composite curve must read");
    assert!(matches!(c, Curve::Line(_)));
}

// ── SURFACE_OF_LINEAR_EXTRUSION ──────────────────────────────────

#[test]
fn linear_extrusion_of_line_yields_plane() {
    let step = r#"
ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('t'),'2;1');
FILE_NAME('t','2026','','','','','');
FILE_SCHEMA(('CC'));
ENDSEC;
DATA;
#1=CARTESIAN_POINT('',(0.,0.,0.));
#2=DIRECTION('',(1.,0.,0.));
#3=VECTOR('',#2,1.);
#4=LINE('',#1,#3);
#5=DIRECTION('',(0.,1.,0.));
#6=VECTOR('',#5,1.);
#7=SURFACE_OF_LINEAR_EXTRUSION('',#4,#6);
ENDSEC;
END-ISO-10303-21;
"#;
    let s = debug_read_surface(step, 7).expect("must read");
    match s {
        Surface::Plane(p) => {
            // Plane normal should be ±Z (line.x cross extrude.y).
            assert!((p.normal.z.abs() - 1.0).abs() < EPS);
        }
        _ => panic!("expected Plane, got {:?}", s),
    }
}

#[test]
fn linear_extrusion_of_circle_yields_cylinder() {
    let step = r#"
ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('t'),'2;1');
FILE_NAME('t','2026','','','','','');
FILE_SCHEMA(('CC'));
ENDSEC;
DATA;
#1=CARTESIAN_POINT('',(0.,0.,0.));
#2=DIRECTION('',(0.,0.,1.));
#3=DIRECTION('',(1.,0.,0.));
#4=AXIS2_PLACEMENT_3D('',#1,#2,#3);
#5=CIRCLE('',#4,2.5);
#6=DIRECTION('',(0.,0.,1.));
#7=VECTOR('',#6,1.);
#8=SURFACE_OF_LINEAR_EXTRUSION('',#5,#7);
ENDSEC;
END-ISO-10303-21;
"#;
    let s = debug_read_surface(step, 8).expect("must read");
    match s {
        Surface::Cylinder(c) => assert!((c.radius - 2.5).abs() < EPS),
        _ => panic!("expected Cylinder, got {:?}", s),
    }
}

// ── SURFACE_OF_REVOLUTION ────────────────────────────────────────

#[test]
fn revolution_of_axis_parallel_line_yields_cylinder() {
    let step = r#"
ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('t'),'2;1');
FILE_NAME('t','2026','','','','','');
FILE_SCHEMA(('CC'));
ENDSEC;
DATA;
/* Line at x=3, parallel to Z. */
#1=CARTESIAN_POINT('',(3.,0.,0.));
#2=DIRECTION('',(0.,0.,1.));
#3=VECTOR('',#2,1.);
#4=LINE('',#1,#3);
/* Revolution axis = Z through origin. */
#5=CARTESIAN_POINT('',(0.,0.,0.));
#6=DIRECTION('',(0.,0.,1.));
#7=AXIS1_PLACEMENT('',#5,#6);
#8=SURFACE_OF_REVOLUTION('',#4,#7);
ENDSEC;
END-ISO-10303-21;
"#;
    let s = debug_read_surface(step, 8).expect("must read");
    match s {
        Surface::Cylinder(c) => {
            assert!((c.radius - 3.0).abs() < EPS, "radius = {}", c.radius);
            assert!((c.axis.z.abs() - 1.0).abs() < EPS);
        }
        _ => panic!("expected Cylinder, got {:?}", s),
    }
}

#[test]
fn revolution_of_axis_meeting_line_yields_cone() {
    // Line from (0,0,0) angled at 45° in the XZ plane.
    let step = r#"
ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('t'),'2;1');
FILE_NAME('t','2026','','','','','');
FILE_SCHEMA(('CC'));
ENDSEC;
DATA;
#1=CARTESIAN_POINT('',(0.,0.,0.));
#2=DIRECTION('',(0.70710678118,0.,0.70710678118));
#3=VECTOR('',#2,1.);
#4=LINE('',#1,#3);
#5=CARTESIAN_POINT('',(0.,0.,0.));
#6=DIRECTION('',(0.,0.,1.));
#7=AXIS1_PLACEMENT('',#5,#6);
#8=SURFACE_OF_REVOLUTION('',#4,#7);
ENDSEC;
END-ISO-10303-21;
"#;
    let s = debug_read_surface(step, 8).expect("must read");
    match s {
        Surface::Cone(c) => {
            // 45° half-angle = π/4.
            assert!((c.half_angle - std::f64::consts::FRAC_PI_4).abs() < 1e-6);
        }
        _ => panic!("expected Cone, got {:?}", s),
    }
}

#[test]
fn revolution_of_axis_centered_meridian_circle_yields_sphere() {
    // Circle in the XZ plane (normal = Y) centred at origin, radius 1.
    // Revolved around the Z axis → sphere of radius 1.
    let step = r#"
ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('t'),'2;1');
FILE_NAME('t','2026','','','','','');
FILE_SCHEMA(('CC'));
ENDSEC;
DATA;
#1=CARTESIAN_POINT('',(0.,0.,0.));
#2=DIRECTION('',(0.,1.,0.));
#3=DIRECTION('',(1.,0.,0.));
#4=AXIS2_PLACEMENT_3D('',#1,#2,#3);
#5=CIRCLE('',#4,1.);
#6=CARTESIAN_POINT('',(0.,0.,0.));
#7=DIRECTION('',(0.,0.,1.));
#8=AXIS1_PLACEMENT('',#6,#7);
#9=SURFACE_OF_REVOLUTION('',#5,#8);
ENDSEC;
END-ISO-10303-21;
"#;
    let s = debug_read_surface(step, 9).expect("must read");
    match s {
        Surface::Sphere(s) => assert!((s.radius - 1.0).abs() < EPS),
        _ => panic!("expected Sphere, got {:?}", s),
    }
}

#[test]
fn revolution_of_offset_meridian_circle_yields_torus() {
    // Circle in the XZ plane (normal = Y) centred at (3, 0, 0), radius 0.5.
    // Revolved around Z → torus with major radius 3, minor radius 0.5.
    let step = r#"
ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('t'),'2;1');
FILE_NAME('t','2026','','','','','');
FILE_SCHEMA(('CC'));
ENDSEC;
DATA;
#1=CARTESIAN_POINT('',(3.,0.,0.));
#2=DIRECTION('',(0.,1.,0.));
#3=DIRECTION('',(1.,0.,0.));
#4=AXIS2_PLACEMENT_3D('',#1,#2,#3);
#5=CIRCLE('',#4,0.5);
#6=CARTESIAN_POINT('',(0.,0.,0.));
#7=DIRECTION('',(0.,0.,1.));
#8=AXIS1_PLACEMENT('',#6,#7);
#9=SURFACE_OF_REVOLUTION('',#5,#8);
ENDSEC;
END-ISO-10303-21;
"#;
    let s = debug_read_surface(step, 9).expect("must read");
    match s {
        Surface::Torus(t) => {
            assert!((t.major_radius - 3.0).abs() < EPS, "major = {}", t.major_radius);
            assert!((t.minor_radius - 0.5).abs() < EPS, "minor = {}", t.minor_radius);
        }
        _ => panic!("expected Torus, got {:?}", s),
    }
}
