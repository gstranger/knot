use nalgebra::{Point3, Vector3};

/// Axis-aligned bounding box in 3D.
#[derive(Clone, Copy, Debug)]
pub struct Aabb3 {
    pub min: Point3<f64>,
    pub max: Point3<f64>,
}

impl Aabb3 {
    pub fn new(min: Point3<f64>, max: Point3<f64>) -> Self {
        Self { min, max }
    }

    pub fn from_points(pts: &[Point3<f64>]) -> Option<Self> {
        if pts.is_empty() {
            return None;
        }
        let mut min = pts[0];
        let mut max = pts[0];
        for p in &pts[1..] {
            min.x = min.x.min(p.x);
            min.y = min.y.min(p.y);
            min.z = min.z.min(p.z);
            max.x = max.x.max(p.x);
            max.y = max.y.max(p.y);
            max.z = max.z.max(p.z);
        }
        Some(Self { min, max })
    }

    pub fn intersects(&self, other: &Aabb3) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.y <= other.max.y
            && self.max.y >= other.min.y
            && self.min.z <= other.max.z
            && self.max.z >= other.min.z
    }

    pub fn union(&self, other: &Aabb3) -> Aabb3 {
        Aabb3 {
            min: Point3::new(
                self.min.x.min(other.min.x),
                self.min.y.min(other.min.y),
                self.min.z.min(other.min.z),
            ),
            max: Point3::new(
                self.max.x.max(other.max.x),
                self.max.y.max(other.max.y),
                self.max.z.max(other.max.z),
            ),
        }
    }

    pub fn expand(&self, margin: f64) -> Aabb3 {
        Aabb3 {
            min: Point3::new(
                self.min.x - margin,
                self.min.y - margin,
                self.min.z - margin,
            ),
            max: Point3::new(
                self.max.x + margin,
                self.max.y + margin,
                self.max.z + margin,
            ),
        }
    }

    pub fn center(&self) -> Point3<f64> {
        nalgebra::center(&self.min, &self.max)
    }

    pub fn diagonal(&self) -> Vector3<f64> {
        self.max - self.min
    }

    pub fn diagonal_length(&self) -> f64 {
        self.diagonal().norm()
    }
}
