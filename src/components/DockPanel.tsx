import { useState, useCallback, useRef, useEffect } from "react";
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
import { createPortal } from "react-dom";

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

// Global drag state for cross-dock communication
let draggingPanel: { panelId: PanelId; fromDock: string } | null = null;
const dockElements = new Map<string, HTMLElement>();
// Global ghost state — updated by source, read by all
let globalGhost: { panelId: PanelId; x: number; y: number } | null = null;
const ghostListeners = new Set<() => void>();
function notifyGhostChange() { ghostListeners.forEach(fn => fn()); }

export function registerDockElement(dockId: string, el: HTMLElement | null) {
  if (el) dockElements.set(dockId, el);
  else dockElements.delete(dockId);
}

function findTargetDock(x: number, y: number): string | null {
  for (const [dockId, el] of dockElements) {
    const rect = el.getBoundingClientRect();
    if (x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom) {
      return dockId;
    }
  }
  return null;
}

export function DockPanel({ dockId, onCollapse }: DockPanelProps) {
  const tabs = useLayoutStore((s) => s.docks[dockId]);
  const activeTab = useLayoutStore((s) => s.activeTab[dockId]);
  const setDockActiveTab = useLayoutStore((s) => s.setDockActiveTab);
  const movePanel = useLayoutStore((s) => s.movePanel);
  const containerRef = useRef<HTMLDivElement>(null);
  const [dragOver, setDragOver] = useState(false);
  const [, forceUpdate] = useState(0);

  // Subscribe to global ghost changes
  useEffect(() => {
    const fn = () => forceUpdate(n => n + 1);
    ghostListeners.add(fn);
    return () => { ghostListeners.delete(fn); };
  }, []);

  // Register this dock element for hit testing
  useEffect(() => {
    registerDockElement(dockId, containerRef.current);
    return () => registerDockElement(dockId, null);
  }, [dockId]);

  // Listen for global drag events from other docks
  useEffect(() => {
    const onMove = (e: MouseEvent) => {
      if (!draggingPanel) return;
      globalGhost = { panelId: draggingPanel.panelId, x: e.clientX, y: e.clientY };
      notifyGhostChange();
      // Check if hovering over this dock
      if (containerRef.current) {
        const rect = containerRef.current.getBoundingClientRect();
        const over = e.clientX >= rect.left && e.clientX <= rect.right && e.clientY >= rect.top && e.clientY <= rect.bottom;
        setDragOver(over);
      }
    };

    const onUp = (e: MouseEvent) => {
      if (!draggingPanel) return;
      const target = findTargetDock(e.clientX, e.clientY);
      if (target) {
        movePanel(draggingPanel.panelId, target as "left" | "right" | "bottom");
      } else {
        // Dropped outside any dock — float the panel
        useLayoutStore.getState().floatPanel(draggingPanel.panelId, e.clientX - 100, e.clientY - 15);
      }
      draggingPanel = null;
      globalGhost = null;
      notifyGhostChange();
      setDragOver(false);
    };

    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    return () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
  }, [dockId, movePanel]);

  const startTabDrag = useCallback((e: React.MouseEvent, panelId: PanelId) => {
    // Only start drag on middle or if moved enough
    const startX = e.clientX, startY = e.clientY;
    let started = false;

    const onMove = (ev: MouseEvent) => {
      if (!started && Math.abs(ev.clientX - startX) + Math.abs(ev.clientY - startY) > 8) {
        started = true;
        draggingPanel = { panelId, fromDock: dockId };
        globalGhost = { panelId, x: ev.clientX, y: ev.clientY };
        notifyGhostChange();
      }
      if (started) {
        globalGhost = { panelId, x: ev.clientX, y: ev.clientY };
        notifyGhostChange();
      }
    };

    const onUp = () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
      if (!started) {
        // It was just a click, not a drag — switch tab
        setDockActiveTab(dockId, panelId);
      }
    };

    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
  }, [dockId, setDockActiveTab]);

  const CollapseIcon = COLLAPSE_ICONS[dockId];
  const ActiveComponent = activeTab ? PANEL_COMPONENTS[activeTab as PanelId] : null;

  return (
    <div ref={containerRef} className={cn("flex flex-col h-full overflow-hidden", dragOver && "ring-2 ring-accent ring-inset")}>
      {/* Tab bar */}
      <div className="flex items-center h-8 bg-bg-tertiary border-b border-border-subtle select-none">
        {tabs.map((panelId) => (
          <button
            key={panelId}
            onMouseDown={(e) => { if (e.button === 0) startTabDrag(e, panelId); }}
            className={cn(
              "h-full px-3 text-[10px] font-semibold uppercase tracking-wider transition-colors whitespace-nowrap cursor-default",
              activeTab === panelId
                ? "text-text-secondary border-b-2 border-accent"
                : "text-text-muted/40 hover:text-text-muted/70"
            )}
          >
            {getTabLabel(panelId)}
          </button>
        ))}
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

      {/* Floating ghost tab (rendered via portal, only from left dock to avoid duplicates) */}
      {dockId === "left" && globalGhost && createPortal(
        <div
          className="fixed z-[9999] pointer-events-none px-3 py-1 bg-accent/90 text-white text-[10px] font-semibold uppercase tracking-wider rounded shadow-lg"
          style={{ left: globalGhost.x - 30, top: globalGhost.y - 12 }}
        >
          {getTabLabel(globalGhost.panelId)}
        </div>,
        document.body
      )}
    </div>
  );
}
