import type { Knot } from '../../kernel';
import { ZERO } from '../../math/vec3';
import { defineNode } from './define';

export const makeSphereNode = (knot: Knot) =>
  defineNode({
    id: 'knot.sphere',
    label: 'Sphere',
    inputs: {
      center: { kind: 'vec3', default: ZERO },
      radius: { kind: 'number', default: 1 },
    },
    outputs: { brep: { kind: 'brep' } },
    evaluate: ({ center, radius }) => ({ brep: knot.sphere({ center, radius }) }),
  });
