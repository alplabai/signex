import { X, FileText, Cpu, Library, PackageOpen } from "lucide-react";
import { cn } from "@/lib/utils";
import { useProjectStore } from "@/stores/project";
import type { DocumentType } from "@/types";

const typeIcons: Record<DocumentType, React.ReactNode> = {
  schematic: <FileText size={13} />,
  pcb: <Cpu size={13} />,
  library: <Library size={13} />,
  "output-job": <PackageOpen size={13} />,
  "3d-view": <Cpu size={13} />,
};

export function DocumentTabBar() {
  const { openTabs, activeTabId, setActiveTab, closeTab } = useProjectStore();

  if (openTabs.length === 0) return null;

  return (
    <div className="flex items-center h-8 bg-bg-tertiary border-b border-border overflow-x-auto">
      {openTabs.map((tab) => (
        <div
          key={tab.id}
          className={cn(
            "flex items-center gap-1.5 px-3 h-full border-r border-border cursor-pointer text-xs",
            "hover:bg-bg-hover transition-colors min-w-0 shrink-0",
            activeTabId === tab.id
              ? "bg-bg-primary text-text-primary border-b-2 border-b-accent"
              : "text-text-secondary"
          )}
          onClick={() => setActiveTab(tab.id)}
        >
          <span className="text-text-muted">{typeIcons[tab.type]}</span>
          <span className="truncate max-w-[120px]">{tab.name}</span>
          {tab.dirty && <span className="text-accent">*</span>}
          <button
            className="p-0.5 rounded hover:bg-bg-hover ml-1 opacity-50 hover:opacity-100"
            onClick={(e) => {
              e.stopPropagation();
              closeTab(tab.id);
            }}
          >
            <X size={12} />
          </button>
        </div>
      ))}
    </div>
  );
}
