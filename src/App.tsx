import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { MenuBar } from "@/components/MenuBar";
import { ToolbarStrip } from "@/components/ToolbarStrip";
import { DocumentTabBar } from "@/components/DocumentTabBar";
import { StatusBar } from "@/components/StatusBar";
import { ProjectPanel } from "@/panels/ProjectPanel";
import { PropertiesPanel } from "@/panels/PropertiesPanel";
import { MessagesPanel } from "@/panels/MessagesPanel";
import { EditorCanvas } from "@/canvas/EditorCanvas";
import { useLayoutStore } from "@/stores/layout";
import { cn } from "@/lib/utils";
import {
  PanelLeftClose,
  PanelLeftOpen,
  PanelRightClose,
  PanelRightOpen,
  PanelBottomClose,
  PanelBottomOpen,
} from "lucide-react";
import type { AppInfo } from "@/types";

function PanelHeader({
  title,
  className,
}: {
  title: string;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "flex items-center h-7 px-3 text-[11px] font-medium text-text-secondary uppercase tracking-wider bg-bg-secondary border-b border-border",
        className
      )}
    >
      {title}
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
    toggleLeft,
    toggleRight,
    toggleBottom,
  } = useLayoutStore();

  useEffect(() => {
    invoke<AppInfo>("get_app_info").then((info) => {
      document.title = `${info.name} v${info.version}`;
    }).catch(() => {
      // Running in dev without Tauri — use default title
    });
  }, []);

  return (
    <div className="flex flex-col h-screen w-screen overflow-hidden bg-bg-primary text-text-primary">
      {/* Menu Bar */}
      <MenuBar />

      {/* Toolbar */}
      <ToolbarStrip />

      {/* Tab Bar */}
      <DocumentTabBar />

      {/* Main Content Area */}
      <div className="flex flex-1 min-h-0">
        {/* Left Panel */}
        <div
          className={cn(
            "flex flex-col border-r border-border bg-bg-secondary transition-all duration-200 overflow-hidden",
            leftCollapsed ? "w-0" : ""
          )}
          style={{ width: leftCollapsed ? 0 : leftPanelWidth }}
        >
          <PanelHeader title="Projects" />
          <div className="flex-1 overflow-y-auto">
            <ProjectPanel />
          </div>
        </div>

        {/* Left toggle + Center + Right toggle */}
        <div className="flex flex-col flex-1 min-w-0">
          {/* Canvas + Right Panel */}
          <div className="flex flex-1 min-h-0">
            {/* Left collapse button */}
            <button
              className="flex items-center justify-center w-5 hover:bg-bg-hover transition-colors border-r border-border"
              onClick={toggleLeft}
              title={leftCollapsed ? "Show Projects" : "Hide Projects"}
            >
              {leftCollapsed ? (
                <PanelLeftOpen size={14} className="text-text-muted" />
              ) : (
                <PanelLeftClose size={14} className="text-text-muted" />
              )}
            </button>

            {/* Central Canvas */}
            <div className="flex-1 min-w-0">
              <EditorCanvas />
            </div>

            {/* Right collapse button */}
            <button
              className="flex items-center justify-center w-5 hover:bg-bg-hover transition-colors border-l border-border"
              onClick={toggleRight}
              title={rightCollapsed ? "Show Properties" : "Hide Properties"}
            >
              {rightCollapsed ? (
                <PanelRightOpen size={14} className="text-text-muted" />
              ) : (
                <PanelRightClose size={14} className="text-text-muted" />
              )}
            </button>
          </div>

          {/* Bottom Panel */}
          {!bottomCollapsed && (
            <div
              className="border-t border-border bg-bg-secondary"
              style={{ height: bottomPanelHeight }}
            >
              <PanelHeader title="Messages" />
              <div className="overflow-y-auto" style={{ height: bottomPanelHeight - 28 }}>
                <MessagesPanel />
              </div>
            </div>
          )}

          {/* Bottom toggle */}
          <button
            className="flex items-center justify-center h-5 hover:bg-bg-hover transition-colors border-t border-border"
            onClick={toggleBottom}
            title={bottomCollapsed ? "Show Messages" : "Hide Messages"}
          >
            {bottomCollapsed ? (
              <PanelBottomOpen size={14} className="text-text-muted" />
            ) : (
              <PanelBottomClose size={14} className="text-text-muted" />
            )}
          </button>
        </div>

        {/* Right Panel */}
        <div
          className={cn(
            "flex flex-col border-l border-border bg-bg-secondary transition-all duration-200 overflow-hidden",
            rightCollapsed ? "w-0" : ""
          )}
          style={{ width: rightCollapsed ? 0 : rightPanelWidth }}
        >
          <PanelHeader title="Properties" />
          <div className="flex-1 overflow-y-auto">
            <PropertiesPanel />
          </div>
        </div>
      </div>

      {/* Status Bar */}
      <StatusBar />
    </div>
  );
}

export default App;
