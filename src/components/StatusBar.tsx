import { useState, useRef, useEffect, useCallback } from "react";
import { useEditorStore } from "@/stores/editor";
import { useLayoutStore } from "@/stores/layout";
import { getPanelsForContext } from "@/lib/panelRegistry";
import type { PanelId } from "@/lib/panelRegistry";
import { Crosshair, Grid3x3, Magnet, Layers, MousePointer2, Zap, PanelTop, Check } from "lucide-react";
import { cn } from "@/lib/utils";

// Unit conversion for display
function mmToDisplay(mm: number, unit: "mm" | "mil" | "inch"): string {
  if (unit === "mil") return (mm / 0.0254).toFixed(0);
  if (unit === "inch") return (mm / 25.4).toFixed(4);
  return mm.toFixed(2);
}

function StatusItem({
  icon,
  children,
  onClick,
  active,
  title,
}: {
  icon?: React.ReactNode;
  children: React.ReactNode;
  onClick?: () => void;
  active?: boolean;
  title?: string;
}) {
  return (
    <div
      onClick={onClick}
      title={title}
      className={cn(
        "flex items-center gap-1.5",
        onClick ? "cursor-pointer hover:text-text-primary transition-colors" : "",
        active ? "text-accent" : "text-text-muted",
      )}
    >
      {icon && <span className={active ? "text-accent" : "text-text-muted/60"}>{icon}</span>}
      <span>{children}</span>
    </div>
  );
}

function Divider() {
  return <div className="w-px h-3 bg-border-subtle" />;
}

export function StatusBar() {
  const cursorX = useEditorStore((s) => s.statusBar.cursorPosition.x);
  const cursorY = useEditorStore((s) => s.statusBar.cursorPosition.y);
  const gridSize = useEditorStore((s) => s.statusBar.gridSize);
  const gridVisible = useEditorStore((s) => s.gridVisible);
  const snapEnabled = useEditorStore((s) => s.statusBar.snapEnabled);
  const units = useEditorStore((s) => s.statusBar.units);
  const activeLayer = useEditorStore((s) => s.statusBar.activeLayer);
  const editorMode = useEditorStore((s) => s.mode);
  const currentMode = useEditorStore((s) => s.statusBar.currentMode);
  const zoom = useEditorStore((s) => s.statusBar.zoom);
  const toggleGrid = useEditorStore((s) => s.toggleGrid);
  const toggleSnap = useEditorStore((s) => s.toggleSnap);
  const cycleUnits = () => {
    const u = units;
    useEditorStore.getState().updateStatusBar({ units: u === "mm" ? "mil" : u === "mil" ? "inch" : "mm" });
  };

  return (
    <div className="flex items-center h-[22px] bg-bg-tertiary border-t border-border-subtle px-3 text-[11px] gap-3 select-none">
      {/* Cursor position with unit conversion */}
      <StatusItem icon={<Crosshair size={11} />} title="Cursor position">
        <span className="font-mono tabular-nums">
          X:{mmToDisplay(cursorX, units)} Y:{mmToDisplay(cursorY, units)}
        </span>
      </StatusItem>
      <Divider />

      {/* Grid with unit conversion */}
      <StatusItem icon={<Grid3x3 size={11} />} onClick={toggleGrid} active={gridVisible}
        title="Grid size (G to cycle, Shift+Ctrl+G to toggle)">
        <span className="font-mono">{mmToDisplay(gridSize, units)}{units}</span>
      </StatusItem>
      <Divider />

      {/* Snap */}
      <StatusItem icon={<Magnet size={11} />} onClick={toggleSnap} active={snapEnabled}
        title="Snap to grid">
        {snapEnabled ? "Snap" : "Free"}
      </StatusItem>
      <Divider />

      {/* Electrical snap */}
      <StatusItem icon={<Zap size={11} />} active={true}
        title="Electrical grid snap (Shift+E)">
        E-Snap
      </StatusItem>
      <Divider />

      {/* Layer — only show in PCB mode */}
      {editorMode === "pcb" && (
        <>
          <StatusItem icon={<Layers size={11} />}>
            {activeLayer}
          </StatusItem>
          <Divider />
        </>
      )}

      {/* Mode */}
      <StatusItem icon={<MousePointer2 size={11} />}>
        {currentMode}
      </StatusItem>

      <div className="flex-1" />

      {/* Zoom */}
      <span className="text-text-muted/60 font-mono tabular-nums">
        {zoom}%
      </span>
      <Divider />

      {/* Units (clickable to cycle) */}
      <button onClick={cycleUnits} title="Toggle units (Ctrl+Q)"
        className="text-text-muted/60 uppercase font-mono text-[10px] hover:text-accent transition-colors px-1">
        {units}
      </button>
      <Divider />

      {/* Panels button */}
      <PanelsButton />
    </div>
  );
}

function PanelsButton() {
  const [open, setOpen] = useState(false);
  const btnRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const docks = useLayoutStore((s) => s.docks);
  const movePanel = useLayoutStore((s) => s.movePanel);
  const removePanel = useLayoutStore((s) => s.removePanel);
  const editorMode = useEditorStore((s) => s.mode) as "schematic" | "pcb";

  // Build a set of currently visible panel IDs across all docks
  const visiblePanels = new Set<string>([
    ...docks.left,
    ...docks.right,
    ...docks.bottom,
  ]);

  const handleTogglePanel = useCallback(
    (panelId: PanelId, defaultDock: "left" | "right" | "bottom") => {
      if (visiblePanels.has(panelId)) {
        removePanel(panelId);
      } else {
        movePanel(panelId, defaultDock);
        // Ensure the dock is expanded
        const store = useLayoutStore.getState();
        if (defaultDock === "left" && store.leftCollapsed) store.toggleLeft();
        if (defaultDock === "right" && store.rightCollapsed) store.toggleRight();
        if (defaultDock === "bottom" && store.bottomCollapsed) store.toggleBottom();
      }
    },
    [visiblePanels, movePanel, removePanel],
  );

  // Close on outside click
  useEffect(() => {
    if (!open) return;
    const handleClick = (e: MouseEvent) => {
      if (
        menuRef.current &&
        !menuRef.current.contains(e.target as Node) &&
        btnRef.current &&
        !btnRef.current.contains(e.target as Node)
      ) {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, [open]);

  return (
    <div className="relative">
      <button
        ref={btnRef}
        onClick={() => setOpen((v) => !v)}
        title="Toggle panels"
        className={cn(
          "flex items-center gap-1 px-1.5 text-[10px] uppercase font-semibold tracking-wider transition-colors",
          open
            ? "text-accent"
            : "text-text-muted/60 hover:text-accent",
        )}
      >
        <PanelTop size={11} />
        Panels
      </button>

      {open && (
        <div
          ref={menuRef}
          className="absolute bottom-full right-0 mb-1 w-48 bg-bg-secondary border border-border-subtle rounded shadow-lg py-1 z-50"
        >
          {getPanelsForContext(editorMode).map((def) => {
            const isVisible = visiblePanels.has(def.id);
            return (
              <button
                key={def.id}
                className="flex items-center w-full px-3 py-1 text-[11px] text-left hover:bg-bg-hover transition-colors"
                onClick={() => handleTogglePanel(def.id, def.defaultDock)}
              >
                <span className="w-4 mr-2 flex-shrink-0">
                  {isVisible && <Check size={11} className="text-accent" />}
                </span>
                <span className={isVisible ? "text-text-secondary" : "text-text-muted"}>
                  {def.title}
                </span>
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
