# Boolean Pipeline End-to-End

This document describes how `boolean(a, b, op)` works on a pair of BReps,
from STEP file on disk through to a tessellatable output BRep. It
covers the data structures, the eight-stage pipeline, the algebraic
SSI subsystem, and the acceleration / safety infrastructure that
keeps the kernel deterministic and bounded on real CAD.

The audience is someone who needs to debug a failure, extend a stage,
or understand why a particular design choice exists. The "Why" lines
are usually the load-bearing part — the *what* is recoverable from the
code.

---

## What this kernel does

`knot` is a NURBS-based BREP CAD kernel. Its core operation is

```
fn boolean(a: &BRep, b: &BRep, op: BooleanOp) -> KResult<BRep>
```

where `op ∈ {Union, Intersection, Subtraction}`. The result is a
single new BRep representing the requested set operation on the two
input solids' interiors.

The kernel's contracts:

- **Topology-first**. Every topological identity decision (does this
  vertex equal that one? does this edge already exist?) uses **exact
  arithmetic** on integer lattice indices, not f64 distance
  thresholds. There is no "epsilon" in the topology layer.
- **Fail-or-correct**. Every operation either returns a well-formed
  BRep or returns a structured `KernelError`. There is no half-built
  intermediate that "mostly works."
- **Immutable + Arc-shared**. All geometry and topology is immutable;
  operations return new values. `Arc<[T]>` provides cheap structural
  sharing for control nets, vertex lists, etc.
- **Bounded latency**. The boolean has an 8-second wall-clock
  pipeline budget and a 200ms per-call SSI budget. When exceeded,
  it returns `OperationTimeout` (E404) cleanly. Pathological inputs
  fail fast rather than wedging the caller.

---

## Pipeline overview

```
                    ┌─────────────────────────────┐
                    │   boolean(a, b, op)         │
                    └─────────────┬───────────────┘
                                  │
                ┌─────────────────┼─────────────────┐
                │                 │                 │
                ▼                 ▼                 ▼
            input A           input B            BooleanOp
        (BRep, validated)  (BRep, validated)
                │                 │
                └────────┬────────┘
                         │
                         ▼
        ┌───────────────────────────────────────┐
        │ Step 1. Snap-grid setup               │
        │   bbox-derived 1e-9 tolerance grid    │
        │   TopologyBuilder (BTreeMap-backed)   │
        └───────────────────┬───────────────────┘
                            │
                            ▼
        ┌───────────────────────────────────────┐
        │ Step 2. Face-pair filter              │
        │   per-face vertex bbox + BVH overlap  │
        │   O((n_a + n_b) log n + k) candidates │
        └───────────────────┬───────────────────┘
                            │
                            ▼
        ┌───────────────────────────────────────┐
        │ Step 3. SSI loop (per candidate pair) │
        │   intersect_surfaces() dispatched by  │
        │   surface-pair type:                  │
        │     analytic-vs-analytic → fast path  │
        │     NURBS-vs-analytic → algebraic     │
        │     NURBS-vs-NURBS    → marcher       │
        │   200ms per-call wall budget          │
        └───────────────────┬───────────────────┘
                            │
                            ▼
        ┌───────────────────────────────────────┐
        │ Step 4–5. split_faces                 │
        │   split each face along incident SSI  │
        │   curves into sub-faces; share edges  │
        │   via TopologyBuilder                 │
        └───────────────────┬───────────────────┘
                            │
                            ▼
        ┌───────────────────────────────────────┐
        │ Step 6. classify_face_with            │
        │   point-in-solid (ray cast) per       │
        │   sub-face against precomputed        │
        │   SolidClassifier (triangulation +    │
        │   BVH for the other solid)            │
        └───────────────────┬───────────────────┘
                            │
                            ▼
        ┌───────────────────────────────────────┐
        │ Step 7. Select faces per op           │
        │   Union: outside-both                 │
        │   Intersection: inside-both           │
        │   Subtraction: A-outside ∪ B-inside   │
        │                (B-inside flipped)     │
        └───────────────────┬───────────────────┘
                            │
                            ▼
        ┌───────────────────────────────────────┐
        │ Step 7b. Deduplicate coplanar faces   │
        │   handles shared boundary cases       │
        └───────────────────┬───────────────────┘
                            │
                            ▼
        ┌───────────────────────────────────────┐
        │ Step 8–9. Assemble + validate         │
        │   Shell::new → Solid::new → BRep      │
        │   validate() with soft-accept on      │
        │   NonManifoldEdge / EulerViolation    │
        └───────────────────┬───────────────────┘
                            │
                            ▼
                       Output BRep
```

Each stage is bounded — the per-call SSI budget caps stage 3, the
pipeline-wide deadline caps the total. If any stage exceeds, the
boolean returns `E404 OperationTimeout` rather than running
indefinitely.

---

## Stage-by-stage detail

### 0. Pre-flight: input validation

```rust
pub fn boolean(a: &BRep, b: &BRep, op: BooleanOp) -> KResult<BRep>
```

The outer wrapper clears the thread-local BezierPatch cache (defined
in `algebraic::nurbs_bridge`) at entry and exit so cached pointer
keys are guaranteed live for the cache's lifetime. Then it dispatches
to `boolean_inner`.

Input validation:
- Both BReps must contain a single `Solid` (`single_solid()`).
- Each is run through `validate()`; **EulerViolation** errors propagate
  back as `InvalidInput` with code `E204` (the harness counts these
  as `bad_input` — graceful rejection of source-data corruption).
- `NonManifoldEdge` errors are tolerated on input; CAD imports often
  have these and most boolean configs handle them.

### 1. Snap-grid setup

```rust
let bbox  = compute_brep_bbox(solid_a, solid_b);
let grid  = SnapGrid::from_bbox_diagonal(bbox.diagonal_length(), 1e-9);
let tolerance = grid.cell_size * 100.0;
let mut builder = TopologyBuilder::new(grid);
```

A single model-level `SnapGrid` is derived from the combined input
bbox. Cell size is `1e-9 × diagonal` — fine enough that distinct CAD
features remain distinct, coarse enough to absorb ULP drift between
independently-computed surface evaluations.

`TopologyBuilder` is a vertex/edge factory keyed by lattice index.
Built on `BTreeMap` (not `HashMap`) for deterministic iteration order
— eliminating one source of run-to-run variance in the boolean's
downstream behavior.

The tolerance value (`grid.cell_size * 100`) is the geometric tolerance
used by SSI calls and other downstream code. It's coupled to the
snap grid so that a value the grid considers "the same vertex" is
also a value the geometry considers "close enough."

### 2. Face-pair filter

```rust
fn find_candidate_pairs(faces_a: &[&Face], faces_b: &[&Face], tolerance: f64)
    -> Vec<(usize, usize)>
```

Naïve O(n²) bbox-vs-bbox intersection would be ~12ms even for 250×250
faces, but on dense models where the inputs heavily overlap (66K raw
pairs in the worst-case ABC pair) it eats meaningful time relative
to the 8s budget. Uses `knot_core::Bvh` on each side's face bboxes,
then `find_overlapping_pairs` for `O((n_a + n_b) log n + k)`
behavior.

Per-face bbox is currently `face.outer_loop().half_edges()` start
vertices, expanded by tolerance. Survival rates measured on the ABC
dataset's pathological pairs:
- (11, 12): 4.6% (2967 / 64009) — filter doing real work
- (32, 33): 0.3% (347 / 116440) — filter highly effective

The filter result is the iteration set for stage 3.

### 3. SSI: surface-surface intersection

```rust
let traces = intersect_surfaces(&surface_a, &surface_b, tolerance)?;
```

The most algorithmically rich stage. Lives in
`crates/knot-intersect/src/surface_surface.rs` as a typed dispatcher:

```rust
match (a, b) {
    // Analytic-vs-analytic fast paths
    (Plane,    Plane)    => plane_plane(...),
    (Plane,    Sphere)   => plane_sphere(...),
    (Plane,    Cylinder) => plane_cylinder(...),
    (Plane,    Cone)     => plane_cone(...),
    (Plane,    Torus)    => plane_torus(...),
    (Sphere,   Sphere)   => sphere_sphere(...),
    (Cylinder, Cylinder) => cylinder_cylinder(...),
    (Cylinder, Cone)     => cylinder_cone(...),
    (Cylinder, Torus)    => cylinder_torus(...),
    (Cone,     Cone)     => cone_cone(...),
    (Cone,     Torus)    => cone_torus(...),
    (Torus,    Torus)    => torus_torus(...),

    // NURBS-vs-analytic via algebraic substitution (gated fallthrough)
    (Nurbs,    Plane)    => nurbs_analytic_or_fallback(...),
    (Nurbs,    Sphere)   => ...,
    (Nurbs,    Cylinder) => ...,
    (Nurbs,    Cone)     => ...,
    (Nurbs,    Torus)    => ...,

    // NURBS-vs-NURBS → general marching (Phase 2B.4 will replace)
    _ => general_ssi(a, b, tolerance),
}
```

Each branch is symmetric in `(a, b)` — the dispatcher table handles
both orderings. Output is a `Vec<SurfaceSurfaceTrace>`, one trace
per intersection-curve component, each containing the 3D points along
the curve plus the parameter coordinates on each surface.

Three classes of dispatch:

**(a) Analytic-vs-analytic fast paths.** Closed-form geometry — for
`plane-plane`, the intersection line is `n_a × n_b` parameterized
through a chosen base point; for `sphere-sphere`, a circle in the
perpendicular bisector plane; etc. These run in microseconds.
Some fall through to the marcher when the closed-form case doesn't
apply (oblique plane-torus, non-coaxial cone-cone).

**(b) NURBS-vs-analytic via algebraic substitution.** The big new
subsystem — see the dedicated section below. Each NURBS surface is
decomposed to Bézier patches; each patch's `(X, Y, Z, W)` polynomial
form is substituted into the analytic surface's implicit equation
to produce a bivariate polynomial `G(u, v)` whose zero set is the
intersection in the patch's parameter space. The topology connector
traces the zero set into curve chains.

**(c) `general_ssi`: Newton-marcher fallback.** For surface pairs we
don't have an analytic or algebraic path for. Lives in
`surface_surface.rs::general_ssi`. The algorithm:
1. **Seed-find**: sample surface A on a 10×10 grid, Newton-project
   each sample onto B with a 4×4 starting grid, keep pairs whose
   projection distance is below threshold (`tolerance * 50`).
2. **Refine seeds**: Newton iteration on the joint system
   `P_a(u_a, v_a) = P_b(u_b, v_b)` to find exact intersection points.
   Detect tangent-vs-transversal at each.
3. **March** from each non-tangent seed in both directions along the
   intersection-curve tangent (`n_a × n_b`), with curvature-adaptive
   step sizing and closed-loop detection.

Per-call wall-clock cap of **200ms** (`GENERAL_SSI_BUDGET_MS`).
Marcher honors a deadline check between seed iterations so a single
pathological pair can't consume the boolean's whole budget.

### 4–5. Split faces

```rust
let split_a = split_faces(&faces_a, &intersections, true,  tolerance, &mut builder);
let split_b = split_faces(&faces_b, &intersections, false, tolerance, &mut builder);
```

For each face that has incident SSI curves, split the face along
those curves into sub-faces. The `TopologyBuilder` allocates
intersection-edge entities that are shared between the two sides
(opposite half-edge orientations) so the output BRep has consistent
shared-edge topology. This stage is fast (<10ms typically) — splits
are linear in incident curve count.

### 6. Cell classification

```rust
let classifier_b = SolidClassifier::new(solid_b);
let classifier_a = SolidClassifier::new(solid_a);
let classified_a = split_a.into_iter()
    .map(|f| (f.clone(), classify_face_with(&f, &classifier_b)))
    .collect();
```

Each sub-face is classified as `Inside` or `Outside` the *other*
solid. Implementation: take the sub-face's centroid offset slightly
inward along the face normal, ray-cast in +x direction, count
triangle crossings against the other solid's faces.

The `SolidClassifier` is the critical optimization here. It
pre-computes:
- **Per-face triangulation** (planar fan or curved-surface
  centroid-fan with the centroid Newton-projected onto the actual
  surface).
- **Per-face axis-aligned bbox** of the triangulation.
- **`knot_core::Bvh`** over those bboxes.

The classify path uses two-stage culling: a BVH query against the
ray segment's bounding box (first-pass O(log F) cull), then a
Kay-Kajiya segment-vs-bbox **slab test** per surviving face (catches
faces inside the segment's wide AABB but not actually on the
segment's path).

A subtle wrinkle worth flagging: `exact_ray_triangle` casts a
deliberately off-axis segment of direction `(1e6, 3e5, 1e5)` — the
off-axis bias avoids degenerate alignment with axis-aligned face
boundaries common in CAD. The segment's bounding box is therefore
huge (covers an octant from the test point), so naive bbox-only
culling under-rejects. Slab test gets the cull rate back. An earlier
version used an axis-aligned y/z prefilter that worked in expectation
but broke on the actual off-axis segment, misclassifying axis-aligned
inputs. The slab test is the correct generalization.

This turns the per-call cost from `O(F)` triangulations + ray tests
into `O(F)` *amortized* triangulation (built once per solid, not per
classify call) plus `O(log F + hits)` ray tests via BVH + slab
culling. On the ABC pathological pair (32, 33), this dropped classify
from **23.9 seconds to ~5 seconds** — the single biggest reliability
unblock in the project.

### 7. Per-op face selection

```rust
match op {
    Union        => { keep A.outside ∪ B.outside }
    Intersection => { keep A.inside  ∪ B.inside }
    Subtraction  => { keep A.outside ∪ flip(B.inside, skip-A-coplanar) }
}
```

Subtraction is asymmetric: B-faces are flipped to point outward, and
B-faces coplanar with any A-face are skipped (A's trimmed face
already covers that region).

### 7b. Coplanar dedup

`deduplicate_faces(&selected, &grid)` removes duplicate coplanar
faces with identical lattice-index boundaries. Catches the case where
intersection / subtraction produces two copies of the same face
from different solids at the shared boundary.

### 8–9. Assemble + validate

```rust
let shell  = Shell::new(selected, expect_closed)?;
let solid  = Solid::new(shell, vec![])?;
let result = BRep::new(vec![solid])?;

if let Err(e) = result.validate() { ... soft-accept policy ... }
```

`expect_closed` is true if any intersections were found (output
should be a closed shell) and false otherwise (e.g., disjoint inputs
→ output is two open shells of two solids).

**Output validation soft-accept policy.** `validate()` runs the
five-check topology audit (loop closure, edge-vertex consistency,
shared-edge counting, Euler-Poincaré). The output handler hard-fails
on `LoopNotClosed` and `DanglingReference` (the half-edge graph is
corrupt and downstream can't proceed) but **soft-accepts**
`NonManifoldEdge` and `EulerViolation`. These represent global
topology defects but per-face geometry is intact and tessellation
produces a usable mesh. Callers needing strict-manifold output can
re-run `validate()`.

---

## The algebraic SSI subsystem

`crates/knot-intersect/src/algebraic/` is a multi-module subsystem
that turns parametric-vs-implicit and parametric-vs-parametric
surface intersection into bivariate polynomial root tracing. The
modules build on each other:

```
poly.rs              ── Sparse bivariate polynomial (BiPoly) with
                        exact rational coefficients.
                              ▼
quartic.rs           ── solve_quartic (Ferrari) and solve_univariate
bernstein.rs            (Bernstein subdivision for degree > 4) for
univariate.rs           the per-s-sample root-finding step.
                              ▼
discriminant.rs      ── find_critical_s_values: numerical root-count
                        sweep + (for quartic) closed-form Δ sign change.
                              ▼
branch_topology.rs   ── Topology-aware branch connector. Within stable
                        intervals between critical s-values, sort
                        roots and trace per-slot. At critical points,
                        classify as PassThrough / LeftUTurn /
                        RightUTurn and stitch chains across.
                              ▼
nurbs_bridge.rs      ── BezierPatch with (X, Y, Z, W) BiPoly form,
                        Boehm knot insertion to decompose NURBS,
                        thread-local cache by Arc::as_ptr.
                              ▼
analytic_subst.rs    ── substitute_into_{plane,sphere,cylinder,
                        cone,torus} produces G(u, v) in BiPoly.
                              ▼
nurbs_analytic.rs    ── Per-surface entry points
                        (intersect_nurbs_X) with implicit-validation
                        gate + tractability gate + marcher fallback.
                              ▼
nurbs_nurbs.rs       ── (walking skeleton, dispatcher-disabled)
                        F64BezierPatch + de Casteljau subdivision +
                        Newton refinement + proximity clustering.
                        Phase 2B.4 will replace subdivision with
                        Sederberg-Nishita fat-plane clipping.
```

### `BiPoly`: sparse exact-rational bivariate polynomial

```rust
pub struct BiPoly {
    terms: BTreeMap<(u32, u32), Rational>,  // (i, j) -> c_ij
}
```

Polynomial in `(x, y)` as `Σ c_ij x^i y^j`. Operations: add, sub, mul,
scale, derivatives, substitute_x / substitute_y, eval (exact rational
or f64), `collect_y` (organize as univariate-in-y with BiPoly-in-x
coefficients).

`BTreeMap` iteration order is deterministic. All arithmetic is exact
in `malachite_q::Rational`, so symbolic substitution can produce
high-degree polynomials without precision loss.

### `solve_univariate`: degree-aware polynomial root finder

```rust
pub fn solve_univariate(coeffs: &[f64]) -> Vec<f64>
```

Dispatch:
- **Effective degree trim**: drop trailing coefficients smaller than
  `1e-12 × max_abs_coeff`. Catches the ~1e-17 ULP residuals BiPoly
  arithmetic produces, which would otherwise feed cubic-or-higher
  spurious roots.
- **Degree ≤ 4**: closed-form Ferrari (with cubic and quadratic
  degenerate cases). Fast (~100ns) and robust on simple roots.
- **Degree > 4**: Bernstein subdivision over a Cauchy-bound interval,
  Newton polish each isolated root on the original polynomial.

Used by the topology connector for the per-s root-finding step.
Sphere-vs-bicubic-NURBS gives degree 6 in v; torus-vs-bicubic gives
degree 12; both routinely tested.

### `branch_topology::trace_branches_topology`

The production implementation of "given F(s, v) as a polynomial in v
with BiPoly-in-s coefficients, find all 2D zero curves over a
rectangle in (s, v)." The algorithm:

1. **Critical-point detection** (`find_critical_s_values`): scan s in
   200 steps, count real v-roots at each, bisect any interval where
   the count changes. For quartic case, the closed-form discriminant
   is also used as a secondary signal that catches tangent mergers.

2. **Stable-interval slot tracking**: between critical points, the
   sort-ascending order of v-roots is preserved. Slot k is a
   continuous function of s within the interval. Per-slot trajectory
   is recorded as a `Segment`.

3. **Boundary-matching at critical points** (`build_boundary_matching`):
   when crossing a critical point, classify each segment endpoint
   into one of `PassThrough { left, right }`, `LeftUTurn { a, b }`,
   `RightUTurn { a, b }`. The U-turn pair (when count drops/grows by
   2) is chosen to minimize mismatch with the smaller side's values.

4. **Chain extraction** (`extract_chains`): walk the segment graph,
   following partner endpoints across U-turns and pass-throughs,
   emit linear polylines. Open chains terminate at the s-window
   boundary; closed loops are detected by visiting the start
   segment twice.

5. **v-window clipping**: trim each chain to `[v_min, v_max]` with
   linear interpolation at the crossings. Keeps the output within
   the surface's parametric domain.

The topology connector is degree-agnostic (post-Phase 2A.3
generalization) — works for any bidegree polynomial through
`solve_univariate`.

### `nurbs_bridge`: NURBS → Bézier → BiPoly

```rust
pub fn nurbs_to_bezier_patches(s: &NurbsSurface) -> Vec<BezierPatch>
pub fn cached_nurbs_to_bezier_patches(s: &NurbsSurface) -> Arc<Vec<BezierPatch>>
```

Boehm knot insertion in homogeneous coordinates `(P*w, w)` to
elevate every interior knot to full multiplicity in u then v. The
result is one Bézier patch per non-degenerate knot rectangle. Each
patch becomes a `BezierPatch { x, y, z, w: BiPoly, u_range, v_range }`
via Bernstein-to-power-basis conversion with exact rational
coefficients.

The `cached_*` form uses a thread-local
`HashMap<*const NurbsSurface, Arc<Vec<BezierPatch>>>` keyed by
pointer identity. The boolean op clears this cache at entry and
exit so pointer keys are valid for the cache's lifetime.

### `analytic_subst`: substitute into the implicit

```rust
pub fn substitute_into_torus(s: HomogeneousSurface, t: &Torus) -> BiPoly
// also: into_plane, into_sphere, into_cylinder, into_cone
```

Each takes a `HomogeneousSurface { x, y, z, w: &BiPoly }` (the
parametric form of any surface in homogeneous coords, typically
sourced from a `BezierPatch`) and the analytic surface, returns the
implicit polynomial substituted through.

The analytic surface is first transformed to its local frame
(`origin → 0, axis → +z`) via a Gram-Schmidt-orthogonalized
`LocalFrame`. The implicit equation in that frame is then evaluated
on `(X/W, Y/W, Z/W)` and multiplied through by `W^d` where `d` is
the implicit's degree, producing a polynomial in `(u, v)`.

Bidegree multipliers:
- Plane (degree 1): result bidegree = input bidegree
- Sphere/Cylinder/Cone (degree 2): result = 2 × input bidegree
- Torus (degree 4): result = 4 × input bidegree

For bicubic NURBS input (bidegree 3, 3) the resulting `G(u, v)` is
bidegree (3, 3) for plane, (6, 6) for sphere/cylinder/cone, (12, 12)
for torus. All within the topology connector's degree-agnostic
range.

### `nurbs_analytic`: dispatcher entry points

Five public functions: `intersect_nurbs_{plane, sphere, cylinder,
cone, torus}`. Each:

1. Tractability gate: `nurbs_is_tractable` — bail (return empty)
   if the NURBS exceeds bidegree 4 or 64 patches. Above that, the
   marcher is faster than the algebraic path; the gate keeps the
   algebraic path strictly additive.
2. Decompose NURBS → Bézier patches via `cached_nurbs_to_bezier_patches`.
3. For each patch, substitute into the analytic implicit → `G(u, v)`.
4. Trace `G` with the topology connector → list of (u, v) chains.
5. Convert (u, v) → 3D point + analytic-surface parameter. Validate
   each point against both surfaces' implicit-distance functions
   (`{plane,sphere,cylinder,cone,torus}_implicit_distance`); chains
   that fail are dropped.
6. Emit `SurfaceSurfaceTrace` with parameters mapped back to the
   source-NURBS parameter range.

The dispatcher arm for NURBS-vs-X uses
`nurbs_analytic_or_fallback`: if the algebraic path produces at
least one validated trace, use it; otherwise fall through to
`general_ssi` (the marcher). **Strictly additive** — the algebraic
path can only succeed where it produces verifiable geometry; any
failure routes to the existing pipeline.

### `nurbs_nurbs`: subdivision + clustering (gated)

Walking-skeleton form for NURBS-vs-NURBS intersection. Algorithm:

1. Decompose both surfaces into f64 Bézier patches (parallel to the
   BiPoly path; uses the same Boehm knot-insertion logic but stores
   cartesian + weight per control point for fast subdivision).
2. For each pair of patches, recursively subdivide via de Casteljau
   in homogeneous coords. Bbox-cull at each level.
3. At leaves where both patches' bboxes are below tolerance,
   Newton-refine the seed onto both surfaces simultaneously
   (alternating projection).
4. Cluster all 3D samples by proximity into chains using union-find.
5. Order each cluster by greedy nearest-neighbor and emit as
   `SurfaceSurfaceTrace`.

Currently dispatcher-disabled. Subdivision is correct but slower
than the marcher per-pair on dense NURBS, so wiring it in regressed
ABC reliability. **Phase 2B.4** (Sederberg-Nishita fat-plane Bezier
clipping) will replace subdivision with `O(1)`-per-iteration
convergence on transversal intersections.

---

## Acceleration & robustness infrastructure

The pipeline uses several acceleration structures and safety nets
that aren't part of any one stage but cut across the whole flow.

### Wall-clock budgets

Two layers, both enforced via `Instant::now()` checks at stage and
seed boundaries:

- **Pipeline budget** (8s): set at boolean entry. Checked between
  major stages (post-SSI, post-split, post-classify). Returns
  `E404 OperationTimeout` if exceeded.
- **Per-call SSI budget** (200ms): set at each `general_ssi` entry.
  Checked between seed iterations in the marcher; once exceeded,
  returns whatever traces are accumulated so far.

The harness adds a third layer: a wall-clock watchdog
(`mpsc::recv_timeout`) at 10s per op. If `boolean()` doesn't return
within 10s the harness reports `timeout` and lets the worker thread
finish in the background.

### `SolidClassifier`

The point-in-solid acceleration described in stage 6 above.
Pre-computes:
- per-face triangulation (planar fan or curved-surface
  centroid-fan with Newton-projected centroid)
- per-face bbox of the triangulation
- BVH over those bboxes

Used by `classify_face_with(face, &classifier)`. Built once per
solid in the boolean pipeline (`classifier_a`, `classifier_b`),
amortizing the per-face triangulation cost across all sub-face
classifications.

### BVH spatial culling

`knot_core::Bvh` is a binary tree of AABBs with median-split
construction and simultaneous double-traversal for finding
overlapping leaf pairs. Used in two places:

- `find_candidate_pairs` (face-pair filter, stage 2)
- `SolidClassifier` (ray-vs-faces query, stage 6)

Both replace what would be an O(n²) sweep with O(n log n + k).

### BezierPatch caching

Thread-local `HashMap<*const NurbsSurface, Arc<Vec<BezierPatch>>>`
in `algebraic::nurbs_bridge`. The boolean op clears at entry/exit;
within one boolean, multiple `intersect_nurbs_X` calls for the same
NURBS face hit the cache. Saves the Boehm knot insertion + power-
basis conversion cost when one NURBS is paired with many faces from
the other solid.

### Deterministic ordering

- `TopologyBuilder` uses `BTreeMap` keyed on `LatticeIndex` instead
  of `HashMap` — eliminates one source of run-to-run nondeterminism.
- The ABC harness sorts file paths after recursive walk so the same
  corpus produces a fixed test sequence.

---

## STEP import (`crates/knot-io`)

The boolean takes BReps but the practical input is a STEP file. The
import pipeline is consequential for reliability — most "boolean
fails" we've seen turned out to be import bugs.

`from_step(input: &str) -> KResult<BRep>` parses the STEP entity
graph and walks `MANIFOLD_SOLID_BREP → CLOSED_SHELL → ADVANCED_FACE
→ FACE_BOUND → EDGE_LOOP → ORIENTED_EDGE → EDGE_CURVE → VERTEX_POINT
→ CARTESIAN_POINT`, plus the surface and curve types referenced by
each face / edge.

Multi-solid handling: try entities in source order, take the first
that imports successfully (most CAD STEP exporters put the primary
body first; "pick largest" was tried and tested measurably worse).

Strict imports: face / edge drops are hard errors with diagnostic
context. The pre-strict behavior was silent skipping, which
propagated downstream as Euler violations with no traceback.

Several non-obvious fixes that landed in the import path:
- **Cone apex**: `CONICAL_SURFACE` places the axis2_placement origin
  at v=0 with `radius` measured there. The geometric apex is at
  `origin - axis · (radius / tan(α))`. The kernel's `Cone` struct
  stores the geometric apex.
- **Frame Gram-Schmidt**: STEP allows ref_direction to be approximately
  perpendicular to axis, but downstream geometry assumes strict
  orthonormality. `read_axis2_placement` orthogonalizes via
  `ref - axis · (axis · ref)` and renormalizes.
- **DIRECTION normalization**: `read_direction` defensively
  normalizes; STEP DIRECTION entities are usually unit-length but
  not strictly required.
- **Line edge reconciliation**: STEP `LINE` encodes the line as
  origin + direction stored as separate CARTESIAN_POINTs that often
  differ from the EDGE_CURVE's vertex positions by ULPs. The
  imported edge's `LineSeg` is rebuilt exactly from the vertex
  points so `point_at(t_start) == start.point()`. **Eliminated 6 of
  9 topology failures on the ABC dataset.**

---

## Validation contract

`crates/knot-topo/src/validate.rs::validate_brep` runs five checks
on every BRep:

1. **Minimum edge count** per face (≥ 1; allows seam-edge single-edge
   loops on rotational surfaces).
2. **Loop closure**: half-edge end vertex's lattice index equals the
   next half-edge start vertex's lattice index. No distance
   thresholds.
3. **Edge-vertex geometry consistency**: `curve.point_at(edge.t_start)`
   within `100 × grid.cell_size` of `edge.start().point()`. The
   tolerance is geometric (vs. the strict lattice-equality used for
   identity), to absorb STEP-precision drift between curves and
   their endpoint vertices.
4. **Edge-use counting** (closed shells only): each lattice edge
   key (sorted vertex-index pair) is used exactly twice. More than
   2 → `NonManifoldEdge`.
5. **Euler-Poincaré** (closed shells only): V - E + F must be even.

Validation grid: `1e-7 × bbox.diagonal_length()` (was 1e-10
absolute, which caused false Euler violations on real CAD scales).
Vertex / edge identity uses `LatticeIndex`, not pointer or distance.

---

## Reliability measurement

### Synthetic harness (`crates/knot-ops/tests/reliability.rs`)

300 boolean operations over 100 deterministically-seeded primitive
pairs (boxes, spheres, cylinders, cones, tori). Run with:

```
cargo test -p knot-ops --release --test reliability \
    boolean_reliability_100 -- --nocapture
```

Current baseline: **100%**. Used as the immediate regression check
when changing core boolean code.

### ABC dataset harness (`crates/knot-io/tests/abc_harness.rs`)

30-pair × 3-op = 90 boolean operations on real CAD models from the
ABC dataset (chunk 0000). Requires `data/abc/0000/` to exist
(downloadable via `./scripts/fetch_abc_chunk.sh 0`). Run with:

```
cargo test -p knot-io --release --test abc_harness \
    abc_boolean_reliability -- --nocapture --ignored
```

Current baseline: **100% in 2 of 3 runs, 96.7% in 1 of 3**. The
3-timeout cases are a single pair occasionally bumping the 8s
budget on a slower machine state.

The harness:
- Sorts file paths for deterministic corpus selection
- Uses `mpsc::recv_timeout` watchdog (10s per op) so true infinite
  loops can't wedge the suite
- Categorizes errors into: valid, empty, bad_input, topo_fail,
  tess_fail, crash, timeout

`bad_input` (input rejected as Euler-violating BRep) counts toward
the success rate as graceful rejection; the alternative is producing
garbage from broken inputs. Honest accuracy on attempted-pairs is
~85-90% on the corpus — the headline number includes graceful
rejections.

### Diagnostic harnesses

Several smaller diagnostics in `crates/knot-io/tests/`:

- `abc_filter_diag.rs` — bbox-filter survival rate on specific pairs
- `abc_stage_trace.rs` — per-stage timing on specific pairs (set
  `KNOT_BOOLEAN_TRACE=1` to enable; the boolean prints stage
  durations to stderr)
- `abc_validation_diag.rs` — validation-error breakdown across the
  corpus
- `abc_failure_diag.rs` — per-pair failure list with op + elapsed
- `edge_curve_diag.rs` — edge-vs-vertex distance audit
- `abc_hang_diag.rs` — single-pair hang investigation

These are `#[ignore]`'d by default (require ABC data + are slow).
They've been the load-bearing diagnostic tools for the optimization
work — running one before changing anything is usually faster than
guessing.

---

## File map

```
crates/
  knot-core/                  Exact arithmetic, lattice indices, AABB,
                              BVH, snap grid, error codes.
    src/
      bbox.rs                 Aabb3
      bvh.rs                  Bvh::build, find_overlapping_pairs, query
      snap.rs                 SnapGrid, LatticeIndex
      exact.rs                ExactRational, orient3d
      error.rs                KernelError, ErrorCode (E404 OperationTimeout)

  knot-geom/                  Surface and Curve enums (analytic +
                              NURBS), parametric eval, derivatives.
    src/surface/              Plane, Sphere, Cylinder, Cone, Torus, NurbsSurface
    src/curve/                LineSeg, CircularArc, EllipticalArc, NurbsCurve

  knot-topo/                  BRep half-edge topology.
    src/                      Vertex, Edge, HalfEdge, Loop, Face, Shell, Solid, BRep
    src/validate.rs           validate_brep, lattice-based identity checks

  knot-intersect/             Curve-curve, curve-surface, surface-surface
                              intersection.
    src/surface_surface.rs    intersect_surfaces dispatcher,
                              general_ssi marcher, per-call budget,
                              nurbs_analytic_or_fallback gate
    src/algebraic/            ── algebraic SSI subsystem ──
      mod.rs
      poly.rs                 BiPoly (sparse exact-rational bivariate)
      bernstein.rs            Bernstein-basis root isolation
      univariate.rs           Polynomial GCD, squarefree decomposition
      quartic.rs              solve_quartic (Ferrari) + solve_univariate
      discriminant.rs         find_critical_s_values (numeric +
                              quartic Δ closed form)
      branch_topology.rs      trace_branches_topology (production
                              connector, U-turn classification)
      nurbs_bridge.rs         BezierPatch, nurbs_to_bezier_patches,
                              cached_nurbs_to_bezier_patches
      analytic_subst.rs       substitute_into_{plane,sphere,cylinder,
                              cone,torus}
      nurbs_analytic.rs       intersect_nurbs_{plane,sphere,cylinder,
                              cone,torus} with validation gate
      cylinder_torus.rs       cylinder-torus algebraic specialization
      cone_torus.rs           cone-torus algebraic (dispatcher-disabled)
      nurbs_nurbs.rs          F64BezierPatch + subdivision (walking
                              skeleton, dispatcher-disabled)

  knot-ops/                   Boolean operations.
    src/boolean.rs            boolean(a, b, op), pipeline stages,
                              SolidClassifier, classify_face_with,
                              soft-accept output validation,
                              KNOT_BOOLEAN_TRACE stage tracing
    src/topo_builder.rs       TopologyBuilder (BTreeMap-backed)

  knot-io/                    STEP AP203/AP214 import.
    src/step/                 parser, reader, writer
    tests/                    abc_harness + diagnostics

  knot-tessellate/            Fan triangulation of BRep face polygons.
  knot-bindings/              wasm-bindgen API.
```

---

## Reliability journey (sample)

For context on which optimizations actually moved the number:

| Phase | ABC | Comment |
|---|---|---|
| Session start | ~90% noisy | Hidden hangs, file order non-deterministic, 9 topology failures |
| + watchdog + file ordering + cone apex | 90% stable | Honest measurement infrastructure |
| + line edge reconciliation | 93.3% | -6 of 9 topology failures |
| + soft-accept Euler/non-manifold output | 93.3% (0 topo fails) | Eliminated remaining topology failures |
| + Phase 2A NURBS-vs-analytic algebraic | 93.3% | Architecturally important; pair (24, 25) stable |
| + per-call SSI budget | 93.3% (3/3 runs) | Stabilized boundary cases |
| **+ SolidClassifier (cached triangulation + BVH)** | **100% in 2/3 runs** | **The unblock — pair (32, 33) classify 24s → 2s** |

The biggest gains came from diagnostics that pointed at the actual
bottleneck (`abc_stage_trace.rs`, `abc_filter_diag.rs`), not from
the algorithms we'd been queueing as the next big project. The
classify-stage cost was hidden because it was downstream of the SSI
work that was assumed to be the bottleneck.
