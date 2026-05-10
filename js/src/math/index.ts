/**
 * Pure-TypeScript math primitives for the kernel and graph runtime.
 *
 * Each module is namespaced at the top level so the names that would
 * collide (`length`, `distance`, `project`, `pointAt`) read clearly
 * at the call site:
 *
 * ```ts
 * import { Vec3, Plane, Line, Transform } from 'knot-cad/math';
 * const v   = Vec3.normalize(Vec3.sub(b, a));
 * const pl  = Plane.plane(origin, normal);
 * const t   = Transform.translation(v);
 * const hit = Line.intersectPlane(line, pl);
 * ```
 *
 * The interfaces are accessible both as `Vec3.Vec3` (via the
 * namespace) and via direct imports from the per-module path:
 *
 * ```ts
 * import type { Vec3 } from 'knot-cad/math/vec3';
 * function f(p: Vec3): number { ... }
 * ```
 */

export * as Vec3 from './vec3';
export * as Plane from './plane';
export * as Line from './line';
export * as Transform from './transform';
