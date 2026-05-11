/**
 * FormView — "form mode" for a Graph. Renders the graph's
 * `extractFormFields` output as a column of HTML controls; each edit
 * pushes back into the graph via `setFormValue` and calls the
 * supplied `onChange` so the host can re-run the evaluator.
 *
 * Headless w.r.t. the viewport: it doesn't render geometry. The host
 * lays the `FormView` next to a `<Canvas>` + `<KnotMesh>` or any
 * other display. That keeps the component small and reusable.
 */
import { type CSSProperties, type ReactNode } from 'react';
import type { Graph, FormField, Vec3FormField } from '../graph';
import { extractFormFields, setFormValue } from '../graph';

export interface FormViewProps {
  graph: Graph;
  /** Called after every value change, with the field that mutated. */
  onChange?: (field: FormField) => void;
  /** Override styling on the outer container. */
  style?: CSSProperties;
  /** Title rendered above the field list. */
  title?: string;
  /** Optional empty-state element when the graph exposes no fields. */
  emptyState?: ReactNode;
}

export function FormView({
  graph,
  onChange,
  style,
  title = 'Inputs',
  emptyState,
}: FormViewProps) {
  const fields = extractFormFields(graph);

  if (fields.length === 0) {
    return (
      <div style={{ ...containerStyle, ...style }}>
        <h3 style={headingStyle}>{title}</h3>
        {emptyState ?? (
          <p style={emptyStyle}>
            No exposed inputs. Add Number / Slider / Toggle / Vec3 nodes
            to the graph; any with no incoming wires appear here.
          </p>
        )}
      </div>
    );
  }

  const handleChange = (field: FormField, value: FormField['value']) => {
    setFormValue(graph, field, value);
    onChange?.(field);
  };

  return (
    <div style={{ ...containerStyle, ...style }}>
      <h3 style={headingStyle}>{title}</h3>
      <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
        {fields.map((f) => (
          <FieldRow key={f.nodeId} field={f} onChange={handleChange} />
        ))}
      </div>
    </div>
  );
}

function FieldRow({
  field,
  onChange,
}: {
  field: FormField;
  onChange: (f: FormField, value: FormField['value']) => void;
}) {
  switch (field.kind) {
    case 'number':
      return (
        <label style={labelStyle}>
          <span style={labelTextStyle}>{field.label}</span>
          <div style={{ display: 'flex', gap: 8 }}>
            {field.min !== undefined && field.max !== undefined ? (
              <input
                type="range"
                min={field.min}
                max={field.max}
                step={field.step ?? 0.01}
                value={field.value}
                onChange={(e) => onChange(field, Number(e.target.value))}
                style={{ flex: 1 }}
              />
            ) : null}
            <input
              type="number"
              value={field.value}
              step={field.step ?? 'any'}
              onChange={(e) => onChange(field, Number(e.target.value))}
              style={numberInputStyle}
            />
          </div>
        </label>
      );
    case 'bool':
      return (
        <label style={labelStyle}>
          <span style={labelTextStyle}>{field.label}</span>
          <input
            type="checkbox"
            checked={field.value}
            onChange={(e) => onChange(field, e.target.checked)}
          />
        </label>
      );
    case 'vec3':
      return <Vec3Row field={field} onChange={onChange} />;
  }
}

function Vec3Row({
  field,
  onChange,
}: {
  field: Vec3FormField;
  onChange: (f: FormField, value: FormField['value']) => void;
}) {
  const setComp = (k: 'x' | 'y' | 'z', v: number) => {
    onChange(field, { ...field.value, [k]: v });
  };
  return (
    <div style={labelStyle}>
      <span style={labelTextStyle}>{field.label}</span>
      <div style={{ display: 'flex', gap: 4 }}>
        {(['x', 'y', 'z'] as const).map((k) => (
          <input
            key={k}
            type="number"
            value={field.value[k]}
            step="any"
            onChange={(e) => setComp(k, Number(e.target.value))}
            style={vec3InputStyle}
            aria-label={`${field.label} ${k}`}
          />
        ))}
      </div>
    </div>
  );
}

const containerStyle: CSSProperties = {
  background: '#1a1a2e',
  color: '#e0e0f0',
  padding: 16,
  fontFamily: 'system-ui, -apple-system, sans-serif',
  fontSize: 13,
  overflowY: 'auto',
};

const headingStyle: CSSProperties = {
  margin: '0 0 16px 0',
  fontSize: 14,
  fontWeight: 600,
  letterSpacing: 0.5,
  textTransform: 'uppercase',
  color: '#9090b0',
};

const emptyStyle: CSSProperties = {
  color: '#6b6b87',
  fontStyle: 'italic',
  lineHeight: 1.4,
};

const labelStyle: CSSProperties = {
  display: 'flex',
  flexDirection: 'column',
  gap: 4,
};

const labelTextStyle: CSSProperties = {
  fontSize: 12,
  color: '#c0c0d0',
};

const numberInputStyle: CSSProperties = {
  width: 80,
  background: '#0e0e1a',
  color: '#e0e0f0',
  border: '1px solid #333',
  padding: '4px 6px',
  borderRadius: 4,
};

const vec3InputStyle: CSSProperties = {
  ...numberInputStyle,
  flex: 1,
  width: undefined,
};
