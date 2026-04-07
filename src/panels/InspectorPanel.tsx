import { useSchematicStore } from "@/stores/schematic";

/**
 * Inspector Panel — detailed object inspection (Altium-style).
 * Shows all properties of the selected object in a read-only table format.
 */
export function InspectorPanel() {
  const data = useSchematicStore((s) => s.data);
  const selectedIds = useSchematicStore((s) => s.selectedIds);

  if (!data) return <div className="p-4 text-xs text-text-muted/50">No schematic loaded</div>;
  if (selectedIds.size === 0) return <div className="p-4 text-xs text-text-muted/50">Select an object to inspect</div>;

  const uuid = [...selectedIds][0];

  // Find object
  const sym = data.symbols.find((s) => s.uuid === uuid);
  if (sym) return <SymbolInspector sym={sym} />;

  const wire = data.wires.find((w) => w.uuid === uuid);
  if (wire) return <ObjectTable title="Wire" fields={{
    UUID: wire.uuid,
    "Start X": wire.start.x.toFixed(3),
    "Start Y": wire.start.y.toFixed(3),
    "End X": wire.end.x.toFixed(3),
    "End Y": wire.end.y.toFixed(3),
    Length: Math.hypot(wire.end.x - wire.start.x, wire.end.y - wire.start.y).toFixed(3),
  }} />;

  const label = data.labels.find((l) => l.uuid === uuid);
  if (label) return <ObjectTable title={`Label: ${label.text}`} fields={{
    UUID: label.uuid,
    Text: label.text,
    Type: label.label_type,
    X: label.position.x.toFixed(3),
    Y: label.position.y.toFixed(3),
    Rotation: `${label.rotation}\u00b0`,
    "Font Size": label.font_size.toFixed(2),
    Shape: label.shape || "none",
    Justify: label.justify,
  }} />;

  const junction = data.junctions.find((j) => j.uuid === uuid);
  if (junction) return <ObjectTable title="Junction" fields={{
    UUID: junction.uuid,
    X: junction.position.x.toFixed(3),
    Y: junction.position.y.toFixed(3),
  }} />;

  const nc = data.no_connects.find((n) => n.uuid === uuid);
  if (nc) return <ObjectTable title="No Connect" fields={{
    UUID: nc.uuid,
    X: nc.position.x.toFixed(3),
    Y: nc.position.y.toFixed(3),
  }} />;

  return <div className="p-4 text-xs text-text-muted/50">Unknown object type</div>;
}

function SymbolInspector({ sym }: { sym: import("@/types").SchSymbol }) {
  const fields: Record<string, string> = {
    UUID: sym.uuid,
    Reference: sym.reference,
    Value: sym.value,
    Footprint: sym.footprint || "(none)",
    "Library ID": sym.lib_id,
    X: sym.position.x.toFixed(3),
    Y: sym.position.y.toFixed(3),
    Rotation: `${sym.rotation}\u00b0`,
    "Mirror X": sym.mirror_x ? "Yes" : "No",
    "Mirror Y": sym.mirror_y ? "Yes" : "No",
    Unit: String(sym.unit),
    "Is Power": sym.is_power ? "Yes" : "No",
    DNP: sym.dnp ? "Yes" : "No",
    "In BOM": sym.in_bom ? "Yes" : "No",
    "On Board": sym.on_board ? "Yes" : "No",
    "Exclude Sim": sym.exclude_from_sim ? "Yes" : "No",
    Locked: sym.locked ? "Yes" : "No",
  };

  // Add custom fields
  for (const [k, v] of Object.entries(sym.fields)) {
    fields[`Field: ${k}`] = v;
  }

  return <ObjectTable title={`Component: ${sym.reference}`} fields={fields} />;
}

function ObjectTable({ title, fields }: { title: string; fields: Record<string, string> }) {
  return (
    <div className="text-xs">
      <div className="px-3 py-2 border-b border-border-subtle">
        <span className="text-[11px] font-semibold text-text-secondary">{title}</span>
      </div>
      <div className="overflow-y-auto">
        {Object.entries(fields).map(([key, value]) => (
          <div key={key} className="flex items-center px-3 py-0.5 border-b border-border-subtle/20 hover:bg-bg-hover/30">
            <span className="w-24 shrink-0 text-[10px] text-text-muted/60">{key}</span>
            <span className="flex-1 text-[10px] font-mono text-text-primary truncate select-all">{value}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
