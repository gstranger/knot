import { Handle, Position, type NodeProps } from '@xyflow/react';
import { TriangleAlert } from 'lucide-react';
import { portColor } from './port-colors';
import { cn } from '@/lib/utils';
import { Input } from '@/components/ui/input';
import { Slider } from '@/components/ui/slider';
import { Switch } from '@/components/ui/switch';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';

export interface CadNodeData {
  label: string;
  defId: string;
  inputs: { name: string; kind: string }[];
  outputs: { name: string; kind: string }[];
  constants: Record<string, unknown>;
  onConstantChange?: (port: string, value: unknown) => void;
  previews?: Record<string, { text: string; error?: boolean }>;
}

/** Custom React Flow node for CAD graph operations. */
export function CadNode({ data, selected }: NodeProps & { data: CadNodeData }) {
  const isSlider = data.defId === 'core.slider';
  const isToggle = data.defId === 'core.toggle';
  const isExpression = data.defId === 'math.expression';

  const erroredEntry = data.previews
    ? Object.entries(data.previews).find(([, p]) => p.error)
    : undefined;
  const hasError = !!erroredEntry;
  const errorMsg = erroredEntry?.[1].text;

  return (
    <div
      className={cn(
        'rounded-md border bg-card text-card-foreground text-xs font-sans shadow-sm',
        isSlider ? 'min-w-[200px]' : 'min-w-[160px]',
        hasError ? 'border-destructive ring-1 ring-destructive/35' :
                   selected ? 'border-primary' : 'border-border',
      )}
    >
      {/* Header */}
      <div className="flex items-center justify-between border-b border-border px-2.5 py-1.5 text-[13px] font-semibold">
        <span>{data.label}</span>
        {hasError && (
          <Tooltip>
            <TooltipTrigger asChild>
              <TriangleAlert className="size-3.5 text-destructive cursor-help" />
            </TooltipTrigger>
            <TooltipContent>{errorMsg}</TooltipContent>
          </Tooltip>
        )}
      </div>

      {/* Slider body */}
      {isSlider && data.onConstantChange && (
        <div className="px-2.5 py-1.5">
          <Slider
            min={(data.constants._min as number) ?? 0}
            max={(data.constants._max as number) ?? 10}
            step={(data.constants._step as number) ?? 0.1}
            value={[(data.constants.value as number) ?? 0.5]}
            onValueChange={(v) => data.onConstantChange?.('value', v[0])}
            className="nodrag"
          />
          <div className="mt-1 flex justify-between text-[10px] text-muted-foreground">
            <span>{(data.constants._min as number) ?? 0}</span>
            <span className="font-semibold text-foreground">
              {((data.constants.value as number) ?? 0.5).toFixed(2)}
            </span>
            <span>{(data.constants._max as number) ?? 10}</span>
          </div>
        </div>
      )}

      {/* Toggle body */}
      {isToggle && data.onConstantChange && (
        <div className="flex items-center gap-2 px-2.5 py-1.5">
          <Switch
            checked={(data.constants.value as boolean) ?? false}
            onCheckedChange={(v) => data.onConstantChange?.('value', v)}
            className="nodrag"
          />
          <span className="text-muted-foreground">
            {(data.constants.value as boolean) ? 'true' : 'false'}
          </span>
        </div>
      )}

      {/* Expression editor */}
      {isExpression && data.onConstantChange && (
        <div className="px-2.5 py-1">
          <Input
            type="text"
            value={(data.constants.expr as string) ?? 'a'}
            onChange={(e) => data.onConstantChange?.('expr', e.target.value)}
            placeholder="a * sin(b)"
            className="nodrag h-6 font-mono text-[11px] text-emerald-400"
          />
        </div>
      )}

      {/* Ports */}
      <div className="py-1">
        {data.inputs.map((inp) => {
          // Hide the `value` input row on slider/toggle nodes; the
          // body control above is the canonical editor for those.
          if ((isSlider || isToggle) && inp.name === 'value') {
            return (
              <div key={`in-${inp.name}`} className="relative h-0">
                <Handle
                  type="target" position={Position.Left} id={inp.name}
                  style={{ background: portColor(inp.kind), width: 10, height: 10, border: '2px solid var(--card)', top: 14 }}
                />
              </div>
            );
          }
          return (
            <div key={`in-${inp.name}`} className="relative px-2.5 py-0.5 pl-4">
              <Handle
                type="target" position={Position.Left} id={inp.name}
                style={{ background: portColor(inp.kind), width: 10, height: 10, border: '2px solid var(--card)', top: '50%' }}
              />
              <div className="flex items-center justify-between gap-1">
                <span className="text-muted-foreground">{inp.name}</span>
                {inp.kind === 'number' && data.onConstantChange && (
                  <Input
                    type="number"
                    value={(data.constants[inp.name] as number) ?? 0}
                    onChange={(e) => data.onConstantChange?.(inp.name, parseFloat(e.target.value) || 0)}
                    className="nodrag h-5 w-14 px-1.5 text-right text-[11px]"
                  />
                )}
              </div>
            </div>
          );
        })}
        {data.outputs.map((out) => {
          const preview = data.previews?.[out.name];
          return (
            <div key={`out-${out.name}`} className="relative px-2.5 py-0.5 pr-4 text-right">
              <Handle
                type="source" position={Position.Right} id={out.name}
                style={{ background: portColor(out.kind), width: 10, height: 10, border: '2px solid var(--card)', top: '50%' }}
              />
              <div className="flex items-center justify-between gap-1.5">
                {preview ? (
                  <span
                    title={preview.error ? preview.text : undefined}
                    className={cn(
                      'flex-1 truncate text-left font-mono text-[10px]',
                      preview.error ? 'text-destructive' : 'text-muted-foreground',
                    )}
                    style={{ maxWidth: 90 }}
                  >
                    {preview.error ? '⚠ error' : `→ ${preview.text}`}
                  </span>
                ) : <span className="flex-1" />}
                <span className="text-muted-foreground">{out.name}</span>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
