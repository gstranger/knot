import { ZERO } from '../../math/vec3';
import { defineNode } from './define';

export const TranslateNode = defineNode({
  id: 'core.translate',
  label: 'Translate',
  inputs: {
    brep: { kind: 'brep' },
    offset: { kind: 'vec3', default: ZERO },
  },
  outputs: { brep: { kind: 'brep' } },
  evaluate: ({ brep, offset }) => ({ brep: brep.translate(offset.x, offset.y, offset.z) }),
});
