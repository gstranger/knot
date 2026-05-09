//! Knot — A NURBS-based CAD kernel.
//!
//! This is the facade crate that re-exports all sub-crates.

pub use knot_core as core;
pub use knot_geom as geom;
pub use knot_topo as topo;
pub use knot_intersect as intersect;
pub use knot_ops as ops;
pub use knot_tessellate as tessellate;
pub use knot_bindings as bindings;

#[cfg(feature = "io")]
pub use knot_io as io;
