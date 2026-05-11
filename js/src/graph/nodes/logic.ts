/**
 * Logic nodes: comparison, boolean operators, conditional select.
 *
 * Pure-JS, no kernel dependencies. These enable conditional flow in
 * the graph editor — gate a downstream operation on a comparison, or
 * pick one of two values based on a boolean.
 *
 * "If" is provided per-port-kind (number, bool) rather than as a single
 * polymorphic node because the port system is type-driven. Add more
 * `IfX` variants as the need arises.
 */
import { defineNode } from './define';

// ── Comparison: number × number → bool ───────────────────────────

/**
 * Floating-point comparisons use exact equality. For tolerant
 * equality, route through a Subtract + Abs + LessThan compared to
 * a tolerance — explicit beats hidden epsilon magic.
 */
export const EqualsNode = defineNode({
  id: 'logic.equals',
  label: 'Equals',
  inputs: {
    a: { kind: 'number' as const, default: 0 },
    b: { kind: 'number' as const, default: 0 },
  },
  outputs: { result: { kind: 'bool' as const } },
  evaluate: ({ a, b }) => ({ result: a === b }),
});

export const NotEqualsNode = defineNode({
  id: 'logic.notEquals',
  label: 'Not Equals',
  inputs: {
    a: { kind: 'number' as const, default: 0 },
    b: { kind: 'number' as const, default: 0 },
  },
  outputs: { result: { kind: 'bool' as const } },
  evaluate: ({ a, b }) => ({ result: a !== b }),
});

export const GreaterNode = defineNode({
  id: 'logic.greater',
  label: 'Greater',
  inputs: {
    a: { kind: 'number' as const, default: 0 },
    b: { kind: 'number' as const, default: 0 },
  },
  outputs: { result: { kind: 'bool' as const } },
  evaluate: ({ a, b }) => ({ result: a > b }),
});

export const LessNode = defineNode({
  id: 'logic.less',
  label: 'Less',
  inputs: {
    a: { kind: 'number' as const, default: 0 },
    b: { kind: 'number' as const, default: 0 },
  },
  outputs: { result: { kind: 'bool' as const } },
  evaluate: ({ a, b }) => ({ result: a < b }),
});

export const GreaterEqualNode = defineNode({
  id: 'logic.greaterEqual',
  label: 'Greater or Equal',
  inputs: {
    a: { kind: 'number' as const, default: 0 },
    b: { kind: 'number' as const, default: 0 },
  },
  outputs: { result: { kind: 'bool' as const } },
  evaluate: ({ a, b }) => ({ result: a >= b }),
});

export const LessEqualNode = defineNode({
  id: 'logic.lessEqual',
  label: 'Less or Equal',
  inputs: {
    a: { kind: 'number' as const, default: 0 },
    b: { kind: 'number' as const, default: 0 },
  },
  outputs: { result: { kind: 'bool' as const } },
  evaluate: ({ a, b }) => ({ result: a <= b }),
});

// ── Boolean operators: bool × bool → bool ────────────────────────

export const AndNode = defineNode({
  id: 'logic.and',
  label: 'And',
  inputs: {
    a: { kind: 'bool' as const, default: false },
    b: { kind: 'bool' as const, default: false },
  },
  outputs: { result: { kind: 'bool' as const } },
  evaluate: ({ a, b }) => ({ result: a && b }),
});

export const OrNode = defineNode({
  id: 'logic.or',
  label: 'Or',
  inputs: {
    a: { kind: 'bool' as const, default: false },
    b: { kind: 'bool' as const, default: false },
  },
  outputs: { result: { kind: 'bool' as const } },
  evaluate: ({ a, b }) => ({ result: a || b }),
});

export const NotNode = defineNode({
  id: 'logic.not',
  label: 'Not',
  inputs: {
    value: { kind: 'bool' as const, default: false },
  },
  outputs: { result: { kind: 'bool' as const } },
  evaluate: ({ value }) => ({ result: !value }),
});

export const XorNode = defineNode({
  id: 'logic.xor',
  label: 'Xor',
  inputs: {
    a: { kind: 'bool' as const, default: false },
    b: { kind: 'bool' as const, default: false },
  },
  outputs: { result: { kind: 'bool' as const } },
  evaluate: ({ a, b }) => ({ result: a !== b }),
});

// ── Conditional select ────────────────────────────────────────────

/**
 * Pick `whenTrue` when `condition` is true, otherwise `whenFalse`.
 * Both branches always evaluate (the graph is data-flow, not
 * control-flow) — use this for value selection, not as a side-effect
 * gate.
 */
export const IfNumberNode = defineNode({
  id: 'logic.ifNumber',
  label: 'If (Number)',
  inputs: {
    condition: { kind: 'bool' as const, default: false },
    whenTrue:  { kind: 'number' as const, default: 1 },
    whenFalse: { kind: 'number' as const, default: 0 },
  },
  outputs: { result: { kind: 'number' as const } },
  evaluate: ({ condition, whenTrue, whenFalse }) => ({
    result: condition ? whenTrue : whenFalse,
  }),
});

export const IfBoolNode = defineNode({
  id: 'logic.ifBool',
  label: 'If (Bool)',
  inputs: {
    condition: { kind: 'bool' as const, default: false },
    whenTrue:  { kind: 'bool' as const, default: true },
    whenFalse: { kind: 'bool' as const, default: false },
  },
  outputs: { result: { kind: 'bool' as const } },
  evaluate: ({ condition, whenTrue, whenFalse }) => ({
    result: condition ? whenTrue : whenFalse,
  }),
});
