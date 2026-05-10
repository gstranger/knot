import type { Knot } from '../../kernel';
import { ZERO, X_AXIS, Z_AXIS } from '../../math/vec3';
import { defineNode } from './define';

export const makeLineNode = (knot: Knot) =>
  defineNode({
    id: 'knot.line',
    label: 'Line',
    inputs: {
      a: { kind: 'vec3', default: ZERO },
      b: { kind: 'vec3', default: X_AXIS },
    },
    outputs: { curve: { kind: 'curve' } },
    evaluate: ({ a, b }) => ({ curve: knot.line(a, b) }),
  });

export const makeArcNode = (knot: Knot) =>
  defineNode({
    id: 'knot.arc',
    label: 'Arc',
    inputs: {
      center:     { kind: 'vec3',   default: ZERO },
      normal:     { kind: 'vec3',   default: Z_AXIS },
      refAxis:    { kind: 'vec3',   default: X_AXIS },
      radius:     { kind: 'number', default: 1 },
      startAngle: { kind: 'number', default: 0 },
      endAngle:   { kind: 'number', default: Math.PI * 2 },
    },
    outputs: { curve: { kind: 'curve' } },
    evaluate: (i) => ({
      curve: knot.arc({
        center: i.center, normal: i.normal, refAxis: i.refAxis,
        radius: i.radius, startAngle: i.startAngle, endAngle: i.endAngle,
      }),
    }),
  });

export const makeSweepNode = (knot: Knot) =>
  defineNode({
    id: 'knot.sweep',
    label: 'Sweep',
    inputs: {
      profile: { kind: 'brep' },
      rail:    { kind: 'curve' },
    },
    outputs: { brep: { kind: 'brep' } },
    evaluate: ({ profile, rail }) => ({ brep: knot.sweep(profile, rail) }),
  });

/** Divide a curve into `n` equal-parameter segments; output is the first parameter. */
export const PointAtNode = defineNode({
  id: 'core.curve.pointAt',
  label: 'Point At',
  inputs: {
    curve: { kind: 'curve' },
    t:     { kind: 'number', default: 0 },
  },
  outputs: { point: { kind: 'vec3' } },
  evaluate: ({ curve, t }) => ({ point: curve.pointAt(t) }),
});

/**
 * Divide a curve into `n` equal-parameter segments.
 *
 * For M0/M1 we don't yet have a list port kind, so this node returns the
 * `i`-th sample point — useful enough for "place this thing at 1/4 along the
 * curve" graphs. Once data trees land in M3 this becomes a List<Vec3>.
 */
export const DivideNode = defineNode({
  id: 'core.curve.divide',
  label: 'Divide (point at i/n)',
  inputs: {
    curve: { kind: 'curve' },
    n:     { kind: 'number', default: 10 },
    i:     { kind: 'number', default: 0 },
  },
  outputs: { point: { kind: 'vec3' } },
  evaluate: ({ curve, n, i }) => {
    const segments = Math.max(1, Math.round(n));
    const idx = Math.max(0, Math.min(segments, Math.round(i)));
    const params = curve.divide(segments);
    return { point: curve.pointAt(params[idx]!) };
  },
});
