import { useState } from "react";
import { useEditorStore } from "@/stores/editor";
import { X } from "lucide-react";
import { cn } from "@/lib/utils";

interface Props {
  open: boolean;
  onClose: () => void;
}

export function PreferencesDialog({ open, onClose }: Props) {
  const [tab, setTab] = useState<"general" | "display" | "erc">("general");
  const gridSize = useEditorStore((s) => s.statusBar.gridSize);
  const snapEnabled = useEditorStore((s) => s.statusBar.snapEnabled);
  const units = useEditorStore((s) => s.statusBar.units);

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="bg-bg-surface border border-border rounded-lg shadow-2xl w-[520px] max-h-[80vh] flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-border-subtle">
          <h2 className="text-sm font-semibold text-text-primary">Preferences</h2>
          <button onClick={onClose} className="p-1 rounded hover:bg-bg-hover text-text-muted"><X size={16} /></button>
        </div>

        {/* Tabs */}
        <div className="flex border-b border-border-subtle px-4">
          {(["general", "display", "erc"] as const).map(t => (
            <button key={t} onClick={() => setTab(t)}
              className={cn("px-3 py-2 text-xs border-b-2 transition-colors capitalize",
                tab === t ? "border-accent text-accent font-semibold" : "border-transparent text-text-muted hover:text-text-secondary")}>
              {t === "erc" ? "ERC" : t}
            </button>
          ))}
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto p-4 text-xs space-y-4">
          {tab === "general" && (
            <>
              <Section title="Grid & Snap">
                <Row label="Default Grid Size">
                  <input type="number" value={gridSize} step={0.01}
                    onChange={(e) => useEditorStore.getState().setGridSize(parseFloat(e.target.value) || 1.27)}
                    className="w-20 bg-bg-primary border border-border-subtle rounded px-2 py-1 text-[11px] font-mono outline-none focus:border-accent" />
                  <span className="text-text-muted/50 text-[10px]">{units}</span>
                </Row>
                <Row label="Snap Enabled">
                  <input type="checkbox" checked={snapEnabled}
                    onChange={() => useEditorStore.getState().toggleSnap()} />
                </Row>
                <Row label="Electrical Snap Range">
                  <input type="number" value={2.0} step={0.1}
                    className="w-20 bg-bg-primary border border-border-subtle rounded px-2 py-1 text-[11px] font-mono outline-none" />
                  <span className="text-text-muted/50 text-[10px]">world units</span>
                </Row>
              </Section>
              <Section title="Editing">
                <Row label="Auto-Junction at T-Intersections"><input type="checkbox" defaultChecked /></Row>
                <Row label="Break Wires at Auto-Junctions"><input type="checkbox" defaultChecked /></Row>
                <Row label="Optimize Wires & Buses"><input type="checkbox" defaultChecked /></Row>
                <Row label="Enable In-Place Editing (F2)"><input type="checkbox" defaultChecked /></Row>
              </Section>
              <Section title="Auto-Increment">
                <Row label="Primary (Numeric)"><input type="checkbox" defaultChecked /></Row>
                <Row label="Secondary (Alpha)"><input type="checkbox" /></Row>
              </Section>
            </>
          )}
          {tab === "display" && (
            <>
              <Section title="Units">
                <Row label="Default Units">
                  <select value={units} onChange={(e) => useEditorStore.getState().updateStatusBar({ units: e.target.value as "mm" | "mil" | "inch" })}
                    className="bg-bg-primary border border-border-subtle rounded px-2 py-1 text-[11px] outline-none focus:border-accent">
                    <option value="mm">Millimeters (mm)</option>
                    <option value="mil">Mils</option>
                    <option value="inch">Inches</option>
                  </select>
                </Row>
              </Section>
              <Section title="Cursor">
                <Row label="Cursor Type">
                  <select className="bg-bg-primary border border-border-subtle rounded px-2 py-1 text-[11px] outline-none">
                    <option>Large 90</option><option>Small 90</option><option>Small 45</option><option>Tiny 45</option>
                  </select>
                </Row>
              </Section>
              <Section title="Rendering">
                <Row label="Display Cross-Overs"><input type="checkbox" defaultChecked /></Row>
                <Row label="Display Net Color Override"><input type="checkbox" /></Row>
                <Row label="AutoFocus (Dim Unconnected)"><input type="checkbox" defaultChecked /></Row>
              </Section>
            </>
          )}
          {tab === "erc" && (
            <>
              <Section title="Violation Severity">
                <Row label="Duplicate Designators">
                  <select className="bg-bg-primary border border-border-subtle rounded px-2 py-1 text-[11px] outline-none">
                    <option>Error</option><option>Warning</option><option>No Report</option>
                  </select>
                </Row>
                <Row label="Unconnected Pins">
                  <select className="bg-bg-primary border border-border-subtle rounded px-2 py-1 text-[11px] outline-none">
                    <option>Warning</option><option>Error</option><option>No Report</option>
                  </select>
                </Row>
                <Row label="Output-to-Output Conflict">
                  <select className="bg-bg-primary border border-border-subtle rounded px-2 py-1 text-[11px] outline-none">
                    <option>Error</option><option>Warning</option><option>No Report</option>
                  </select>
                </Row>
                <Row label="Single Pin Net">
                  <select className="bg-bg-primary border border-border-subtle rounded px-2 py-1 text-[11px] outline-none">
                    <option>Warning</option><option>Error</option><option>No Report</option>
                  </select>
                </Row>
                <Row label="No Driver on Net">
                  <select className="bg-bg-primary border border-border-subtle rounded px-2 py-1 text-[11px] outline-none">
                    <option>Warning</option><option>Error</option><option>No Report</option>
                  </select>
                </Row>
              </Section>
            </>
          )}
        </div>

        {/* Footer */}
        <div className="flex justify-end gap-2 px-4 py-3 border-t border-border-subtle">
          <button onClick={onClose}
            className="px-4 py-1.5 rounded text-xs bg-bg-hover text-text-secondary hover:bg-bg-surface transition-colors">
            Cancel
          </button>
          <button onClick={onClose}
            className="px-4 py-1.5 rounded text-xs bg-accent/20 text-accent hover:bg-accent/30 transition-colors">
            OK
          </button>
        </div>
      </div>
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div>
      <h3 className="text-[11px] font-semibold text-text-secondary uppercase tracking-wider mb-2">{title}</h3>
      <div className="space-y-2 pl-1">{children}</div>
    </div>
  );
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-3">
      <span className="text-text-muted/70 text-[11px]">{label}</span>
      <div className="flex items-center gap-2">{children}</div>
    </div>
  );
}
