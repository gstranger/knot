import type { Knot, Brep } from '../../kernel';
import type { NodeDef, InputMap, OutputMap } from '../types';
import { Registry } from '../graph';

import { NumberNode } from './scalar';
import { Vec3Node } from './vec3';
import { TranslateNode } from './translate';
import { ViewNode } from './view';
import { makeBoxNode } from './box';
import { makeSphereNode } from './sphere';
import { makeBooleanNode } from './boolean';
import {
  makeLineNode, makeArcNode, makeSweepNode, PointAtNode, DivideNode,
} from './curve';
import { makeTriangleNode } from './profile';

export { NumberNode, Vec3Node, TranslateNode, ViewNode };
export { makeBoxNode, makeSphereNode, makeBooleanNode };
export { makeLineNode, makeArcNode, makeSweepNode, PointAtNode, DivideNode };
export { makeTriangleNode };
export type { ViewConstants } from './view';
export type { BooleanOp } from './boolean';
export { defineNode } from './define';

/**
 * Build a registry pre-populated with the M0 node set.
 * Kernel-backed nodes are bound to the supplied `Knot` instance.
 */
export function buildDefaultRegistry(knot: Knot): Registry {
  const r = new Registry();
  const defs: ReadonlyArray<NodeDef<InputMap, OutputMap>> = [
    NumberNode,
    Vec3Node,
    TranslateNode,
    ViewNode,
    PointAtNode,
    DivideNode,
    makeBoxNode(knot),
    makeSphereNode(knot),
    makeBooleanNode(knot),
    makeLineNode(knot),
    makeArcNode(knot),
    makeSweepNode(knot),
    makeTriangleNode(knot),
  ];
  for (const d of defs) r.register(d);
  return r;
}

// Re-export Brep for nodes/graph consumers that care.
export type { Brep };
