import type { Knot } from '../../kernel';
import { defineNode } from './define';

export type BooleanOp = 'union' | 'intersection' | 'subtraction';

export const makeBooleanNode = (knot: Knot) =>
  defineNode({
    id: 'knot.boolean',
    label: 'Boolean',
    inputs: {
      a: { kind: 'brep' },
      b: { kind: 'brep' },
      // Encoded as number (0=union, 1=intersection, 2=subtraction) so it can
      // be wired from a slider/picker. M0 has no enum kind.
      op: { kind: 'number', default: 0 },
    },
    outputs: { brep: { kind: 'brep' } },
    evaluate: ({ a, b, op }) => {
      const opName = decodeOp(op);
      switch (opName) {
        case 'union':        return { brep: knot.union(a, b) };
        case 'intersection': return { brep: knot.intersection(a, b) };
        case 'subtraction':  return { brep: knot.subtraction(a, b) };
      }
    },
  });

const decodeOp = (n: number): BooleanOp => {
  switch (Math.round(n)) {
    case 0: return 'union';
    case 1: return 'intersection';
    case 2: return 'subtraction';
    default: throw new Error(`Boolean op code out of range: ${n}`);
  }
};
