import { vec3, add, sub, scale, cross, dot, length, normalize, ZERO } from '../../math/vec3';
import { defineNode } from './define';

/** Decompose a Vec3 into its x, y, z components. */
export const DeconstructVec3Node = defineNode({
  id: 'core.deconstructVec3',
  label: 'Deconstruct',
  inputs: { value: { kind: 'vec3', default: ZERO } },
  outputs: {
    x: { kind: 'number' },
    y: { kind: 'number' },
    z: { kind: 'number' },
  },
  evaluate: ({ value }) => ({ x: value.x, y: value.y, z: value.z }),
});

export const AddVec3Node = defineNode({
  id: 'vec3.add',
  label: 'Add Vec3',
  inputs: {
    a: { kind: 'vec3', default: ZERO },
    b: { kind: 'vec3', default: ZERO },
  },
  outputs: { result: { kind: 'vec3' } },
  evaluate: ({ a, b }) => ({ result: add(a, b) }),
});

export const SubVec3Node = defineNode({
  id: 'vec3.subtract',
  label: 'Subtract Vec3',
  inputs: {
    a: { kind: 'vec3', default: ZERO },
    b: { kind: 'vec3', default: ZERO },
  },
  outputs: { result: { kind: 'vec3' } },
  evaluate: ({ a, b }) => ({ result: sub(a, b) }),
});

export const ScaleVec3Node = defineNode({
  id: 'vec3.scale',
  label: 'Scale Vec3',
  inputs: {
    vec: { kind: 'vec3', default: ZERO },
    factor: { kind: 'number', default: 1 },
  },
  outputs: { result: { kind: 'vec3' } },
  evaluate: ({ vec, factor }) => ({ result: scale(vec, factor) }),
});

export const CrossNode = defineNode({
  id: 'vec3.cross',
  label: 'Cross Product',
  inputs: {
    a: { kind: 'vec3', default: ZERO },
    b: { kind: 'vec3', default: ZERO },
  },
  outputs: { result: { kind: 'vec3' } },
  evaluate: ({ a, b }) => ({ result: cross(a, b) }),
});

export const DotNode = defineNode({
  id: 'vec3.dot',
  label: 'Dot Product',
  inputs: {
    a: { kind: 'vec3', default: ZERO },
    b: { kind: 'vec3', default: ZERO },
  },
  outputs: { result: { kind: 'number' } },
  evaluate: ({ a, b }) => ({ result: dot(a, b) }),
});

export const LengthNode = defineNode({
  id: 'vec3.length',
  label: 'Length',
  inputs: { vec: { kind: 'vec3', default: ZERO } },
  outputs: { result: { kind: 'number' } },
  evaluate: ({ vec }) => ({ result: length(vec) }),
});

export const NormalizeNode = defineNode({
  id: 'vec3.normalize',
  label: 'Normalize',
  inputs: { vec: { kind: 'vec3', default: ZERO } },
  outputs: { result: { kind: 'vec3' } },
  evaluate: ({ vec }) => ({ result: normalize(vec) }),
});
