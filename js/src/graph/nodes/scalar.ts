import { defineNode } from './define';

/** A literal number source. The graph wires its output to anything expecting a number. */
export const NumberNode = defineNode({
  id: 'core.number',
  label: 'Number',
  inputs: { value: { kind: 'number', default: 0 } },
  outputs: { value: { kind: 'number' } },
  evaluate: ({ value }) => ({ value }),
});
