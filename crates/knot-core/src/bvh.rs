//! Bounding Volume Hierarchy for spatial culling.
//!
//! A binary tree of AABBs used to accelerate overlap queries between
//! two sets of geometric objects. Each leaf holds an index into the
//! original object array; internal nodes hold the union AABB of their
//! children.
//!
//! Construction: top-down median split on the longest axis.
//! Query: simultaneous traversal of two BVHs, pruning pairs whose
//! AABBs don't overlap.

use crate::bbox::Aabb3;

/// A node in the BVH tree.
#[derive(Debug)]
enum BvhNode {
    Leaf {
        index: usize,
        bbox: Aabb3,
    },
    Internal {
        bbox: Aabb3,
        left: Box<BvhNode>,
        right: Box<BvhNode>,
    },
}

/// Bounding Volume Hierarchy over a set of AABBs.
#[derive(Debug)]
pub struct Bvh {
    root: Option<BvhNode>,
}

impl Bvh {
    /// Build a BVH from a set of (index, AABB) pairs.
    pub fn build(items: &[(usize, Aabb3)]) -> Self {
        if items.is_empty() {
            return Bvh { root: None };
        }
        let mut sorted: Vec<(usize, Aabb3)> = items.to_vec();
        Bvh {
            root: Some(build_node(&mut sorted)),
        }
    }

    /// Find all pairs (i, j) where item i from this BVH and item j from
    /// `other` have overlapping AABBs.
    pub fn find_overlapping_pairs(&self, other: &Bvh) -> Vec<(usize, usize)> {
        let mut pairs = Vec::new();
        if let (Some(a), Some(b)) = (&self.root, &other.root) {
            traverse_pair(a, b, &mut pairs);
        }
        pairs
    }

    /// Find all items in this BVH whose AABB overlaps the query box.
    pub fn query(&self, query: &Aabb3) -> Vec<usize> {
        let mut results = Vec::new();
        if let Some(root) = &self.root {
            query_node(root, query, &mut results);
        }
        results
    }
}

fn build_node(items: &mut [(usize, Aabb3)]) -> BvhNode {
    if items.len() == 1 {
        return BvhNode::Leaf {
            index: items[0].0,
            bbox: items[0].1,
        };
    }

    // Compute the union AABB
    let bbox = items.iter().fold(items[0].1, |acc, (_, b)| acc.union(b));

    // Split on the longest axis at the median
    let diag = bbox.diagonal();
    let axis = if diag.x >= diag.y && diag.x >= diag.z {
        0
    } else if diag.y >= diag.z {
        1
    } else {
        2
    };

    items.sort_by(|a, b| {
        let ca = a.1.center();
        let cb = b.1.center();
        let va = match axis { 0 => ca.x, 1 => ca.y, _ => ca.z };
        let vb = match axis { 0 => cb.x, 1 => cb.y, _ => cb.z };
        va.partial_cmp(&vb).unwrap_or(std::cmp::Ordering::Equal)
    });

    let mid = items.len() / 2;
    let (left_items, right_items) = items.split_at_mut(mid);

    BvhNode::Internal {
        bbox,
        left: Box::new(build_node(left_items)),
        right: Box::new(build_node(right_items)),
    }
}

/// Simultaneous traversal of two BVH trees to find overlapping leaf pairs.
fn traverse_pair(a: &BvhNode, b: &BvhNode, pairs: &mut Vec<(usize, usize)>) {
    let a_bbox = node_bbox(a);
    let b_bbox = node_bbox(b);

    if !a_bbox.intersects(b_bbox) {
        return;
    }

    match (a, b) {
        (BvhNode::Leaf { index: ia, .. }, BvhNode::Leaf { index: ib, .. }) => {
            pairs.push((*ia, *ib));
        }
        (BvhNode::Internal { left: al, right: ar, .. }, BvhNode::Leaf { .. }) => {
            traverse_pair(al, b, pairs);
            traverse_pair(ar, b, pairs);
        }
        (BvhNode::Leaf { .. }, BvhNode::Internal { left: bl, right: br, .. }) => {
            traverse_pair(a, bl, pairs);
            traverse_pair(a, br, pairs);
        }
        (
            BvhNode::Internal { left: al, right: ar, .. },
            BvhNode::Internal { left: bl, right: br, .. },
        ) => {
            traverse_pair(al, bl, pairs);
            traverse_pair(al, br, pairs);
            traverse_pair(ar, bl, pairs);
            traverse_pair(ar, br, pairs);
        }
    }
}

fn query_node(node: &BvhNode, query: &Aabb3, results: &mut Vec<usize>) {
    let bbox = node_bbox(node);
    if !bbox.intersects(query) {
        return;
    }
    match node {
        BvhNode::Leaf { index, .. } => results.push(*index),
        BvhNode::Internal { left, right, .. } => {
            query_node(left, query, results);
            query_node(right, query, results);
        }
    }
}

fn node_bbox(node: &BvhNode) -> &Aabb3 {
    match node {
        BvhNode::Leaf { bbox, .. } => bbox,
        BvhNode::Internal { bbox, .. } => bbox,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra::Point3;

    fn bb(x0: f64, y0: f64, z0: f64, x1: f64, y1: f64, z1: f64) -> Aabb3 {
        Aabb3::new(Point3::new(x0, y0, z0), Point3::new(x1, y1, z1))
    }

    #[test]
    fn overlapping_pairs() {
        let bvh_a = Bvh::build(&[
            (0, bb(0.0, 0.0, 0.0, 1.0, 1.0, 1.0)),
            (1, bb(5.0, 5.0, 5.0, 6.0, 6.0, 6.0)),
        ]);
        let bvh_b = Bvh::build(&[
            (0, bb(0.5, 0.5, 0.5, 1.5, 1.5, 1.5)),
            (1, bb(10.0, 10.0, 10.0, 11.0, 11.0, 11.0)),
        ]);
        let pairs = bvh_a.find_overlapping_pairs(&bvh_b);
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0], (0, 0));
    }

    #[test]
    fn no_overlap() {
        let bvh_a = Bvh::build(&[(0, bb(0.0, 0.0, 0.0, 1.0, 1.0, 1.0))]);
        let bvh_b = Bvh::build(&[(0, bb(5.0, 5.0, 5.0, 6.0, 6.0, 6.0))]);
        assert!(bvh_a.find_overlapping_pairs(&bvh_b).is_empty());
    }

    #[test]
    fn many_items() {
        // 100 items spread along x-axis, only adjacent ones overlap
        let items_a: Vec<(usize, Aabb3)> = (0..100)
            .map(|i| (i, bb(i as f64, 0.0, 0.0, i as f64 + 1.5, 1.0, 1.0)))
            .collect();
        let items_b: Vec<(usize, Aabb3)> = (0..100)
            .map(|i| (i, bb(i as f64 + 0.5, 0.0, 0.0, i as f64 + 2.0, 1.0, 1.0)))
            .collect();
        let bvh_a = Bvh::build(&items_a);
        let bvh_b = Bvh::build(&items_b);
        let pairs = bvh_a.find_overlapping_pairs(&bvh_b);
        // Each A[i] overlaps B[i] and B[i-1] (except edges)
        // Brute force would be 10000 checks; BVH should be ~200-300
        assert!(pairs.len() < 500, "BVH should cull most pairs, got {}", pairs.len());
        assert!(pairs.len() > 100, "should find real overlaps, got {}", pairs.len());
    }

    #[test]
    fn query_single() {
        let bvh = Bvh::build(&[
            (0, bb(0.0, 0.0, 0.0, 1.0, 1.0, 1.0)),
            (1, bb(5.0, 0.0, 0.0, 6.0, 1.0, 1.0)),
            (2, bb(0.5, 0.0, 0.0, 1.5, 1.0, 1.0)),
        ]);
        let hits = bvh.query(&bb(0.0, 0.0, 0.0, 0.8, 0.8, 0.8));
        assert!(hits.contains(&0));
        assert!(hits.contains(&2));
        assert!(!hits.contains(&1));
    }
}
