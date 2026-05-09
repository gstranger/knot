# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Is

Knot is a NURBS-based CAD kernel in Rust targeting WASM. It provides BREP modeling: NURBS primitives, curve/surface intersection, BREP topology (radial-edge), boolean operations, and tessellation. The kernel runs in browsers, edge workers, and native.

## Build & Test Commands

```bash
# Build (native)
cargo build

# Build WASM
wasm-pack build --target web --out-dir pkg

# Run all tests
cargo test --workspace

# Run all tests including IO (STEP import)
cargo test --workspace --features io

# Run a single test
cargo test -p knot-ops --test boolean intersection_overlapping

# Run boolean reliability harness (synthetic primitives, ~2min)
cargo test -p knot-ops --test reliability -- boolean_reliability_100 --nocapture

# Run ABC dataset harness (requires downloaded data, ~13min)
cargo test -p knot-io --test abc_harness -- --nocapture --ignored

# Serve web demo
python3 -m http.server 8080  # then open http://localhost:8080/web/
```

## Architecture

Cargo workspace with a facade crate (`knot`) re-exporting 8 sub-crates. The dependency DAG is enforced at compile time:

```
knot-bindings → knot-ops → knot-intersect → knot-geom → knot-core
                knot-ops → knot-topo       → knot-geom
                knot-tessellate → knot-topo, knot-geom
                knot-io → knot-topo, knot-geom, knot-core
```

**knot-core** — Exact rational arithmetic (`ExactRational` wrapping `malachite_q::Rational`), orientation predicates (`orient3d`, `orient2d`, `point_side_of_plane`), interval arithmetic, snap-rounding grid with `LatticeIndex` integer vertex keys, AABB, error types (`KResult<T>`), content-addressable `Id<T>`.

**knot-geom** — `Curve` and `Surface` are **enums** (not trait objects) for pattern-matching analytical fast-paths in intersection. Variants: `Curve::{Nurbs, Line, CircularArc, EllipticalArc}`, `Surface::{Nurbs, Plane, Sphere, Cylinder, Cone, Torus}`. All geometry is immutable, stored behind `Arc<[T]>`. Analytical primitives are stored as their analytical types (spheres remain spheres), not converted to NURBS.

**knot-topo** — Radial-edge BREP: `Vertex` → `Edge` → `HalfEdge` → `Loop` → `Face` → `Shell` → `Solid` → `BRep`. Faces carry `Arc<Surface>`, edges carry `Arc<Curve>`. All topology is immutable and `Arc`-shared. BRep validation uses `LatticeIndex` for all identity comparisons (no float distance thresholds).

**knot-intersect** — Curve-curve (line-line analytical, general subdivision+Newton), curve-surface (line-plane/sphere/cylinder analytical, general Newton), surface-surface (plane-plane/sphere/cylinder analytical, general marching with curvature-adaptive stepping, tangent detection, closed-loop detection).

**knot-ops** — Boolean operations (union/intersection/subtraction), primitive constructors (`make_box`, `make_sphere`, `make_cylinder`), operation history tree (`OpNode`). Boolean pipeline: bbox filter → SSI → split faces → exact classification (orient3d predicates) → select → deduplicate → snap-round → validate.

**knot-tessellate** — Fan triangulation of BRep face polygons with Newell's method normals and face ID mapping.

**knot-io** — STEP AP203/AP214 import (hand-written parser + entity-to-BRep mapper). Handles `MANIFOLD_SOLID_BREP`, `CLOSED_SHELL`, `ADVANCED_FACE`, `PLANE`, `CYLINDRICAL_SURFACE`, `SPHERICAL_SURFACE`, `CONICAL_SURFACE`, `TOROIDAL_SURFACE`, `B_SPLINE_SURFACE_WITH_KNOTS`, `LINE`, `CIRCLE`, `B_SPLINE_CURVE_WITH_KNOTS`. Also handles complex entities (`#ID = (TYPE1(...) TYPE2(...))`).

**knot-bindings** — `wasm-bindgen` API: `JsCurve`, `JsSurface`, `JsBrep`, `JsSurfaceMesh`, plus `create_box`, `create_sphere_brep`, `create_cylinder_brep`, `boolean_union/intersection/subtraction`.

## Key Invariants

- **Topology-first**: Topological decisions use exact predicates (`ExactRational`/`orient3d`) before geometry is approximated. Geometry never drives topology.
- **Snap-rounded coincidence**: All vertex identity uses `LatticeIndex` (integer lattice keys from `SnapGrid`), never f64 distance thresholds. One model-level grid, no per-entity tolerances.
- **Fail-or-correct**: Every operation returns `KResult<BRep>` — valid result or structured error. The boolean pipeline runs `validate()` on output. No silent garbage.
- **Immutable + Arc-shared**: All geometry/topology is immutable. Operations return new values. `Arc<[T]>` for structural sharing.
- **Enum geometry**: `Curve` and `Surface` are closed enums enabling exhaustive match for analytical fast-paths. If adding a new surface type, update all match arms in `knot-geom/src/surface/mod.rs` and `knot-intersect`.

## Reliability Measurement

Current baseline (ABC dataset chunk 0000): **60% boolean success rate** on real CAD, **98% on synthetic primitives**, 0 crashes. The three gaps: SSI timeouts on complex models, shared-edge topology at split boundaries, STEP import coverage. The harness reports are the ground truth for whether changes improve or regress reliability.

## knot-io is feature-gated

`knot-io` is an optional dependency of the root crate (`features = ["io"]`). This keeps STEP parsing out of the WASM binary. Tests that need it use `--features io` or are in `knot-io`'s own test directory.

## ABC Dataset

One chunk (10K STEP files) is stored in `data/abc/0000/` (gitignored). Download with `./scripts/fetch_abc_chunk.sh 0`. The ABC harness tests are `#[ignore]` — run with `--ignored`.
