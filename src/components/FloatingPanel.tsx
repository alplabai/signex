import { useRef, useCallback } from "react";
import { useLayoutStore } from "@/stores/layout";
import type { PanelId } from "@/lib/panelRegistry";
import { PANEL_DEFS } from "@/lib/panelRegistry";
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
import { X, Minimize2 } from "lucide-react";
import { createPortal } from "react-dom";

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

function getTitle(panelId: string): string {
  return PANEL_DEFS.find((d) => d.id === panelId)?.title || panelId;
}

interface FloatingPanelProps {
  panelId: string;
  x: number;
  y: number;
  width: number;
  height: number;
}

export function FloatingPanel({ panelId, x, y, width, height }: FloatingPanelProps) {
  const updatePos = useLayoutStore((s) => s.updateFloatingPosition);
  const closePanel = useLayoutStore((s) => s.closeFloatingPanel);
  const dockPanel = useLayoutStore((s) => s.dockFloatingPanel);
  const dragging = useRef(false);
  const offset = useRef({ x: 0, y: 0 });

  const onTitleMouseDown = useCallback((e: React.MouseEvent) => {
    if (e.button !== 0) return;
    dragging.current = true;
    offset.current = { x: e.clientX - x, y: e.clientY - y };

    const onMove = (ev: MouseEvent) => {
      if (!dragging.current) return;
      updatePos(panelId, ev.clientX - offset.current.x, ev.clientY - offset.current.y);
    };
    const onUp = () => {
      dragging.current = false;
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
  }, [panelId, x, y, updatePos, dockPanel]);

  const Component = PANEL_COMPONENTS[panelId as PanelId];

  return createPortal(
    <div
      className="fixed z-[100] bg-bg-secondary border border-border rounded-lg shadow-2xl shadow-black/60 flex flex-col overflow-hidden"
      style={{ left: x, top: y, width, height }}
    >
      {/* Title bar */}
      <div
        className="flex items-center h-7 bg-bg-tertiary border-b border-border-subtle px-2 select-none cursor-move shrink-0"
        onMouseDown={onTitleMouseDown}
      >
        <span className="text-[10px] font-semibold uppercase tracking-wider text-text-secondary flex-1">
          {getTitle(panelId)}
        </span>
        <button
          onClick={() => {
            const def = PANEL_DEFS.find(d => d.id === panelId);
            dockPanel(panelId, def?.defaultDock || "right");
          }}
          title="Dock panel"
          className="p-0.5 rounded hover:bg-bg-hover text-text-muted/40 hover:text-text-secondary transition-colors"
        >
          <Minimize2 size={11} />
        </button>
        <button
          onClick={() => closePanel(panelId)}
          title="Close"
          className="p-0.5 rounded hover:bg-error/20 text-text-muted/40 hover:text-error transition-colors ml-0.5"
        >
          <X size={11} />
        </button>
      </div>
      {/* Content */}
      <div className="flex-1 overflow-y-auto">
        {Component && <Component />}
      </div>
    </div>,
    document.body
  );
}
