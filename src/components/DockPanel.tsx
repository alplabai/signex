import { useState, useCallback, useRef } from "react";
import { useLayoutStore } from "@/stores/layout";
import { PANEL_DEFS } from "@/lib/panelRegistry";
import type { PanelId } from "@/lib/panelRegistry";
import { ProjectPanel } from "@/panels/ProjectPanel";
import { ComponentPanel } from "@/panels/ComponentPanel";
import { NavigatorPanel } from "@/panels/NavigatorPanel";
import { PropertiesPanel } from "@/panels/PropertiesPanel";
import { FilterPanel } from "@/panels/FilterPanel";
import { ListPanel } from "@/panels/ListPanel";
import { MessagesPanel } from "@/panels/MessagesPanel";
import { OutputJobsPanel } from "@/panels/OutputJobsPanel";
import { SignalPanel } from "@/panels/SignalPanel";
import { InspectorPanel } from "@/panels/InspectorPanel";
import { DrcPanel } from "@/panels/DrcPanel";
import { LayerStackPanel } from "@/panels/LayerStackPanel";
import { SnippetsPanel } from "@/panels/SnippetsPanel";
import { VariantPanel } from "@/panels/VariantPanel";
import { BoardCrossSectionPanel } from "@/panels/BoardCrossSectionPanel";
import { PanelLeftClose, PanelRightClose, PanelBottomClose } from "lucide-react";
import { cn } from "@/lib/utils";

interface DockPanelProps {
  dockId: "left" | "right" | "bottom";
  onCollapse: () => void;
}

const PANEL_COMPONENTS: Record<PanelId, React.FC> = {
  projects: ProjectPanel,
  components: ComponentPanel,
  navigator: NavigatorPanel,
  properties: PropertiesPanel,
  filter: FilterPanel,
  list: ListPanel,
  messages: MessagesPanel,
  "output-jobs": OutputJobsPanel,
  signal: SignalPanel,
  inspector: InspectorPanel,
  drc: DrcPanel,
  layerStack: LayerStackPanel,
  snippets: SnippetsPanel,
  variants: VariantPanel,
  boardCrossSection: BoardCrossSectionPanel,
};

function getTabLabel(panelId: PanelId): string {
  const def = PANEL_DEFS.find((d) => d.id === panelId);
  if (def) return def.title;
  // Fallback for panels not in PANEL_DEFS
  switch (panelId) {
    case "inspector": return "Inspector";
    case "drc": return "DRC";
    case "layerStack": return "Layers";
    case "snippets": return "Snippets";
    case "variants": return "Variants";
    case "boardCrossSection": return "Cross Section";
    default: return panelId;
  }
}

const COLLAPSE_ICONS = {
  left: PanelLeftClose,
  right: PanelRightClose,
  bottom: PanelBottomClose,
};

export function DockPanel({ dockId, onCollapse }: DockPanelProps) {
  const tabs = useLayoutStore((s) => s.docks[dockId]);
  const activeTab = useLayoutStore((s) => s.activeTab[dockId]);
  const setDockActiveTab = useLayoutStore((s) => s.setDockActiveTab);
  const movePanel = useLayoutStore((s) => s.movePanel);
  const [dropIndex, setDropIndex] = useState<number | null>(null);
  const tabBarRef = useRef<HTMLDivElement>(null);

  const CollapseIcon = COLLAPSE_ICONS[dockId];

  const handleDragStart = useCallback(
    (e: React.DragEvent, panelId: PanelId) => {
      e.dataTransfer.setData("text/plain", panelId);
      e.dataTransfer.effectAllowed = "move";
    },
    []
  );

  const handleDragOver = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      e.dataTransfer.dropEffect = "move";

      // Calculate drop index based on mouse position relative to tabs
      if (!tabBarRef.current) return;
      const tabButtons = tabBarRef.current.querySelectorAll("[data-tab-id]");
      let idx = tabs.length;
      for (let i = 0; i < tabButtons.length; i++) {
        const rect = tabButtons[i].getBoundingClientRect();
        const midX = rect.left + rect.width / 2;
        if (e.clientX < midX) {
          idx = i;
          break;
        }
      }
      setDropIndex(idx);
    },
    [tabs.length]
  );

  const handleDragLeave = useCallback(() => {
    setDropIndex(null);
  }, []);

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      const panelId = e.dataTransfer.getData("text/plain");
      if (panelId) {
        movePanel(panelId, dockId, dropIndex ?? undefined);
      }
      setDropIndex(null);
    },
    [dockId, dropIndex, movePanel]
  );

  const ActiveComponent = activeTab ? PANEL_COMPONENTS[activeTab as PanelId] : null;

  return (
    <div className="flex flex-col h-full overflow-hidden">
      {/* Tab bar */}
      <div
        ref={tabBarRef}
        className="flex items-center h-8 bg-bg-tertiary border-b border-border-subtle select-none"
        onDragOver={handleDragOver}
        onDragLeave={handleDragLeave}
        onDrop={handleDrop}
      >
        {tabs.map((panelId, i) => (
          <div key={panelId} className="flex items-center h-full relative">
            {/* Drop indicator before this tab */}
            {dropIndex === i && (
              <div className="absolute left-0 top-1 bottom-1 w-[2px] bg-accent z-10" />
            )}
            <button
              data-tab-id={panelId}
              draggable
              onDragStart={(e) => handleDragStart(e, panelId)}
              className={cn(
                "flex-1 h-full px-3 text-[10px] font-semibold uppercase tracking-wider transition-colors whitespace-nowrap",
                activeTab === panelId
                  ? "text-text-secondary border-b-2 border-accent"
                  : "text-text-muted/40 hover:text-text-muted/70"
              )}
              onClick={() => setDockActiveTab(dockId, panelId)}
            >
              {getTabLabel(panelId)}
            </button>
          </div>
        ))}
        {/* Drop indicator at end */}
        {dropIndex === tabs.length && (
          <div className="w-[2px] h-4 bg-accent self-center" />
        )}
        <div className="flex-1" />
        <button
          onClick={onCollapse}
          className="p-1 mx-1 rounded hover:bg-bg-hover text-text-muted/40 hover:text-text-secondary transition-colors"
        >
          <CollapseIcon size={14} />
        </button>
      </div>

      {/* Panel content */}
      <div className="flex-1 overflow-hidden overflow-y-auto">
        {ActiveComponent && <ActiveComponent />}
      </div>
    </div>
  );
}
