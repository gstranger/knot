import type { Graph } from './graph';
import type {
  NodeId,
  PortName,
  Port,
  PortKind,
  EvalContext,
  NodeDef,
  InputMap,
  OutputMap,
  PortValueByKind,
} from './types';
import { errPort, isError } from './types';
import { Brep, Curve } from '../kernel';

interface CacheEntry {
  /** Outputs from the most recent successful (or failed) evaluate call. */
  readonly outputs: Record<PortName, Port>;
}

export interface EvaluatorOptions {
  /** Optional logger for tracing which nodes evaluated each run. */
  readonly onEvaluate?: (nodeId: string, defId: string) => void;
  /** Optional abort signal forwarded to every node's evaluate. */
  readonly signal?: AbortSignal;
}

/**
 * Walks a Graph in topological order and evaluates dirty nodes,
 * caching outputs and propagating errors.
 *
 * Memory model (M0): brep outputs are owned by the cache. They are freed
 * when re-evaluated or evicted. External consumers (e.g. View) must extract
 * what they need (e.g. tessellate to a mesh) within a `run()` boundary —
 * the brep handle is not safe to hold across the next run. M1 will add
 * refcounted Owned&lt;T&gt; for consumers that need to retain.
 *
 * Concurrency: M0 evaluates sequentially in topo order. Independent-subtree
 * concurrency is a future optimization.
 */
export class Evaluator {
  private cache = new Map<NodeId, CacheEntry>();
  private dirty = new Set<NodeId>();
  private lastGraphVersion = -1;
  private cachedTopo: NodeId[] | null = null;

  constructor(private readonly opts: EvaluatorOptions = {}) {}

  /** Mark a node (and transitively its successors) for re-evaluation on next run. */
  markDirty(id: NodeId): void {
    this.dirty.add(id);
  }

  /** Read the most recent output for a port. Returns `undefined` if never evaluated. */
  getOutput(nodeId: NodeId, port: PortName): Port | undefined {
    return this.cache.get(nodeId)?.outputs[port];
  }

  /** Has this node been evaluated since the last graph mutation that affected it? */
  isClean(nodeId: NodeId): boolean {
    return this.cache.has(nodeId) && !this.dirty.has(nodeId);
  }

  /**
   * Evaluate every dirty node (and any node transitively downstream of one).
   * Returns the set of node IDs that were evaluated this run.
   */
  async run(graph: Graph): Promise<Set<NodeId>> {
    this.refreshTopo(graph);
    const evaluatedThisRun = new Set<NodeId>();

    for (const id of this.cachedTopo!) {
      const explicitlyDirty = this.dirty.has(id);
      const upstreamRecomputed = graph.predecessors(id).some(p => evaluatedThisRun.has(p));
      const neverEvaluated = !this.cache.has(id);
      if (!explicitlyDirty && !upstreamRecomputed && !neverEvaluated) continue;

      await this.evaluateOne(graph, id);
      evaluatedThisRun.add(id);
      this.dirty.delete(id);
    }
    return evaluatedThisRun;
  }

  /** Drop a node's cached outputs, releasing any owned resources. */
  evict(nodeId: NodeId): void {
    const entry = this.cache.get(nodeId);
    if (!entry) return;
    for (const port of Object.values(entry.outputs)) releasePortStorage(port);
    this.cache.delete(nodeId);
  }

  /** Drop everything. Call when discarding the evaluator. */
  dispose(): void {
    for (const id of [...this.cache.keys()]) this.evict(id);
    this.dirty.clear();
    this.cachedTopo = null;
    this.lastGraphVersion = -1;
  }

  // ── Internals ────────────────────────────────────────────────

  private refreshTopo(graph: Graph): void {
    if (graph.version === this.lastGraphVersion && this.cachedTopo) return;
    this.cachedTopo = graph.topoSort();
    // Drop cache for nodes that no longer exist in the graph.
    for (const id of [...this.cache.keys()]) {
      if (!graph.getNode(id)) this.evict(id);
    }
    this.lastGraphVersion = graph.version;
  }

  private async evaluateOne(graph: Graph, id: NodeId): Promise<void> {
    const def = graph.getDef(id);
    const node = graph.getNode(id)!;

    // Resolve every input — either from upstream cache or from constants/defaults.
    const inputs: Record<PortName, unknown> = {};
    let inputError: Port | null = null;

    for (const [portName, spec] of Object.entries(def.inputs)) {
      const wire = graph.incomingWire(id, portName);
      if (wire) {
        const upstream = this.cache.get(wire.fromNode)?.outputs[wire.fromPort];
        if (!upstream) {
          inputError = errPort(`upstream ${wire.fromNode}.${wire.fromPort} not evaluated`, id);
          break;
        }
        if (isError(upstream)) {
          inputError = errPort(upstream.message, id);
          break;
        }
        if (upstream.kind !== spec.kind) {
          inputError = errPort(
            `kind mismatch on ${id}.${portName}: expected ${spec.kind}, got ${upstream.kind}`,
            id,
          );
          break;
        }
        inputs[portName] = upstream.value;
      } else if (portName in node.constants) {
        inputs[portName] = node.constants[portName];
      } else if (spec.default !== undefined) {
        inputs[portName] = spec.default;
      } else {
        inputError = errPort(`required input '${portName}' has no wire and no constant`, id);
        break;
      }
    }

    // Build new outputs: poison everything if any input was errored.
    let newOutputs: Record<PortName, Port>;
    if (inputError) {
      newOutputs = poisonAll(def, inputError.kind === 'error' ? inputError.message : 'error', id);
    } else {
      const ctx: EvalContext = { nodeId: id, signal: this.opts.signal, constants: node.constants };
      try {
        this.opts.onEvaluate?.(id, def.id);
        const raw = await def.evaluate(inputs as never, ctx);
        newOutputs = wrapOutputs(def, raw);
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        newOutputs = poisonAll(def, msg, id);
      }
    }

    // Replace cache entry, releasing any prior owned outputs.
    const prior = this.cache.get(id);
    if (prior) for (const p of Object.values(prior.outputs)) releasePortStorage(p);
    this.cache.set(id, { outputs: newOutputs });
  }
}

// ── Helpers ──────────────────────────────────────────────────

function poisonAll(def: NodeDef<InputMap, OutputMap>, message: string, nodeId: string): Record<PortName, Port> {
  const out: Record<PortName, Port> = {};
  for (const port of Object.keys(def.outputs)) out[port] = errPort(message, nodeId);
  return out;
}

function wrapOutputs(def: NodeDef<InputMap, OutputMap>, raw: unknown): Record<PortName, Port> {
  if (raw === null || typeof raw !== 'object') {
    throw new Error(`evaluate must return an object of outputs, got ${typeof raw}`);
  }
  const out: Record<PortName, Port> = {};
  const r = raw as Record<string, unknown>;
  for (const [port, spec] of Object.entries(def.outputs)) {
    if (!(port in r)) throw new Error(`evaluate omitted output port '${port}'`);
    out[port] = wrapValue(spec.kind, r[port]);
  }
  return out;
}

function wrapValue<K extends PortKind>(kind: K, value: unknown): Port {
  switch (kind) {
    case 'number':
      if (typeof value !== 'number') throw new Error(`expected number, got ${typeof value}`);
      return { kind: 'number', value: value as PortValueByKind['number'] };
    case 'bool':
      if (typeof value !== 'boolean') throw new Error(`expected bool, got ${typeof value}`);
      return { kind: 'bool', value: value as PortValueByKind['bool'] };
    case 'vec3':
      // Trust the node — vec3 is a plain {x,y,z}; runtime check would be cheap but noisy.
      return { kind: 'vec3', value: value as PortValueByKind['vec3'] };
    case 'brep':
      if (!(value instanceof Brep)) throw new Error('expected Brep instance');
      return { kind: 'brep', value };
    case 'curve':
      if (!(value instanceof Curve)) throw new Error('expected Curve instance');
      return { kind: 'curve', value };
    default:
      throw new Error(`unknown port kind '${kind}'`);
  }
}

function releasePortStorage(p: Port): void {
  if (p.kind === 'brep'  && p.value instanceof Brep)  p.value.free();
  if (p.kind === 'curve' && p.value instanceof Curve) p.value.free();
}
