import { useState } from "react";
import { Play, Plus, Trash2, ChevronDown, ChevronUp, Settings2, PlayCircle, Loader2 } from "lucide-react";
import { useOutputJobsStore, getJobTypeLabel } from "@/stores/outputJobs";
import { useSchematicStore } from "@/stores/schematic";
import { invoke } from "@tauri-apps/api/core";
import { exportSchematicPdf } from "@/lib/pdfExport";
import { cn } from "@/lib/utils";
import type { OutputJob, OutputJobType } from "@/stores/outputJobs";

export function OutputJobsPanel() {
  const jobs = useOutputJobsStore((s) => s.jobs);
  const addJob = useOutputJobsStore((s) => s.addJob);
  const removeJob = useOutputJobsStore((s) => s.removeJob);
  const toggleJob = useOutputJobsStore((s) => s.toggleJob);
  const updateJob = useOutputJobsStore((s) => s.updateJob);
  const reorderJob = useOutputJobsStore((s) => s.reorderJob);
  const data = useSchematicStore((s) => s.data);
  const [runningId, setRunningId] = useState<string | null>(null);
  const [runningAll, setRunningAll] = useState(false);
  const [configId, setConfigId] = useState<string | null>(null);
  const [showAddMenu, setShowAddMenu] = useState(false);

  const jobTypes: OutputJobType[] = ["bom", "netlist", "pdf", "png"];

  const runJob = async (job: OutputJob) => {
    if (!data) return;
    setRunningId(job.id);
    try {
      switch (job.type) {
        case "bom": {
          const c = job.config;
          const result = await invoke<string>("generate_bom_configured", {
            data,
            columns: c.bomColumns || ["Designator", "Value", "Footprint", "Library", "Quantity"],
            groupBy: c.bomGroupBy || ["Value", "Footprint"],
            format: c.bomFormat || "csv",
          });
          const ext = c.bomFormat === "tsv" ? "tsv" : "csv";
          downloadBlob(result, `bom.${ext}`, c.bomFormat === "tsv" ? "text/tab-separated-values" : "text/csv");
          break;
        }
        case "netlist": {
          const cmd = job.config.netlistFormat === "xml" ? "export_netlist_xml" : "export_netlist";
          const result = await invoke<string>(cmd, { data });
          const ext = job.config.netlistFormat === "xml" ? "xml" : "net";
          downloadBlob(result, `netlist.${ext}`, job.config.netlistFormat === "xml" ? "application/xml" : "text/plain");
          break;
        }
        case "pdf": {
          const blob = await exportSchematicPdf(data, {
            dpi: job.config.pdfDpi || 300,
            showGrid: job.config.pdfShowGrid || false,
            colorMode: job.config.pdfColorMode || "color",
          });
          const url = URL.createObjectURL(blob);
          const a = document.createElement("a");
          a.href = url; a.download = "schematic.pdf"; a.click();
          URL.revokeObjectURL(url);
          break;
        }
        case "png": {
          window.dispatchEvent(new CustomEvent("alp-export-png"));
          break;
        }
      }
    } catch (e) {
      console.error(`Job "${job.name}" failed:`, e);
    } finally {
      setRunningId(null);
    }
  };

  const runAllEnabled = async () => {
    setRunningAll(true);
    for (const job of jobs) {
      if (job.enabled) await runJob(job);
    }
    setRunningAll(false);
  };

  return (
    <div className="text-xs h-full flex flex-col">
      {/* Toolbar */}
      <div className="flex items-center gap-2 px-3 py-1.5 border-b border-border-subtle shrink-0">
        <div className="relative">
          <button onClick={() => setShowAddMenu(!showAddMenu)}
            className="flex items-center gap-1.5 px-2 py-0.5 rounded bg-accent/15 text-accent hover:bg-accent/25 transition-colors text-[11px]">
            <Plus size={11} /> Add Job
          </button>
          {showAddMenu && (
            <div className="absolute top-full left-0 mt-1 bg-[#1e1e2e] border border-[#45475a] rounded shadow-xl z-50 py-1 min-w-[140px]">
              {jobTypes.map((t) => (
                <button key={t} onClick={() => { addJob(t); setShowAddMenu(false); }}
                  className="w-full px-3 py-1.5 text-left text-[11px] text-[#cdd6f4] hover:bg-[#313244] transition-colors">
                  {getJobTypeLabel(t)}
                </button>
              ))}
            </div>
          )}
        </div>
        <button onClick={runAllEnabled}
          disabled={!data || runningAll || jobs.filter((j) => j.enabled).length === 0}
          className="flex items-center gap-1.5 px-2 py-0.5 rounded bg-success/15 text-success hover:bg-success/25 transition-colors text-[11px] disabled:opacity-40 disabled:pointer-events-none">
          {runningAll ? <Loader2 size={11} className="animate-spin" /> : <PlayCircle size={11} />}
          Run All
        </button>
        <div className="flex-1" />
        <span className="text-text-muted/40 text-[10px]">
          {jobs.length} job{jobs.length !== 1 ? "s" : ""}{jobs.filter((j) => j.enabled).length < jobs.length && ` (${jobs.filter((j) => j.enabled).length} enabled)`}
        </span>
      </div>

      {/* Job list */}
      <div className="flex-1 overflow-y-auto">
        {jobs.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full text-text-muted/40 gap-2 py-8">
            <Settings2 size={20} />
            <span className="text-[11px]">No output jobs configured</span>
            <span className="text-[10px]">Click "Add Job" to create one</span>
          </div>
        ) : (
          jobs.map((job, idx) => (
            <div key={job.id} className={cn(
              "border-b border-border-subtle",
              !job.enabled && "opacity-50"
            )}>
              {/* Job row */}
              <div className="flex items-center gap-2 px-3 py-1.5">
                <input type="checkbox" checked={job.enabled} onChange={() => toggleJob(job.id)}
                  className="accent-[#89b4fa]" />
                <span className={cn("flex-1 text-[11px] truncate",
                  job.enabled ? "text-text-primary" : "text-text-muted"
                )}>
                  {job.name}
                </span>
                <span className="text-[9px] text-text-muted/50 uppercase tracking-wider px-1.5 py-0.5 bg-bg-surface rounded">
                  {job.type}
                </span>
                <button onClick={() => runJob(job)} disabled={!data || runningId === job.id}
                  className="p-0.5 rounded text-text-muted/50 hover:text-success hover:bg-success/10 transition-colors disabled:opacity-30">
                  {runningId === job.id ? <Loader2 size={12} className="animate-spin" /> : <Play size={12} />}
                </button>
                <button onClick={() => setConfigId(configId === job.id ? null : job.id)}
                  className="p-0.5 rounded text-text-muted/50 hover:text-accent hover:bg-accent/10 transition-colors">
                  <Settings2 size={12} />
                </button>
                <button onClick={() => reorderJob(job.id, "up")} disabled={idx === 0}
                  className="p-0.5 rounded text-text-muted/30 hover:text-text-primary transition-colors disabled:opacity-20">
                  <ChevronUp size={12} />
                </button>
                <button onClick={() => reorderJob(job.id, "down")} disabled={idx === jobs.length - 1}
                  className="p-0.5 rounded text-text-muted/30 hover:text-text-primary transition-colors disabled:opacity-20">
                  <ChevronDown size={12} />
                </button>
                <button onClick={() => removeJob(job.id)}
                  className="p-0.5 rounded text-text-muted/30 hover:text-error hover:bg-error/10 transition-colors">
                  <Trash2 size={12} />
                </button>
              </div>

              {/* Config panel (inline) */}
              {configId === job.id && (
                <JobConfigPanel job={job} onUpdate={(updates) => updateJob(job.id, updates)} />
              )}
            </div>
          ))
        )}
      </div>
    </div>
  );
}

function JobConfigPanel({ job, onUpdate }: { job: OutputJob; onUpdate: (u: Partial<OutputJob>) => void }) {
  const updateConfig = (key: string, value: unknown) => {
    onUpdate({ config: { ...job.config, [key]: value } });
  };

  return (
    <div className="px-4 py-2 bg-bg-surface/30 border-t border-border-subtle space-y-2">
      {/* Name */}
      <div className="flex items-center gap-2">
        <span className="text-[10px] text-text-muted w-14">Name</span>
        <input value={job.name} onChange={(e) => onUpdate({ name: e.target.value })}
          onKeyDown={(e) => e.stopPropagation()}
          className="flex-1 bg-bg-surface border border-border-subtle rounded px-2 py-0.5 text-[10px] font-mono text-text-primary outline-none focus:border-accent" />
      </div>

      {job.type === "bom" && (
        <>
          <div className="flex items-center gap-2">
            <span className="text-[10px] text-text-muted w-14">Format</span>
            <select value={job.config.bomFormat || "csv"} onChange={(e) => updateConfig("bomFormat", e.target.value)}
              className="bg-bg-surface border border-border-subtle rounded px-2 py-0.5 text-[10px] text-text-primary outline-none focus:border-accent">
              <option value="csv">CSV</option>
              <option value="tsv">TSV</option>
            </select>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-[10px] text-text-muted w-14">Group by</span>
            <div className="flex gap-2">
              {["Value", "Footprint", "Library"].map((g) => (
                <label key={g} className="flex items-center gap-1 cursor-pointer">
                  <input type="checkbox" className="accent-[#89b4fa]"
                    checked={(job.config.bomGroupBy || []).includes(g)}
                    onChange={(e) => {
                      const cur = job.config.bomGroupBy || [];
                      updateConfig("bomGroupBy", e.target.checked ? [...cur, g] : cur.filter((x) => x !== g));
                    }} />
                  <span className="text-[10px] text-text-primary">{g}</span>
                </label>
              ))}
            </div>
          </div>
        </>
      )}

      {job.type === "netlist" && (
        <div className="flex items-center gap-2">
          <span className="text-[10px] text-text-muted w-14">Format</span>
          <select value={job.config.netlistFormat || "kicad"} onChange={(e) => updateConfig("netlistFormat", e.target.value)}
            className="bg-bg-surface border border-border-subtle rounded px-2 py-0.5 text-[10px] text-text-primary outline-none focus:border-accent">
            <option value="kicad">KiCad S-expression</option>
            <option value="xml">Generic XML</option>
          </select>
        </div>
      )}

      {job.type === "pdf" && (
        <>
          <div className="flex items-center gap-2">
            <span className="text-[10px] text-text-muted w-14">DPI</span>
            <select value={job.config.pdfDpi || 300} onChange={(e) => updateConfig("pdfDpi", Number(e.target.value))}
              className="bg-bg-surface border border-border-subtle rounded px-2 py-0.5 text-[10px] text-text-primary outline-none focus:border-accent">
              <option value={150}>150 (Draft)</option>
              <option value={300}>300 (High Quality)</option>
            </select>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-[10px] text-text-muted w-14">Color</span>
            <select value={job.config.pdfColorMode || "color"} onChange={(e) => updateConfig("pdfColorMode", e.target.value)}
              className="bg-bg-surface border border-border-subtle rounded px-2 py-0.5 text-[10px] text-text-primary outline-none focus:border-accent">
              <option value="color">Color (Dark)</option>
              <option value="monochrome">Print (Light)</option>
            </select>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-[10px] text-text-muted w-14">Grid</span>
            <input type="checkbox" checked={job.config.pdfShowGrid || false}
              onChange={(e) => updateConfig("pdfShowGrid", e.target.checked)}
              className="accent-[#89b4fa]" />
          </div>
        </>
      )}

      {job.type === "png" && (
        <div className="text-[10px] text-text-muted/60 italic">
          Uses current canvas settings
        </div>
      )}
    </div>
  );
}

function downloadBlob(content: string, filename: string, mime: string) {
  const blob = new Blob([content], { type: mime });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}
