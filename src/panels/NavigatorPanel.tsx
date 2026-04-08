import { useSchematicStore } from "@/stores/schematic";
import { useProjectStore } from "@/stores/project";
import { zoomToObject, zoomToObjects } from "@/lib/crossProbe";
import { ChevronDown, ChevronRight, FileText, Search } from "lucide-react";
import { useState, useMemo, useCallback } from "react";
import { cn } from "@/lib/utils";

// --- Inline SVG icons for Altium-style decoration ---

/** Small squiggly net icon */
function NetIcon({ className }: { className?: string }) {
  return (
    <svg
      width="10"
      height="10"
      viewBox="0 0 10 10"
      className={cn("shrink-0", className)}
      fill="none"
      stroke="currentColor"
      strokeWidth="1.2"
      strokeLinecap="round"
    >
      <path d="M1 5 Q3 2 5 5 Q7 8 9 5" />
    </svg>
  );
}

/** Small port arrow icon */
function PortIcon({ direction, className }: { direction?: string; className?: string }) {
  // Arrow direction based on port type
  const isInput = direction === "Input" || direction === "input";
  const isOutput = direction === "Output" || direction === "output";
  return (
    <svg
      width="10"
      height="10"
      viewBox="0 0 10 10"
      className={cn("shrink-0", className)}
      fill="currentColor"
      stroke="none"
    >
      {isInput ? (
        // Arrow pointing right (into sheet)
        <polygon points="2,2 8,5 2,8" />
      ) : isOutput ? (
        // Arrow pointing left (out of sheet)
        <polygon points="8,2 2,5 8,8" />
      ) : (
        // Bidirectional diamond
        <polygon points="5,1 9,5 5,9 1,5" />
      )}
    </svg>
  );
}

// --- Collapsible section header (Altium-style bar) ---

function SectionHeader({
  title,
  open,
  onToggle,
  count,
}: {
  title: string;
  open: boolean;
  onToggle: () => void;
  count?: number;
}) {
  return (
    <button
      onClick={onToggle}
      className="flex items-center gap-1 w-full px-2 py-1 text-[11px] font-bold
        bg-bg-secondary/80 border-b border-border-subtle
        hover:bg-bg-hover/60 transition-colors text-text-secondary select-none"
    >
      {open ? <ChevronDown size={10} /> : <ChevronRight size={10} />}
      <span>{title}</span>
      {count !== undefined && (
        <span className="ml-auto text-[10px] text-text-muted/50 font-normal">{count}</span>
      )}
    </button>
  );
}

// --- Table header row ---

function TableHead({ columns }: { columns: { label: string; flex?: string }[] }) {
  return (
    <div className="flex items-center px-2 py-0.5 text-[10px] font-semibold text-text-muted/60
      border-b border-border-subtle bg-bg-secondary/40 select-none">
      {columns.map((col, i) => (
        <span key={i} className={col.flex ?? "flex-1"} style={{ minWidth: 0 }}>
          {col.label}
        </span>
      ))}
    </div>
  );
}

// --- Table row with expand arrow ---

function TableRow({
  cells,
  selected,
  onClick,
  icon,
  expandable,
}: {
  cells: { text: string; flex?: string; mono?: boolean }[];
  selected?: boolean;
  onClick?: () => void;
  icon?: React.ReactNode;
  expandable?: boolean;
}) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "flex items-center w-full px-2 py-[1px] text-[10px] transition-colors text-left gap-0.5",
        selected
          ? "bg-accent/15 text-accent"
          : "text-text-muted hover:bg-bg-hover/50 hover:text-text-primary"
      )}
    >
      {/* Expand arrow (small triangle) */}
      {expandable && (
        <ChevronRight
          size={8}
          className="shrink-0 text-text-muted/40 mr-0.5"
        />
      )}
      {/* Optional leading icon */}
      {icon && <span className="shrink-0 mr-0.5">{icon}</span>}
      {cells.map((cell, i) => (
        <span
          key={i}
          className={cn(
            "truncate",
            cell.flex ?? "flex-1",
            cell.mono && "font-mono"
          )}
          style={{ minWidth: 0 }}
        >
          {cell.text}
        </span>
      ))}
    </button>
  );
}

// --- Documents section ---

function DocumentsSection() {
  const [open, setOpen] = useState(true);
  const project = useProjectStore((s) => s.project);
  const activeTabId = useProjectStore((s) => s.activeTabId);
  const openTabs = useProjectStore((s) => s.openTabs);

  const sheets = project?.sheets ?? [];
  const projectName = project?.name ?? "Untitled";

  return (
    <div>
      <SectionHeader
        title={`Documents for ${projectName}`}
        open={open}
        onToggle={() => setOpen(!open)}
        count={sheets.length || undefined}
      />
      {open && (
        <div>
          {/* Flattened Hierarchy entry */}
          <div className="flex items-center gap-1.5 px-3 py-[2px] text-[10px] text-text-muted/60 font-semibold select-none border-b border-border-subtle/30">
            <ChevronDown size={8} className="text-text-muted/40" />
            <span>Flattened Hierarchy</span>
          </div>
          {sheets.length === 0 ? (
            <div className="px-3 py-1 text-[10px] text-text-muted/40 italic">
              No sheets in project
            </div>
          ) : (
            sheets.map((sheet, idx) => {
              const isActive = openTabs.some(
                (t) => t.id === activeTabId && t.name === sheet.name
              );
              return (
                <button
                  key={sheet.filename}
                  className={cn(
                    "flex items-center gap-1.5 w-full px-4 py-[2px] text-[10px] transition-colors",
                    isActive
                      ? "bg-accent/15 text-accent"
                      : "text-text-muted hover:bg-bg-hover/50 hover:text-text-primary"
                  )}
                >
                  <FileText size={10} className="shrink-0 text-text-muted/50" />
                  <span className="truncate">
                    {sheet.name}
                  </span>
                  <span className="text-text-muted/50 font-mono text-[9px]">
                    ([{idx + 1}] {sheet.filename})
                  </span>
                </button>
              );
            })
          )}
        </div>
      )}
    </div>
  );
}

// --- Instance (Components) section ---

function InstanceSection({ filter = "" }: { filter?: string }) {
  const [open, setOpen] = useState(true);
  const data = useSchematicStore((s) => s.data);
  const selectedIds = useSchematicStore((s) => s.selectedIds);
  const select = useSchematicStore((s) => s.select);

  const components = useMemo(() => {
    const q = filter.toLowerCase();
    return (data?.symbols ?? [])
      .filter((s) => !s.is_power)
      .filter((s) => !q || s.reference.toLowerCase().includes(q) || s.value.toLowerCase().includes(q))
      .sort((a, b) => a.reference.localeCompare(b.reference, undefined, { numeric: true }));
  }, [data?.symbols, filter]);

  return (
    <div>
      <SectionHeader
        title="Instances"
        open={open}
        onToggle={() => setOpen(!open)}
        count={components.length || undefined}
      />
      {open && (
        <div>
          <TableHead
            columns={[
              { label: "", flex: "w-[12px] shrink-0" },
              { label: "Instance", flex: "w-[68px] shrink-0" },
              { label: "Comment", flex: "flex-1" },
              { label: "Type", flex: "w-[72px] shrink-0 text-right" },
            ]}
          />
          {components.length === 0 ? (
            <div className="px-3 py-1 text-[10px] text-text-muted/40 italic">
              No components
            </div>
          ) : (
            <div className="max-h-[240px] overflow-y-auto">
              {components.map((sym) => (
                <TableRow
                  key={sym.uuid}
                  selected={selectedIds.has(sym.uuid)}
                  onClick={() => { select(sym.uuid); zoomToObject(sym.uuid); }}
                  expandable
                  cells={[
                    { text: sym.reference, flex: "w-[68px] shrink-0", mono: true },
                    { text: sym.value || "--", flex: "flex-1" },
                    { text: "Component", flex: "w-[72px] shrink-0 text-right" },
                  ]}
                />
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

// --- Net / Bus section ---

function NetBusSection({ filter = "" }: { filter?: string }) {
  const [open, setOpen] = useState(true);
  const data = useSchematicStore((s) => s.data);
  const selectedIds = useSchematicStore((s) => s.selectedIds);
  const selectMultiple = useSchematicStore((s) => s.selectMultiple);

  const nets = useMemo(() => {
    if (!data) return [];
    const q = filter.toLowerCase();

    const netMap = new Map<string, { uuids: string[]; text: string; labelType: string }>();
    for (const label of data.labels) {
      if (label.label_type === "Net" || label.label_type === "Global" || label.label_type === "Power") {
        const existing = netMap.get(label.text);
        if (existing) {
          existing.uuids.push(label.uuid);
        } else {
          netMap.set(label.text, {
            uuids: [label.uuid],
            text: label.text,
            labelType: label.label_type,
          });
        }
      }
    }

    return Array.from(netMap.values())
      .filter(n => !q || n.text.toLowerCase().includes(q))
      .sort((a, b) => a.text.localeCompare(b.text, undefined, { numeric: true }));
  }, [data?.labels, filter]);

  const getScopeLabel = (labelType: string) => {
    switch (labelType) {
      case "Global":
        return "Sheet Interface";
      case "Power":
        return "Global";
      case "Net":
      default:
        return "Local To Document";
    }
  };

  const handleNetClick = useCallback(
    (net: { uuids: string[] }) => {
      selectMultiple(net.uuids);
      if (net.uuids.length > 0) zoomToObjects(net.uuids);
    },
    [selectMultiple]
  );

  return (
    <div>
      <SectionHeader
        title="Net / Bus"
        open={open}
        onToggle={() => setOpen(!open)}
        count={nets.length || undefined}
      />
      {open && (
        <div>
          <TableHead
            columns={[
              { label: "", flex: "w-[14px] shrink-0" },
              { label: "Net / Bus", flex: "flex-1" },
              { label: "Scope", flex: "w-[110px] shrink-0 text-right" },
            ]}
          />
          {nets.length === 0 ? (
            <div className="px-3 py-1 text-[10px] text-text-muted/40 italic">
              No nets
            </div>
          ) : (
            <div className="max-h-[200px] overflow-y-auto">
              {nets.map((net) => {
                const isSelected = net.uuids.some((u) => selectedIds.has(u));
                return (
                  <TableRow
                    key={net.text}
                    selected={isSelected}
                    onClick={() => handleNetClick(net)}
                    icon={<NetIcon className="text-text-muted/50" />}
                    cells={[
                      { text: net.text, flex: "flex-1", mono: true },
                      { text: getScopeLabel(net.labelType), flex: "w-[110px] shrink-0 text-right" },
                    ]}
                  />
                );
              })}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

// --- Ports section ---

function PortsSection({ filter = "" }: { filter?: string }) {
  const [open, setOpen] = useState(true);
  const data = useSchematicStore((s) => s.data);
  const selectedIds = useSchematicStore((s) => s.selectedIds);
  const select = useSchematicStore((s) => s.select);

  const ports = useMemo(() => {
    if (!data) return [];
    const q = filter.toLowerCase();

    const items: { uuid: string; name: string; portType: string }[] = [];

    for (const label of data.labels) {
      if (label.label_type === "Hierarchical") {
        items.push({ uuid: label.uuid, name: label.text, portType: "Hierarchical" });
      }
    }

    for (const sheet of data.child_sheets) {
      for (const pin of sheet.pins) {
        items.push({ uuid: pin.uuid, name: pin.name, portType: pin.direction || "Bidirectional" });
      }
    }

    return items
      .filter(p => !q || p.name.toLowerCase().includes(q))
      .sort((a, b) => a.name.localeCompare(b.name, undefined, { numeric: true }));
  }, [data?.labels, data?.child_sheets, filter]);

  return (
    <div>
      <SectionHeader
        title="Ports"
        open={open}
        onToggle={() => setOpen(!open)}
        count={ports.length || undefined}
      />
      {open && (
        <div>
          <TableHead
            columns={[
              { label: "", flex: "w-[14px] shrink-0" },
              { label: "Port", flex: "w-[28px] shrink-0" },
              { label: "Name", flex: "flex-1" },
              { label: "Type", flex: "w-[90px] shrink-0 text-right" },
            ]}
          />
          {ports.length === 0 ? (
            <div className="px-3 py-1 text-[10px] text-text-muted/40 italic">
              No ports
            </div>
          ) : (
            <div className="max-h-[160px] overflow-y-auto">
              {ports.map((port, idx) => (
                <TableRow
                  key={port.uuid}
                  selected={selectedIds.has(port.uuid)}
                  onClick={() => { select(port.uuid); zoomToObject(port.uuid); }}
                  icon={<PortIcon direction={port.portType} className="text-text-muted/50" />}
                  cells={[
                    { text: String(idx + 1), flex: "w-[28px] shrink-0", mono: true },
                    { text: port.name, flex: "flex-1", mono: true },
                    { text: port.portType, flex: "w-[90px] shrink-0 text-right" },
                  ]}
                />
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

// --- Main NavigatorPanel ---

export function NavigatorPanel() {
  const data = useSchematicStore((s) => s.data);
  const [search, setSearch] = useState("");

  if (!data) {
    return (
      <div className="p-4 text-xs text-text-muted/50">
        No document loaded
      </div>
    );
  }

  return (
    <div className="text-xs overflow-y-auto h-full flex flex-col">
      {/* Search bar */}
      <div className="px-2 py-1 border-b border-border-subtle bg-bg-surface/80 shrink-0 flex items-center gap-1.5">
        <Search size={10} className="text-text-muted/40 shrink-0" />
        <input
          type="text"
          value={search}
          onChange={e => setSearch(e.target.value)}
          placeholder="Search..."
          className="flex-1 bg-transparent text-[10px] text-text-primary placeholder:text-text-muted/30 outline-none"
        />
        {search && (
          <button onClick={() => setSearch("")} className="text-text-muted/40 hover:text-text-primary text-[10px]">&times;</button>
        )}
      </div>
      <DocumentsSection />
      <InstanceSection filter={search} />
      <NetBusSection filter={search} />
      <PortsSection filter={search} />
      <div className="flex-1" />
    </div>
  );
}
