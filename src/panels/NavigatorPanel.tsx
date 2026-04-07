import { useSchematicStore } from "@/stores/schematic";
import { useProjectStore } from "@/stores/project";
import { ChevronDown, ChevronRight, FileText } from "lucide-react";
import { useState, useMemo } from "react";
import { cn } from "@/lib/utils";

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

// --- Table row ---

function TableRow({
  cells,
  selected,
  onClick,
}: {
  cells: { text: string; flex?: string; mono?: boolean }[];
  selected?: boolean;
  onClick?: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "flex items-center w-full px-2 py-[1px] text-[10px] transition-colors text-left",
        selected
          ? "bg-accent/15 text-accent"
          : "text-text-muted hover:bg-bg-hover/50 hover:text-text-primary"
      )}
    >
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

  return (
    <div>
      <SectionHeader
        title="Documents"
        open={open}
        onToggle={() => setOpen(!open)}
        count={sheets.length || undefined}
      />
      {open && (
        <div>
          {/* Hierarchy label */}
          <div className="px-2 py-0.5 text-[10px] text-text-muted/50 font-semibold select-none">
            Flattened Hierarchy
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
                    "flex items-center gap-1.5 w-full px-3 py-[2px] text-[10px] transition-colors",
                    isActive
                      ? "bg-accent/15 text-accent"
                      : "text-text-muted hover:bg-bg-hover/50 hover:text-text-primary"
                  )}
                >
                  <FileText size={10} className="shrink-0 text-text-muted/50" />
                  <span className="font-mono text-text-muted/60">[{idx + 1}]</span>
                  <span className="truncate">
                    {sheet.name}
                  </span>
                  <span className="ml-auto text-text-muted/40 truncate text-[9px]">
                    {sheet.filename}
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

function InstanceSection() {
  const [open, setOpen] = useState(true);
  const data = useSchematicStore((s) => s.data);
  const selectedIds = useSchematicStore((s) => s.selectedIds);
  const select = useSchematicStore((s) => s.select);

  const components = useMemo(
    () =>
      (data?.symbols ?? [])
        .filter((s) => !s.is_power)
        .sort((a, b) => a.reference.localeCompare(b.reference, undefined, { numeric: true })),
    [data?.symbols]
  );

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
              { label: "Instance", flex: "w-[72px] shrink-0" },
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
                  onClick={() => select(sym.uuid)}
                  cells={[
                    { text: sym.reference, flex: "w-[72px] shrink-0", mono: true },
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

function NetBusSection() {
  const [open, setOpen] = useState(true);
  const data = useSchematicStore((s) => s.data);
  const selectedIds = useSchematicStore((s) => s.selectedIds);
  const select = useSchematicStore((s) => s.select);

  const nets = useMemo(() => {
    if (!data) return [];

    // Collect unique net names from labels, dedup, and determine scope
    const netMap = new Map<string, { uuid: string; text: string; labelType: string }>();
    for (const label of data.labels) {
      if (label.label_type === "Net" || label.label_type === "Global" || label.label_type === "Power") {
        if (!netMap.has(label.text)) {
          netMap.set(label.text, {
            uuid: label.uuid,
            text: label.text,
            labelType: label.label_type,
          });
        }
      }
    }

    return Array.from(netMap.values()).sort((a, b) =>
      a.text.localeCompare(b.text, undefined, { numeric: true })
    );
  }, [data?.labels]);

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
              {nets.map((net) => (
                <TableRow
                  key={net.uuid}
                  selected={selectedIds.has(net.uuid)}
                  onClick={() => select(net.uuid)}
                  cells={[
                    { text: net.text, flex: "flex-1", mono: true },
                    { text: getScopeLabel(net.labelType), flex: "w-[110px] shrink-0 text-right" },
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

// --- Ports section ---

function PortsSection() {
  const [open, setOpen] = useState(true);
  const data = useSchematicStore((s) => s.data);
  const selectedIds = useSchematicStore((s) => s.selectedIds);
  const select = useSchematicStore((s) => s.select);

  const ports = useMemo(() => {
    if (!data) return [];

    const items: { uuid: string; name: string; portType: string }[] = [];

    // Hierarchical labels act as ports (sheet interface)
    for (const label of data.labels) {
      if (label.label_type === "Hierarchical") {
        items.push({
          uuid: label.uuid,
          name: label.text,
          portType: "Hierarchical",
        });
      }
    }

    // Sheet pins are also ports
    for (const sheet of data.child_sheets) {
      for (const pin of sheet.pins) {
        items.push({
          uuid: pin.uuid,
          name: pin.name,
          portType: pin.direction || "Bidirectional",
        });
      }
    }

    return items.sort((a, b) => a.name.localeCompare(b.name, undefined, { numeric: true }));
  }, [data?.labels, data?.child_sheets]);

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
              { label: "Port", flex: "w-[24px] shrink-0" },
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
                  onClick={() => select(port.uuid)}
                  cells={[
                    { text: String(idx + 1), flex: "w-[24px] shrink-0", mono: true },
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

  if (!data) {
    return (
      <div className="p-4 text-xs text-text-muted/50">
        No document loaded
      </div>
    );
  }

  return (
    <div className="text-xs overflow-y-auto h-full flex flex-col">
      <DocumentsSection />
      <InstanceSection />
      <NetBusSection />
      <PortsSection />
      {/* Fill remaining space */}
      <div className="flex-1" />
    </div>
  );
}
