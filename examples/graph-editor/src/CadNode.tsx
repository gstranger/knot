import { Handle, Position, type NodeProps } from '@xyflow/react';
import { portColor } from './port-colors';

export interface CadNodeData {
  label: string;
  defId: string;
  inputs: { name: string; kind: string }[];
  outputs: { name: string; kind: string }[];
  constants: Record<string, unknown>;
  onConstantChange?: (port: string, value: unknown) => void;
  /**
   * Short, human-readable summary of each output port's computed
   * value after the last eval. Keyed by port name. Used purely for
   * display — never round-tripped to the kernel.
   */
  previews?: Record<string, { text: string; error?: boolean }>;
}

/** Custom React Flow node for CAD graph operations. */
export function CadNode({ data, selected }: NodeProps & { data: CadNodeData }) {
  const isSlider = data.defId === 'core.slider';
  const isToggle = data.defId === 'core.toggle';
  const isExpression = data.defId === 'math.expression';

  // First errored output determines the badge tooltip + ring color.
  // We surface only the first error to avoid stacking — clicking
  // into the node and inspecting the offending output's per-port
  // preview gives the full picture.
  const erroredEntry = data.previews
    ? Object.entries(data.previews).find(([, p]) => p.error)
    : undefined;
  const hasError = !!erroredEntry;
  const errorMsg = erroredEntry?.[1].text;

  const borderColor = hasError ? '#ff6666' : (selected ? '#fff' : '#444');

  return (
    <div
      style={{
        background: '#2a2a3e',
        border: `1px solid ${borderColor}`,
        boxShadow: hasError ? '0 0 0 1px rgba(255, 100, 100, 0.35)' : undefined,
        borderRadius: 6,
        minWidth: isSlider ? 200 : 160,
        fontSize: 12,
        fontFamily: 'system-ui, sans-serif',
        color: '#e0e0e0',
      }}
    >
      {/* Header */}
      <div
        style={{
          padding: '6px 10px', borderBottom: '1px solid #444',
          fontWeight: 600, fontSize: 13,
          display: 'flex', alignItems: 'center', justifyContent: 'space-between',
        }}
      >
        <span>{data.label}</span>
        {hasError && (
          <span title={errorMsg} style={{ color: '#ff6666', fontSize: 12, cursor: 'help' }}>⚠</span>
        )}
      </div>

      {/* Slider body */}
      {isSlider && data.onConstantChange && (
        <div style={{ padding: '6px 10px' }}>
          <input
            type="range"
            min={(data.constants._min as number) ?? 0}
            max={(data.constants._max as number) ?? 10}
            step={(data.constants._step as number) ?? 0.1}
            value={(data.constants.value as number) ?? 0.5}
            onChange={(e) => data.onConstantChange?.('value', parseFloat(e.target.value))}
            style={{ width: '100%', accentColor: portColor('number') }}
            className="nodrag"
          />
          <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 10, color: '#888', marginTop: 2 }}>
            <span>{(data.constants._min as number) ?? 0}</span>
            <span style={{ color: '#e0e0e0', fontWeight: 600 }}>{
              ((data.constants.value as number) ?? 0.5).toFixed(2)
            }</span>
            <span>{(data.constants._max as number) ?? 10}</span>
          </div>
        </div>
      )}

      {/* Toggle body */}
      {isToggle && data.onConstantChange && (
        <div style={{ padding: '6px 10px', display: 'flex', alignItems: 'center', gap: 8 }}>
          <input
            type="checkbox"
            checked={(data.constants.value as boolean) ?? false}
            onChange={(e) => data.onConstantChange?.('value', e.target.checked)}
            className="nodrag"
          />
          <span style={{ color: '#aaa' }}>{(data.constants.value as boolean) ? 'true' : 'false'}</span>
        </div>
      )}

      {/* Expression editor */}
      {isExpression && data.onConstantChange && (
        <div style={{ padding: '4px 10px' }}>
          <input
            type="text"
            value={(data.constants.expr as string) ?? 'a'}
            onChange={(e) => data.onConstantChange?.('expr', e.target.value)}
            placeholder="a * sin(b)"
            style={{
              width: '100%', background: '#1a1a2e', border: '1px solid #555',
              borderRadius: 3, color: '#4aff8b', padding: '3px 6px', fontSize: 11,
              fontFamily: 'monospace',
            }}
            className="nodrag"
          />
        </div>
      )}

      {/* Ports */}
      <div style={{ padding: '4px 0' }}>
        {data.inputs.map((inp) => {
          // Skip rendering the value port for special input nodes.
          if ((isSlider || isToggle) && inp.name === 'value') {
            return (
              <div key={`in-${inp.name}`} style={{ position: 'relative', height: 0 }}>
                <Handle type="target" position={Position.Left} id={inp.name}
                  style={{ background: portColor(inp.kind), width: 10, height: 10, border: '2px solid #1a1a2e', top: 14 }}
                />
              </div>
            );
          }
          return (
            <div key={`in-${inp.name}`} style={{ position: 'relative', padding: '3px 10px 3px 16px' }}>
              <Handle type="target" position={Position.Left} id={inp.name}
                style={{ background: portColor(inp.kind), width: 10, height: 10, border: '2px solid #1a1a2e', top: '50%' }}
              />
              <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                <span style={{ color: '#aaa' }}>{inp.name}</span>
                {inp.kind === 'number' && data.onConstantChange && (
                  <input type="number"
                    value={(data.constants[inp.name] as number) ?? 0}
                    onChange={(e) => data.onConstantChange?.(inp.name, parseFloat(e.target.value) || 0)}
                    style={{ width: 50, background: '#1a1a2e', border: '1px solid #555', borderRadius: 3, color: '#e0e0e0', padding: '1px 4px', fontSize: 11, textAlign: 'right' }}
                    className="nodrag"
                  />
                )}
              </div>
            </div>
          );
        })}
        {data.outputs.map((out) => {
          const preview = data.previews?.[out.name];
          return (
            <div key={`out-${out.name}`} style={{ position: 'relative', padding: '3px 16px 3px 10px', textAlign: 'right' }}>
              <Handle type="source" position={Position.Right} id={out.name}
                style={{ background: portColor(out.kind), width: 10, height: 10, border: '2px solid #1a1a2e', top: '50%' }}
              />
              <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 6 }}>
                {preview ? (
                  <span
                    title={preview.error ? preview.text : undefined}
                    style={{
                      color: preview.error ? '#ff6666' : '#666',
                      fontSize: 10, fontFamily: 'monospace',
                      overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
                      maxWidth: 90, textAlign: 'left', flex: 1,
                    }}
                  >{preview.error ? '⚠ error' : `→ ${preview.text}`}</span>
                ) : <span style={{ flex: 1 }} />}
                <span style={{ color: '#aaa' }}>{out.name}</span>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
