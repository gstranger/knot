import type { DemoMode, DemoParams } from './App';

interface SidebarProps {
  params: DemoParams;
  onChange: (params: DemoParams) => void;
  onExport: (format: 'stl' | 'glb' | 'step') => void;
  faceCount: number | null;
  loading: boolean;
  error: Error | null;
}

const MODES: { value: DemoMode; label: string }[] = [
  { value: 'box', label: 'Box' },
  { value: 'extrude', label: 'Extrude' },
  { value: 'boolean', label: 'Boolean' },
  { value: 'revolve', label: 'Revolve' },
];

export function Sidebar({ params, onChange, onExport, faceCount, loading, error }: SidebarProps) {
  const set = <K extends keyof DemoParams>(key: K, value: DemoParams[K]) =>
    onChange({ ...params, [key]: value });

  return (
    <div style={sidebar}>
      <h2 style={{ margin: '0 0 16px', fontSize: 18 }}>Knot CAD</h2>

      {loading && <p style={{ color: '#aaa' }}>Loading kernel...</p>}

      <label style={labelStyle}>Mode</label>
      <select
        value={params.mode}
        onChange={(e) => set('mode', e.target.value as DemoMode)}
        style={selectStyle}
      >
        {MODES.map((m) => (
          <option key={m.value} value={m.value}>
            {m.label}
          </option>
        ))}
      </select>

      <Slider label="Size" value={params.size} min={0.5} max={4} step={0.1}
        onChange={(v) => set('size', v)} />

      {params.mode === 'extrude' && (
        <Slider label="Height" value={params.extrudeHeight} min={0.1} max={5} step={0.1}
          onChange={(v) => set('extrudeHeight', v)} />
      )}

      {params.mode === 'boolean' && (
        <Slider label="Offset" value={params.boolOffset} min={0} max={3} step={0.05}
          onChange={(v) => set('boolOffset', v)} />
      )}

      {params.mode === 'revolve' && (
        <Slider label="Angle" value={params.revolveAngle} min={30} max={360} step={10}
          onChange={(v) => set('revolveAngle', v)} />
      )}

      {error && (
        <p style={{ marginTop: 16, color: '#ff6b6b', fontSize: 13, wordBreak: 'break-word' }}>
          Error: {error.message}
        </p>
      )}

      {faceCount !== null && (
        <p style={{ marginTop: 16, color: '#aaa', fontSize: 13 }}>
          {faceCount} faces
        </p>
      )}

      <div style={{ marginTop: 'auto', display: 'flex', flexDirection: 'column', gap: 8 }}>
        <label style={{ ...labelStyle, marginTop: 16 }}>Export</label>
        <div style={{ display: 'flex', gap: 8 }}>
          <button style={btnStyle} onClick={() => onExport('stl')}>STL</button>
          <button style={btnStyle} onClick={() => onExport('glb')}>GLB</button>
          <button style={btnStyle} onClick={() => onExport('step')}>STEP</button>
        </div>
      </div>
    </div>
  );
}

function Slider({
  label,
  value,
  min,
  max,
  step,
  onChange,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  onChange: (v: number) => void;
}) {
  return (
    <div style={{ marginTop: 12 }}>
      <label style={labelStyle}>
        {label}: {value.toFixed(1)}
      </label>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        style={{ width: '100%', accentColor: '#4499dd' }}
      />
    </div>
  );
}

const sidebar: React.CSSProperties = {
  width: 260,
  padding: 20,
  background: '#16163a',
  borderRight: '1px solid #2a2a4e',
  display: 'flex',
  flexDirection: 'column',
  overflowY: 'auto',
};

const labelStyle: React.CSSProperties = {
  fontSize: 12,
  color: '#888',
  textTransform: 'uppercase',
  letterSpacing: 1,
  display: 'block',
  marginBottom: 4,
};

const selectStyle: React.CSSProperties = {
  width: '100%',
  padding: '6px 8px',
  background: '#1a1a2e',
  color: '#e0e0e0',
  border: '1px solid #3a3a5e',
  borderRadius: 4,
  fontSize: 14,
};

const btnStyle: React.CSSProperties = {
  flex: 1,
  padding: '8px 0',
  background: '#2a2a5e',
  color: '#e0e0e0',
  border: '1px solid #4a4a7e',
  borderRadius: 4,
  cursor: 'pointer',
  fontSize: 13,
};
