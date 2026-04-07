import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { MenuBar } from "@/components/MenuBar";
import { ToolbarStrip } from "@/components/ToolbarStrip";
import { DocumentTabBar } from "@/components/DocumentTabBar";
import { StatusBar } from "@/components/StatusBar";
import { ProjectPanel } from "@/panels/ProjectPanel";
import { PropertiesPanel } from "@/panels/PropertiesPanel";
import { MessagesPanel } from "@/panels/MessagesPanel";
import { ComponentPanel } from "@/panels/ComponentPanel";
import { FilterPanel } from "@/panels/FilterPanel";
import { ListPanel } from "@/panels/ListPanel";
import { NavigatorPanel } from "@/panels/NavigatorPanel";
import { ComponentSearch } from "@/components/ComponentSearch";
import { SignalPanel } from "@/panels/SignalPanel";
import { EditorCanvas } from "@/canvas/EditorCanvas";
import { LibraryEditorCanvas } from "@/canvas/LibraryEditorCanvas";
import { PcbRenderer } from "@/canvas/PcbRenderer";
import { PcbToolbar } from "@/components/PcbToolbar";
import { ExportPdfDialog } from "@/components/ExportPdfDialog";
import { BomConfigDialog } from "@/components/BomConfigDialog";
import { NetlistExportDialog } from "@/components/NetlistExportDialog";
import { LibraryEditorToolbar } from "@/components/LibraryEditorToolbar";
import { OutputJobsPanel } from "@/panels/OutputJobsPanel";
import { AnnotationDialog } from "@/components/AnnotationDialog";
import { PreferencesDialog } from "@/components/PreferencesDialog";
import { FindSimilarDialog } from "@/components/FindSimilarDialog";
import { ParameterManager } from "@/components/ParameterManager";
import { useLayoutStore } from "@/stores/layout";
import { useEditorStore } from "@/stores/editor";
import { useProjectStore } from "@/stores/project";
import { useSchematicStore } from "@/stores/schematic";
import { useLibraryEditorStore } from "@/stores/libraryEditor";
import { useResizable } from "@/hooks/useResizable";
import { printSchematic } from "@/lib/pdfExport";
import { cn } from "@/lib/utils";
import {
  PanelLeftClose,
  PanelRightClose,
  PanelBottomClose,
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

function App() {
  const [componentSearchOpen, setComponentSearchOpen] = useState(false);
  const [showPdfExport, setShowPdfExport] = useState(false);
  const [showBomConfig, setShowBomConfig] = useState(false);
  const [showNetlistExport, setShowNetlistExport] = useState(false);
  const [showAnnotation, setShowAnnotation] = useState(false);
  const [showPreferences, setShowPreferences] = useState(false);
  const [showFindSimilar, setShowFindSimilar] = useState(false);
  const [showParamManager, setShowParamManager] = useState(false);
  const [leftTab, setLeftTab] = useState<"projects" | "components" | "navigator">("projects");
  const [rightTab, setRightTab] = useState<"properties" | "filter" | "list">("properties");
  const [bottomTab, setBottomTab] = useState<"messages" | "output-jobs" | "signal">("messages");
  const libEditorActive = useLibraryEditorStore((s) => s.active);
  const editorMode = useEditorStore((s) => s.mode);

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
        setLeftTab("components");
        const layout = useLayoutStore.getState();
        if (layout.leftCollapsed) layout.toggleLeft();
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
        setBottomTab("signal");
        if (useLayoutStore.getState().bottomCollapsed) useLayoutStore.getState().toggleBottom();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  return (
    <div className="flex flex-col h-screen w-screen overflow-hidden bg-bg-primary text-text-primary">
      <MenuBar
        onOpenProject={openProjectFlow}
        onSave={saveSchematicFlow}
        onOpenComponentSearch={() => { setLeftTab("components"); if (useLayoutStore.getState().leftCollapsed) useLayoutStore.getState().toggleLeft(); }}
        onExportPdf={() => setShowPdfExport(true)}
        onExportBom={() => setShowBomConfig(true)}
        onExportNetlist={() => setShowNetlistExport(true)}
        onOpenOutputJobs={() => { setBottomTab("output-jobs"); if (useLayoutStore.getState().bottomCollapsed) useLayoutStore.getState().toggleBottom(); }}
        onAnnotate={() => setShowAnnotation(true)}
        onPreferences={() => setShowPreferences(true)}
        onFindSimilar={() => setShowFindSimilar(true)}
        onParameterManager={() => setShowParamManager(true)}
        onPrint={() => {
          const data = useSchematicStore.getState().data;
          if (data) printSchematic(data);
        }}
      />
      {libEditorActive ? <LibraryEditorToolbar /> : editorMode === "pcb" ? <PcbToolbar /> : <ToolbarStrip />}
      <DocumentTabBar />

      <div className="flex flex-1 min-h-0">
        {leftCollapsed ? (
          <CollapsedRail label="Explorer" icon={<FolderOpen size={15} />} onClick={toggleLeft} side="left" />
        ) : (
          <>
            <div className="flex flex-col bg-bg-secondary overflow-hidden shrink-0" style={{ width: leftPanelWidth }}>
              {/* Tab bar */}
              <div className="flex items-center h-8 bg-bg-tertiary border-b border-border-subtle select-none">
                {(["projects", "components", "navigator"] as const).map(t => (
                  <button key={t}
                    className={cn("flex-1 h-full text-[10px] font-semibold uppercase tracking-wider transition-colors",
                      leftTab === t ? "text-text-secondary border-b-2 border-accent" : "text-text-muted/40 hover:text-text-muted/70")}
                    onClick={() => setLeftTab(t)}>
                    {t === "navigator" ? "Nav" : t.charAt(0).toUpperCase() + t.slice(1)}
                  </button>
                ))}
                <button onClick={toggleLeft}
                  className="p-1 mx-1 rounded hover:bg-bg-hover text-text-muted/40 hover:text-text-secondary transition-colors">
                  <PanelLeftClose size={14} />
                </button>
              </div>
              <div className="flex-1 overflow-hidden overflow-y-auto">
                {leftTab === "projects" && <ProjectPanel />}
                {leftTab === "components" && <ComponentPanel />}
                {leftTab === "navigator" && <NavigatorPanel />}
              </div>
            </div>
            <ResizeHandle direction="horizontal" onMouseDown={(e) => leftResize.onMouseDown(e, leftPanelWidth)} />
          </>
        )}

        <div className="flex flex-col flex-1 min-w-0">
          <div className="flex-1 min-h-0">
            {libEditorActive ? <LibraryEditorCanvas /> : editorMode === "pcb" ? <PcbRenderer /> : <EditorCanvas onOpenProject={openProjectFlow} />}
          </div>

          {!bottomCollapsed ? (
            <>
              <ResizeHandle direction="vertical" onMouseDown={(e) => bottomResize.onMouseDown(e, bottomPanelHeight)} />
              <div className="bg-bg-secondary shrink-0" style={{ height: bottomPanelHeight }}>
                <div className="flex items-center h-8 bg-bg-tertiary border-b border-border-subtle select-none">
                  {(["messages", "output-jobs", "signal"] as const).map(t => (
                    <button key={t}
                      className={cn("flex-1 h-full text-[10px] font-semibold uppercase tracking-wider transition-colors",
                        bottomTab === t ? "text-text-secondary border-b-2 border-accent" : "text-text-muted/40 hover:text-text-muted/70")}
                      onClick={() => setBottomTab(t)}>
                      {t === "messages" ? "Messages" : t === "output-jobs" ? "Output Jobs" : "Signal"}
                    </button>
                  ))}
                  <button onClick={toggleBottom}
                    className="p-1 mx-1 rounded hover:bg-bg-hover text-text-muted/40 hover:text-text-secondary transition-colors">
                    <PanelBottomClose size={14} />
                  </button>
                </div>
                <div className="overflow-y-auto" style={{ height: bottomPanelHeight - 32 }}>
                  {bottomTab === "messages" && <MessagesPanel />}
                  {bottomTab === "output-jobs" && <OutputJobsPanel />}
                  {bottomTab === "signal" && <SignalPanel />}
                </div>
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
              <div className="flex items-center h-8 bg-bg-tertiary border-b border-border-subtle select-none">
                {(["properties", "filter", "list"] as const).map(t => (
                  <button key={t}
                    className={cn("flex-1 h-full text-[10px] font-semibold uppercase tracking-wider transition-colors",
                      rightTab === t ? "text-text-secondary border-b-2 border-accent" : "text-text-muted/40 hover:text-text-muted/70")}
                    onClick={() => setRightTab(t)}>
                    {t === "properties" ? "Props" : t === "filter" ? "Filter" : "List"}
                  </button>
                ))}
                <button onClick={toggleRight}
                  className="p-1 mx-1 rounded hover:bg-bg-hover text-text-muted/40 hover:text-text-secondary transition-colors">
                  <PanelRightClose size={14} />
                </button>
              </div>
              <div className="flex-1 overflow-y-auto">
                {rightTab === "properties" && <PropertiesPanel />}
                {rightTab === "filter" && <FilterPanel />}
                {rightTab === "list" && <ListPanel />}
              </div>
            </div>
          </>
        )}
      </div>

      <StatusBar />
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
