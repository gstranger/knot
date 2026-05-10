/**
 * Core kernel wrapper — handles WASM initialization and provides
 * a typed, ergonomic API over the raw wasm-bindgen bindings.
 */

// We import types from the wasm-pack output.  At runtime,
// the consumer calls createKnot() which does the async init.
import type {
  JsBrep as RawBrep,
  JsCurve as RawCurve,
  JsSurfaceMesh as RawMesh,
  InitInput as WasmInitInput,
} from 'knot-wasm';

/** Path, URL, Response, or BufferSource to load the .wasm file from. */
export type InitInput = WasmInitInput;

// ── Types ──────────────────────────────────────────────────────

export interface Vec3 {
  x: number;
  y: number;
  z: number;
}

export interface MeshData {
  positions: Float32Array;
  normals: Float32Array;
  indices: Uint32Array;
  /** Per-triangle source face index (maps each triangle back to its BRep face). */
  faceIds: Uint32Array;
  vertexCount: number;
  triangleCount: number;
}

export interface TessellateOptions {
  /** Max normal deviation in radians (smaller = finer mesh). Default: 0.1. */
  normalTolerance?: number;
  /** Max triangle edge length (smaller = finer mesh). Default: Infinity. */
  maxEdgeLength?: number;
}

export interface BoundingBox {
  min: Vec3;
  max: Vec3;
}

export interface SphereOptions {
  center?: Vec3;
  radius: number;
  segments?: number;
  rings?: number;
}

export interface CylinderOptions {
  center?: Vec3;
  radius: number;
  height: number;
  sides?: number;
}

export interface ExtrudeOptions {
  /** Extrusion direction. Defaults to {x:0, y:0, z:1}. */
  direction?: Vec3;
  /** Extrusion distance along the direction vector. */
  distance: number;
}

export interface RevolveOptions {
  /** A point on the revolution axis. Defaults to origin. */
  axisOrigin?: Vec3;
  /** The axis direction. Defaults to Y axis. */
  axisDirection?: Vec3;
  /** Revolution angle in radians. Defaults to 2*PI (full revolution). */
  angle?: number;
}

export interface EdgeRef {
  /** Start point of the edge. */
  start: Vec3;
  /** End point of the edge. */
  end: Vec3;
}

/** Discriminator returned by `JsCurve.curve_type()`. */
export type CurveType = 'line' | 'arc' | 'elliptical_arc' | 'nurbs';

export interface ArcOptions {
  /** Center of the arc. */
  center: Vec3;
  /** Plane normal. The arc lies in the plane through `center` with this normal. */
  normal: Vec3;
  /** Radius. */
  radius: number;
  /** Reference axis: defines the angle = 0 direction. Should lie in the arc plane. */
  refAxis: Vec3;
  /** Start angle in radians. Default 0. */
  startAngle?: number;
  /** End angle in radians. Default 2*PI. */
  endAngle?: number;
}

export interface NurbsCurveOptions {
  controlPoints: Vec3[];
  /** Per-control-point weights. Length must match controlPoints. Default: all 1. */
  weights?: number[];
  /** Knot vector. Length = controlPoints.length + degree + 1. */
  knots: number[];
  degree: number;
}

// ── Curve wrapper ──────────────────────────────────────────────

/** Ergonomic wrapper around the kernel's opaque Curve handle. */
export class Curve {
  /** @internal */ _raw: RawCurve;
  /** @internal */ constructor(raw: RawCurve) { this._raw = raw; }

  /** Discriminator: 'line' | 'arc' | 'elliptical_arc' | 'nurbs'. */
  get type(): CurveType { return this._raw.curve_type() as CurveType; }

  /** Parameter domain `[t_start, t_end]`. */
  domain(): [number, number] {
    const d = this._raw.domain();
    return [d[0]!, d[1]!];
  }

  /** Point on the curve at parameter `t`. */
  pointAt(t: number): Vec3 {
    const p = this._raw.point_at(t);
    return { x: p[0]!, y: p[1]!, z: p[2]! };
  }

  /** Unit tangent vector at `t`. */
  tangentAt(t: number): Vec3 {
    const v = this._raw.tangent_at(t);
    return { x: v[0]!, y: v[1]!, z: v[2]! };
  }

  /** Sample `n` evenly-spaced parameters; returns the corresponding points. */
  sample(n: number): Vec3[] {
    const flat = this._raw.sample(n);
    const out: Vec3[] = [];
    for (let i = 0; i < flat.length; i += 3) out.push({ x: flat[i]!, y: flat[i + 1]!, z: flat[i + 2]! });
    return out;
  }

  /** `n+1` parameter values dividing the domain into `n` equal-parameter segments. */
  divide(n: number): number[] {
    return Array.from(this._raw.divide(n));
  }

  /** Closest point on the curve to `q`. Returns parameter, point, and distance. */
  closestPoint(q: Vec3): { param: number; point: Vec3; distance: number } {
    const r = this._raw.closest_point(q.x, q.y, q.z);
    return {
      param: r[0]!,
      point: { x: r[1]!, y: r[2]!, z: r[3]! },
      distance: r[4]!,
    };
  }

  /** Axis-aligned bounding box. */
  boundingBox(): BoundingBox {
    const b = this._raw.bounding_box();
    return { min: { x: b[0]!, y: b[1]!, z: b[2]! }, max: { x: b[3]!, y: b[4]!, z: b[5]! } };
  }

  /**
   * Offset this curve by `distance` in the plane with the given normal.
   *
   * The offset direction at parameter `t` is `planeNormal × tangent(t)`.
   * Exact only for lines and circular arcs; throws for NURBS and elliptical
   * arcs (their exact offset is not the same curve type).
   */
  offset(distance: number, planeNormal: Vec3): Curve {
    return new Curve(this._raw.offset(distance, planeNormal.x, planeNormal.y, planeNormal.z));
  }

  /** Release WASM memory. */
  free(): void { this._raw.free(); }
  [Symbol.dispose](): void { this.free(); }
}

// ── Brep wrapper ───────────────────────────────────────────────

/** Ergonomic wrapper around the kernel's opaque BRep handle. */
export class Brep {
  /** @internal */
  _raw: RawBrep;

  /** @internal */
  constructor(raw: RawBrep) {
    this._raw = raw;
  }

  /** Number of faces. */
  get faceCount(): number {
    return this._raw.face_count();
  }

  /** Tessellate to a triangle mesh. */
  tessellate(opts?: TessellateOptions): MeshData {
    const mesh = opts
      ? this._raw.tessellate_with(opts.normalTolerance ?? 0.1, opts.maxEdgeLength ?? Infinity)
      : this._raw.tessellate();
    const data = extractMesh(mesh);
    mesh.free();
    return data;
  }

  /** Validate the BRep topology. Throws on error. */
  validate(): void {
    this._raw.validate();
  }

  /** Axis-aligned bounding box. */
  boundingBox(): BoundingBox {
    const b = this._raw.bounding_box();
    return {
      min: { x: b[0], y: b[1], z: b[2] },
      max: { x: b[3], y: b[4], z: b[5] },
    };
  }

  /** Serialize to CBOR bytes for persistence. Deserialize with `knot.fromCBOR()`. */
  toCBOR(): Uint8Array {
    return this._raw.to_cbor();
  }

  /** Export as binary STL. */
  toSTL(): Uint8Array {
    return this._raw.to_stl();
  }

  /** Export as GLB (binary glTF 2.0). */
  toGLB(): Uint8Array {
    return this._raw.to_glb();
  }

  /** Export as STEP file string. */
  toSTEP(): string {
    return (this._raw as any).export_step
      ? (this._raw as any).export_step()
      : _wasm!.export_step(this._raw);
  }

  /** Translate (move) by dx, dy, dz. Returns a new Brep. */
  translate(dx: number, dy: number, dz: number): Brep {
    return new Brep(this._raw.translate(dx, dy, dz));
  }

  /** Rotate around an axis through the origin. Angle in radians. Returns a new Brep. */
  rotate(axis: Vec3, angle: number): Brep {
    return new Brep(this._raw.rotate(axis.x, axis.y, axis.z, angle));
  }

  /**
   * Scale by (sx, sy, sz). Returns a new Brep.
   *
   * Uniform scaling (same value for all axes) works on all geometry.
   * Non-uniform scaling works on planar/NURBS geometry but will throw
   * on analytical curved surfaces (spheres, cylinders, etc.).
   *
   * Pass a single number for uniform scaling.
   */
  scale(sx: number, sy?: number, sz?: number): Brep {
    const _sy = sy ?? sx;
    const _sz = sz ?? sx;
    return new Brep(this._raw.scale(sx, _sy, _sz));
  }

  /**
   * Extrude this profile Brep along a direction to create a solid.
   *
   * The Brep should be a planar profile (e.g. from `knot.profile()`).
   */
  extrude(opts: ExtrudeOptions): Brep {
    const d = opts.direction ?? { x: 0, y: 0, z: 1 };
    return new Brep(_wasm!.extrude(this._raw, d.x, d.y, d.z, opts.distance));
  }

  /**
   * Revolve this profile Brep around an axis to create a solid.
   *
   * The Brep should be a planar profile (e.g. from `knot.profile()`).
   */
  revolve(opts?: RevolveOptions): Brep {
    const o = opts?.axisOrigin ?? { x: 0, y: 0, z: 0 };
    const a = opts?.axisDirection ?? { x: 0, y: 1, z: 0 };
    const angle = opts?.angle ?? Math.PI * 2;
    return new Brep(_wasm!.revolve_brep(this._raw, o.x, o.y, o.z, a.x, a.y, a.z, angle));
  }

  /**
   * Fillet (round) edges with a constant radius.
   *
   * Edges are identified by their start/end vertex coordinates.
   * Both adjacent faces must be planar.
   */
  fillet(edges: EdgeRef[], radius: number): Brep {
    return new Brep(_wasm!.fillet_edges(this._raw, flattenEdgeRefs(edges), radius));
  }

  /**
   * Chamfer (bevel) edges with a constant distance.
   *
   * Edges are identified by their start/end vertex coordinates.
   * Both adjacent faces must be planar.
   */
  chamfer(edges: EdgeRef[], distance: number): Brep {
    return new Brep(_wasm!.chamfer_edges(this._raw, flattenEdgeRefs(edges), distance));
  }

  /** Boolean union: this ∪ other. */
  union(other: Brep): Brep {
    return new Brep(_wasm!.boolean_union(this._raw, other._raw));
  }

  /** Boolean intersection: this ∩ other. */
  intersect(other: Brep): Brep {
    return new Brep(_wasm!.boolean_intersection(this._raw, other._raw));
  }

  /** Boolean subtraction: this \ other. */
  subtract(other: Brep): Brep {
    return new Brep(_wasm!.boolean_subtraction(this._raw, other._raw));
  }

  /** Release WASM memory. Call when done, or use `using brep = ...` */
  free(): void {
    this._raw.free();
  }

  [Symbol.dispose](): void {
    this.free();
  }
}

// ── Kernel singleton ───────────────────────────────────────────

/** The loaded WASM module. Set once by createKnot(). */
let _wasm: typeof import('knot-wasm') | null = null;
let _initPromise: Promise<Knot> | null = null;

/** The public API surface returned by createKnot(). */
export interface Knot {
  /** Kernel version string. */
  version(): string;

  /**
   * Create a planar polygon profile from 2D or 3D points.
   *
   * The result is a single-face open BRep suitable for `.extrude()` and `.revolve()`.
   *
   * 2D points are placed in the z=0 plane. 3D points define their own plane.
   */
  profile(points: Vec3[] | [number, number][]): Brep;

  /** Create a box centered at the origin. */
  box(sx: number, sy: number, sz: number): Brep;

  /** Create a sphere BRep. */
  sphere(opts: SphereOptions): Brep;

  /** Create a cylinder BRep. */
  cylinder(opts: CylinderOptions): Brep;

  /** Boolean union of two Breps. */
  union(a: Brep, b: Brep): Brep;

  /** Boolean intersection of two Breps. */
  intersection(a: Brep, b: Brep): Brep;

  /** Boolean subtraction: a \ b. */
  subtraction(a: Brep, b: Brep): Brep;

  /** Create a straight line segment from `a` to `b`. */
  line(a: Vec3, b: Vec3): Curve;

  /** Create a circular arc. */
  arc(opts: ArcOptions): Curve;

  /** Create a NURBS curve from control points, weights, knot vector, and degree. */
  nurbsCurve(opts: NurbsCurveOptions): Curve;

  /** Sweep a planar profile BRep along a curve rail to create a solid. */
  sweep(profile: Brep, rail: Curve): Brep;

  /**
   * Loft between two or three planar profile BReps. All profiles must share
   * outer-loop vertex count; vertex `i` of profile `k` is connected to
   * vertex `i` of profile `k+1`. Throws for unsupported lengths until
   * list-typed graph ports land.
   */
  loft(profiles: Brep[]): Brep;

  /** Import a BRep from a STEP file string. */
  importSTEP(stepString: string): Brep;

  /** Export a BRep as a STEP file string. */
  exportSTEP(brep: Brep): string;

  /** Deserialize a BRep from CBOR bytes (produced by `brep.toCBOR()`). */
  fromCBOR(data: Uint8Array): Brep;
}

/**
 * Initialize the WASM kernel and return the modeling API.
 *
 * Safe to call multiple times — returns the same instance after first init.
 *
 * @param wasmPath - Optional path or URL to the .wasm file.
 *                   If omitted, resolved from the knot-wasm package location.
 */
export async function createKnot(wasmPath?: InitInput): Promise<Knot> {
  if (_initPromise) return _initPromise;

  _initPromise = (async () => {
    const mod = await import('knot-wasm');
    await mod.default(wasmPath);
    _wasm = mod;

    const knot: Knot = {
      version: () => mod.version(),

      profile: (points) => {
        const first = points[0];
        if (Array.isArray(first)) {
          // [number, number][] → 2D
          const flat = new Float64Array((points as [number, number][]).flatMap(([x, y]) => [x, y]));
          return new Brep(mod.create_profile(flat, 2));
        } else {
          // Vec3[]
          const flat = new Float64Array((points as Vec3[]).flatMap((p) => [p.x, p.y, p.z]));
          return new Brep(mod.create_profile(flat, 3));
        }
      },

      box: (sx, sy, sz) => new Brep(mod.create_box(sx, sy, sz)),

      sphere: (opts) => {
        const c = opts.center ?? { x: 0, y: 0, z: 0 };
        const seg = opts.segments ?? 24;
        const rings = opts.rings ?? 12;
        return new Brep(mod.create_sphere_brep(c.x, c.y, c.z, opts.radius, seg, rings));
      },

      cylinder: (opts) => {
        const c = opts.center ?? { x: 0, y: 0, z: 0 };
        const sides = opts.sides ?? 24;
        return new Brep(mod.create_cylinder_brep(c.x, c.y, c.z, opts.radius, opts.height, sides));
      },

      union: (a, b) => new Brep(mod.boolean_union(a._raw, b._raw)),
      intersection: (a, b) => new Brep(mod.boolean_intersection(a._raw, b._raw)),
      subtraction: (a, b) => new Brep(mod.boolean_subtraction(a._raw, b._raw)),

      line: (a, b) => new Curve(mod.create_line(a.x, a.y, a.z, b.x, b.y, b.z)),

      arc: (opts) => {
        const startAngle = opts.startAngle ?? 0;
        const endAngle = opts.endAngle ?? Math.PI * 2;
        return new Curve(mod.create_arc(
          opts.center.x, opts.center.y, opts.center.z,
          opts.normal.x, opts.normal.y, opts.normal.z,
          opts.radius,
          opts.refAxis.x, opts.refAxis.y, opts.refAxis.z,
          startAngle, endAngle,
        ));
      },

      nurbsCurve: ({ controlPoints, weights, knots, degree }) => {
        const cp = new Float64Array(controlPoints.flatMap(p => [p.x, p.y, p.z]));
        const w = new Float64Array(weights ?? controlPoints.map(() => 1));
        if (w.length !== controlPoints.length) {
          throw new Error('nurbsCurve: weights length must match controlPoints length');
        }
        return new Curve(mod.create_nurbs_curve(cp, w, new Float64Array(knots), degree));
      },

      sweep: (profile, rail) => new Brep(mod.sweep(profile._raw, rail._raw)),

      loft: (profiles) => {
        if (profiles.length === 2) {
          return new Brep(mod.loft2(profiles[0]!._raw, profiles[1]!._raw));
        }
        if (profiles.length === 3) {
          return new Brep(mod.loft3(profiles[0]!._raw, profiles[1]!._raw, profiles[2]!._raw));
        }
        throw new Error(`loft: only 2 or 3 profiles supported in this build, got ${profiles.length}`);
      },

      importSTEP: (s) => new Brep(mod.import_step(s)),
      exportSTEP: (brep) => mod.export_step(brep._raw),
      fromCBOR: (data) => new Brep(mod.from_cbor(data)),
    };

    return knot;
  })();

  return _initPromise;
}

// ── Helpers ────────────────────────────────────────────────────

function flattenEdgeRefs(edges: EdgeRef[]): Float64Array {
  const flat = new Float64Array(edges.length * 6);
  for (let i = 0; i < edges.length; i++) {
    const { start: s, end: e } = edges[i];
    flat[i * 6]     = s.x;
    flat[i * 6 + 1] = s.y;
    flat[i * 6 + 2] = s.z;
    flat[i * 6 + 3] = e.x;
    flat[i * 6 + 4] = e.y;
    flat[i * 6 + 5] = e.z;
  }
  return flat;
}

function extractMesh(mesh: RawMesh): MeshData {
  // Kernel gives Float64Array; Three.js / WebGL wants Float32Array
  const positions = new Float32Array(mesh.positions());
  const normals = new Float32Array(mesh.normals());
  const indices = mesh.indices();
  const faceIds = mesh.face_ids();

  return {
    positions,
    normals,
    indices,
    faceIds,
    vertexCount: mesh.vertex_count(),
    triangleCount: mesh.triangle_count(),
  };
}
