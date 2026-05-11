import type { Knot, Brep } from '../../kernel';
import type { NodeDef, InputMap, OutputMap } from '../types';
import { Registry } from '../graph';

// ── Core ─────────────────────────────────────────────────────────
import { NumberNode } from './scalar';
import { Vec3Node } from './vec3';
import { TranslateNode } from './translate';
import { ViewNode } from './view';

// ── Math ─────────────────────────────────────────────────────────
import {
  SliderNode, ToggleNode,
  AddNode, SubtractNode, MultiplyNode, DivideNode as DivideNumNode,
  NegateNode, AbsNode, SinNode, CosNode, RemapNode, ExpressionNode,
} from './math';

// ── Vector ───────────────────────────────────────────────────────
import {
  DeconstructVec3Node, AddVec3Node, SubVec3Node, ScaleVec3Node,
  CrossNode, DotNode, LengthNode, NormalizeNode,
} from './vector';

// ── Geometry (non-kernel) ────────────────────────────────────────
import { ScaleNode, RotateNode } from './geometry';

// ── Curve ────────────────────────────────────────────────────────
import {
  makeLineNode, makeArcNode, makeSweepNode,
  PointAtNode, DivideNode, OffsetNode,
} from './curve';

// ── Kernel primitives/ops ────────────────────────────────────────
import { makeBoxNode } from './box';
import { makeSphereNode } from './sphere';
import { makeBooleanNode } from './boolean';
import { makeCylinderNode, makeExtrudeNode, makeRevolveNode } from './geometry';
import { makeTriangleNode } from './profile';
import { makeLoft2Node, makeLoft3Node } from './loft';

// ── Re-exports ───────────────────────────────────────────────────
export { NumberNode, Vec3Node, TranslateNode, ViewNode };
export { SliderNode, ToggleNode, AddNode, SubtractNode, MultiplyNode, DivideNumNode, NegateNode, AbsNode, SinNode, CosNode, RemapNode, ExpressionNode };
export { DeconstructVec3Node, AddVec3Node, SubVec3Node, ScaleVec3Node, CrossNode, DotNode, LengthNode, NormalizeNode };
export { ScaleNode, RotateNode };
export { makeBoxNode, makeSphereNode, makeBooleanNode };
export { makeCylinderNode, makeExtrudeNode, makeRevolveNode };
export { makeLineNode, makeArcNode, makeSweepNode, PointAtNode, DivideNode, OffsetNode };
export { makeTriangleNode };
export { makeLoft2Node, makeLoft3Node };
export type { ViewConstants } from './view';
export type { BooleanOp } from './boolean';
export { defineNode } from './define';

/**
 * Build a registry pre-populated with the full node set.
 * Kernel-backed nodes are bound to the supplied `Knot` instance.
 */
export function buildDefaultRegistry(knot: Knot): Registry {
  const r = new Registry();
  const defs: ReadonlyArray<NodeDef<InputMap, OutputMap>> = [
    // Core
    NumberNode, Vec3Node, TranslateNode, ViewNode,
    // Math
    SliderNode, ToggleNode,
    AddNode, SubtractNode, MultiplyNode, DivideNumNode,
    NegateNode, AbsNode, SinNode, CosNode, RemapNode, ExpressionNode,
    // Vector
    DeconstructVec3Node, AddVec3Node, SubVec3Node, ScaleVec3Node,
    CrossNode, DotNode, LengthNode, NormalizeNode,
    // Geometry (non-kernel)
    ScaleNode, RotateNode,
    // Curve (non-kernel)
    PointAtNode, DivideNode, OffsetNode,
    // Kernel primitives
    makeBoxNode(knot), makeSphereNode(knot), makeCylinderNode(knot),
    makeBooleanNode(knot),
    // Kernel operations
    makeExtrudeNode(knot), makeRevolveNode(knot),
    makeSweepNode(knot),
    makeLoft2Node(knot), makeLoft3Node(knot),
    // Kernel curves
    makeLineNode(knot), makeArcNode(knot),
    // Profile
    makeTriangleNode(knot),
  ];
  for (const d of defs) r.register(d);
  return r;
}

export type { Brep };
