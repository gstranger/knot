export { createKnot, Brep, Curve } from './kernel';
export type {
  Knot,
  MeshData,
  Vec3,
  BoundingBox,
  SphereOptions,
  CylinderOptions,
  ExtrudeOptions,
  RevolveOptions,
  EdgeRef,
  TessellateOptions,
  InitInput,
  CurveType,
  ArcOptions,
  NurbsCurveOptions,
} from './kernel';

export { KnotError, parseKnotError, isKnotError } from './error';
export type { KnotErrorCode, KnotErrorKind } from './error';
