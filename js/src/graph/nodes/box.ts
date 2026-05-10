import type { Knot } from '../../kernel';
import { defineNode } from './define';

export const makeBoxNode = (knot: Knot) =>
  defineNode({
    id: 'knot.box',
    label: 'Box',
    inputs: {
      sx: { kind: 'number', default: 1 },
      sy: { kind: 'number', default: 1 },
      sz: { kind: 'number', default: 1 },
    },
    outputs: { brep: { kind: 'brep' } },
    evaluate: ({ sx, sy, sz }) => ({ brep: knot.box(sx, sy, sz) }),
  });
