use knot_geom::Point3;
use knot_ops::primitives;
use knot_ops::boolean::{boolean, BooleanOp};

struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self { Self(seed) }
    fn next(&mut self) -> u64 { self.0 ^= self.0 << 13; self.0 ^= self.0 >> 7; self.0 ^= self.0 << 17; self.0 }
    fn uniform(&mut self, lo: f64, hi: f64) -> f64 { let t = (self.next() & 0xFFFFFFFF) as f64 / 0xFFFFFFFF_u64 as f64; lo + t * (hi - lo) }
    fn range(&mut self, n: usize) -> usize { (self.next() as usize) % n }
}

#[test]
fn debug_pair_14() {
    let mut rng = Rng::new(42);
    for pair_idx in 0..15 {
        let shape_a = rng.range(3);
        let _ox = rng.uniform(-2.0, 2.0); let _oy = rng.uniform(-2.0, 2.0);
        let _oz = rng.uniform(-2.0, 2.0); let size = rng.uniform(0.5, 3.0);
        // consume the shape-specific RNG calls
        match shape_a {
            0 => { rng.uniform(0.5, size); rng.uniform(0.5, size); rng.uniform(0.5, size); }
            1 => { rng.uniform(0.3, size * 0.6); rng.range(10); rng.range(5); }
            _ => { rng.uniform(0.2, size * 0.4); rng.uniform(0.5, size); rng.range(12); }
        }
        let shape_b = rng.range(3);
        let _ox = rng.uniform(-2.0, 2.0); let _oy = rng.uniform(-2.0, 2.0);
        let _oz = rng.uniform(-2.0, 2.0); let size = rng.uniform(0.5, 3.0);
        match shape_b {
            0 => { rng.uniform(0.5, size); rng.uniform(0.5, size); rng.uniform(0.5, size); }
            1 => { rng.uniform(0.3, size * 0.6); rng.range(10); rng.range(5); }
            _ => { rng.uniform(0.2, size * 0.4); rng.uniform(0.5, size); rng.range(12); }
        }

        if pair_idx == 14 {
            eprintln!("Pair 14: shape_a={}, shape_b={}", shape_a, shape_b);
        }
    }
}
