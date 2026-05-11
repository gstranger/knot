/**
 * Typed wrapper around kernel errors that crossed the WASM boundary.
 *
 * The Rust kernel's `KernelError` is converted to a `JsError` with a
 * stable message format: `"E<code>:<kind>:<detail>"`, e.g.
 *
 *   "E404:OperationFailed:boolean budget exceeded at stage SSI"
 *
 * On the JS side the message becomes the thrown Error's `.message`.
 * `parseKnotError` inspects an unknown caught error and, if its message
 * matches the kernel format, returns a typed `KnotError` with split
 * `code` / `kind` / `detail` fields. Otherwise it returns `null`.
 *
 * Usage:
 *
 *   try {
 *     const result = knot.union(a, b);
 *   } catch (e) {
 *     const ke = parseKnotError(e);
 *     if (ke?.code === 'E404') {
 *       // SSI timed out — retry with relaxed tolerance, or fall back
 *     } else if (ke?.code === 'E400') {
 *       // Empty result — semantically valid for some inputs
 *     } else {
 *       throw e;
 *     }
 *   }
 */

/** Numeric/code → semantic-bucket name (matches Rust ErrorCode enum). */
export type KnotErrorCode =
  // Geometry
  | 'E100' // InvalidKnotVector
  | 'E101' // InsufficientControlPoints
  | 'E102' // NegativeWeight
  | 'E103' // DegenerateCurve
  | 'E104' // DegenerateSurface
  // Topology
  | 'E200' // OpenShell
  | 'E201' // NonManifoldEdge
  | 'E202' // InconsistentOrientation
  | 'E203' // DanglingReference
  | 'E204' // EulerViolation
  | 'E205' // LoopNotClosed
  // Intersection
  | 'E300' // NoConvergence
  | 'E301' // MissedBranch
  | 'E302' // DegenerateIntersection
  // Operation
  | 'E400' // EmptyResult
  | 'E401' // SelfIntersecting
  | 'E402' // UnsupportedConfiguration
  | 'E403' // HistoryConflict
  | 'E404' // OperationTimeout
  // Input
  | 'E500' // MalformedInput
  | 'E501' // UnsupportedFormat
  // Io (no specific code; Rust uses placeholder)
  | 'E000';

/** Kernel error variant name (matches Rust KernelError variant names). */
export type KnotErrorKind =
  | 'InvalidGeometry'
  | 'TopoInconsistency'
  | 'IntersectionFailure'
  | 'OperationFailed'
  | 'InvalidInput'
  | 'Degenerate'
  | 'NumericalFailure'
  | 'Io';

/** Structured kernel error. Instances come from `parseKnotError`. */
export class KnotError extends Error {
  readonly code: KnotErrorCode;
  readonly kind: KnotErrorKind;
  readonly detail: string;

  constructor(code: KnotErrorCode, kind: KnotErrorKind, detail: string) {
    super(`${code}:${kind}:${detail}`);
    this.name = 'KnotError';
    this.code = code;
    this.kind = kind;
    this.detail = detail;
  }
}

const KERNEL_ERROR_RE = /^(E\d{3}):([A-Za-z]+):([\s\S]*)$/;

/**
 * If `e` is an Error whose `.message` matches the kernel error format,
 * return a typed `KnotError`. Otherwise return `null`.
 *
 * Never throws.
 */
export function parseKnotError(e: unknown): KnotError | null {
  if (!(e instanceof Error)) return null;
  const m = KERNEL_ERROR_RE.exec(e.message);
  if (!m) return null;
  return new KnotError(m[1]! as KnotErrorCode, m[2]! as KnotErrorKind, m[3]!);
}

/**
 * Convenience type guard. Equivalent to `parseKnotError(e) !== null`.
 */
export function isKnotError(e: unknown): boolean {
  return parseKnotError(e) !== null;
}
