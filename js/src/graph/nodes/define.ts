import type { NodeDef, InputMap, OutputMap } from '../types';

/** Identity helper — gives TypeScript the input/output port shape for inference. */
export function defineNode<I extends InputMap, O extends OutputMap>(d: NodeDef<I, O>): NodeDef<I, O> {
  return d;
}
