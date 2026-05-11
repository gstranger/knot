import type { Vec3 } from '../math/vec3';
import type { Brep, Curve } from '../kernel';

// ── Port kinds ────────────────────────────────────────────────

/**
 * The set of value kinds that can flow on a wire.
 *
 * Adding a kind: extend this union AND `PortValueByKind`. The evaluator
 * and validation are kind-driven, so they pick up new kinds automatically.
 */
export type PortKind = 'number' | 'bool' | 'vec3' | 'brep' | 'curve' | 'list';

/** Maps port kind → unwrapped value type seen by node `evaluate`. */
export interface PortValueByKind {
  number: number;
  bool: boolean;
  vec3: Vec3;
  brep: Brep;
  curve: Curve;
  list: unknown[];
}

/** Per-kind value port. Spelled out so TypeScript can narrow on `port.kind`. */
export type ValuePort = {
  [K in PortKind]: { readonly kind: K; readonly value: PortValueByKind[K] }
}[PortKind];

/** Sentinel that propagates through wires when an upstream node fails. */
export interface ErrorPort {
  readonly kind: 'error';
  readonly message: string;
  readonly nodeId?: string;
}

/** A typed value flowing on a wire, or an error sentinel. */
export type Port = ValuePort | ErrorPort;

export const errPort = (message: string, nodeId?: string): ErrorPort =>
  ({ kind: 'error', message, nodeId });

export const isError = (p: Port): p is ErrorPort => p.kind === 'error';

// ── Node definitions ──────────────────────────────────────────

/**
 * Declaration of one input port. If `default` is omitted the port is required:
 * an unwired, unset port produces an error rather than evaluating with a fallback.
 */
export interface InputSpec<K extends PortKind> {
  readonly kind: K;
  readonly default?: PortValueByKind[K];
  readonly label?: string;
}

/** Declaration of one output port. */
export interface OutputSpec<K extends PortKind> {
  readonly kind: K;
  readonly label?: string;
}

export type InputMap = Readonly<Record<string, InputSpec<PortKind>>>;
export type OutputMap = Readonly<Record<string, OutputSpec<PortKind>>>;

/** The unwrapped values delivered to `evaluate`, keyed by input port name. */
export type EvalInputs<I extends InputMap> = {
  readonly [K in keyof I]: PortValueByKind[I[K]['kind']];
};

/** The unwrapped values returned by `evaluate`, keyed by output port name. */
export type EvalOutputs<O extends OutputMap> = {
  readonly [K in keyof O]: PortValueByKind[O[K]['kind']];
};

export interface EvalContext {
  readonly nodeId: string;
  readonly signal?: AbortSignal;
  /**
   * The node's per-instance constants, raw. Mostly used as fallback
   * values for unwired input ports, but also a usable escape hatch for
   * sink nodes that need non-port data (e.g. a callback for a View node).
   */
  readonly constants: Readonly<Record<string, unknown>>;
}

/**
 * The static definition of a node type. Pure data + an `evaluate` function.
 * Node defs are registered once; a graph contains many `NodeInstance`s referencing them.
 */
export interface NodeDef<I extends InputMap = InputMap, O extends OutputMap = OutputMap> {
  readonly id: string;
  readonly label?: string;
  readonly inputs: I;
  readonly outputs: O;
  evaluate(inputs: EvalInputs<I>, ctx: EvalContext): EvalOutputs<O> | Promise<EvalOutputs<O>>;
}

// ── Graph instances ───────────────────────────────────────────

export type NodeId = string;
export type PortName = string;

/** A node placed in a graph. Holds per-instance constant input values for unwired ports. */
export interface NodeInstance {
  readonly id: NodeId;
  readonly defId: string;
  /**
   * Constant values for inputs without a connected wire. Missing keys fall back
   * to the input's declared default.
   */
  readonly constants: Readonly<Partial<Record<PortName, unknown>>>;
}

/** Connection from one node's output to another node's input. */
export interface Wire {
  readonly fromNode: NodeId;
  readonly fromPort: PortName;
  readonly toNode: NodeId;
  readonly toPort: PortName;
}
