/**
 * Modification nodes — operations that take a Brep and produce a new
 * Brep with localized changes (fillet, chamfer). Includes an edge
 * enumerator that surfaces every edge of a Brep as a list so users
 * can fillet all edges, or filter the list before piping in.
 *
 * Edges flow on the graph as plain `EdgeRef = { start: Vec3, end: Vec3 }`
 * objects inside a `list` port. The Fillet / Chamfer nodes assert that
 * shape at evaluate time so a misconnected list produces a helpful
 * error instead of a confusing kernel error downstream.
 */
import type { Knot, EdgeRef, Vec3 } from '../../kernel';
import { defineNode } from './define';

/** Enumerate every unique edge of a BRep as a list of `EdgeRef`. */
export const BrepEdgesNode = defineNode({
  id: 'core.brep.edges',
  label: 'Brep Edges',
  inputs: {
    brep: { kind: 'brep' as const },
  },
  outputs: {
    edges: { kind: 'list' as const },
    count: { kind: 'number' as const },
  },
  evaluate: ({ brep }) => {
    const edges = brep.edges();
    return { edges, count: edges.length };
  },
});

/**
 * Fillet (round) the given edges with a constant radius. The `edges`
 * input expects a list of `EdgeRef = { start: Vec3, end: Vec3 }` —
 * typically the output of `core.brep.edges`, optionally filtered.
 *
 * Both adjacent faces of each filleted edge must be planar; non-planar
 * edges produce a kernel error which propagates as an error port.
 */
export const makeFilletNode = (_knot: Knot) =>
  defineNode({
    id: 'core.brep.fillet',
    label: 'Fillet',
    inputs: {
      brep: { kind: 'brep' as const },
      edges: { kind: 'list' as const, default: [] },
      radius: { kind: 'number' as const, default: 0.1 },
    },
    outputs: { brep: { kind: 'brep' as const } },
    evaluate: ({ brep, edges, radius }) => {
      const refs = coerceEdgeRefs(edges);
      return { brep: brep.fillet(refs, radius) };
    },
  });

/** Chamfer (bevel) the given edges with a constant distance. */
export const makeChamferNode = (_knot: Knot) =>
  defineNode({
    id: 'core.brep.chamfer',
    label: 'Chamfer',
    inputs: {
      brep: { kind: 'brep' as const },
      edges: { kind: 'list' as const, default: [] },
      distance: { kind: 'number' as const, default: 0.1 },
    },
    outputs: { brep: { kind: 'brep' as const } },
    evaluate: ({ brep, edges, distance }) => {
      const refs = coerceEdgeRefs(edges);
      return { brep: brep.chamfer(refs, distance) };
    },
  });

/**
 * Validate that every element of a list-port value has the
 * `{ start, end }` shape EdgeRef expects. Throws with a useful
 * message otherwise (which the evaluator promotes to an error
 * port that propagates downstream).
 */
function coerceEdgeRefs(value: unknown[]): EdgeRef[] {
  const out: EdgeRef[] = [];
  for (let i = 0; i < value.length; i++) {
    const v = value[i];
    if (!isEdgeRef(v)) {
      throw new Error(
        `edges[${i}] is not an EdgeRef (expected { start: Vec3, end: Vec3 })`,
      );
    }
    out.push(v);
  }
  return out;
}

function isEdgeRef(v: unknown): v is EdgeRef {
  if (typeof v !== 'object' || v === null) return false;
  const o = v as { start?: unknown; end?: unknown };
  return isVec3(o.start) && isVec3(o.end);
}

function isVec3(v: unknown): v is Vec3 {
  if (typeof v !== 'object' || v === null) return false;
  const o = v as { x?: unknown; y?: unknown; z?: unknown };
  return typeof o.x === 'number' && typeof o.y === 'number' && typeof o.z === 'number';
}

