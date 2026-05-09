use knot_geom::Point3;
use knot_ops::primitives;
use knot_ops::boolean::{boolean, BooleanOp};

#[test]
fn trace_box_box_intersection() {
    // Two overlapping boxes — simplest possible case
    let a = primitives::make_box(2.0, 2.0, 2.0).unwrap(); // -1..1 in all axes
    let b = make_offset_box(0.5, 0.0, 0.0, 2.0, 2.0, 2.0); // -0.5..1.5 in x

    // The intersection should be x: -0.5..1.0, y: -1..1, z: -1..1
    // That's a box with 6 faces.

    // Let's check how many faces each solid has
    let a_faces = a.single_solid().unwrap().outer_shell().face_count();
    let b_faces = b.single_solid().unwrap().outer_shell().face_count();
    eprintln!("A faces: {}, B faces: {}", a_faces, b_faces);

    // Check if SSI finds intersections
    let solid_a = a.single_solid().unwrap();
    let solid_b = b.single_solid().unwrap();

    let mut ssi_count = 0;
    for fa in solid_a.outer_shell().faces() {
        for fb in solid_b.outer_shell().faces() {
            let traces = knot_intersect::surface_surface::intersect_surfaces(
                fa.surface(), fb.surface(), 1e-6
            ).unwrap();
            if !traces.is_empty() {
                ssi_count += 1;
                for trace in &traces {
                    eprintln!("SSI: {} points", trace.points.len());
                }
            }
        }
    }
    eprintln!("SSI pairs with intersections: {}", ssi_count);

    let result = boolean(&a, &b, BooleanOp::Intersection);
    match result {
        Ok(brep) => {
            eprintln!("Intersection succeeded: {} faces", brep.single_solid().unwrap().outer_shell().face_count());
        }
        Err(e) => {
            eprintln!("Intersection FAILED: {}", e);
        }
    }
}

fn make_offset_box(ox: f64, oy: f64, oz: f64, sx: f64, sy: f64, sz: f64) -> knot_topo::BRep {
    use std::sync::Arc;
    use knot_geom::Vector3;
    use knot_geom::curve::{Curve, LineSeg};
    use knot_geom::surface::{Surface, Plane};
    use knot_topo::*;
    let hx = sx/2.0; let hy = sy/2.0; let hz = sz/2.0;
    let v = [
        Arc::new(Vertex::new(Point3::new(ox-hx, oy-hy, oz-hz))),
        Arc::new(Vertex::new(Point3::new(ox+hx, oy-hy, oz-hz))),
        Arc::new(Vertex::new(Point3::new(ox+hx, oy+hy, oz-hz))),
        Arc::new(Vertex::new(Point3::new(ox-hx, oy+hy, oz-hz))),
        Arc::new(Vertex::new(Point3::new(ox-hx, oy-hy, oz+hz))),
        Arc::new(Vertex::new(Point3::new(ox+hx, oy-hy, oz+hz))),
        Arc::new(Vertex::new(Point3::new(ox+hx, oy+hy, oz+hz))),
        Arc::new(Vertex::new(Point3::new(ox-hx, oy+hy, oz+hz))),
    ];
    let make_face = |vi: [usize; 4], origin: Point3, normal: Vector3| -> Face {
        let mut edges = Vec::new();
        for i in 0..4 { let j=(i+1)%4;
            let s=v[vi[i]].clone(); let e=v[vi[j]].clone();
            let c=Arc::new(Curve::Line(LineSeg::new(*s.point(),*e.point())));
            edges.push(HalfEdge::new(Arc::new(Edge::new(s,e,c,0.0,1.0)),true));
        }
        Face::new(Arc::new(Surface::Plane(Plane::new(origin,normal))),Loop::new(edges,true).unwrap(),vec![],true).unwrap()
    };
    let faces = vec![
        make_face([0,3,2,1], Point3::new(ox,oy,oz-hz), -Vector3::z()),
        make_face([4,5,6,7], Point3::new(ox,oy,oz+hz), Vector3::z()),
        make_face([0,1,5,4], Point3::new(ox,oy-hy,oz), -Vector3::y()),
        make_face([2,3,7,6], Point3::new(ox,oy+hy,oz), Vector3::y()),
        make_face([0,4,7,3], Point3::new(ox-hx,oy,oz), -Vector3::x()),
        make_face([1,2,6,5], Point3::new(ox+hx,oy,oz), Vector3::x()),
    ];
    let shell = Shell::new(faces, true).unwrap();
    BRep::new(vec![Solid::new(shell, vec![]).unwrap()]).unwrap()
}
