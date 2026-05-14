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
import {
  HelpCircle, Undo2, Redo2, Save, FolderOpen,
  BookOpen, Search, ChevronDown,
} from 'lucide-react';

import { createKnot, type MeshData } from 'knot-cad';
import { Graph, Evaluator, buildDefaultRegistry } from 'knot-cad/graph';
import type { NodeRegistry, GraphData } from 'knot-cad/graph';
import { FormView } from 'knot-cad/react';

import { CadNode, type CadNodeData } from './CadNode';
import { portColor } from './port-colors';
import { EXAMPLES } from './examples';

import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip';
import {
  Accordion, AccordionContent, AccordionItem, AccordionTrigger,
} from '@/components/ui/accordion';
import {
  Command, CommandEmpty, CommandGroup, CommandInput, CommandItem, CommandList,
} from '@/components/ui/command';
import {
  DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import {
  AlertDialog, AlertDialogAction, AlertDialogContent, AlertDialogDescription,
  AlertDialogFooter, AlertDialogHeader, AlertDialogTitle,
} from '@/components/ui/alert-dialog';
import { ScrollArea } from '@/components/ui/scroll-area';
import { Separator } from '@/components/ui/separator';

// ── Tour ────────────────────────────────────────────────────────
const TOUR_STEPS: Step[] = [
  {
    target: 'body',
    placement: 'center',
    title: 'Welcome to the Knot Graph Editor',
    content: 'This is a node-based parametric CAD tool. You build 3D geometry by connecting nodes in a visual graph — similar to Grasshopper or Geometry Nodes.',
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
    content: 'Pan: drag the background. Zoom: scroll wheel. Select a node and press Backspace to delete it. Number inputs on nodes can be edited directly — just click and type.',
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
    content: '1. Add a Box (Primitives › Box)\n2. Add a Cylinder\n3. Add a Boolean (Operations › Boolean)\n4. Connect the Box brep output → Boolean input "a"\n5. Connect the Cylinder brep output → Boolean input "b"\n6. Set Boolean "op" to 2 (subtraction)\n\nThe cylinder is cut from the box in the viewport!',
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

  // Error dialog replaces native alert(). One queue, shown one at
  // a time — keeps the UX consistent with the rest of the shadcn
  // surface.
  const [errorDialog, setErrorDialog] = useState<{ title: string; message: string } | null>(null);
  const showError = useCallback((title: string, message: string) => {
    setErrorDialog({ title, message });
  }, []);

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
    setRfNodes((nds) => nds.map((n) => {
      const next = previewMap[n.id] ?? {};
      const data = n.data as unknown as CadNodeData;
      const cur = data.previews ?? {};
      if (previewsEqual(cur, next)) return n;
      return { ...n, data: { ...data, previews: next } as unknown as Record<string, unknown> };
    }));
  }, [setRfNodes]);

  // ── Undo / Redo ──────────────────────────────────────────────
  const undoStackRef = useRef<string[]>([]);
  const redoStackRef = useRef<string[]>([]);
  const UNDO_LIMIT = 50;
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
      if (defId === 'core.slider') {
        constants._min ??= 0;
        constants._max ??= 10;
        constants._step ??= 0.1;
      }
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
  const handleSave = useCallback(() => {
    const graph = graphRef.current;
    if (!graph) return;
    let graphData: GraphData;
    try {
      graphData = graph.toJSON();
    } catch (e) {
      console.error('Save failed (non-JSON-safe constant?):', e);
      showError('Save failed', (e as Error).message);
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
  }, [rfNodes, showError]);

  const loadInputRef = useRef<HTMLInputElement>(null);

  type LoadedFile = {
    graph: GraphData;
    layout?: { positions?: Record<string, { x: number; y: number }>; nextPos?: { x: number; y: number } };
  };

  const applyGraphData = useCallback((parsed: LoadedFile) => {
    const registry = registryRef.current;
    const ev = evalRef.current;
    if (!registry || !ev) return;
    const nextGraph = Graph.fromJSON(parsed.graph, registry);

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
      showError('Load failed', `Not valid JSON (${(e as Error).message})`);
      return;
    }
    if (parsed.formatVersion !== 1) {
      showError('Load failed', `Unsupported formatVersion ${parsed.formatVersion}`);
      return;
    }
    if (!parsed.graph) {
      showError('Load failed', 'File has no graph');
      return;
    }
    pushUndo();
    try {
      applyGraphData(parsed);
    } catch (e) {
      showError('Load failed', (e as Error).message);
    }
  }, [applyGraphData, pushUndo, showError]);

  const handleLoadClick = useCallback(() => {
    loadInputRef.current?.click();
  }, []);

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
    setRfNodes((nds) => [
      ...nds.map((n) => (n.selected ? { ...n, selected: false } : n)),
      ...created.map((n) => ({ ...n, selected: true })),
    ]);
    runEval();
  }, [rfNodes, toRfNode, pushUndo, setRfNodes, runEval]);

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

  const [paletteQuery, setPaletteQuery] = useState('');
  const filteredPalette = (() => {
    const q = paletteQuery.trim().toLowerCase();
    if (!q) return PALETTE.map((g) => ({ group: g, entries: g.entries }));
    return PALETTE
      .map((g) => ({ group: g, entries: g.entries.filter((e) => e.label.toLowerCase().includes(q)) }))
      .filter((g) => g.entries.length > 0);
  })();
  // When the user is searching we force-open every matching group;
  // when they're not, we let Accordion remember the user's choices.
  const [openGroups, setOpenGroups] = useState<string[]>(() => PALETTE.map((g) => g.label));
  const accordionValue = paletteQuery.trim()
    ? filteredPalette.map((g) => g.group.label)
    : openGroups;

  const [mode, setMode] = useState<'editor' | 'form'>('editor');

  // Right-click "add node" context menu.
  const rfRef = useRef<ReactFlowInstance | null>(null);
  const [ctxMenu, setCtxMenu] = useState<{ screenX: number; screenY: number; flowX: number; flowY: number } | null>(null);
  const onPaneContextMenu = useCallback((e: React.MouseEvent | MouseEvent) => {
    e.preventDefault();
    const rf = rfRef.current;
    if (!rf) return;
    const flow = rf.screenToFlowPosition({ x: e.clientX, y: e.clientY });
    setCtxMenu({ screenX: e.clientX, screenY: e.clientY, flowX: flow.x, flowY: flow.y });
  }, []);
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

  const handleLoadExample = useCallback((id: string) => {
    const ex = EXAMPLES.find((e) => e.id === id);
    if (!ex) return;
    pushUndo();
    try {
      applyGraphData(ex.file);
    } catch (e) {
      showError('Example load failed', (e as Error).message);
    }
  }, [applyGraphData, pushUndo, showError]);

  const [runTour, setRunTour] = useState(false);
  const handleTourEvent = useCallback((data: EventData) => {
    if (data.status === STATUS.FINISHED || data.status === STATUS.SKIPPED) {
      setRunTour(false);
    }
  }, []);

  if (loading) {
    return (
      <div className="flex h-screen w-screen items-center justify-center bg-background text-muted-foreground">
        Loading kernel…
      </div>
    );
  }

  const viewport = (
    <div
      data-tour="viewport"
      className="border-l border-border"
      style={{ flex: mode === 'form' ? 1 : undefined, width: mode === 'editor' ? 400 : undefined }}
    >
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
    <div className="fixed top-3 right-3 z-50">
      <Tabs value={mode} onValueChange={(v) => setMode(v as 'editor' | 'form')}>
        <TabsList>
          <TabsTrigger value="editor">Editor</TabsTrigger>
          <TabsTrigger value="form">Form</TabsTrigger>
        </TabsList>
      </Tabs>
    </div>
  );

  const content = mode === 'form' ? (
    <div className="flex h-screen w-screen bg-background text-foreground">
      {modeToggle}
      <aside className="w-80 border-r border-border bg-card">
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
      </aside>
      {viewport}
    </div>
  ) : (
    <div className="flex h-screen w-screen bg-background text-foreground">
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
      <aside
        data-tour="palette"
        className="flex w-56 flex-col gap-2 border-r border-border bg-card p-3"
      >
        <input
          ref={loadInputRef}
          type="file"
          accept="application/json,.json"
          className="hidden"
          onChange={(e) => {
            const f = e.target.files?.[0];
            if (f) handleLoadFile(f);
            e.target.value = '';
          }}
        />

        {/* Toolbar — Undo / Redo */}
        <div className="flex gap-1">
          <Tooltip>
            <TooltipTrigger asChild>
              <Button onClick={handleUndo} variant="outline" size="sm" className="flex-1">
                <Undo2 /> Undo
              </Button>
            </TooltipTrigger>
            <TooltipContent>Undo (⌘Z)</TooltipContent>
          </Tooltip>
          <Tooltip>
            <TooltipTrigger asChild>
              <Button onClick={handleRedo} variant="outline" size="sm" className="flex-1">
                <Redo2 /> Redo
              </Button>
            </TooltipTrigger>
            <TooltipContent>Redo (⇧⌘Z)</TooltipContent>
          </Tooltip>
        </div>

        {/* Toolbar — Save / Load */}
        <div className="flex gap-1">
          <Tooltip>
            <TooltipTrigger asChild>
              <Button onClick={handleSave} variant="outline" size="sm" className="flex-1">
                <Save /> Save
              </Button>
            </TooltipTrigger>
            <TooltipContent>Download graph as JSON (⌘S)</TooltipContent>
          </Tooltip>
          <Tooltip>
            <TooltipTrigger asChild>
              <Button onClick={handleLoadClick} variant="outline" size="sm" className="flex-1">
                <FolderOpen /> Load
              </Button>
            </TooltipTrigger>
            <TooltipContent>Load a saved graph</TooltipContent>
          </Tooltip>
        </div>

        {/* Examples dropdown */}
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="outline" size="sm" className="w-full justify-between">
              <span className="flex items-center gap-1.5"><BookOpen /> Examples</span>
              <ChevronDown />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start" className="w-56">
            {EXAMPLES.map((ex) => (
              <DropdownMenuItem
                key={ex.id}
                onSelect={() => handleLoadExample(ex.id)}
                className="flex-col items-start gap-0.5"
              >
                <span className="font-medium">{ex.label}</span>
                <span className="text-muted-foreground text-[10px]">{ex.description}</span>
              </DropdownMenuItem>
            ))}
          </DropdownMenuContent>
        </DropdownMenu>

        <Separator className="my-1" />

        {/* Header + tour button */}
        <div className="flex items-center justify-between">
          <span className="text-sm font-semibold">Add Node</span>
          <Tooltip>
            <TooltipTrigger asChild>
              <Button onClick={() => setRunTour(true)} variant="ghost" size="icon-xs">
                <HelpCircle />
              </Button>
            </TooltipTrigger>
            <TooltipContent>Take a guided tour</TooltipContent>
          </Tooltip>
        </div>

        {/* Search */}
        <div className="relative">
          <Search className="absolute left-2 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground" />
          <Input
            type="search"
            placeholder="Search nodes…"
            value={paletteQuery}
            onChange={(e) => setPaletteQuery(e.target.value)}
            className="h-8 pl-7 text-xs"
          />
        </div>

        {/* Palette accordion */}
        <ScrollArea className="flex-1 -mx-1">
          <div className="px-1">
            {filteredPalette.length === 0 ? (
              <div className="py-2 text-center text-xs text-muted-foreground">
                No nodes match “{paletteQuery}”
              </div>
            ) : (
              <Accordion
                type="multiple"
                value={accordionValue}
                onValueChange={(v) => { if (!paletteQuery.trim()) setOpenGroups(v); }}
              >
                {filteredPalette.map(({ group, entries }) => (
                  <AccordionItem
                    key={group.label}
                    value={group.label}
                    data-tour={`palette-${group.label.toLowerCase()}`}
                    className="border-b-0"
                  >
                    <AccordionTrigger className="py-1.5 text-[10px] font-medium uppercase tracking-wider text-muted-foreground hover:no-underline">
                      {group.label}
                    </AccordionTrigger>
                    <AccordionContent className="pb-2">
                      <div className="flex flex-col gap-1">
                        {entries.map((p) => (
                          <Button
                            key={p.defId}
                            onClick={() => addNode(p.defId)}
                            variant="secondary"
                            size="sm"
                            className="h-7 justify-start text-xs"
                          >
                            {p.label}
                          </Button>
                        ))}
                      </div>
                    </AccordionContent>
                  </AccordionItem>
                ))}
              </Accordion>
            )}
          </div>
        </ScrollArea>
      </aside>

      {/* Graph canvas */}
      <div data-tour="graph-canvas" className="flex-1">
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
          className="fixed z-[100] w-60 overflow-hidden rounded-md border border-border bg-popover text-popover-foreground shadow-lg"
          style={{
            left: Math.min(ctxMenu.screenX, window.innerWidth - 240),
            top: Math.min(ctxMenu.screenY, window.innerHeight - 360),
          }}
        >
          <Command>
            <CommandInput placeholder="Search nodes…" autoFocus />
            <CommandList className="max-h-72">
              <CommandEmpty>No matches</CommandEmpty>
              {PALETTE.map((group) => (
                <CommandGroup key={group.label} heading={group.label}>
                  {group.entries.map((p) => (
                    <CommandItem
                      key={p.defId}
                      value={`${group.label} ${p.label}`}
                      onSelect={() => {
                        addNode(p.defId, { x: ctxMenu.flowX, y: ctxMenu.flowY });
                        setCtxMenu(null);
                      }}
                    >
                      {p.label}
                    </CommandItem>
                  ))}
                </CommandGroup>
              ))}
            </CommandList>
          </Command>
        </div>
      )}
    </div>
  );

  return (
    <TooltipProvider delayDuration={300}>
      {content}
      <AlertDialog
        open={errorDialog !== null}
        onOpenChange={(open) => { if (!open) setErrorDialog(null); }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{errorDialog?.title ?? ''}</AlertDialogTitle>
            <AlertDialogDescription className="break-words">
              {errorDialog?.message ?? ''}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogAction onClick={() => setErrorDialog(null)}>OK</AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </TooltipProvider>
  );
}

// ── Output preview formatting ────────────────────────────────────
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
