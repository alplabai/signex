import { usePcbStore } from "@/stores/pcb";
import { cn } from "@/lib/utils";

export function PcbPropertiesPanel() {
  const data = usePcbStore((s) => s.data);
  const selectedIds = usePcbStore((s) => s.selectedIds);
  if (!data) {
    return <div className="p-4 text-xs text-text-muted/50">No PCB loaded</div>;
  }

  if (selectedIds.size === 0) {
    return <BoardProperties />;
  }

  // Find selected object
  const uuid = [...selectedIds][0];

  const fp = data.footprints.find((f) => f.uuid === uuid);
  if (fp) return <FootprintProperties fp={fp} />;

  const seg = data.segments.find((s) => s.uuid === uuid);
  if (seg) return <SegmentProperties seg={seg} netName={data.nets.find((n) => n.number === seg.net)?.name || ""} />;

  const via = data.vias.find((v) => v.uuid === uuid);
  if (via) return <ViaProperties via={via} netName={data.nets.find((n) => n.number === via.net)?.name || ""} />;

  return <BoardProperties />;
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="border-b border-border-subtle pb-2 mb-2">
      <div className="px-3 py-1 text-[10px] font-semibold text-text-muted uppercase tracking-wider">{title}</div>
      <div className="px-3 space-y-1">{children}</div>
    </div>
  );
}

function Row({ label, value, mono }: { label: string; value: string | number; mono?: boolean }) {
  return (
    <div className="flex items-center justify-between gap-2 text-[11px]">
      <span className="text-text-muted/60">{label}</span>
      <span className={cn("text-text-primary truncate max-w-[150px]", mono && "font-mono text-[10px]")}>{value}</span>
    </div>
  );
}

function BoardProperties() {
  const data = usePcbStore((s) => s.data);
  if (!data) return null;

  return (
    <div className="text-xs">
      <div className="px-3 py-2 border-b border-border-subtle">
        <span className="text-[11px] font-semibold text-text-secondary">Board Properties</span>
      </div>
      <Section title="General">
        <Row label="Thickness" value={`${data.board.thickness} mm`} />
        <Row label="Copper Layers" value={data.board.layers.copperCount} />
        <Row label="Generator" value={data.board.generator} />
      </Section>
      <Section title="Setup">
        <Row label="Grid" value={`${data.board.setup.gridSize} mm`} mono />
        <Row label="Trace Width" value={`${data.board.setup.traceWidth} mm`} mono />
        <Row label="Via Diameter" value={`${data.board.setup.viaDiameter} mm`} mono />
        <Row label="Via Drill" value={`${data.board.setup.viaDrill} mm`} mono />
        <Row label="Clearance" value={`${data.board.setup.clearance} mm`} mono />
      </Section>
      <Section title="Statistics">
        <Row label="Footprints" value={data.footprints.length} />
        <Row label="Pads" value={data.footprints.reduce((s, f) => s + f.pads.length, 0)} />
        <Row label="Segments" value={data.segments.length} />
        <Row label="Vias" value={data.vias.length} />
        <Row label="Zones" value={data.zones.length} />
        <Row label="Nets" value={data.nets.length} />
      </Section>
      <Section title="Design Rules">
        {data.designRules.map((rule) => (
          <div key={rule.uuid} className="flex items-center gap-2 text-[10px]">
            <span className={cn("w-2 h-2 rounded-full", rule.enabled ? "bg-success" : "bg-text-muted/20")} />
            <span className="text-text-muted/60">{rule.name}</span>
            <span className="text-text-primary font-mono ml-auto">
              {rule.min !== undefined ? `${rule.min}` : ""}{rule.preferred !== undefined ? ` / ${rule.preferred}` : ""}{rule.max !== undefined ? ` / ${rule.max}` : ""} mm
            </span>
          </div>
        ))}
      </Section>
    </div>
  );
}

function FootprintProperties({ fp }: { fp: import("@/types/pcb").PcbFootprint }) {
  return (
    <div className="text-xs">
      <div className="px-3 py-2 border-b border-border-subtle">
        <span className="text-[11px] font-semibold text-text-secondary">Footprint: {fp.reference}</span>
      </div>
      <Section title="General">
        <Row label="Reference" value={fp.reference} />
        <Row label="Value" value={fp.value} />
        <Row label="Footprint" value={fp.footprintId} />
        <Row label="Layer" value={fp.layer} />
        <Row label="Locked" value={fp.locked ? "Yes" : "No"} />
      </Section>
      <Section title="Position">
        <Row label="X" value={`${fp.position.x.toFixed(3)} mm`} mono />
        <Row label="Y" value={`${fp.position.y.toFixed(3)} mm`} mono />
        <Row label="Rotation" value={`${fp.rotation}°`} mono />
      </Section>
      <Section title="Pads">
        {fp.pads.slice(0, 20).map((pad) => (
          <div key={pad.uuid} className="flex items-center gap-2 text-[10px] py-0.5">
            <span className="font-mono text-accent w-6">{pad.number}</span>
            <span className="text-text-muted/60">{pad.shape} {pad.type}</span>
            <span className="ml-auto font-mono text-text-primary">{pad.size[0].toFixed(2)}x{pad.size[1].toFixed(2)}</span>
            {pad.net && <span className="text-accent/50 text-[9px]">{pad.net.name}</span>}
          </div>
        ))}
        {fp.pads.length > 20 && (
          <div className="text-[10px] text-text-muted/40">... and {fp.pads.length - 20} more</div>
        )}
      </Section>
    </div>
  );
}

function SegmentProperties({ seg, netName }: { seg: import("@/types/pcb").PcbSegment; netName: string }) {
  const length = Math.hypot(seg.end.x - seg.start.x, seg.end.y - seg.start.y);
  return (
    <div className="text-xs">
      <div className="px-3 py-2 border-b border-border-subtle">
        <span className="text-[11px] font-semibold text-text-secondary">Trace Segment</span>
      </div>
      <Section title="Properties">
        <Row label="Net" value={netName || `Net ${seg.net}`} />
        <Row label="Layer" value={seg.layer} />
        <Row label="Width" value={`${seg.width.toFixed(3)} mm`} mono />
        <Row label="Length" value={`${length.toFixed(3)} mm`} mono />
      </Section>
      <Section title="Position">
        <Row label="Start X" value={`${seg.start.x.toFixed(3)} mm`} mono />
        <Row label="Start Y" value={`${seg.start.y.toFixed(3)} mm`} mono />
        <Row label="End X" value={`${seg.end.x.toFixed(3)} mm`} mono />
        <Row label="End Y" value={`${seg.end.y.toFixed(3)} mm`} mono />
      </Section>
    </div>
  );
}

function ViaProperties({ via, netName }: { via: import("@/types/pcb").PcbVia; netName: string }) {
  return (
    <div className="text-xs">
      <div className="px-3 py-2 border-b border-border-subtle">
        <span className="text-[11px] font-semibold text-text-secondary">Via</span>
      </div>
      <Section title="Properties">
        <Row label="Net" value={netName || `Net ${via.net}`} />
        <Row label="Type" value={via.type} />
        <Row label="Diameter" value={`${via.diameter.toFixed(3)} mm`} mono />
        <Row label="Drill" value={`${via.drill.toFixed(3)} mm`} mono />
        <Row label="Layers" value={`${via.layers[0]} → ${via.layers[1]}`} />
      </Section>
      <Section title="Position">
        <Row label="X" value={`${via.position.x.toFixed(3)} mm`} mono />
        <Row label="Y" value={`${via.position.y.toFixed(3)} mm`} mono />
      </Section>
    </div>
  );
}
