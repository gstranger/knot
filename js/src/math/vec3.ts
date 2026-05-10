/**
 * 3D vector / point in pure TypeScript.
 *
 * Convention notes:
 * - Immutable. Every operation returns a new `Vec3`. We're not
 *   building a hot inner loop here — we're building primitives the
 *   graph runtime calls. Allocation cost is dwarfed by graph
 *   bookkeeping.
 * - Free functions, not methods. Easier to import the slice you
 *   need, easier to currying-compose, plays nicely with type
 *   narrowing. Methods would force a class wrapper that defeats
 *   the cheap-handle property.
 * - `Vec3` is both a position and a direction; the kernel
 *   distinguishes Point3 / Vector3 in Rust but JS doesn't need the
 *   ceremony. Functions whose semantics differ are named
 *   accordingly (e.g. `transformPoint` vs `transformVector`).
 * - `equals` is bit-exact. For tolerance comparisons use
 *   `approxEquals`.
 *
 * Naming aligns with `knot-geom::Vector3` / `Point3` where it makes
 * sense, so when the M0 graph runtime ports a value across the WASM
 * boundary the developer's mental model carries over.
 */

export interface Vec3 {
  readonly x: number;
  readonly y: number;
  readonly z: number;
}

export const vec3 = (x: number, y: number, z: number): Vec3 => ({ x, y, z });

export const ZERO: Vec3 = vec3(0, 0, 0);
export const ONE: Vec3 = vec3(1, 1, 1);
export const X_AXIS: Vec3 = vec3(1, 0, 0);
export const Y_AXIS: Vec3 = vec3(0, 1, 0);
export const Z_AXIS: Vec3 = vec3(0, 0, 1);

// ─── Construction / conversion ─────────────────────────────────────

export const fromArray = (a: readonly number[]): Vec3 => {
  if (a.length < 3) {
    throw new Error(`Vec3.fromArray: need at least 3 elements, got ${a.length}`);
  }
  return vec3(a[0]!, a[1]!, a[2]!);
};

export const toArray = (v: Vec3): [number, number, number] => [v.x, v.y, v.z];

// ─── Arithmetic ────────────────────────────────────────────────────

export const add = (a: Vec3, b: Vec3): Vec3 => vec3(a.x + b.x, a.y + b.y, a.z + b.z);
export const sub = (a: Vec3, b: Vec3): Vec3 => vec3(a.x - b.x, a.y - b.y, a.z - b.z);
export const neg = (a: Vec3): Vec3 => vec3(-a.x, -a.y, -a.z);
export const scale = (a: Vec3, s: number): Vec3 => vec3(a.x * s, a.y * s, a.z * s);

/** Component-wise multiplication. Distinct from `scale`. */
export const mul = (a: Vec3, b: Vec3): Vec3 => vec3(a.x * b.x, a.y * b.y, a.z * b.z);

/** Component-wise division. Returns `Infinity`/`NaN` on divide-by-zero. */
export const div = (a: Vec3, b: Vec3): Vec3 => vec3(a.x / b.x, a.y / b.y, a.z / b.z);

// ─── Vector ops ────────────────────────────────────────────────────

export const dot = (a: Vec3, b: Vec3): number => a.x * b.x + a.y * b.y + a.z * b.z;

export const cross = (a: Vec3, b: Vec3): Vec3 =>
  vec3(a.y * b.z - a.z * b.y, a.z * b.x - a.x * b.z, a.x * b.y - a.y * b.x);

export const lengthSq = (a: Vec3): number => dot(a, a);
export const length = (a: Vec3): number => Math.sqrt(lengthSq(a));

export const distance = (a: Vec3, b: Vec3): number => length(sub(a, b));
export const distanceSq = (a: Vec3, b: Vec3): number => lengthSq(sub(a, b));

/**
 * Unit-length version of `a`. Returns `ZERO` for zero-length input
 * (rather than throwing or returning NaN). Callers that need to
 * distinguish "valid zero" from "valid normalized" should check
 * `lengthSq(a) > 0` first.
 */
export const normalize = (a: Vec3): Vec3 => {
  const len = length(a);
  return len === 0 ? ZERO : scale(a, 1 / len);
};

/** Linear interpolation: `(1-t)·a + t·b`. Not clamped — `t` outside [0,1] extrapolates. */
export const lerp = (a: Vec3, b: Vec3, t: number): Vec3 => add(scale(a, 1 - t), scale(b, t));

/**
 * Project `a` onto `onto`. Returns the component of `a` parallel to
 * `onto`. If `onto` is zero, returns `ZERO`.
 */
export const project = (a: Vec3, onto: Vec3): Vec3 => {
  const ls = lengthSq(onto);
  if (ls === 0) return ZERO;
  return scale(onto, dot(a, onto) / ls);
};

/**
 * Component of `a` perpendicular to `onto`. `a = project(a, onto) + reject(a, onto)`.
 */
export const reject = (a: Vec3, onto: Vec3): Vec3 => sub(a, project(a, onto));

/**
 * Reflect `a` across the plane normal to `n`. Mirror-symmetry helper.
 * `n` should be unit length; correctness degrades quadratically in `|n|`.
 */
export const reflect = (a: Vec3, n: Vec3): Vec3 => sub(a, scale(n, 2 * dot(a, n)));

/**
 * Angle in radians between `a` and `b`, in [0, π]. Returns 0 if either
 * is zero-length (rather than NaN from acos).
 */
export const angleBetween = (a: Vec3, b: Vec3): number => {
  const denom = length(a) * length(b);
  if (denom === 0) return 0;
  // Clamp to absorb f64 drift past ±1.
  const cos = Math.max(-1, Math.min(1, dot(a, b) / denom));
  return Math.acos(cos);
};

/** Component-wise minimum. Useful for AABB construction. */
export const min = (a: Vec3, b: Vec3): Vec3 =>
  vec3(Math.min(a.x, b.x), Math.min(a.y, b.y), Math.min(a.z, b.z));

/** Component-wise maximum. */
export const max = (a: Vec3, b: Vec3): Vec3 =>
  vec3(Math.max(a.x, b.x), Math.max(a.y, b.y), Math.max(a.z, b.z));

/** Component-wise absolute value. */
export const abs = (a: Vec3): Vec3 => vec3(Math.abs(a.x), Math.abs(a.y), Math.abs(a.z));

// ─── Comparisons ───────────────────────────────────────────────────

/** Bit-exact equality. For tolerant comparison use `approxEquals`. */
export const equals = (a: Vec3, b: Vec3): boolean => a.x === b.x && a.y === b.y && a.z === b.z;

/**
 * Tolerant equality. Default tolerance `1e-9` is comfortable for
 * the kernel's snap grid; increase for noisy intermediate results.
 */
export const approxEquals = (a: Vec3, b: Vec3, tol = 1e-9): boolean =>
  Math.abs(a.x - b.x) <= tol && Math.abs(a.y - b.y) <= tol && Math.abs(a.z - b.z) <= tol;

// ─── Type guard ────────────────────────────────────────────────────

export const isVec3 = (v: unknown): v is Vec3 =>
  typeof v === 'object' &&
  v !== null &&
  typeof (v as Vec3).x === 'number' &&
  typeof (v as Vec3).y === 'number' &&
  typeof (v as Vec3).z === 'number';
