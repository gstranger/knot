import { useCallback, useEffect, useRef, useState } from 'react';
import {
  ReactFlow,
  Background,
  Controls,
  MiniMap,
  useNodesState,
  useEdgesState,
  addEdge,
  type Node,
  type Edge,
  type Connection,
  type NodeTypes,
  type OnNodesDelete,
  type OnEdgesDelete,
  type ReactFlowInstance,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import { Canvas } from '@react-three/fiber';
import { OrbitControls } from '@react-three/drei';
import { Joyride, type Step, type EventData, STATUS } from 'react-joyride';

import { createKnot, type Knot, type MeshData } from 'knot-cad';
import { Graph, Evaluator, buildDefaultRegistry } from 'knot-cad/graph';
import type { NodeRegistry, GraphData } from 'knot-cad/graph';
import { FormView } from 'knot-cad/react';

import { CadNode, type CadNodeData } from './CadNode';
import { portColor } from './port-colors';
import { EXAMPLES } from './examples';

// ── Tour ────────────────────────────────────────────────────────
const TOUR_STEPS: Step[] = [
  {
    target: 'body',
    placement: 'center',
    title: 'Welcome to the Knot Graph Editor',
    content: 'This is a node-based parametric CAD tool. You build 3D geometry by connecting nodes in a visual graph \u2014 similar to Grasshopper or Geometry Nodes.',
    skipBeacon: true,
  },
  {
    target: '[data-tour="palette"]',
    placement: 'right',
    title: 'Node Palette',
    content: 'This sidebar lists all available nodes, organized by category. Click any button to add that node to the graph canvas. Click a category header to expand or collapse it.',
  },
  {
    target: '[data-tour="palette-primitives"]',
    placement: 'right',
    title: 'Primitives',
    content: 'Start here. Box, Sphere, and Cylinder create solid 3D shapes that appear in the viewport.',
  },
  {
    target: '[data-tour="palette-operations"]',
    placement: 'right',
    title: 'Operations',
    content: 'Boolean (union / intersection / subtraction), Extrude, Revolve, Sweep, and Loft let you combine and transform shapes into complex geometry.',
  },
  {
    target: '[data-tour="graph-canvas"]',
    placement: 'left',
    title: 'Graph Canvas',
    content: 'Nodes you add appear here. Each node has colored input ports on the left and output ports on the right. Drag from an output to an input to connect them. The graph re-evaluates automatically.',
  },
  {
    target: '[data-tour="graph-canvas"]',
    placement: 'left',
    title: 'Editing Tips',
    content: 'Pan: drag the background. Zoom: scroll wheel. Select a node and press Backspace to delete it. Number inputs on nodes can be edited directly \u2014 just click and type.',
  },
  {
    target: '[data-tour="viewport"]',
    placement: 'left',
    title: '3D Viewport',
    content: 'The result of your graph renders here in real time. Drag to orbit, scroll to zoom. Every node that produces a solid shows up as blue geometry.',
  },
  {
    target: 'body',
    placement: 'center',
    title: 'Try It: Subtract a Cylinder from a Box',
    content: '1. Add a Box (Primitives \u203A Box)\n2. Add a Cylinder\n3. Add a Boolean (Operations \u203A Boolean)\n4. Connect the Box brep output \u2192 Boolean input "a"\n5. Connect the Cylinder brep output \u2192 Boolean input "b"\n6. Set Boolean "op" to 2 (subtraction)\n\nThe cylinder is cut from the box in the viewport!',
  },
];

// ── Types ────────────────────────────────────────────────────────
const nodeTypes: NodeTypes = { cad: CadNode as any };

interface PaletteEntry { defId: string; label: string }

interface PaletteGroup { label: string; entries: PaletteEntry[] }

const PALETTE: PaletteGroup[] = [
  { label: 'Input', entries: [
    { defId: 'core.number', label: 'Number' },
    { defId: 'core.slider', label: 'Slider' },
    { defId: 'core.toggle', label: 'Toggle' },
    { defId: 'core.vec3', label: 'Vec3' },
  ]},
  { label: 'Math', entries: [
    { defId: 'math.add', label: 'Add' },
    { defId: 'math.subtract', label: 'Subtract' },
    { defId: 'math.multiply', label: 'Multiply' },
    { defId: 'math.divide', label: 'Divide' },
    { defId: 'math.negate', label: 'Negate' },
    { defId: 'math.sin', label: 'Sin' },
    { defId: 'math.cos', label: 'Cos' },
    { defId: 'math.remap', label: 'Remap' },
    { defId: 'math.expression', label: 'Expression' },
  ]},
  { label: 'Vector', entries: [
    { defId: 'core.deconstructVec3', label: 'Deconstruct' },
    { defId: 'vec3.add', label: 'Add Vec3' },
    { defId: 'vec3.scale', label: 'Scale Vec3' },
    { defId: 'vec3.cross', label: 'Cross' },
    { defId: 'vec3.dot', label: 'Dot' },
    { defId: 'vec3.length', label: 'Length' },
  ]},
  { label: 'Primitives', entries: [
    { defId: 'knot.box', label: 'Box' },
    { defId: 'knot.sphere', label: 'Sphere' },
    { defId: 'knot.cylinder', label: 'Cylinder' },
    { defId: 'knot.triangle', label: 'Triangle' },
  ]},
  { label: 'Transform', entries: [
    { defId: 'core.translate', label: 'Translate' },
    { defId: 'core.rotate', label: 'Rotate' },
    { defId: 'core.scale', label: 'Scale' },
  ]},
  { label: 'Operations', entries: [
    { defId: 'knot.boolean', label: 'Boolean' },
    { defId: 'knot.extrude', label: 'Extrude' },
    { defId: 'knot.revolve', label: 'Revolve' },
    { defId: 'knot.sweep', label: 'Sweep' },
    { defId: 'knot.loft2', label: 'Loft (2)' },
  ]},
  { label: 'Curve', entries: [
    { defId: 'knot.line', label: 'Line' },
    { defId: 'knot.arc', label: 'Arc' },
    { defId: 'core.curve.pointAt', label: 'Point At' },
    { defId: 'core.curve.divide', label: 'Divide' },
    { defId: 'core.curve.offset', label: 'Offset' },
  ]},
  { label: 'List', entries: [
    { defId: 'list.range', label: 'Range' },
    { defId: 'list.series', label: 'Series' },
    { defId: 'list.item', label: 'List Item' },
    { defId: 'list.length', label: 'List Length' },
    { defId: 'list.repeat', label: 'Repeat' },
    { defId: 'list.flatten', label: 'Flatten' },
  ]},
];

// ── App ──────────────────────────────────────────────────────────
export function App() {
  const [loading, setLoading] = useState(true);
  const [meshes, setMeshes] = useState<MeshData[]>([]);

  const registryRef = useRef<NodeRegistry | null>(null);
  const graphRef = useRef<Graph | null>(null);
  const evalRef = useRef<Evaluator | null>(null);

  const [rfNodes, setRfNodes, onNodesChange] = useNodesState([] as Node[]);
  const [rfEdges, setRfEdges, onEdgesChange] = useEdgesState([] as Edge[]);
  const nextPos = useRef({ x: 50, y: 50 });

  // ── Init ─────────────────────────────────────────────────────
  useEffect(() => {
    let disposed = false;
    (async () => {
      const k = await createKnot();
      if (disposed) return;

      const reg = buildDefaultRegistry(k);
      registryRef.current = reg;
      graphRef.current = new Graph(reg);
      evalRef.current = new Evaluator({
        onEvaluate: (id, defId) => console.log(`[eval] ${id} (${defId})`),
      });
      setLoading(false);
    })();
    return () => { disposed = true; evalRef.current?.dispose(); };
  }, []);

  // ── Evaluate + collect meshes ────────────────────────────────
  const runEval = useCallback(async () => {
    const graph = graphRef.current;
    const ev = evalRef.current;
    if (!graph || !ev) return;

    await ev.run(graph);

    const newMeshes: MeshData[] = [];
    const previewMap: Record<string, Record<string, { text: string; error?: boolean }>> = {};
    for (const [id] of graph.nodes) {
      const def = graph.getDef(id);
      const nodePrev: Record<string, { text: string; error?: boolean }> = {};
      for (const port of Object.keys(def.outputs)) {
        const out = ev.getOutput(id, port);
        if (!out) continue;
        nodePrev[port] = formatPortPreview(out);
        if (out.kind === 'brep') {
          try { newMeshes.push((out.value as any).tessellate()); } catch { /* skip */ }
        } else if (out.kind === 'list' && Array.isArray(out.value)) {
          for (const item of out.value as any[]) {
            if (item && typeof item === 'object' && typeof item.tessellate === 'function') {
              try { newMeshes.push(item.tessellate()); } catch { /* skip */ }
            }
          }
        }
      }
      previewMap[id] = nodePrev;
    }
    setMeshes(newMeshes);
    // Splice previews into each node's data so CadNode re-renders
    // with the new values. Skip if previews are identical to avoid
    // a needless RF reconciliation churn.
    setRfNodes((nds) => nds.map((n) => {
      const next = previewMap[n.id] ?? {};
      const data = n.data as unknown as CadNodeData;
      const cur = data.previews ?? {};
      if (previewsEqual(cur, next)) return n;
      return { ...n, data: { ...data, previews: next } as unknown as Record<string, unknown> };
    }));
  }, [setRfNodes]);

  // ── Undo / Redo ──────────────────────────────────────────────
  //
  // Snapshot-based: every mutation pushes the current graph state
  // onto an undo stack before running. Cmd-Z pops, restores, and
  // pushes the previous state onto a redo stack. Cmd-Shift-Z does
  // the reverse.
  //
  // Snapshots are full `Graph.toJSON()` serializations plus the
  // editor's RF node positions, so visual layout round-trips too.
  // Stack capped at UNDO_LIMIT entries to bound memory.
  const undoStackRef = useRef<string[]>([]);
  const redoStackRef = useRef<string[]>([]);
  const UNDO_LIMIT = 50;
  // Set while applying a snapshot so any cascading mutation hooks
  // don't push themselves onto the undo stack and corrupt redo.
  const restoringRef = useRef(false);

  const captureSnapshot = useCallback((): string | null => {
    const graph = graphRef.current;
    if (!graph) return null;
    try {
      return JSON.stringify({
        graph: graph.toJSON(),
        layout: {
          positions: rfNodes.reduce<Record<string, { x: number; y: number }>>((acc, n) => {
            acc[n.id] = { x: n.position.x, y: n.position.y };
            return acc;
          }, {}),
          nextPos: { ...nextPos.current },
        },
      });
    } catch {
      // Non-JSON-safe constants — skip the snapshot. The mutation
      // proceeds; undo will jump past this state but never lose
      // earlier ones.
      return null;
    }
  }, [rfNodes]);

  const pushUndo = useCallback(() => {
    if (restoringRef.current) return;
    const snap = captureSnapshot();
    if (!snap) return;
    const stack = undoStackRef.current;
    stack.push(snap);
    if (stack.length > UNDO_LIMIT) stack.shift();
    redoStackRef.current.length = 0;
  }, [captureSnapshot]);

  // ── Wire-color helpers ───────────────────────────────────────
  //
  // The edge's stroke color is derived from the source port's kind
  // so a glance at the canvas tells you what's flowing where —
  // yellow for lists, orange for breps, purple for curves, etc.
  const edgeStyleFor = useCallback((fromNode: string, fromPort: string) => {
    const graph = graphRef.current;
    if (!graph) return { strokeWidth: 2 };
    try {
      const kind = graph.getDef(fromNode).outputs[fromPort]?.kind ?? 'number';
      return { strokeWidth: 2, stroke: portColor(kind) };
    } catch {
      return { strokeWidth: 2 };
    }
  }, []);

  const wireToEdge = useCallback(
    (w: { fromNode: string; fromPort: string; toNode: string; toPort: string }): Edge => ({
      id: `${w.fromNode}.${w.fromPort}-${w.toNode}.${w.toPort}`,
      source: w.fromNode, sourceHandle: w.fromPort,
      target: w.toNode, targetHandle: w.toPort,
      style: edgeStyleFor(w.fromNode, w.fromPort),
    }),
    [edgeStyleFor],
  );

  // ── Graph node → RF node ─────────────────────────────────────
  const toRfNode = useCallback(
    (nodeId: string, pos: { x: number; y: number }): Node => {
      const graph = graphRef.current!;
      const inst = graph.getNode(nodeId)!;
      const def = graph.getDef(nodeId);
      const data: CadNodeData = {
        label: def.label ?? def.id,
        defId: def.id,
        inputs: Object.entries(def.inputs).map(([name, spec]: [string, any]) => ({ name, kind: spec.kind })),
        outputs: Object.entries(def.outputs).map(([name, spec]: [string, any]) => ({ name, kind: spec.kind })),
        constants: { ...inst.constants },
        onConstantChange: (port, value) => {
          pushUndo();
          graph.setConstant(nodeId, port, value);
          evalRef.current?.markDirty(nodeId);
          runEval();
          setRfNodes((nds) =>
            nds.map((n) =>
              n.id === nodeId
                ? { ...n, data: { ...n.data, constants: { ...graph.getNode(nodeId)!.constants } } }
                : n,
            ),
          );
        },
      };
      return { id: nodeId, type: 'cad', position: pos, data } as unknown as Node;
    },
    [runEval, pushUndo, setRfNodes],
  );

  // ── Add node ─────────────────────────────────────────────────
  //
  // `at` overrides the cascading-diagonal default position — used
  // by the right-click context menu to drop the node where the
  // cursor was.
  const addNode = useCallback(
    (defId: string, at?: { x: number; y: number }) => {
      const graph = graphRef.current;
      if (!graph) return;
      const def = graph.registry.get(defId);
      if (!def) return;
      const constants: Record<string, unknown> = {};
      for (const [name, spec] of Object.entries(def.inputs) as [string, any][]) {
        if (spec.default !== undefined) constants[name] = spec.default;
      }
      // Slider metadata defaults.
      if (defId === 'core.slider') {
        constants._min ??= 0;
        constants._max ??= 10;
        constants._step ??= 0.1;
      }
      // Expression default.
      if (defId === 'math.expression') {
        constants.expr ??= 'a';
      }
      pushUndo();
      const nodeId = graph.addNode(defId, constants);
      let pos: { x: number; y: number };
      if (at) {
        pos = at;
      } else {
        pos = { ...nextPos.current };
        nextPos.current = { x: pos.x + 30, y: pos.y + 40 };
      }
      setRfNodes((nds) => [...nds, toRfNode(nodeId, pos)]);
      runEval();
    },
    [toRfNode, runEval, pushUndo, setRfNodes],
  );

  // ── Connect ──────────────────────────────────────────────────
  const onConnect = useCallback(
    (conn: Connection) => {
      const graph = graphRef.current;
      if (!graph || !conn.source || !conn.target || !conn.sourceHandle || !conn.targetHandle) return;
      pushUndo();
      try { graph.connect(conn.source, conn.sourceHandle, conn.target, conn.targetHandle); }
      catch (e) {
        console.warn('connect rejected:', (e as Error).message);
        // Roll back the snapshot we just pushed — the user didn't
        // actually mutate, so undo shouldn't bring them back to
        // pre-attempt state.
        undoStackRef.current.pop();
        return;
      }
      setRfEdges((eds) => addEdge({ ...conn, style: edgeStyleFor(conn.source!, conn.sourceHandle!) }, eds));
      evalRef.current?.markDirty(conn.target);
      runEval();
    },
    [runEval, pushUndo, setRfEdges, edgeStyleFor],
  );

  // ── Delete nodes ─────────────────────────────────────────────
  const onNodesDelete: OnNodesDelete = useCallback(
    (deleted) => {
      const graph = graphRef.current;
      if (!graph) return;
      pushUndo();
      for (const n of deleted) { evalRef.current?.evict(n.id); graph.removeNode(n.id); }
      setRfEdges(graph.wires.map(wireToEdge));
      runEval();
    },
    [runEval, pushUndo, setRfEdges, wireToEdge],
  );

  // ── Save / Load ──────────────────────────────────────────────
  //
  // File format: `{ formatVersion, graph: GraphData, layout }`.
  // `graph` is the kernel's serializable form (nodes + wires +
  // constants). `layout` carries the editor-only state — React Flow
  // positions and the next-spawn cursor — so a reload puts every
  // node back where the author dropped it.
  const handleSave = useCallback(() => {
    const graph = graphRef.current;
    if (!graph) return;
    let graphData: GraphData;
    try {
      graphData = graph.toJSON();
    } catch (e) {
      console.error('Save failed (non-JSON-safe constant?):', e);
      alert(`Save failed: ${(e as Error).message}`);
      return;
    }
    const file = {
      formatVersion: 1 as const,
      graph: graphData,
      layout: {
        positions: rfNodes.reduce<Record<string, { x: number; y: number }>>((acc, n) => {
          acc[n.id] = { x: n.position.x, y: n.position.y };
          return acc;
        }, {}),
        nextPos: { ...nextPos.current },
      },
    };
    const blob = new Blob([JSON.stringify(file, null, 2)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `knot-graph-${new Date().toISOString().slice(0, 10)}.json`;
    document.body.appendChild(a);
    a.click();
    a.remove();
    URL.revokeObjectURL(url);
  }, [rfNodes]);

  // Hidden file input — clicking the Load button trips this.
  const loadInputRef = useRef<HTMLInputElement>(null);

  type LoadedFile = {
    graph: GraphData;
    layout?: { positions?: Record<string, { x: number; y: number }>; nextPos?: { x: number; y: number } };
  };

  // Apply a parsed graph+layout to the editor. Shared by Load,
  // Undo, and Redo — the only difference between them is how the
  // snapshot got chosen. Throws on registry/wire/cycle errors;
  // callers surface those to the user as appropriate.
  const applyGraphData = useCallback((parsed: LoadedFile) => {
    const registry = registryRef.current;
    const ev = evalRef.current;
    if (!registry || !ev) return;
    const nextGraph = Graph.fromJSON(parsed.graph, registry);

    // Swap in the new graph; dispose the old evaluator so cached
    // outputs keyed by old node ids don't leak.
    graphRef.current = nextGraph;
    ev.dispose();
    evalRef.current = new Evaluator({
      onEvaluate: (id: string, defId: string) => console.log(`[eval] ${id} (${defId})`),
    });

    const positions = parsed.layout?.positions ?? {};
    const newRfNodes: Node[] = [];
    for (const [id] of nextGraph.nodes) {
      const pos = positions[id] ?? { x: 50 + newRfNodes.length * 30, y: 50 + newRfNodes.length * 40 };
      newRfNodes.push(toRfNode(id, pos));
    }
    const newRfEdges: Edge[] = nextGraph.wires.map(wireToEdge);
    setRfNodes(newRfNodes);
    setRfEdges(newRfEdges);

    if (parsed.layout?.nextPos) nextPos.current = { ...parsed.layout.nextPos };
    runEval();
  }, [toRfNode, runEval, setRfNodes, setRfEdges, wireToEdge]);

  const handleLoadFile = useCallback(async (file: File) => {
    let parsed: { formatVersion?: number } & LoadedFile;
    try {
      parsed = JSON.parse(await file.text());
    } catch (e) {
      alert(`Load failed: not valid JSON (${(e as Error).message})`);
      return;
    }
    if (parsed.formatVersion !== 1) {
      alert(`Load failed: unsupported formatVersion ${parsed.formatVersion}`);
      return;
    }
    if (!parsed.graph) {
      alert('Load failed: file has no graph');
      return;
    }
    pushUndo();
    try {
      applyGraphData(parsed);
    } catch (e) {
      alert(`Load failed: ${(e as Error).message}`);
    }
  }, [applyGraphData, pushUndo]);

  const handleLoadClick = useCallback(() => {
    loadInputRef.current?.click();
  }, []);

  // ── Delete edges ─────────────────────────────────────────────
  const onEdgesDelete: OnEdgesDelete = useCallback(
    (deleted) => {
      const graph = graphRef.current;
      if (!graph) return;
      pushUndo();
      for (const e of deleted) {
        if (e.targetHandle) { graph.disconnect(e.target, e.targetHandle); evalRef.current?.markDirty(e.target); }
      }
      runEval();
    },
    [runEval, pushUndo],
  );

  // ── Undo / Redo handlers ────────────────────────────────────
  const handleUndo = useCallback(() => {
    const undoStack = undoStackRef.current;
    if (undoStack.length === 0) return;
    const current = captureSnapshot();
    const prev = undoStack.pop()!;
    if (current) redoStackRef.current.push(current);
    restoringRef.current = true;
    try {
      const parsed = JSON.parse(prev);
      applyGraphData(parsed);
    } catch (e) {
      console.warn('Undo failed:', e);
    } finally {
      restoringRef.current = false;
    }
  }, [captureSnapshot, applyGraphData]);

  const handleRedo = useCallback(() => {
    const redoStack = redoStackRef.current;
    if (redoStack.length === 0) return;
    const current = captureSnapshot();
    const next = redoStack.pop()!;
    if (current) undoStackRef.current.push(current);
    restoringRef.current = true;
    try {
      const parsed = JSON.parse(next);
      applyGraphData(parsed);
    } catch (e) {
      console.warn('Redo failed:', e);
    } finally {
      restoringRef.current = false;
    }
  }, [captureSnapshot, applyGraphData]);

  // Duplicate selected nodes. Each duplicate is a fresh kernel node
  // (new id, copied constants) offset by (30, 30) from its source
  // so it doesn't sit perfectly on top. Wires aren't cloned —
  // matches Grasshopper/Blender's "node-only" Cmd-D semantics.
  const handleDuplicate = useCallback(() => {
    const graph = graphRef.current;
    if (!graph) return;
    const selected = rfNodes.filter((n) => n.selected);
    if (selected.length === 0) return;
    pushUndo();
    const created: Node[] = [];
    for (const n of selected) {
      const inst = graph.getNode(n.id);
      if (!inst) continue;
      const newId = graph.addNode(inst.defId, { ...inst.constants });
      const pos = { x: n.position.x + 30, y: n.position.y + 30 };
      created.push(toRfNode(newId, pos));
    }
    if (created.length === 0) return;
    // Deselect the originals so the duplicates become the new
    // selection — a second Cmd-D then duplicates those, like every
    // other editor.
    setRfNodes((nds) => [
      ...nds.map((n) => (n.selected ? { ...n, selected: false } : n)),
      ...created.map((n) => ({ ...n, selected: true })),
    ]);
    runEval();
  }, [rfNodes, toRfNode, pushUndo, setRfNodes, runEval]);

  // Cmd/Ctrl-Z, Cmd/Ctrl-Shift-Z, Cmd/Ctrl-S, Cmd/Ctrl-D global
  // keybindings. We skip when the focused element is an <input> or
  // <textarea> so the browser's native input behavior (cursor undo,
  // text duplication via Cmd-D in some browsers) still works inside
  // those.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const mod = e.metaKey || e.ctrlKey;
      if (!mod) return;
      const tag = (e.target as HTMLElement | null)?.tagName;
      if (tag === 'INPUT' || tag === 'TEXTAREA') return;
      const key = e.key.toLowerCase();
      if (key === 'z') {
        e.preventDefault();
        if (e.shiftKey) handleRedo();
        else handleUndo();
      } else if (key === 's' && !e.shiftKey) {
        e.preventDefault();
        handleSave();
      } else if (key === 'd' && !e.shiftKey) {
        e.preventDefault();
        handleDuplicate();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [handleUndo, handleRedo, handleSave, handleDuplicate]);

  // Accordion state — all groups open by default
  const [openGroups, setOpenGroups] = useState<Set<string>>(() => new Set(PALETTE.map((g) => g.label)));
  const toggleGroup = useCallback((label: string) => {
    setOpenGroups((prev) => {
      const next = new Set(prev);
      if (next.has(label)) next.delete(label); else next.add(label);
      return next;
    });
  }, []);

  // Palette search. While `paletteQuery` is non-empty, the palette
  // shows only matching entries and force-expands any group that
  // has any. Empty query falls back to the user's accordion state.
  const [paletteQuery, setPaletteQuery] = useState('');
  const filteredPalette = (() => {
    const q = paletteQuery.trim().toLowerCase();
    if (!q) return PALETTE.map((g) => ({ group: g, entries: g.entries }));
    return PALETTE
      .map((g) => ({ group: g, entries: g.entries.filter((e) => e.label.toLowerCase().includes(q)) }))
      .filter((g) => g.entries.length > 0);
  })();

  // Editor / Form mode toggle.
  //
  // Form mode hides the palette + node canvas and renders only the
  // exposed input controls + viewport — useful when sharing a graph
  // as a tunable "definition" with someone who doesn't need to see
  // the wiring.
  const [mode, setMode] = useState<'editor' | 'form'>('editor');

  // Right-click "add node" context menu.
  //
  // The RF instance gives us `screenToFlowPosition`, which translates
  // a viewport click into canvas coords (accounting for pan/zoom),
  // so the newly-added node lands exactly under the cursor. Without
  // it a right-click far from the origin would still spawn nodes
  // near (0, 0).
  const rfRef = useRef<ReactFlowInstance | null>(null);
  const [ctxMenu, setCtxMenu] = useState<{ screenX: number; screenY: number; flowX: number; flowY: number } | null>(null);
  const [ctxQuery, setCtxQuery] = useState('');
  const onPaneContextMenu = useCallback((e: React.MouseEvent | MouseEvent) => {
    e.preventDefault();
    const rf = rfRef.current;
    if (!rf) return;
    const flow = rf.screenToFlowPosition({ x: e.clientX, y: e.clientY });
    setCtxMenu({ screenX: e.clientX, screenY: e.clientY, flowX: flow.x, flowY: flow.y });
    setCtxQuery('');
  }, []);
  // Close on outside click / Escape.
  useEffect(() => {
    if (!ctxMenu) return;
    const onDown = (e: MouseEvent) => {
      const t = e.target as HTMLElement | null;
      if (t?.closest('[data-ctx-menu]')) return;
      setCtxMenu(null);
    };
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') setCtxMenu(null); };
    window.addEventListener('mousedown', onDown);
    window.addEventListener('keydown', onKey);
    return () => {
      window.removeEventListener('mousedown', onDown);
      window.removeEventListener('keydown', onKey);
    };
  }, [ctxMenu]);
  const ctxFiltered = (() => {
    const q = ctxQuery.trim().toLowerCase();
    if (!q) return PALETTE.map((g) => ({ group: g, entries: g.entries }));
    return PALETTE
      .map((g) => ({ group: g, entries: g.entries.filter((e) => e.label.toLowerCase().includes(q)) }))
      .filter((g) => g.entries.length > 0);
  })();

  // Examples dropdown state. Loading an example funnels through
  // `applyGraphData` (the same path Load uses) so undo/redo, the
  // RF layout, and evaluator re-init all work identically.
  const [showExamples, setShowExamples] = useState(false);
  const handleLoadExample = useCallback((id: string) => {
    const ex = EXAMPLES.find((e) => e.id === id);
    if (!ex) return;
    pushUndo();
    setShowExamples(false);
    try {
      applyGraphData(ex.file);
    } catch (e) {
      alert(`Example load failed: ${(e as Error).message}`);
    }
  }, [applyGraphData, pushUndo]);

  // Tour state
  const [runTour, setRunTour] = useState(false);
  const handleTourEvent = useCallback((data: EventData) => {
    if (data.status === STATUS.FINISHED || data.status === STATUS.SKIPPED) {
      setRunTour(false);
    }
  }, []);

  if (loading) {
    return <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100vh', width: '100vw' }}>Loading kernel…</div>;
  }

  const viewport = (
    <div data-tour="viewport" style={{ flex: mode === 'form' ? 1 : undefined, width: mode === 'editor' ? 400 : undefined, borderLeft: '1px solid #333' }}>
      <Canvas camera={{ position: [5, 5, 5], fov: 50 }}>
        <ambientLight intensity={0.4} />
        <directionalLight position={[5, 10, 5]} intensity={0.8} />
        <OrbitControls />
        {meshes.map((mesh, i) => <BrepMesh key={i} mesh={mesh} />)}
        <gridHelper args={[10, 10, '#444', '#333']} />
      </Canvas>
    </div>
  );

  const modeToggle = (
    <div style={{ position: 'fixed', top: 12, right: 12, zIndex: 1000, display: 'flex', background: '#16162a', border: '1px solid #333', borderRadius: 6, padding: 2 }}>
      {(['editor', 'form'] as const).map((m) => (
        <button
          key={m}
          onClick={() => setMode(m)}
          style={{
            background: mode === m ? '#4a9eff' : 'transparent',
            color: mode === m ? '#0e0e1a' : '#aaa',
            border: 'none',
            padding: '6px 14px',
            borderRadius: 4,
            cursor: 'pointer',
            fontSize: 11,
            fontWeight: 600,
            textTransform: 'uppercase',
            letterSpacing: 0.5,
          }}
          title={m === 'editor' ? 'Show palette + node canvas' : 'Hide the graph; expose only inputs'}
        >
          {m}
        </button>
      ))}
    </div>
  );

  if (mode === 'form') {
    return (
      <div style={{ display: 'flex', height: '100vh', width: '100vw' }}>
        {modeToggle}
        <div style={{ width: 320, background: '#16162a', borderRight: '1px solid #333' }}>
          {graphRef.current && (
            <FormView
              graph={graphRef.current}
              title="Parameters"
              onChange={(field) => {
                evalRef.current?.markDirty(field.nodeId);
                runEval();
              }}
              style={{ height: '100%' }}
            />
          )}
        </div>
        {viewport}
      </div>
    );
  }

  return (
    <div style={{ display: 'flex', height: '100vh', width: '100vw' }}>
      {modeToggle}
      <Joyride
        steps={TOUR_STEPS}
        run={runTour}
        continuous
        buttons={['back', 'primary', 'skip']}
        onEvent={handleTourEvent}
        primaryColor="#4a9eff"
        backgroundColor="#2a2a3e"
        textColor="#e0e0e0"
        overlayColor="rgba(0, 0, 0, 0.7)"
      />
      {/* Palette */}
      <div data-tour="palette" style={{ width: 180, background: '#16162a', borderRight: '1px solid #333', padding: 12, display: 'flex', flexDirection: 'column', gap: 6 }}>
        <input
          ref={loadInputRef}
          type="file"
          accept="application/json,.json"
          style={{ display: 'none' }}
          onChange={(e) => {
            const f = e.target.files?.[0];
            if (f) handleLoadFile(f);
            // Reset so picking the same file again re-fires onChange.
            e.target.value = '';
          }}
        />
        <div style={{ display: 'flex', gap: 4, marginBottom: 6 }}>
          <button
            onClick={handleUndo}
            style={{ flex: 1, background: '#2a2a3e', border: '1px solid #444', borderRadius: 4, color: '#e0e0e0', padding: '4px 0', cursor: 'pointer', fontSize: 11 }}
            title="Undo (Cmd/Ctrl-Z)"
          >↶ Undo</button>
          <button
            onClick={handleRedo}
            style={{ flex: 1, background: '#2a2a3e', border: '1px solid #444', borderRadius: 4, color: '#e0e0e0', padding: '4px 0', cursor: 'pointer', fontSize: 11 }}
            title="Redo (Cmd/Ctrl-Shift-Z)"
          >↷ Redo</button>
        </div>
        <div style={{ display: 'flex', gap: 4, marginBottom: 6 }}>
          <button
            onClick={handleSave}
            style={{ flex: 1, background: '#2a2a3e', border: '1px solid #444', borderRadius: 4, color: '#e0e0e0', padding: '4px 0', cursor: 'pointer', fontSize: 11 }}
            title="Download graph as JSON"
          >Save</button>
          <button
            onClick={handleLoadClick}
            style={{ flex: 1, background: '#2a2a3e', border: '1px solid #444', borderRadius: 4, color: '#e0e0e0', padding: '4px 0', cursor: 'pointer', fontSize: 11 }}
            title="Load a previously saved graph"
          >Load</button>
        </div>
        <div style={{ position: 'relative', marginBottom: 6 }}>
          <button
            onClick={() => setShowExamples((s) => !s)}
            style={{ width: '100%', background: '#2a2a3e', border: '1px solid #444', borderRadius: 4, color: '#e0e0e0', padding: '4px 8px', cursor: 'pointer', fontSize: 11, display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}
            title="Load a built-in example graph"
          >
            <span>Examples</span>
            <span style={{ color: '#888' }}>▾</span>
          </button>
          {showExamples && (
            <div style={{
              position: 'absolute', top: '100%', left: 0, right: 0, marginTop: 2,
              background: '#1a1a2e', border: '1px solid #555', borderRadius: 4,
              zIndex: 50, padding: 4, display: 'flex', flexDirection: 'column', gap: 2,
              boxShadow: '0 4px 12px rgba(0,0,0,0.4)',
            }}>
              {EXAMPLES.map((ex) => (
                <button
                  key={ex.id}
                  onClick={() => handleLoadExample(ex.id)}
                  title={ex.description}
                  style={{ background: 'transparent', border: 'none', borderRadius: 3, color: '#e0e0e0', padding: '5px 8px', cursor: 'pointer', textAlign: 'left', fontSize: 11 }}
                  onMouseEnter={(e) => { (e.currentTarget as HTMLButtonElement).style.background = '#2a2a3e'; }}
                  onMouseLeave={(e) => { (e.currentTarget as HTMLButtonElement).style.background = 'transparent'; }}
                >
                  <div style={{ fontWeight: 600 }}>{ex.label}</div>
                  <div style={{ color: '#888', fontSize: 10, marginTop: 1 }}>{ex.description}</div>
                </button>
              ))}
            </div>
          )}
        </div>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 8 }}>
          <span style={{ fontWeight: 700, fontSize: 14 }}>Add Node</span>
          <button
            onClick={() => setRunTour(true)}
            style={{ background: '#2a2a3e', border: '1px solid #555', borderRadius: 4, color: '#888', padding: '2px 8px', cursor: 'pointer', fontSize: 11 }}
            title="Take a guided tour"
          >?</button>
        </div>
        <input
          type="search"
          placeholder="Search nodes…"
          value={paletteQuery}
          onChange={(e) => setPaletteQuery(e.target.value)}
          style={{
            background: '#1a1a2e', border: '1px solid #444', borderRadius: 4,
            color: '#e0e0e0', padding: '4px 8px', fontSize: 11, marginBottom: 6,
          }}
        />
        <div style={{ overflowY: 'auto', flex: 1, display: 'flex', flexDirection: 'column', gap: 2 }}>
          {filteredPalette.length === 0 && (
            <div style={{ color: '#666', fontSize: 11, padding: '8px 0', textAlign: 'center' }}>
              No nodes match “{paletteQuery}”
            </div>
          )}
          {filteredPalette.map(({ group, entries }) => {
            const isOpen = paletteQuery.trim() ? true : openGroups.has(group.label);
            return (
              <div key={group.label} data-tour={`palette-${group.label.toLowerCase()}`}>
                <button
                  onClick={() => toggleGroup(group.label)}
                  style={{
                    width: '100%', background: 'none', border: 'none', color: '#888',
                    fontSize: 10, textTransform: 'uppercase', letterSpacing: 1,
                    padding: '6px 0 4px', cursor: 'pointer', textAlign: 'left',
                    display: 'flex', alignItems: 'center', gap: 4,
                  }}
                >
                  <span style={{ display: 'inline-block', width: 10, fontSize: 8, transition: 'transform 0.15s', transform: isOpen ? 'rotate(90deg)' : 'rotate(0deg)' }}>&#9654;</span>
                  {group.label}
                </button>
                {isOpen && (
                  <div style={{ display: 'flex', flexDirection: 'column', gap: 2, paddingLeft: 2 }}>
                    {entries.map((p) => (
                      <button
                        key={p.defId}
                        onClick={() => addNode(p.defId)}
                        style={{ background: '#2a2a3e', border: '1px solid #444', borderRadius: 4, color: '#e0e0e0', padding: '4px 8px', cursor: 'pointer', textAlign: 'left', fontSize: 11 }}
                      >
                        {p.label}
                      </button>
                    ))}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      </div>

      {/* Graph canvas */}
      <div data-tour="graph-canvas" style={{ flex: 1 }}>
        <ReactFlow
          nodes={rfNodes} edges={rfEdges}
          onNodesChange={onNodesChange} onEdgesChange={onEdgesChange}
          onConnect={onConnect}
          onNodesDelete={onNodesDelete} onEdgesDelete={onEdgesDelete}
          onInit={(inst) => { rfRef.current = inst; }}
          onPaneContextMenu={onPaneContextMenu}
          nodeTypes={nodeTypes}
          defaultEdgeOptions={{ style: { strokeWidth: 2 } }}
          fitView colorMode="dark" deleteKeyCode="Backspace"
        >
          <Background color="#333" gap={20} />
          <Controls />
          <MiniMap style={{ background: '#1a1a2e' }} nodeColor="#2a2a3e" maskColor="rgba(0,0,0,0.6)" />
        </ReactFlow>
      </div>

      {/* 3D viewport */}
      {viewport}

      {/* Right-click context menu */}
      {ctxMenu && (
        <div
          data-ctx-menu
          style={{
            position: 'fixed',
            // Clamp inside the viewport so a menu opened near the
            // right/bottom edge stays fully visible. 240×360 matches
            // the menu's intrinsic size below.
            left: Math.min(ctxMenu.screenX, window.innerWidth - 240),
            top: Math.min(ctxMenu.screenY, window.innerHeight - 360),
            width: 240, maxHeight: 360,
            background: '#1a1a2e', border: '1px solid #555', borderRadius: 4,
            zIndex: 100, padding: 6, display: 'flex', flexDirection: 'column', gap: 4,
            boxShadow: '0 6px 20px rgba(0,0,0,0.5)',
          }}
        >
          <input
            type="search"
            placeholder="Search nodes…"
            autoFocus
            value={ctxQuery}
            onChange={(e) => setCtxQuery(e.target.value)}
            style={{
              background: '#16162a', border: '1px solid #444', borderRadius: 4,
              color: '#e0e0e0', padding: '4px 8px', fontSize: 11,
            }}
          />
          <div style={{ overflowY: 'auto', flex: 1, display: 'flex', flexDirection: 'column', gap: 2 }}>
            {ctxFiltered.length === 0 && (
              <div style={{ color: '#666', fontSize: 11, padding: '8px 0', textAlign: 'center' }}>
                No matches
              </div>
            )}
            {ctxFiltered.map(({ group, entries }) => (
              <div key={group.label}>
                <div style={{
                  color: '#888', fontSize: 9, textTransform: 'uppercase',
                  letterSpacing: 1, padding: '4px 6px 2px',
                }}>{group.label}</div>
                {entries.map((p) => (
                  <button
                    key={p.defId}
                    onClick={() => {
                      addNode(p.defId, { x: ctxMenu.flowX, y: ctxMenu.flowY });
                      setCtxMenu(null);
                    }}
                    style={{
                      width: '100%', background: 'transparent', border: 'none',
                      borderRadius: 3, color: '#e0e0e0', padding: '4px 8px',
                      cursor: 'pointer', textAlign: 'left', fontSize: 11,
                    }}
                    onMouseEnter={(e) => { (e.currentTarget as HTMLButtonElement).style.background = '#2a2a3e'; }}
                    onMouseLeave={(e) => { (e.currentTarget as HTMLButtonElement).style.background = 'transparent'; }}
                  >{p.label}</button>
                ))}
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

// ── Output preview formatting ────────────────────────────────────
//
// `out` is whatever `Evaluator.getOutput` returns — a Port
// `{ kind, value }` for success, or an error record with a
// `message` field. We render a compact, type-aware label next to
// each output port so the user can see scalar values, list lengths,
// and error states without opening devtools.
function formatPortPreview(out: { kind: string; value?: unknown; message?: string }): { text: string; error?: boolean } {
  if (out.kind === 'error') return { text: out.message ?? 'error', error: true };
  const v = out.value;
  switch (out.kind) {
    case 'number': {
      const n = v as number;
      if (!Number.isFinite(n)) return { text: String(n) };
      return { text: Number.isInteger(n) ? String(n) : n.toFixed(3) };
    }
    case 'bool': return { text: v ? 'true' : 'false' };
    case 'vec3': {
      const a = v as { x: number; y: number; z: number };
      return { text: `(${a.x.toFixed(1)}, ${a.y.toFixed(1)}, ${a.z.toFixed(1)})` };
    }
    case 'list': {
      const arr = (v as unknown[]) ?? [];
      if (arr.length === 0) return { text: '[]' };
      const first = arr[0];
      const hint = typeof first === 'number' ? 'num' :
                   typeof first === 'boolean' ? 'bool' :
                   first && typeof first === 'object' ? 'obj' : 'val';
      return { text: `[${arr.length} ${hint}]` };
    }
    case 'brep': return { text: 'brep' };
    case 'curve': return { text: 'curve' };
    default: return { text: out.kind };
  }
}

function previewsEqual(a: Record<string, { text: string; error?: boolean }>, b: Record<string, { text: string; error?: boolean }>): boolean {
  const ak = Object.keys(a), bk = Object.keys(b);
  if (ak.length !== bk.length) return false;
  for (const k of ak) {
    const av = a[k], bv = b[k];
    if (!bv || av.text !== bv.text || !!av.error !== !!bv.error) return false;
  }
  return true;
}

// ── Mesh renderer ────────────────────────────────────────────────
function BrepMesh({ mesh }: { mesh: MeshData }) {
  const positions = new Float32Array(mesh.positions);
  const normals = new Float32Array(mesh.normals);
  const indices = new Uint32Array(mesh.indices);
  return (
    <mesh>
      <bufferGeometry>
        <bufferAttribute attach="attributes-position" args={[positions, 3]} />
        <bufferAttribute attach="attributes-normal" args={[normals, 3]} />
        <bufferAttribute attach="index" args={[indices, 1]} />
      </bufferGeometry>
      <meshStandardMaterial color="#6a9eff" flatShading />
    </mesh>
  );
}
