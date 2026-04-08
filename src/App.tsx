import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { MenuBar } from "@/components/MenuBar";
import { ToolbarStrip } from "@/components/ToolbarStrip";
import { DocumentTabBar } from "@/components/DocumentTabBar";
import { StatusBar } from "@/components/StatusBar";
import { DockPanel } from "@/components/DockPanel";
import { FloatingPanel } from "@/components/FloatingPanel";
import { ComponentSearch } from "@/components/ComponentSearch";
import { EditorCanvas } from "@/canvas/EditorCanvas";
import { LibraryEditorCanvas } from "@/canvas/LibraryEditorCanvas";
import { PcbRenderer } from "@/canvas/PcbRenderer";
import { PcbToolbar } from "@/components/PcbToolbar";
import { ExportPdfDialog } from "@/components/ExportPdfDialog";
import { BomConfigDialog } from "@/components/BomConfigDialog";
import { NetlistExportDialog } from "@/components/NetlistExportDialog";
import { FootprintEditorCanvas } from "@/canvas/FootprintEditorCanvas";
import { useFootprintEditorStore } from "@/stores/footprintEditor";
import { AnnotationDialog } from "@/components/AnnotationDialog";
import { PreferencesDialog } from "@/components/PreferencesDialog";
import { FindSimilarDialog } from "@/components/FindSimilarDialog";
import { ParameterManager } from "@/components/ParameterManager";
import { useLayoutStore } from "@/stores/layout";
import { useProjectStore } from "@/stores/project";
import { useSchematicStore } from "@/stores/schematic";
import { useEditorStore } from "@/stores/editor";
import { useLibraryEditorStore } from "@/stores/libraryEditor";
import { useThemeStore } from "@/stores/theme";
import { useResizable } from "@/hooks/useResizable";
import { printSchematic } from "@/lib/pdfExport";
import { cn } from "@/lib/utils";
import {
  FolderOpen,
  Settings,
  MessageSquare,
  ChevronDown,
} from "lucide-react";
import type { AppInfo, ProjectInfo } from "@/types";

// Stable reference — defined outside component to avoid re-render issues
async function openProjectFlow() {
  const { setProject, openTab, addRecentProject } = useProjectStore.getState();
  try {
    const info = await invoke<ProjectInfo | null>("pick_and_open_project");
    if (!info) return;
    setProject(info);
    addRecentProject(info.path);
    const rootSheet = info.sheets[0];
    if (rootSheet) {
      openTab({
        id: `sch-${info.path}:${rootSheet.filename}`,
        name: rootSheet.name,
        type: "schematic",
        path: info.path,
        dirty: false,
      });
    }
  } catch (err) {
    alert(`Failed to open project: ${err}`);
  }
}

async function saveSchematicFlow() {
  const project = useProjectStore.getState().project;
  const { data } = useSchematicStore.getState();
  if (!project || !data) return;

  const activeTabId = useProjectStore.getState().activeTabId;
  const sheet = project.sheets.find(
    (s) => `sch-${project.path}:${s.filename}` === activeTabId
  );
  const filename = sheet?.filename || project.schematic_root;
  if (!filename) return;

  try {
    await invoke("save_schematic", {
      projectDir: project.dir,
      filename,
      data,
    });
    useSchematicStore.setState({ dirty: false });
  } catch (err) {
    alert(`Failed to save: ${err}`);
  }
}

function CollapsedRail({
  label,
  icon,
  onClick,
  side,
}: {
  label: string;
  icon: React.ReactNode;
  onClick: () => void;
  side: "left" | "right";
}) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "flex items-center justify-center shrink-0 bg-bg-secondary hover:bg-bg-hover transition-colors group cursor-pointer",
        side === "left" ? "border-r border-border-subtle" : "border-l border-border-subtle"
      )}
      style={{ width: 28 }}
    >
      <div className="flex flex-col items-center gap-2 py-3">
        <span className="text-text-muted/50 group-hover:text-accent transition-colors">{icon}</span>
        <span
          className="text-[10px] font-semibold text-text-muted/40 group-hover:text-text-secondary uppercase tracking-widest transition-colors"
          style={{ writingMode: "vertical-lr", textOrientation: "mixed" }}
        >
          {label}
        </span>
      </div>
    </button>
  );
}

function CollapsedBottomBar({
  label,
  icon,
  onClick,
}: {
  label: string;
  icon: React.ReactNode;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className="flex items-center gap-1.5 h-6 px-3 bg-bg-secondary border-t border-border-subtle hover:bg-bg-hover transition-colors group cursor-pointer shrink-0"
    >
      <span className="text-text-muted/50 group-hover:text-accent transition-colors">{icon}</span>
      <span className="text-[10px] font-semibold text-text-muted/40 group-hover:text-text-secondary uppercase tracking-wider transition-colors">
        {label}
      </span>
      <ChevronDown size={10} className="text-text-muted/30 group-hover:text-text-secondary rotate-180 transition-colors" />
    </button>
  );
}

function ResizeHandle({
  direction,
  onMouseDown,
}: {
  direction: "horizontal" | "vertical";
  onMouseDown: (e: React.MouseEvent) => void;
}) {
  return (
    <div
      onMouseDown={onMouseDown}
      className={cn(
        "group flex items-center justify-center shrink-0 z-10",
        direction === "horizontal"
          ? "w-[5px] cursor-col-resize hover:bg-accent/30 active:bg-accent/50"
          : "h-[5px] cursor-row-resize hover:bg-accent/30 active:bg-accent/50"
      )}
    >
      <div
        className={cn(
          "bg-transparent group-hover:bg-accent/60 group-active:bg-accent transition-colors rounded-full",
          direction === "horizontal" ? "w-[2px] h-8" : "h-[2px] w-8"
        )}
      />
    </div>
  );
}

function FloatingPanelsRenderer() {
  const floatingPanels = useLayoutStore((s) => s.floatingPanels);
  return (
    <>
      {Object.entries(floatingPanels).map(([panelId, state]) => (
        <FloatingPanel key={panelId} panelId={panelId} x={state.x} y={state.y} width={state.width} height={state.height} />
      ))}
    </>
  );
}

function App() {
  const [componentSearchOpen, setComponentSearchOpen] = useState(false);
  const [showPdfExport, setShowPdfExport] = useState(false);
  const [showBomConfig, setShowBomConfig] = useState(false);
  const [showNetlistExport, setShowNetlistExport] = useState(false);
  const [showAnnotation, setShowAnnotation] = useState(false);
  const [showPreferences, setShowPreferences] = useState(false);
  const [showFindSimilar, setShowFindSimilar] = useState(false);
  const [showParamManager, setShowParamManager] = useState(false);
  const setDockActiveTab = useLayoutStore((s) => s.setDockActiveTab);
  const libEditorActive = useLibraryEditorStore((s) => s.active);
  const fpEditorActive = useFootprintEditorStore((s) => s.active);
  const activeTabId = useProjectStore((s) => s.activeTabId);
  const openTabs = useProjectStore((s) => s.openTabs);
  const activeTabType = activeTabId ? openTabs.find((t) => t.id === activeTabId)?.type : undefined;
  const isPcbView = activeTabType === "pcb";
  const isLibraryView = activeTabType === "library" || libEditorActive;
  const isFpLibraryView = fpEditorActive;

  const leftCollapsed = useLayoutStore((s) => s.leftCollapsed);
  const rightCollapsed = useLayoutStore((s) => s.rightCollapsed);
  const bottomCollapsed = useLayoutStore((s) => s.bottomCollapsed);
  const leftPanelWidth = useLayoutStore((s) => s.leftPanelWidth);
  const rightPanelWidth = useLayoutStore((s) => s.rightPanelWidth);
  const bottomPanelHeight = useLayoutStore((s) => s.bottomPanelHeight);
  const setLeftWidth = useLayoutStore((s) => s.setLeftWidth);
  const setRightWidth = useLayoutStore((s) => s.setRightWidth);
  const setBottomHeight = useLayoutStore((s) => s.setBottomHeight);
  const toggleLeft = useLayoutStore((s) => s.toggleLeft);
  const toggleRight = useLayoutStore((s) => s.toggleRight);
  const toggleBottom = useLayoutStore((s) => s.toggleBottom);

  const leftResize = useResizable({ direction: "horizontal", onResize: setLeftWidth, min: 180, max: 500 });
  const rightResize = useResizable({ direction: "horizontal", onResize: setRightWidth, min: 200, max: 500, reverse: true });
  const bottomResize = useResizable({ direction: "vertical", onResize: setBottomHeight, min: 100, max: 400, reverse: true });

  useEffect(() => {
    useThemeStore.getState().applyActiveTheme();
  }, []);

  useEffect(() => {
    invoke<AppInfo>("get_app_info")
      .then((info) => { document.title = `${info.name} v${info.version}`; })
      .catch(() => {});
  }, []);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.key === "o") { e.preventDefault(); openProjectFlow(); }
      if (e.ctrlKey && e.key === "s") { e.preventDefault(); saveSchematicFlow(); }
      if (e.ctrlKey && e.key === "p") {
        e.preventDefault();
        const data = useSchematicStore.getState().data;
        if (data) printSchematic(data);
      }
      if (e.ctrlKey && e.key === "c") {
        if (!(e.target instanceof HTMLInputElement) && !(e.target instanceof HTMLTextAreaElement)) {
          e.preventDefault(); useSchematicStore.getState().copySelected();
        }
      }
      if (e.ctrlKey && e.key === "x") {
        if (!(e.target instanceof HTMLInputElement) && !(e.target instanceof HTMLTextAreaElement)) {
          e.preventDefault();
          useSchematicStore.getState().copySelected();
          useSchematicStore.getState().deleteSelected();
        }
      }
      if (e.ctrlKey && e.key === "v") {
        if (!(e.target instanceof HTMLInputElement) && !(e.target instanceof HTMLTextAreaElement)) {
          e.preventDefault(); useSchematicStore.getState().pasteClipboard({ x: 5, y: 5 });
        }
      }
      // P key (place component) — switch to components tab
      if (e.key === "p" && !e.ctrlKey && !e.altKey && !e.metaKey &&
          !(e.target instanceof HTMLInputElement) && !(e.target instanceof HTMLTextAreaElement)) {
        useLayoutStore.getState().setDockActiveTab("left", "components");
        const layout = useLayoutStore.getState();
        if (layout.leftCollapsed) layout.toggleLeft();
      }
      // Tab — Pause placement and open Properties panel (Altium behavior)
      if (e.key === "Tab" &&
          !(e.target instanceof HTMLInputElement) && !(e.target instanceof HTMLTextAreaElement)) {
        e.preventDefault();
        const editor = useEditorStore.getState();
        const schematic = useSchematicStore.getState();
        const layout = useLayoutStore.getState();

        if (schematic.editMode !== "select" || schematic.placingSymbol) {
          // Toggle placement pause
          const newPaused = !editor.placementPaused;
          editor.setPlacementPaused(newPaused);
          if (newPaused) {
            // Pause: open properties panel and focus it
            layout.setDockActiveTab("right", "properties");
            if (layout.rightCollapsed) layout.toggleRight();
          }
        } else if (schematic.selectedIds.size > 0) {
          // Object selected: just open properties
          layout.setDockActiveTab("right", "properties");
          if (layout.rightCollapsed) layout.toggleRight();
        }
      }
      // Shift+F — Find Similar Objects
      if (e.key === "F" && e.shiftKey && !e.ctrlKey &&
          !(e.target instanceof HTMLInputElement) && !(e.target instanceof HTMLTextAreaElement)) {
        e.preventDefault();
        setShowFindSimilar(true);
      }
      // Ctrl+Shift+A — Open Signal AI panel
      if (e.key === "A" && e.ctrlKey && e.shiftKey &&
          !(e.target instanceof HTMLInputElement) && !(e.target instanceof HTMLTextAreaElement)) {
        e.preventDefault();
        useLayoutStore.getState().setDockActiveTab("bottom", "signal");
        if (useLayoutStore.getState().bottomCollapsed) useLayoutStore.getState().toggleBottom();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  // Disable browser context menu globally (EDA apps use custom menus)
  useEffect(() => {
    const prevent = (e: MouseEvent) => e.preventDefault();
    document.addEventListener("contextmenu", prevent);
    return () => document.removeEventListener("contextmenu", prevent);
  }, []);

  return (
    <div className="flex flex-col h-screen w-screen overflow-hidden bg-bg-primary text-text-primary">
      <MenuBar
        onOpenProject={openProjectFlow}
        onSave={saveSchematicFlow}
        onOpenComponentSearch={() => { setDockActiveTab("left", "components"); if (useLayoutStore.getState().leftCollapsed) useLayoutStore.getState().toggleLeft(); }}
        onExportPdf={() => setShowPdfExport(true)}
        onExportBom={() => setShowBomConfig(true)}
        onExportNetlist={() => setShowNetlistExport(true)}
        onOpenOutputJobs={() => { setDockActiveTab("bottom", "output-jobs"); if (useLayoutStore.getState().bottomCollapsed) useLayoutStore.getState().toggleBottom(); }}
        onAnnotate={() => setShowAnnotation(true)}
        onPreferences={() => setShowPreferences(true)}
        onFindSimilar={() => setShowFindSimilar(true)}
        onParameterManager={() => setShowParamManager(true)}
        onPrint={() => {
          const data = useSchematicStore.getState().data;
          if (data) printSchematic(data);
        }}
      />
      {isLibraryView || isFpLibraryView ? null : isPcbView ? <PcbToolbar /> : <ToolbarStrip />}
      <DocumentTabBar />

      <div className="flex flex-1 min-h-0">
        {leftCollapsed ? (
          <CollapsedRail label="Explorer" icon={<FolderOpen size={15} />} onClick={toggleLeft} side="left" />
        ) : (
          <>
            <div className="flex flex-col bg-bg-secondary overflow-hidden shrink-0" style={{ width: leftPanelWidth }}>
              <DockPanel dockId="left" onCollapse={toggleLeft} />
            </div>
            <ResizeHandle direction="horizontal" onMouseDown={(e) => leftResize.onMouseDown(e, leftPanelWidth)} />
          </>
        )}

        <div className="flex flex-col flex-1 min-w-0">
          <div className="flex-1 min-h-0">
            {isLibraryView ? <LibraryEditorCanvas /> : isFpLibraryView ? <FootprintEditorCanvas /> : isPcbView ? <PcbRenderer /> : <EditorCanvas onOpenProject={openProjectFlow} />}
          </div>

          {!bottomCollapsed ? (
            <>
              <ResizeHandle direction="vertical" onMouseDown={(e) => bottomResize.onMouseDown(e, bottomPanelHeight)} />
              <div className="bg-bg-secondary shrink-0" style={{ height: bottomPanelHeight }}>
                <DockPanel dockId="bottom" onCollapse={toggleBottom} />
              </div>
            </>
          ) : (
            <CollapsedBottomBar label="Messages" icon={<MessageSquare size={12} />} onClick={toggleBottom} />
          )}
        </div>

        {rightCollapsed ? (
          <CollapsedRail label="Properties" icon={<Settings size={15} />} onClick={toggleRight} side="right" />
        ) : (
          <>
            <ResizeHandle direction="horizontal" onMouseDown={(e) => rightResize.onMouseDown(e, rightPanelWidth)} />
            <div className="flex flex-col bg-bg-secondary overflow-hidden shrink-0" style={{ width: rightPanelWidth }}>
              <DockPanel dockId="right" onCollapse={toggleRight} />
            </div>
          </>
        )}
      </div>

      <StatusBar />
      <FloatingPanelsRenderer />
      <ComponentSearch open={componentSearchOpen} onClose={() => setComponentSearchOpen(false)} />
      <ExportPdfDialog open={showPdfExport} onClose={() => setShowPdfExport(false)} />
      <BomConfigDialog open={showBomConfig} onClose={() => setShowBomConfig(false)} />
      <NetlistExportDialog open={showNetlistExport} onClose={() => setShowNetlistExport(false)} />
      <AnnotationDialog open={showAnnotation} onClose={() => setShowAnnotation(false)} />
      <PreferencesDialog open={showPreferences} onClose={() => setShowPreferences(false)} />
      <FindSimilarDialog open={showFindSimilar} onClose={() => setShowFindSimilar(false)} />
      <ParameterManager open={showParamManager} onClose={() => setShowParamManager(false)} />
    </div>
  );
}

export default App;
