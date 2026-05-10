# Knot

A NURBS-based CAD kernel in Rust targeting WebAssembly. Provides boundary representation (BRep) modeling with exact topology, boolean operations, feature operations, and multi-format I/O. Runs in browsers, edge workers, and native.

## Features

- **BRep Modeling** -- Radial-edge topology with NURBS and analytical geometry (planes, spheres, cylinders, cones, tori)
- **Boolean Operations** -- Union, intersection, subtraction with shared-edge topology and exact rational predicates
- **Feature Operations** -- Linear extrude, revolve, constant-radius fillet, constant-distance chamfer
- **Primitives** -- Box, sphere, cylinder with configurable resolution
- **Rigid Transforms** -- Translate, rotate, scale
- **I/O** -- STEP AP203 import/export, binary/ASCII STL export, glTF 2.0 (GLB) export
- **Tessellation** -- Ear-clipping triangulation with per-vertex normals and face ID mapping
- **WASM Bindings** -- Full API via `wasm-bindgen` for browser and edge deployment
- **TypeScript/React SDK** -- `knot-cad` npm package with hooks and Three.js integration

## Quick Start

### Native (Rust)

```bash
cargo build
cargo test --workspace
```

### WebAssembly

```bash
wasm-pack build --target web --out-dir pkg
python3 -m http.server 8080  # open http://localhost:8080/web/
```

### JavaScript / TypeScript

```bash
pnpm install
pnpm build        # builds WASM + JS SDK
```

```typescript
import { createKnot } from 'knot-cad';

const knot = await createKnot();
const box = knot.box(2, 3, 4);
const cylinder = knot.cylinder(0, 0, 0, 1, 5, 32);
const result = knot.subtraction(box, cylinder);

const mesh = result.tessellate();
console.log(mesh.triangleCount); // triangle mesh ready for rendering

const step = result.toSTEP();    // STEP AP203 string
const stl = result.toSTL();      // binary STL Uint8Array
const glb = result.toGLB();      // binary glTF Uint8Array
result.free();
```

### React

```tsx
import { useKnot, useBrep, KnotMesh } from 'knot-cad/react';

function Part() {
  const knot = useKnot();
  const brep = useBrep(() => {
    if (!knot) return null;
    return knot.box(2, 2, 2);
  }, [knot]);

  return brep ? <KnotMesh brep={brep} color="steelblue" /> : null;
}
```

## Architecture

Cargo workspace with a facade crate (`knot`) re-exporting 8 sub-crates. The dependency DAG is enforced at compile time:

```
knot (facade)
 |
 +-- knot-bindings  --> knot-ops, knot-tessellate, knot-io
 +-- knot-ops       --> knot-intersect, knot-topo, knot-geom, knot-core
 +-- knot-intersect --> knot-geom, knot-core
 +-- knot-topo      --> knot-geom, knot-core
 +-- knot-tessellate--> knot-topo, knot-geom
 +-- knot-io        --> knot-topo, knot-geom, knot-core
 +-- knot-geom      --> knot-core
 +-- knot-core      (no internal deps)
```

### Crate Responsibilities

| Crate | Purpose |
|-------|---------|
| **knot-core** | Exact rational arithmetic (`ExactRational` via malachite), orientation predicates (`orient3d`, `orient2d`), snap-rounding grid with `LatticeIndex` vertex keys, interval arithmetic, AABB, error types |
| **knot-geom** | `Curve` and `Surface` enums for pattern-matching analytical fast-paths. Curves: `Line`, `CircularArc`, `EllipticalArc`, `Nurbs`. Surfaces: `Plane`, `Sphere`, `Cylinder`, `Cone`, `Torus`, `Nurbs`. All immutable, stored behind `Arc<[T]>` |
| **knot-topo** | Radial-edge BRep: `Vertex` > `Edge` > `HalfEdge` > `Loop` > `Face` > `Shell` > `Solid` > `BRep`. Faces carry `Arc<Surface>`, edges carry `Arc<Curve>`. All topology is immutable and `Arc`-shared |
| **knot-intersect** | Curve-curve, curve-surface, and surface-surface intersection. Analytical fast-paths for primitive pairs (plane-plane, plane-sphere, plane-cylinder, sphere-sphere, cylinder-cylinder). General SSI via hierarchical seed-finding + curvature-adaptive marching |
| **knot-ops** | Boolean operations, fillet/chamfer, extrude/revolve, primitives, rigid transforms. `TopologyBuilder` for snap-grid vertex/edge deduplication. Operation history tree (`OpNode`) |
| **knot-tessellate** | Ear-clipping triangulation of BRep face polygons with Newell's method normals and face ID mapping |
| **knot-io** | STEP AP203/AP214 import and export (hand-written parser + entity mapper). STL binary/ASCII export. glTF 2.0 (GLB) export |
| **knot-bindings** | `wasm-bindgen` API: `JsBrep`, `JsCurve`, `JsSurface`, `JsSurfaceMesh`, plus free functions for primitives, booleans, feature ops, transforms, and I/O |

### Key Invariants

- **Topology-first**: Topological decisions use exact predicates (`ExactRational`/`orient3d`) before geometry is approximated. Geometry never drives topology.
- **Snap-rounded coincidence**: All vertex identity uses `LatticeIndex` (integer lattice keys from `SnapGrid`), never f64 distance thresholds. One model-level grid, no per-entity tolerances.
- **Fail-or-correct**: Every operation returns `KResult<BRep>` -- valid result or structured error. The boolean pipeline runs `validate()` on output. No silent garbage.
- **Immutable + Arc-shared**: All geometry and topology is immutable. Operations return new values. `Arc<[T]>` for structural sharing.
- **Enum geometry**: `Curve` and `Surface` are closed enums enabling exhaustive match for analytical fast-paths in intersection.

## Build & Test

```bash
# Build (native)
cargo build

# Build (WASM)
wasm-pack build --target web --out-dir pkg

# Run all tests
cargo test --workspace

# Run a single crate's tests
cargo test -p knot-ops --test boolean

# Run boolean reliability harness (~2 min)
cargo test -p knot-ops --test reliability -- boolean_reliability_100 --nocapture

# Run ABC dataset harness (~13 min, requires downloaded data)
cargo test -p knot-io --test abc_harness -- --nocapture --ignored

# Build JS SDK
pnpm build

# Serve web demo
python3 -m http.server 8080
```

## STEP I/O

### Import

```rust
use knot_io::from_step;

let brep = from_step(step_file_contents)?;
let solid = brep.single_solid().unwrap();
println!("{} faces", solid.outer_shell().face_count());
```

Supported STEP entities: `MANIFOLD_SOLID_BREP`, `CLOSED_SHELL`, `ADVANCED_FACE`, `PLANE`, `CYLINDRICAL_SURFACE`, `SPHERICAL_SURFACE`, `CONICAL_SURFACE`, `TOROIDAL_SURFACE`, `B_SPLINE_SURFACE_WITH_KNOTS`, `LINE`, `CIRCLE`, `ELLIPSE`, `B_SPLINE_CURVE_WITH_KNOTS`, `SEAM_CURVE`, `SURFACE_CURVE`, `INTERSECTION_CURVE`. Complex entities (`#ID = (TYPE1(...) TYPE2(...))`) are handled.

### Export

```rust
use knot_io::to_step;

let step_string = to_step(&brep)?;
std::fs::write("output.stp", step_string)?;
```

Exports all analytical surface/curve types and NURBS with proper knot compression.

## ABC Dataset Integration Testing

The [ABC Dataset](https://deep-geometry.github.io/abc-dataset/) provides ~1 million real-world CAD models in STEP format, sourced from Onshape. Knot uses it for integration testing against production CAD data.

### Downloading

One chunk (~10,000 STEP files) is stored in `data/abc/0000/` (gitignored). Download with:

```bash
./scripts/fetch_abc_chunk.sh 0
```

This downloads chunk 0000 (~635 MB compressed) from the NYU archive, extracts with 7zip, and places files in `data/abc/0000/`. The archive is kept for re-extraction; delete it manually if disk space is a concern.

### Running the Harness

ABC tests are `#[ignore]`-gated so they don't run in normal CI. Run them explicitly:

```bash
# Import reliability: loads 200 STEP files, reports success rate
cargo test -p knot-io --test abc_harness -- abc_import_report --nocapture --ignored

# Boolean reliability: imports up to 50 models, runs pairwise booleans
cargo test -p knot-io --test abc_harness -- abc_boolean_reliability --nocapture --ignored
```

### What the Harness Measures

**Import harness** (`abc_import_report`):
- Loads 200 STEP files from the ABC chunk
- Reports import success rate, parse failures, and topology failures
- Tracks total face count and average import time per file
- Fault-tolerant: individual face/edge parse failures are skipped with warnings

**Boolean harness** (`abc_boolean_reliability`):
- Imports up to 50 models that parse successfully
- Runs pairwise boolean operations (union, intersection, subtraction) on 30 random model pairs
- Each operation has a 10-second timeout and panic-catching wrapper
- Categorizes outcomes: valid, empty (correct), topology failure, tessellation failure, crash, timeout
- Reports overall success rate

### Reliability Baselines

The harness reports are the ground truth for whether changes improve or regress reliability. Current baselines:

| Metric | Synthetic Primitives | ABC Dataset |
|--------|---------------------|-------------|
| **Success rate** | 98% (300 ops) | ~60% (90 ops) |
| **Crashes** | 0 | 0 |
| **Primary gaps** | SSI on curved shapes | SSI timeouts, STEP entity coverage |

## WASM API

The full kernel is available from JavaScript/TypeScript via `wasm-bindgen`:

### Primitives & Profiles
- `create_box(sx, sy, sz)` -- axis-aligned box centered at origin
- `create_sphere_brep(cx, cy, cz, r, n_lon, n_lat)` -- UV-sphere
- `create_cylinder_brep(cx, cy, cz, r, h, n_sides)` -- z-axis cylinder
- `create_profile(points, stride)` -- planar face from 2D/3D point array

### Feature Operations
- `extrude(profile, dx, dy, dz, distance)` -- linear extrusion
- `revolve_brep(profile, ox, oy, oz, ax, ay, az, angle)` -- revolution
- `fillet_edges(brep, edge_points, radius)` -- constant-radius fillet
- `chamfer_edges(brep, edge_points, distance)` -- constant-distance chamfer

### Boolean Operations
- `boolean_union(a, b)`, `boolean_intersection(a, b)`, `boolean_subtraction(a, b)`

### Transforms (on `JsBrep`)
- `.translate(dx, dy, dz)`, `.rotate(ax, ay, az, angle)`, `.scale(sx, sy, sz)`

### I/O
- `import_step(string)` / `export_step(brep)` -- STEP round-trip
- `.to_stl()` -- binary STL bytes
- `.to_glb()` -- binary glTF 2.0 bytes
- `.tessellate()` -- triangle mesh (positions, normals, indices)

## Web Demo

The `web/` directory contains an interactive demo with three panels:

1. **NURBS Curve** -- draggable control point weight and position, live curve evaluation
2. **BRep Primitives** -- box/cylinder/sphere with adjustable size, face and triangle counts
3. **Boolean Operations** -- union/intersection/subtraction of box and cylinder with timing

Serve locally:

```bash
wasm-pack build --target web --out-dir pkg
python3 -m http.server 8080
# open http://localhost:8080/web/
```

A richer React + react-three-fiber demo lives in `examples/react-demo/`:

```bash
pnpm install
pnpm run demo   # builds wasm + js, then runs vite dev server
```

## Continuous Integration & Deployment

Two GitHub Actions workflows live in `.github/workflows/`:

- **`ci.yml`** runs on every push to `main` and on every PR. Builds
  the workspace, runs all non-`#[ignore]`'d tests (synthetic
  primitives included), and exercises the WASM build via `wasm-pack`.
  ABC dataset diagnostics are skipped — they need ~10K STEP files
  not in the repo.

- **`deploy-demo.yml`** runs on every push to `main` and publishes
  `examples/react-demo/` to GitHub Pages.

**One-time setup to enable Pages deployment** (manual UI step,
required after the first `deploy-demo.yml` run lands):

1. Go to the repository's Settings → Pages.
2. Under "Build and deployment", set "Source" to **"GitHub Actions"**.
3. Push to `main` (or click "Run workflow" on the deploy action).
   The deployment URL appears in the workflow run summary —
   typically `https://<user>.github.io/knot/`.

The Vite config (`examples/react-demo/vite.config.ts`) reads
`BASE_URL` from the environment so asset paths match the GitHub
Pages subpath. Local `pnpm dev` runs unaffected; the env var only
needs to be set during the CI build.

## Project Structure

```
knot/
  Cargo.toml              # workspace root
  CLAUDE.md               # AI assistant instructions
  src/                    # facade crate (re-exports sub-crates)
  crates/
    knot-core/            # exact arithmetic, snap grid, errors
    knot-geom/            # curves, surfaces, transforms
    knot-topo/            # BRep topology, validation
    knot-intersect/       # curve/surface intersection
    knot-ops/             # booleans, fillet, extrude, primitives
    knot-tessellate/      # triangulation
    knot-io/              # STEP, STL, glTF I/O
    knot-bindings/        # wasm-bindgen API
  js/                     # TypeScript/React SDK (knot-cad npm package)
  web/                    # interactive demo
  scripts/                # ABC dataset download
  data/                   # ABC dataset files (gitignored)
```

## License

TBD
