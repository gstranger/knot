//! Primitive shape constructors that produce valid BRep solids.

use std::sync::Arc;
use knot_core::KResult;
use knot_geom::Point3;
use knot_geom::Vector3;
use knot_geom::curve::{Curve, LineSeg};
use knot_geom::surface::{Surface, Plane, Sphere, Cylinder};
use knot_topo::*;
use std::f64::consts::{FRAC_PI_2, TAU};

/// Create a box BRep centered at the origin with given dimensions.
pub fn make_box(sx: f64, sy: f64, sz: f64) -> KResult<BRep> {
    let hx = sx / 2.0;
    let hy = sy / 2.0;
    let hz = sz / 2.0;

    // 8 vertices
    let v = [
        Arc::new(Vertex::new(Point3::new(-hx, -hy, -hz))), // 0: ---
        Arc::new(Vertex::new(Point3::new( hx, -hy, -hz))), // 1: +--
        Arc::new(Vertex::new(Point3::new( hx,  hy, -hz))), // 2: ++-
        Arc::new(Vertex::new(Point3::new(-hx,  hy, -hz))), // 3: -+-
        Arc::new(Vertex::new(Point3::new(-hx, -hy,  hz))), // 4: --+
        Arc::new(Vertex::new(Point3::new( hx, -hy,  hz))), // 5: +-+
        Arc::new(Vertex::new(Point3::new( hx,  hy,  hz))), // 6: +++
        Arc::new(Vertex::new(Point3::new(-hx,  hy,  hz))), // 7: -++
    ];

    // Helper to make a planar face from 4 vertex indices (CCW when viewed from outside)
    let make_face = |vi: [usize; 4], origin: Point3, normal: Vector3| -> KResult<Face> {
        let mut edges = Vec::new();
        for i in 0..4 {
            let j = (i + 1) % 4;
            let start = v[vi[i]].clone();
            let end = v[vi[j]].clone();
            let curve = Arc::new(Curve::Line(LineSeg::new(*start.point(), *end.point())));
            let edge = Arc::new(Edge::new(start, end, curve, 0.0, 1.0));
            edges.push(HalfEdge::new(edge, true));
        }
        let loop_ = Loop::new(edges, true)?;
        let surface = Arc::new(Surface::Plane(Plane::new(origin, normal)));
        Face::new(surface, loop_, vec![], true)
    };

    let faces = vec![
        // Bottom face (z = -hz), normal -z, CCW from outside (looking up)
        make_face([0, 3, 2, 1], Point3::new(0.0, 0.0, -hz), -Vector3::z())?,
        // Top face (z = +hz), normal +z, CCW from outside (looking down)
        make_face([4, 5, 6, 7], Point3::new(0.0, 0.0, hz), Vector3::z())?,
        // Front face (y = -hy), normal -y
        make_face([0, 1, 5, 4], Point3::new(0.0, -hy, 0.0), -Vector3::y())?,
        // Back face (y = +hy), normal +y
        make_face([2, 3, 7, 6], Point3::new(0.0, hy, 0.0), Vector3::y())?,
        // Left face (x = -hx), normal -x
        make_face([0, 4, 7, 3], Point3::new(-hx, 0.0, 0.0), -Vector3::x())?,
        // Right face (x = +hx), normal +x
        make_face([1, 2, 6, 5], Point3::new(hx, 0.0, 0.0), Vector3::x())?,
    ];

    let shell = Shell::new(faces, true)?;
    let solid = Solid::new(shell, vec![])?;
    BRep::new(vec![solid])
}

/// Create a UV-sphere BRep centered at `center` with given `radius`.
/// The sphere is divided into `n_lon` longitude and `n_lat` latitude strips.
pub fn make_sphere(center: Point3, radius: f64, n_lon: u32, n_lat: u32) -> KResult<BRep> {
    let n_lon = n_lon.max(3);
    let n_lat = n_lat.max(2);

    // Create vertices on the UV grid
    // Row 0 = south pole, row n_lat = north pole
    let mut vertices: Vec<Arc<Vertex>> = Vec::new();

    // South pole
    let south = Arc::new(Vertex::new(Point3::new(center.x, center.y, center.z - radius)));
    vertices.push(south.clone());

    // Interior rows
    for j in 1..n_lat {
        let v = -FRAC_PI_2 + (j as f64 / n_lat as f64) * std::f64::consts::PI;
        let cos_v = v.cos();
        let sin_v = v.sin();
        for i in 0..n_lon {
            let u = (i as f64 / n_lon as f64) * TAU;
            let p = Point3::new(
                center.x + radius * cos_v * u.cos(),
                center.y + radius * cos_v * u.sin(),
                center.z + radius * sin_v,
            );
            vertices.push(Arc::new(Vertex::new(p)));
        }
    }

    // North pole
    let north = Arc::new(Vertex::new(Point3::new(center.x, center.y, center.z + radius)));
    vertices.push(north.clone());

    let surface = Arc::new(Surface::Sphere(knot_geom::surface::Sphere::new(center, radius)));

    let get_vertex = |row: u32, col: u32| -> Arc<Vertex> {
        if row == 0 {
            vertices[0].clone()
        } else if row == n_lat {
            vertices.last().unwrap().clone()
        } else {
            let idx = 1 + (row as usize - 1) * n_lon as usize + (col % n_lon) as usize;
            vertices[idx].clone()
        }
    };

    let mut faces = Vec::new();

    // Bottom cap triangles (connecting to south pole)
    for i in 0..n_lon {
        let v0 = get_vertex(0, 0);
        let v1 = get_vertex(1, i);
        let v2 = get_vertex(1, i + 1);

        let edges = vec![
            make_half_edge(&v0, &v1),
            make_half_edge(&v1, &v2),
            make_half_edge(&v2, &v0),
        ];
        let loop_ = Loop::new(edges, true)?;
        faces.push(Face::new(surface.clone(), loop_, vec![], true)?);
    }

    // Middle quad strips
    for j in 1..n_lat - 1 {
        for i in 0..n_lon {
            let v00 = get_vertex(j, i);
            let v10 = get_vertex(j, i + 1);
            let v11 = get_vertex(j + 1, i + 1);
            let v01 = get_vertex(j + 1, i);

            let edges = vec![
                make_half_edge(&v00, &v10),
                make_half_edge(&v10, &v11),
                make_half_edge(&v11, &v01),
                make_half_edge(&v01, &v00),
            ];
            let loop_ = Loop::new(edges, true)?;
            faces.push(Face::new(surface.clone(), loop_, vec![], true)?);
        }
    }

    // Top cap triangles (connecting to north pole)
    for i in 0..n_lon {
        let v0 = get_vertex(n_lat - 1, i);
        let v1 = get_vertex(n_lat - 1, i + 1);
        let v2 = get_vertex(n_lat, 0);

        let edges = vec![
            make_half_edge(&v0, &v1),
            make_half_edge(&v1, &v2),
            make_half_edge(&v2, &v0),
        ];
        let loop_ = Loop::new(edges, true)?;
        faces.push(Face::new(surface.clone(), loop_, vec![], true)?);
    }

    let shell = Shell::new(faces, true)?;
    let solid = Solid::new(shell, vec![])?;
    BRep::new(vec![solid])
}

/// Create a cylinder BRep along the z-axis.
pub fn make_cylinder(center: Point3, radius: f64, height: f64, n_sides: u32) -> KResult<BRep> {
    let n = n_sides.max(3);
    let hz = height / 2.0;

    // Bottom and top ring vertices
    let mut bottom_verts = Vec::new();
    let mut top_verts = Vec::new();
    for i in 0..n {
        let angle = TAU * i as f64 / n as f64;
        let x = center.x + radius * angle.cos();
        let y = center.y + radius * angle.sin();
        bottom_verts.push(Arc::new(Vertex::new(Point3::new(x, y, center.z - hz))));
        top_verts.push(Arc::new(Vertex::new(Point3::new(x, y, center.z + hz))));
    }

    let cyl_surface = Arc::new(Surface::Cylinder(Cylinder {
        origin: center,
        axis: Vector3::z(),
        radius,
        ref_direction: Vector3::x(),
        v_min: -hz,
        v_max: hz,
    }));

    let bottom_surface = Arc::new(Surface::Plane(Plane::new(
        Point3::new(center.x, center.y, center.z - hz), -Vector3::z(),
    )));
    let top_surface = Arc::new(Surface::Plane(Plane::new(
        Point3::new(center.x, center.y, center.z + hz), Vector3::z(),
    )));

    let mut faces = Vec::new();

    // Side faces (quads)
    for i in 0..n {
        let j = (i + 1) % n;
        let v0 = &bottom_verts[i as usize];
        let v1 = &bottom_verts[j as usize];
        let v2 = &top_verts[j as usize];
        let v3 = &top_verts[i as usize];

        let edges = vec![
            make_half_edge(v0, v1),
            make_half_edge(v1, v2),
            make_half_edge(v2, v3),
            make_half_edge(v3, v0),
        ];
        let loop_ = Loop::new(edges, true)?;
        faces.push(Face::new(cyl_surface.clone(), loop_, vec![], true)?);
    }

    // Bottom face (polygon)
    let bottom_edges: Vec<HalfEdge> = (0..n).rev()
        .map(|i| {
            let j = if i == 0 { n - 1 } else { i - 1 };
            make_half_edge(&bottom_verts[i as usize], &bottom_verts[j as usize])
        })
        .collect();
    let bottom_loop = Loop::new(bottom_edges, true)?;
    faces.push(Face::new(bottom_surface, bottom_loop, vec![], true)?);

    // Top face (polygon)
    let top_edges: Vec<HalfEdge> = (0..n)
        .map(|i| {
            let j = (i + 1) % n;
            make_half_edge(&top_verts[i as usize], &top_verts[j as usize])
        })
        .collect();
    let top_loop = Loop::new(top_edges, true)?;
    faces.push(Face::new(top_surface, top_loop, vec![], true)?);

    let shell = Shell::new(faces, true)?;
    let solid = Solid::new(shell, vec![])?;
    BRep::new(vec![solid])
}

fn make_half_edge(start: &Arc<Vertex>, end: &Arc<Vertex>) -> HalfEdge {
    let curve = Arc::new(Curve::Line(LineSeg::new(*start.point(), *end.point())));
    let edge = Arc::new(Edge::new(start.clone(), end.clone(), curve, 0.0, 1.0));
    HalfEdge::new(edge, true)
}
