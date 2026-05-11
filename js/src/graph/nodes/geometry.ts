import type { Knot } from '../../kernel';
import { ZERO, Z_AXIS, Y_AXIS } from '../../math/vec3';
import { defineNode } from './define';

export const makeCylinderNode = (knot: Knot) =>
  defineNode({
    id: 'knot.cylinder',
    label: 'Cylinder',
    inputs: {
      center: { kind: 'vec3', default: ZERO },
      radius: { kind: 'number', default: 1 },
      height: { kind: 'number', default: 2 },
    },
    outputs: { brep: { kind: 'brep' } },
    evaluate: ({ center, radius, height }) => ({
      brep: knot.cylinder({ center, radius, height }),
    }),
  });

export const makeExtrudeNode = (knot: Knot) =>
  defineNode({
    id: 'knot.extrude',
    label: 'Extrude',
    inputs: {
      profile: { kind: 'brep' },
      direction: { kind: 'vec3', default: Z_AXIS },
      distance: { kind: 'number', default: 1 },
    },
    outputs: { brep: { kind: 'brep' } },
    evaluate: ({ profile, direction, distance }) => ({
      brep: profile.extrude({ direction, distance }),
    }),
  });

export const makeRevolveNode = (knot: Knot) =>
  defineNode({
    id: 'knot.revolve',
    label: 'Revolve',
    inputs: {
      profile: { kind: 'brep' },
      axisOrigin: { kind: 'vec3', default: ZERO },
      axisDirection: { kind: 'vec3', default: Y_AXIS },
      angle: { kind: 'number', default: Math.PI * 2 },
    },
    outputs: { brep: { kind: 'brep' } },
    evaluate: ({ profile, axisOrigin, axisDirection, angle }) => ({
      brep: profile.revolve({ axisOrigin, axisDirection, angle }),
    }),
  });

export const ScaleNode = defineNode({
  id: 'core.scale',
  label: 'Scale',
  inputs: {
    brep: { kind: 'brep' },
    factor: { kind: 'number', default: 1 },
  },
  outputs: { brep: { kind: 'brep' } },
  evaluate: ({ brep, factor }) => ({ brep: brep.scale(factor) }),
});

export const RotateNode = defineNode({
  id: 'core.rotate',
  label: 'Rotate',
  inputs: {
    brep: { kind: 'brep' },
    axis: { kind: 'vec3', default: Z_AXIS },
    angle: { kind: 'number', default: 0 },
  },
  outputs: { brep: { kind: 'brep' } },
  evaluate: ({ brep, axis, angle }) => ({ brep: brep.rotate(axis, angle) }),
});
