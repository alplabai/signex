import { useState, useCallback, useRef, useEffect } from "react";
import { useLayoutStore } from "@/stores/layout";
import { useEditorStore } from "@/stores/editor";
import { PANEL_DEFS, PANEL_COMPONENTS } from "@/lib/panelRegistry";
import type { PanelId } from "@/lib/panelRegistry";
import { PanelLeftClose, PanelRightClose, PanelBottomClose } from "lucide-react";
import { cn } from "@/lib/utils";
import { createPortal } from "react-dom";

interface DockPanelProps {
  dockId: "left" | "right" | "bottom";
  onCollapse: () => void;
}

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

// Global drag state for cross-dock communication.
// NOTE(HMR): These module-level mutable globals are intentionally shared across
// all DockPanel instances. In development, Vite HMR re-executes this module,
// resetting them — any in-flight drag will be cancelled. Acceptable for production.
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
  const allTabs = useLayoutStore((s) => s.docks[dockId]);
  const editorMode = useEditorStore((s) => s.mode) as "schematic" | "pcb";
  // Filter tabs by editor context — hide PCB-only panels in schematic mode and vice versa
  const tabs = allTabs.filter(panelId => {
    const def = PANEL_DEFS.find(d => d.id === panelId);
    if (!def) return true;
    return def.context === "both" || def.context === editorMode;
  });
  const rawActiveTab = useLayoutStore((s) => s.activeTab[dockId]);
  const activeTab = tabs.includes(rawActiveTab) ? rawActiveTab : tabs[0];
  const setDockActiveTab = useLayoutStore((s) => s.setDockActiveTab);
  const movePanel = useLayoutStore((s) => s.movePanel);
  const containerRef = useRef<HTMLDivElement>(null);
  const [, setDragOver] = useState(false);
  const dragOverRef = useRef(false);
  const [, forceUpdate] = useState(0);

  // Subscribe to global ghost changes + clear drag highlight when drag ends
  useEffect(() => {
    const fn = () => {
      forceUpdate(n => n + 1);
      // Clear drag highlight when no panel is being dragged.
      // Use dragOverRef instead of dragOver to avoid stale closure with [] deps.
      if (!globalGhost && dragOverRef.current) {
        setDragOver(false);
        dragOverRef.current = false;
      }
    };
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
        dragOverRef.current = over;
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
      dragOverRef.current = false;
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
    <div ref={containerRef} className="flex flex-col h-full overflow-hidden">
      {/* Tab bar */}
      <div className="flex items-center h-8 bg-bg-tertiary border-b border-border-subtle select-none">
        {tabs.map((panelId) => (
          <button
            key={panelId}
            onMouseDown={(e) => { if (e.button === 0) startTabDrag(e, panelId); }}
            className={cn(
              "h-full px-3 text-[10px] font-semibold uppercase tracking-wider transition-colors whitespace-nowrap cursor-default outline-none",
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
