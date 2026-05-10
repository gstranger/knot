import type { Knot } from '../../kernel';
import { defineNode } from './define';

/**
 * A 3-point triangle profile. A stopgap until List<Vec3> ports land in M3 —
 * after that, a single `Profile` node with a points-list input replaces this.
 */
export const makeTriangleNode = (knot: Knot) =>
  defineNode({
    id: 'knot.triangle',
    label: 'Triangle',
    inputs: {
      a: { kind: 'vec3' },
      b: { kind: 'vec3' },
      c: { kind: 'vec3' },
    },
    outputs: { brep: { kind: 'brep' } },
    evaluate: ({ a, b, c }) => ({ brep: knot.profile([a, b, c]) }),
  });
