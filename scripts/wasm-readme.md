# knot-wasm

Low-level WebAssembly build of the [Knot](https://github.com/gstranger/knot)
NURBS CAD kernel.

**Most users want [`knot-cad`](https://www.npmjs.com/package/knot-cad)
instead** — it wraps this package with an ergonomic TypeScript API,
typed errors, a graph-based parametric layer, and optional React /
Three.js integration.

This package exposes the raw `wasm-bindgen` handles (`JsBrep`,
`JsCurve`, `JsSurface`, …) directly. Use it when you need:

- A different JS API layer than `knot-cad` provides
- A non-React frontend
- Server-side / edge-worker WASM execution
- Manual memory management for very large models

## Install

```bash
npm install knot-wasm
# or
pnpm add knot-wasm
```

## Hello world

```ts
import init, * as knot from 'knot-wasm';

await init();

const box = knot.create_box(2, 2, 2);
const sphere = knot.create_sphere_brep(0, 0, 0, 1.2, 24, 12);
const result = knot.boolean_subtraction(box, sphere);

const mesh = result.tessellate();
console.log(mesh.triangle_count(), 'triangles');

// WASM handles are not GC'd — free explicitly:
box.free(); sphere.free(); result.free(); mesh.free();
```

## What's in the kernel

- **BRep modeling** — radial-edge topology with NURBS + analytical
  geometry (planes, spheres, cylinders, cones, tori)
- **Boolean operations** — union, intersection, subtraction, with
  coincident-solid fast-path
- **Feature operations** — linear extrude, revolve, constant-radius
  fillet, constant-distance chamfer, sweep, loft
- **Curve fitting** — exact interpolation and least-squares approximation
- **Surface fitting** — Coons patches
- **I/O** — STEP AP203/AP214 import + export with analytical trim
  curves (CIRCLE / ELLIPSE), binary STL, glTF 2.0 (GLB), CBOR
- **Tessellation** — fan-triangulated face polygons with per-vertex
  normals and face-ID mapping
- **Reliability** — 100% / 100% / 100% deterministic on the ABC
  CAD-dataset benchmark (release mode, chunk 0000)

See the [main repository](https://github.com/gstranger/knot) for the
full Rust source, kernel design notes, and the React graph-editor
demo.

## License

MIT
