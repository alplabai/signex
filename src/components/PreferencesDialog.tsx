import { useEditorStore } from "@/stores/editor";
import { useProjectStore } from "@/stores/project";
import { X } from "lucide-react";
import { cn } from "@/lib/utils";
import { useState } from "react";
import { BUILT_IN_TEMPLATES } from "@/lib/sheetTemplate";

interface Props {
  open: boolean;
  onClose: () => void;
}

export function PreferencesDialog({ open, onClose }: Props) {
  const [tab, setTab] = useState<"general" | "display" | "project" | "erc">("general");
  const gridSize = useEditorStore((s) => s.statusBar.gridSize);
  const snapEnabled = useEditorStore((s) => s.statusBar.snapEnabled);
  const units = useEditorStore((s) => s.statusBar.units);
  const autoJunction = useEditorStore((s) => s.autoJunction);
  const electricalSnapRange = useEditorStore((s) => s.electricalSnapRange);
  const ercSeverity = useEditorStore((s) => s.ercSeverity);
  const activeTemplate = useProjectStore((s) => s.activeTemplate);
  const netScope = useProjectStore((s) => s.netScope);

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="bg-bg-surface border border-border rounded-lg shadow-2xl w-[520px] max-h-[80vh] flex flex-col">
        <div className="flex items-center justify-between px-4 py-3 border-b border-border-subtle">
          <h2 className="text-sm font-semibold text-text-primary">Preferences</h2>
          <button onClick={onClose} className="p-1 rounded hover:bg-bg-hover text-text-muted"><X size={16} /></button>
        </div>

        <div className="flex border-b border-border-subtle px-4">
          {(["general", "display", "project", "erc"] as const).map(t => (
            <button key={t} onClick={() => setTab(t)}
              className={cn("px-3 py-2 text-xs border-b-2 transition-colors capitalize",
                tab === t ? "border-accent text-accent font-semibold" : "border-transparent text-text-muted hover:text-text-secondary")}>
              {t === "erc" ? "ERC" : t}
            </button>
          ))}
        </div>

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
                    onChange={() => useEditorStore.getState().toggleSnap()} className="accent-[#89b4fa]" />
                </Row>
                <Row label="Electrical Snap Range">
                  <input type="number" value={electricalSnapRange} step={0.1} min={0.5} max={10}
                    onChange={(e) => useEditorStore.getState().setElectricalSnapRange(parseFloat(e.target.value) || 2.0)}
                    className="w-20 bg-bg-primary border border-border-subtle rounded px-2 py-1 text-[11px] font-mono outline-none focus:border-accent" />
                  <span className="text-text-muted/50 text-[10px]">mm</span>
                </Row>
              </Section>
              <Section title="Editing">
                <Row label="Auto-Junction at T-Intersections">
                  <input type="checkbox" checked={autoJunction}
                    onChange={(e) => useEditorStore.getState().setAutoJunction(e.target.checked)} className="accent-[#89b4fa]" />
                </Row>
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
              <Section title="Rendering">
                <Row label="Display Net Color Override">
                  <input type="checkbox" checked={useEditorStore.getState().netColorOverride}
                    onChange={() => useEditorStore.getState().toggleNetColors()} className="accent-[#89b4fa]" />
                </Row>
              </Section>
            </>
          )}
          {tab === "project" && (
            <>
              <Section title="Sheet Template">
                <Row label="Active Template">
                  <select value={activeTemplate} onChange={(e) => useProjectStore.getState().setActiveTemplate(e.target.value)}
                    className="bg-bg-primary border border-border-subtle rounded px-2 py-1 text-[11px] outline-none focus:border-accent">
                    {BUILT_IN_TEMPLATES.map(t => <option key={t.name} value={t.name}>{t.name}</option>)}
                  </select>
                </Row>
              </Section>
              <Section title="Net Connectivity">
                <Row label="Net Identifier Scope">
                  <select value={netScope} onChange={(e) => useProjectStore.getState().setNetScope(e.target.value as "global" | "flat" | "hierarchical")}
                    className="bg-bg-primary border border-border-subtle rounded px-2 py-1 text-[11px] outline-none focus:border-accent">
                    <option value="global">Global (all sheets)</option>
                    <option value="flat">Flat (ports connect same-level)</option>
                    <option value="hierarchical">Hierarchical (ports ↔ sheet entries)</option>
                  </select>
                </Row>
              </Section>
            </>
          )}
          {tab === "erc" && (
            <Section title="Violation Severity">
              {[
                ["duplicate_designator", "Duplicate Designators"],
                ["unconnected_pin", "Unconnected Pins"],
                ["output_conflict", "Output-to-Output Conflict"],
                ["single_pin_net", "Single Pin Net"],
                ["no_driver", "No Driver on Net"],
              ].map(([key, label]) => (
                <Row key={key} label={label}>
                  <select value={ercSeverity[key] || "warning"}
                    onChange={(e) => useEditorStore.getState().setErcSeverity(key, e.target.value as "error" | "warning" | "none")}
                    className="bg-bg-primary border border-border-subtle rounded px-2 py-1 text-[11px] outline-none focus:border-accent">
                    <option value="error">Error</option>
                    <option value="warning">Warning</option>
                    <option value="none">No Report</option>
                  </select>
                </Row>
              ))}
            </Section>
          )}
        </div>

        <div className="flex justify-end gap-2 px-4 py-3 border-t border-border-subtle">
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
