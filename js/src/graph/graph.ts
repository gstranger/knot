import type { NodeDef, NodeId, NodeInstance, PortName, Wire, InputMap, OutputMap } from './types';

export interface NodeRegistry {
  get(defId: string): NodeDef<InputMap, OutputMap> | undefined;
}

export class Registry implements NodeRegistry {
  private defs = new Map<string, NodeDef<InputMap, OutputMap>>();

  register<I extends InputMap, O extends OutputMap>(def: NodeDef<I, O>): void {
    if (this.defs.has(def.id)) throw new Error(`Registry: duplicate node def id '${def.id}'`);
    this.defs.set(def.id, def as NodeDef<InputMap, OutputMap>);
  }

  get(defId: string): NodeDef<InputMap, OutputMap> | undefined {
    return this.defs.get(defId);
  }

  has(defId: string): boolean {
    return this.defs.has(defId);
  }
}

/**
 * The pure-structure graph. Holds nodes, wires, and per-node input constants.
 * Holds NO runtime values — those live in the Evaluator's cache.
 *
 * Mutation methods bump `version` so the Evaluator knows to rebuild its topo cache.
 */
export class Graph {
  private _nodes = new Map<NodeId, NodeInstance>();
  private _wires: Wire[] = [];
  private _version = 0;
  private _nextId = 1;

  constructor(public readonly registry: NodeRegistry) {}

  get version(): number { return this._version; }
  get nodes(): ReadonlyMap<NodeId, NodeInstance> { return this._nodes; }
  get wires(): readonly Wire[] { return this._wires; }

  // ── Mutation ─────────────────────────────────────────────────

  addNode(defId: string, constants: Record<string, unknown> = {}, id?: NodeId): NodeId {
    if (!this.registry.get(defId)) throw new Error(`Graph: unknown node def '${defId}'`);
    const nodeId = id ?? `n${this._nextId++}`;
    if (this._nodes.has(nodeId)) throw new Error(`Graph: duplicate node id '${nodeId}'`);
    this._nodes.set(nodeId, { id: nodeId, defId, constants: { ...constants } });
    this._version++;
    return nodeId;
  }

  removeNode(id: NodeId): void {
    if (!this._nodes.delete(id)) return;
    this._wires = this._wires.filter(w => w.fromNode !== id && w.toNode !== id);
    this._version++;
  }

  setConstant(id: NodeId, port: PortName, value: unknown): void {
    const n = this._nodes.get(id);
    if (!n) throw new Error(`Graph: no node '${id}'`);
    this._nodes.set(id, { ...n, constants: { ...n.constants, [port]: value } });
    this._version++;
  }

  connect(fromNode: NodeId, fromPort: PortName, toNode: NodeId, toPort: PortName): Wire {
    this.assertNode(fromNode);
    this.assertNode(toNode);
    this.assertOutputExists(fromNode, fromPort);
    this.assertInputExists(toNode, toPort);
    this.assertKindsMatch(fromNode, fromPort, toNode, toPort);

    // One wire per input port: replace any existing wire into (toNode, toPort).
    this._wires = this._wires.filter(w => !(w.toNode === toNode && w.toPort === toPort));
    const wire: Wire = { fromNode, fromPort, toNode, toPort };
    this._wires.push(wire);

    if (this.detectCycle()) {
      this._wires.pop();
      throw new Error(`Graph: connecting ${fromNode}.${fromPort} -> ${toNode}.${toPort} would create a cycle`);
    }
    this._version++;
    return wire;
  }

  disconnect(toNode: NodeId, toPort: PortName): void {
    const before = this._wires.length;
    this._wires = this._wires.filter(w => !(w.toNode === toNode && w.toPort === toPort));
    if (this._wires.length !== before) this._version++;
  }

  // ── Queries ──────────────────────────────────────────────────

  getNode(id: NodeId): NodeInstance | undefined { return this._nodes.get(id); }

  getDef(id: NodeId): NodeDef<InputMap, OutputMap> {
    const n = this._nodes.get(id);
    if (!n) throw new Error(`Graph: no node '${id}'`);
    const def = this.registry.get(n.defId);
    if (!def) throw new Error(`Graph: missing def '${n.defId}' for node '${id}'`);
    return def;
  }

  /** The wire feeding (toNode, toPort), if any. */
  incomingWire(toNode: NodeId, toPort: PortName): Wire | undefined {
    return this._wires.find(w => w.toNode === toNode && w.toPort === toPort);
  }

  /** Direct upstream node IDs (sources of wires into this node). */
  predecessors(id: NodeId): NodeId[] {
    const set = new Set<NodeId>();
    for (const w of this._wires) if (w.toNode === id) set.add(w.fromNode);
    return [...set];
  }

  /** Direct downstream node IDs (targets of wires out of this node). */
  successors(id: NodeId): NodeId[] {
    const set = new Set<NodeId>();
    for (const w of this._wires) if (w.fromNode === id) set.add(w.toNode);
    return [...set];
  }

  /**
   * Topological order. Throws if a cycle exists (shouldn't, since `connect` rejects them).
   *
   * Multiple wires between the same (from, to) pair (different ports) count as
   * a single edge for ordering purposes — otherwise in-degree double-counts and
   * the destination never reaches zero.
   */
  topoSort(): NodeId[] {
    const inDeg = new Map<NodeId, number>();
    const edges = new Set<string>();
    for (const id of this._nodes.keys()) inDeg.set(id, 0);
    for (const w of this._wires) {
      const key = `${w.fromNode}->${w.toNode}`;
      if (edges.has(key)) continue;
      edges.add(key);
      inDeg.set(w.toNode, (inDeg.get(w.toNode) ?? 0) + 1);
    }

    const queue: NodeId[] = [];
    for (const [id, d] of inDeg) if (d === 0) queue.push(id);

    const order: NodeId[] = [];
    while (queue.length) {
      const id = queue.shift()!;
      order.push(id);
      for (const succ of this.successors(id)) {
        const d = (inDeg.get(succ) ?? 0) - 1;
        inDeg.set(succ, d);
        if (d === 0) queue.push(succ);
      }
    }
    if (order.length !== this._nodes.size) throw new Error('Graph: cycle detected during topo sort');
    return order;
  }

  // ── Validation helpers ───────────────────────────────────────

  private assertNode(id: NodeId): void {
    if (!this._nodes.has(id)) throw new Error(`Graph: no node '${id}'`);
  }

  private assertOutputExists(id: NodeId, port: PortName): void {
    const def = this.getDef(id);
    if (!(port in def.outputs)) throw new Error(`Graph: node '${id}' (${def.id}) has no output '${port}'`);
  }

  private assertInputExists(id: NodeId, port: PortName): void {
    const def = this.getDef(id);
    if (!(port in def.inputs)) throw new Error(`Graph: node '${id}' (${def.id}) has no input '${port}'`);
  }

  private assertKindsMatch(fromNode: NodeId, fromPort: PortName, toNode: NodeId, toPort: PortName): void {
    const fromKind = this.getDef(fromNode).outputs[fromPort].kind;
    const toKind = this.getDef(toNode).inputs[toPort].kind;
    if (fromKind !== toKind) {
      throw new Error(
        `Graph: kind mismatch wiring ${fromNode}.${fromPort}:${fromKind} -> ${toNode}.${toPort}:${toKind}`
      );
    }
  }

  private detectCycle(): boolean {
    try { this.topoSort(); return false; } catch { return true; }
  }
}
