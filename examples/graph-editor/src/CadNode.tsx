import { Handle, Position, type NodeProps } from '@xyflow/react';
import { portColor } from './port-colors';

export interface CadNodeData {
  label: string;
  inputs: { name: string; kind: string }[];
  outputs: { name: string; kind: string }[];
  constants: Record<string, unknown>;
  onConstantChange?: (port: string, value: unknown) => void;
}

/** Custom React Flow node for CAD graph operations. */
export function CadNode({ data, selected }: NodeProps & { data: CadNodeData }) {
  const borderColor = selected ? '#fff' : '#444';

  return (
    <div
      style={{
        background: '#2a2a3e',
        border: `1px solid ${borderColor}`,
        borderRadius: 6,
        minWidth: 160,
        fontSize: 12,
        fontFamily: 'system-ui, sans-serif',
        color: '#e0e0e0',
      }}
    >
      {/* Header */}
      <div
        style={{
          padding: '6px 10px',
          borderBottom: '1px solid #444',
          fontWeight: 600,
          fontSize: 13,
        }}
      >
        {data.label}
      </div>

      {/* Ports */}
      <div style={{ padding: '4px 0' }}>
        {data.inputs.map((inp, i) => (
          <div key={`in-${inp.name}`} style={{ position: 'relative', padding: '3px 10px 3px 16px' }}>
            <Handle
              type="target"
              position={Position.Left}
              id={inp.name}
              style={{
                background: portColor(inp.kind),
                width: 10,
                height: 10,
                border: '2px solid #1a1a2e',
                top: '50%',
              }}
            />
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
              <span style={{ color: '#aaa' }}>{inp.name}</span>
              {inp.kind === 'number' && data.onConstantChange && (
                <input
                  type="number"
                  value={(data.constants[inp.name] as number) ?? 0}
                  onChange={(e) => data.onConstantChange?.(inp.name, parseFloat(e.target.value) || 0)}
                  style={{
                    width: 50,
                    background: '#1a1a2e',
                    border: '1px solid #555',
                    borderRadius: 3,
                    color: '#e0e0e0',
                    padding: '1px 4px',
                    fontSize: 11,
                    textAlign: 'right',
                  }}
                  className="nodrag"
                />
              )}
            </div>
          </div>
        ))}
        {data.outputs.map((out) => (
          <div key={`out-${out.name}`} style={{ position: 'relative', padding: '3px 16px 3px 10px', textAlign: 'right' }}>
            <Handle
              type="source"
              position={Position.Right}
              id={out.name}
              style={{
                background: portColor(out.kind),
                width: 10,
                height: 10,
                border: '2px solid #1a1a2e',
                top: '50%',
              }}
            />
            <span style={{ color: '#aaa' }}>{out.name}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
