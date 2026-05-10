import type { Knot } from '../../kernel';
import { defineNode } from './define';

/**
 * Two-profile loft. Both profiles must share outer-loop vertex count.
 *
 * Once List&lt;Brep&gt; ports land in M3, this and `Loft3` collapse into a
 * single variadic `Loft` node taking a list-of-profiles input.
 */
export const makeLoft2Node = (knot: Knot) =>
  defineNode({
    id: 'knot.loft2',
    label: 'Loft (2 profiles)',
    inputs: {
      a: { kind: 'brep' },
      b: { kind: 'brep' },
    },
    outputs: { brep: { kind: 'brep' } },
    evaluate: ({ a, b }) => ({ brep: knot.loft([a, b]) }),
  });

/** Three-profile loft. */
export const makeLoft3Node = (knot: Knot) =>
  defineNode({
    id: 'knot.loft3',
    label: 'Loft (3 profiles)',
    inputs: {
      a: { kind: 'brep' },
      b: { kind: 'brep' },
      c: { kind: 'brep' },
    },
    outputs: { brep: { kind: 'brep' } },
    evaluate: ({ a, b, c }) => ({ brep: knot.loft([a, b, c]) }),
  });
