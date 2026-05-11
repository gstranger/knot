/** Port kind → wire/handle color. */
export const PORT_COLORS: Record<string, string> = {
  number: '#4a9eff',   // blue
  bool: '#ff4a6a',     // red
  vec3: '#4aff8b',     // green
  brep: '#ffa94a',     // orange
  curve: '#d94aff',    // purple
  list: '#ffdd4a',     // yellow
  error: '#ff2222',    // bright red
};

export const portColor = (kind: string): string => PORT_COLORS[kind] ?? '#888';
