import { vec3 } from '../../math/vec3';
import { defineNode } from './define';

/** Compose three numbers into a Vec3. */
export const Vec3Node = defineNode({
  id: 'core.vec3',
  label: 'Vec3',
  inputs: {
    x: { kind: 'number', default: 0 },
    y: { kind: 'number', default: 0 },
    z: { kind: 'number', default: 0 },
  },
  outputs: { value: { kind: 'vec3' } },
  evaluate: ({ x, y, z }) => ({ value: vec3(x, y, z) }),
});
