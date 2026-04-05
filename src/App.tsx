import { useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { MenuBar } from "@/components/MenuBar";
import { ToolbarStrip } from "@/components/ToolbarStrip";
import { DocumentTabBar } from "@/components/DocumentTabBar";
import { StatusBar } from "@/components/StatusBar";
import { ProjectPanel } from "@/panels/ProjectPanel";
import { PropertiesPanel } from "@/panels/PropertiesPanel";
import { MessagesPanel } from "@/panels/MessagesPanel";
import { EditorCanvas } from "@/canvas/EditorCanvas";
import { useLayoutStore } from "@/stores/layout";
import { useProjectStore } from "@/stores/project";
import { useResizable } from "@/hooks/useResizable";
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

function PanelHeader({
  title,
  onCollapse,
  collapseIcon,
}: {
  title: string;
  onCollapse: () => void;
  collapseIcon: React.ReactNode;
}) {
  return (
    <div className="flex items-center h-8 px-3 text-[11px] font-semibold text-text-muted uppercase tracking-widest bg-bg-tertiary border-b border-border-subtle select-none">
      <span className="flex-1">{title}</span>
      <button
        onClick={onCollapse}
        className="p-0.5 rounded hover:bg-bg-hover text-text-muted/40 hover:text-text-secondary transition-colors"
      >
        {collapseIcon}
      </button>
    </div>
  );
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
        <span className="text-text-muted/50 group-hover:text-accent transition-colors">
          {icon}
        </span>
        <span
          className="text-[10px] font-semibold text-text-muted/40 group-hover:text-text-secondary uppercase tracking-widest transition-colors"
          style={{
            writingMode: "vertical-lr",
            textOrientation: "mixed",
          }}
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
      <span className="text-text-muted/50 group-hover:text-accent transition-colors">
        {icon}
      </span>
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
  const {
    leftCollapsed,
    rightCollapsed,
    bottomCollapsed,
    leftPanelWidth,
    rightPanelWidth,
    bottomPanelHeight,
    setLeftWidth,
    setRightWidth,
    setBottomHeight,
    toggleLeft,
    toggleRight,
    toggleBottom,
  } = useLayoutStore();

  const { setProject, openTab, addRecentProject } = useProjectStore();

  const leftResize = useResizable({
    direction: "horizontal",
    onResize: setLeftWidth,
    min: 180,
    max: 500,
  });

  const rightResize = useResizable({
    direction: "horizontal",
    onResize: setRightWidth,
    min: 200,
    max: 500,
    reverse: true,
  });

  const bottomResize = useResizable({
    direction: "vertical",
    onResize: setBottomHeight,
    min: 100,
    max: 400,
    reverse: true,
  });

  const handleOpenProject = useCallback(async () => {
    try {
      const selected = await open({
        title: "Open Project",
        filters: [
          {
            name: "Alp EDA Project",
            extensions: ["alpproj"],
          },
          {
            name: "KiCad Project (Import)",
            extensions: ["kicad_pro"],
          },
          {
            name: "All Files",
            extensions: ["*"],
          },
        ],
        multiple: false,
        directory: false,
      });

      if (!selected) return;

      const info = await invoke<ProjectInfo>("open_project", { path: selected });

      setProject(info);
      addRecentProject(info.path);
      openTab({
        id: `sch-${info.path}`,
        name: info.name,
        type: "schematic",
        path: info.path,
        dirty: false,
      });
    } catch (err) {
      if (import.meta.env.DEV) {
        console.error("Failed to open project:", err);
      }
    }
  }, [setProject, openTab, addRecentProject]);

  useEffect(() => {
    invoke<AppInfo>("get_app_info")
      .then((info) => {
        document.title = `${info.name} v${info.version}`;
      })
      .catch((err) => {
        if (import.meta.env.DEV) return;
        console.error("get_app_info failed:", err);
      });
  }, []);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.key === "o") {
        e.preventDefault();
        handleOpenProject();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [handleOpenProject]);

  return (
    <div className="flex flex-col h-screen w-screen overflow-hidden bg-bg-primary text-text-primary">
      <MenuBar onOpenProject={handleOpenProject} />
      <ToolbarStrip />
      <DocumentTabBar />

      {/* Main workspace */}
      <div className="flex flex-1 min-h-0">
        {/* Left: Panel or collapsed rail */}
        {leftCollapsed ? (
          <CollapsedRail
            label="Projects"
            icon={<FolderOpen size={15} />}
            onClick={toggleLeft}
            side="left"
          />
        ) : (
          <>
            <div
              className="flex flex-col bg-bg-secondary overflow-hidden shrink-0"
              style={{ width: leftPanelWidth }}
            >
              <PanelHeader
                title="Projects"
                onCollapse={toggleLeft}
                collapseIcon={<PanelLeftClose size={14} />}
              />
              <div className="flex-1 overflow-y-auto">
                <ProjectPanel />
              </div>
            </div>
            <ResizeHandle
              direction="horizontal"
              onMouseDown={(e) => leftResize.onMouseDown(e, leftPanelWidth)}
            />
          </>
        )}

        {/* Center column */}
        <div className="flex flex-col flex-1 min-w-0">
          <div className="flex-1 min-h-0">
            <EditorCanvas onOpenProject={handleOpenProject} />
          </div>

          {/* Bottom panel */}
          {!bottomCollapsed ? (
            <>
              <ResizeHandle
                direction="vertical"
                onMouseDown={(e) =>
                  bottomResize.onMouseDown(e, bottomPanelHeight)
                }
              />
              <div
                className="bg-bg-secondary shrink-0"
                style={{ height: bottomPanelHeight }}
              >
                <PanelHeader
                  title="Messages"
                  onCollapse={toggleBottom}
                  collapseIcon={<PanelBottomClose size={14} />}
                />
                <div
                  className="overflow-y-auto"
                  style={{ height: bottomPanelHeight - 32 }}
                >
                  <MessagesPanel />
                </div>
              </div>
            </>
          ) : (
            <CollapsedBottomBar
              label="Messages"
              icon={<MessageSquare size={12} />}
              onClick={toggleBottom}
            />
          )}
        </div>

        {/* Right: Panel or collapsed rail */}
        {rightCollapsed ? (
          <CollapsedRail
            label="Properties"
            icon={<Settings size={15} />}
            onClick={toggleRight}
            side="right"
          />
        ) : (
          <>
            <ResizeHandle
              direction="horizontal"
              onMouseDown={(e) => rightResize.onMouseDown(e, rightPanelWidth)}
            />
            <div
              className="flex flex-col bg-bg-secondary overflow-hidden shrink-0"
              style={{ width: rightPanelWidth }}
            >
              <PanelHeader
                title="Properties"
                onCollapse={toggleRight}
                collapseIcon={<PanelRightClose size={14} />}
              />
              <div className="flex-1 overflow-y-auto">
                <PropertiesPanel />
              </div>
            </div>
          </>
        )}
      </div>

      <StatusBar />
    </div>
  );
}

export default App;
