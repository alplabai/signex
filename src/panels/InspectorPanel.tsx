import { useState, useCallback } from "react";
import { useSchematicStore } from "@/stores/schematic";
import { zoomToObject } from "@/lib/crossProbe";
import { Copy, Check, Crosshair } from "lucide-react";
import type { SchSymbol } from "@/types";

export function InspectorPanel() {
  const data = useSchematicStore((s) => s.data);
  const selectedIds = useSchematicStore((s) => s.selectedIds);

  if (!data) return <div className="p-4 text-xs text-text-muted/50">No schematic loaded</div>;
  if (selectedIds.size === 0) return <div className="p-4 text-xs text-text-muted/50">Select an object to inspect</div>;

  const uuids = [...selectedIds];

  // Compare mode for 2+ objects
  if (uuids.length >= 2 && uuids.length <= 4) {
    const objects = uuids.map(uuid => resolveObject(uuid, data)).filter(Boolean) as ResolvedObject[];
    if (objects.length >= 2) return <CompareTable objects={objects} />;
  }

  const uuid = uuids[0];

  const sym = data.symbols.find((s) => s.uuid === uuid);
  if (sym) return <SymbolInspector sym={sym} />;

  const wire = data.wires.find((w) => w.uuid === uuid);
  if (wire) return <ObjectTable title="Wire" uuid={wire.uuid} fields={{
    UUID: wire.uuid,
    "Start X": wire.start.x.toFixed(3),
    "Start Y": wire.start.y.toFixed(3),
    "End X": wire.end.x.toFixed(3),
    "End Y": wire.end.y.toFixed(3),
    Length: Math.hypot(wire.end.x - wire.start.x, wire.end.y - wire.start.y).toFixed(3),
  }} />;

  const label = data.labels.find((l) => l.uuid === uuid);
  if (label) return <ObjectTable title={`Label: ${label.text}`} uuid={label.uuid} fields={{
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
  if (junction) return <ObjectTable title="Junction" uuid={junction.uuid} fields={{
    UUID: junction.uuid,
    X: junction.position.x.toFixed(3),
    Y: junction.position.y.toFixed(3),
  }} />;

  const nc = data.no_connects.find((n) => n.uuid === uuid);
  if (nc) return <ObjectTable title="No Connect" uuid={nc.uuid} fields={{
    UUID: nc.uuid,
    X: nc.position.x.toFixed(3),
    Y: nc.position.y.toFixed(3),
  }} />;

  return <div className="p-4 text-xs text-text-muted/50">Unknown object type</div>;
}

interface ResolvedObject {
  uuid: string;
  type: string;
  name: string;
  fields: Record<string, string>;
}

function resolveObject(uuid: string, data: import("@/types").SchematicData): ResolvedObject | null {
  const sym = data.symbols.find(s => s.uuid === uuid);
  if (sym) return {
    uuid, type: "Component", name: sym.reference,
    fields: { Reference: sym.reference, Value: sym.value, Footprint: sym.footprint || "--", X: sym.position.x.toFixed(3), Y: sym.position.y.toFixed(3), Rotation: `${sym.rotation}\u00b0` },
  };

  const label = data.labels.find(l => l.uuid === uuid);
  if (label) return {
    uuid, type: "Label", name: label.text,
    fields: { Text: label.text, Type: label.label_type, X: label.position.x.toFixed(3), Y: label.position.y.toFixed(3), Rotation: `${label.rotation}\u00b0` },
  };

  const wire = data.wires.find(w => w.uuid === uuid);
  if (wire) return {
    uuid, type: "Wire", name: "Wire",
    fields: { "Start X": wire.start.x.toFixed(3), "Start Y": wire.start.y.toFixed(3), "End X": wire.end.x.toFixed(3), "End Y": wire.end.y.toFixed(3) },
  };

  return null;
}

function CompareTable({ objects }: { objects: ResolvedObject[] }) {
  const allKeys = new Set<string>();
  for (const obj of objects) for (const k of Object.keys(obj.fields)) allKeys.add(k);

  return (
    <div className="text-xs">
      <div className="px-3 py-2 border-b border-border-subtle">
        <span className="text-[11px] font-semibold text-text-secondary">Compare ({objects.length} objects)</span>
      </div>
      <div className="overflow-y-auto">
        {/* Header row */}
        <div className="flex items-center px-3 py-0.5 border-b border-border-subtle bg-bg-secondary/40 text-[9px] font-semibold text-text-muted/60">
          <span className="w-20 shrink-0">Property</span>
          {objects.map((obj, i) => (
            <span key={i} className="flex-1 truncate font-mono">{obj.name}</span>
          ))}
        </div>
        {/* Type row */}
        <div className="flex items-center px-3 py-0.5 border-b border-border-subtle/20">
          <span className="w-20 shrink-0 text-[10px] text-text-muted/60">Type</span>
          {objects.map((obj, i) => (
            <span key={i} className="flex-1 text-[10px] font-mono text-text-primary">{obj.type}</span>
          ))}
        </div>
        {[...allKeys].map(key => {
          const values = objects.map(o => o.fields[key] ?? "--");
          const allSame = values.every(v => v === values[0]);
          return (
            <div key={key} className="flex items-center px-3 py-0.5 border-b border-border-subtle/20 hover:bg-bg-hover/30">
              <span className="w-20 shrink-0 text-[10px] text-text-muted/60">{key}</span>
              {values.map((val, i) => (
                <span key={i} className={`flex-1 text-[10px] font-mono truncate ${allSame ? "text-text-primary" : "text-warning"}`}>{val}</span>
              ))}
            </div>
          );
        })}
      </div>
    </div>
  );
}

function SymbolInspector({ sym }: { sym: SchSymbol }) {
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
    Locked: sym.locked ? "Yes" : "No",
  };

  for (const [k, v] of Object.entries(sym.fields)) {
    fields[`Field: ${k}`] = v;
  }

  return <ObjectTable title={`Component: ${sym.reference}`} uuid={sym.uuid} fields={fields} />;
}

function ObjectTable({ title, uuid, fields }: { title: string; uuid: string; fields: Record<string, string> }) {
  const [copied, setCopied] = useState<string | null>(null);

  const copyValue = useCallback((key: string, value: string) => {
    navigator.clipboard.writeText(value);
    setCopied(key);
    setTimeout(() => setCopied(null), 1000);
  }, []);

  return (
    <div className="text-xs">
      <div className="px-3 py-2 border-b border-border-subtle flex items-center gap-2">
        <span className="text-[11px] font-semibold text-text-secondary flex-1">{title}</span>
        <button
          onClick={() => zoomToObject(uuid)}
          className="p-0.5 rounded hover:bg-bg-hover text-text-muted/50 hover:text-accent"
          title="Zoom to object"
        >
          <Crosshair size={12} />
        </button>
      </div>
      <div className="overflow-y-auto">
        {Object.entries(fields).map(([key, value]) => (
          <div
            key={key}
            className="flex items-center px-3 py-0.5 border-b border-border-subtle/20 hover:bg-bg-hover/30 group cursor-pointer"
            onClick={() => copyValue(key, value)}
          >
            <span className="w-24 shrink-0 text-[10px] text-text-muted/60">{key}</span>
            <span className="flex-1 text-[10px] font-mono text-text-primary truncate select-all">{value}</span>
            <span className="opacity-0 group-hover:opacity-100 transition-opacity shrink-0 ml-1">
              {copied === key
                ? <Check size={10} className="text-success" />
                : <Copy size={10} className="text-text-muted/40" />
              }
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}
