//! Knot — A NURBS-based CAD kernel.
//!
//! This is the facade crate that re-exports all sub-crates.

/// Install a panic hook that prints Rust panic messages to the browser console.
/// Called automatically by wasm-bindgen on module init.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
fn init_panic_hook() {
    console_error_panic_hook::set_once();
}

pub use knot_core as core;
pub use knot_geom as geom;
pub use knot_topo as topo;
pub use knot_intersect as intersect;
pub use knot_ops as ops;
pub use knot_tessellate as tessellate;
pub use knot_bindings as bindings;

#[cfg(feature = "io")]
pub use knot_io as io;
