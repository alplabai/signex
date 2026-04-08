import {
  FolderOpen,
  FolderClosed,
  FileText,
  Cpu,
  ChevronRight,
  ChevronDown,
  Component,
  Cable,
  Tag,
  FolderSearch,
  RefreshCw,
  Save,
  FilePlus,
  FolderPlus,
} from "lucide-react";
import { useState, useCallback } from "react";
import { cn } from "@/lib/utils";
import { useProjectStore } from "@/stores/project";

interface TreeNode {
  label: string;
  icon: React.ReactNode;
  expandedIcon?: React.ReactNode;
  badge?: string;
  children?: TreeNode[];
  onClick?: () => void;
  onContextMenu?: (e: React.MouseEvent) => void;
  defaultExpanded?: boolean;
  isFolder?: boolean;
}

function TreeItem({ node, depth = 0 }: { node: TreeNode; depth?: number }) {
  const [expanded, setExpanded] = useState(node.defaultExpanded ?? true);
  const hasChildren = node.children && node.children.length > 0;
  const isExpandable = hasChildren || node.isFolder;

  return (
    <div>
      <div
        className={cn(
          "flex items-center gap-1.5 w-full py-[5px] text-[12px] hover:bg-bg-hover transition-colors text-left text-text-secondary hover:text-text-primary group cursor-pointer"
        )}
        style={{ paddingLeft: `${depth * 14 + 10}px` }}
        onClick={() => {
          if (node.onClick) {
            node.onClick();
          } else if (isExpandable) {
            setExpanded(!expanded);
          }
        }}
        onContextMenu={(e) => {
          e.preventDefault();
          e.stopPropagation();
          if (node.onContextMenu) {
            node.onContextMenu(e);
          }
        }}
      >
        {isExpandable ? (
          <button onClick={(e) => { e.stopPropagation(); setExpanded(!expanded); }}
            className="shrink-0 p-0 hover:text-accent transition-colors">
            {expanded ? <ChevronDown size={11} className="text-text-muted" /> : <ChevronRight size={11} className="text-text-muted" />}
          </button>
        ) : (
          <span className="w-[11px]" />
        )}
        <span className="shrink-0">{expanded && node.expandedIcon ? node.expandedIcon : node.icon}</span>
        <span className="truncate flex-1">{node.label}</span>
        {node.badge && (
          <span className="text-[10px] text-text-muted/50 pr-2 tabular-nums">
            {node.badge}
          </span>
        )}
      </div>
      {isExpandable && expanded && hasChildren && (
        <div>
          {node.children!.map((child, i) => (
            <TreeItem key={i} node={child} depth={depth + 1} />
          ))}
        </div>
      )}
      {isExpandable && expanded && !hasChildren && (
        <div
          className="text-[11px] text-text-muted/30 italic"
          style={{ paddingLeft: `${(depth + 1) * 14 + 10 + 11 + 6}px`, paddingTop: 2, paddingBottom: 2 }}
        >
          (empty)
        </div>
      )}
    </div>
  );
}

// Context menu types
interface CtxMenu {
  x: number;
  y: number;
  type: "panel" | "project" | "sheet";
  sheetIdx?: number;
}

function ContextMenuItem({ label, icon, onClick, disabled, hasSubmenu }: {
  label: string; icon?: React.ReactNode; onClick?: () => void; disabled?: boolean; hasSubmenu?: boolean;
}) {
  return (
    <button onClick={onClick} disabled={disabled}
      className={cn("flex items-center gap-2 w-full px-3 py-1 text-[11px] text-left transition-colors",
        disabled ? "text-text-muted/30 cursor-default" : "text-text-secondary hover:bg-bg-hover hover:text-text-primary"
      )}>
      {icon && <span className="w-4 shrink-0">{icon}</span>}
      <span className="flex-1">{label}</span>
      {hasSubmenu && <ChevronRight size={10} className="text-text-muted/40" />}
    </button>
  );
}

function ContextMenuSep() {
  return <div className="my-1 border-t border-border-subtle" />;
}

export function ProjectPanel() {
  const project = useProjectStore((s) => s.project);
  const openTab = useProjectStore((s) => s.openTab);
  const setActiveTab = useProjectStore((s) => s.setActiveTab);
  const recentProjects = useProjectStore((s) => s.recentProjects);
  const [ctxMenu, setCtxMenu] = useState<CtxMenu | null>(null);

  const closeCtx = useCallback(() => { setCtxMenu(null); }, []);

  const handlePanelContextMenu = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    setCtxMenu({ x: e.clientX, y: e.clientY, type: "panel" });
  }, []);

  const handleProjectContextMenu = useCallback((e: React.MouseEvent) => {
    setCtxMenu({ x: e.clientX, y: e.clientY, type: "project" });
  }, []);

  const handleExplore = useCallback(() => {
    if (!project) return;
    closeCtx();
    import("@tauri-apps/api/core").then(({ invoke }) => invoke("open_path", { path: project.dir })).catch(() => {});
  }, [project, closeCtx]);

  if (!project) {
    return (
      <div className="flex flex-col items-center justify-center h-full text-text-muted text-xs gap-3 p-6"
        onContextMenu={handlePanelContextMenu}>
        <FolderOpen size={28} className="text-text-muted/20" />
        <span className="text-text-muted/50">No project open</span>
        <span className="text-text-muted/30 text-[11px]">Ctrl+O to open</span>
        {ctxMenu?.type === "panel" && (
          <PanelContextMenu x={ctxMenu.x} y={ctxMenu.y} onClose={closeCtx} recentProjects={recentProjects} />
        )}
      </div>
    );
  }

  const sheetsChildren: TreeNode[] = project.sheets.map((sheet, idx) => ({
    label: `[${idx + 1}] ${sheet.name}`,
    icon: <FileText size={12} className="text-warning/70" />,
    badge: `${sheet.symbols_count}c ${sheet.wires_count}w`,
    onContextMenu: handleProjectContextMenu,
    onClick: () => {
      const tabId = `sch-${project.path}:${sheet.filename}`;
      openTab({
        id: tabId,
        name: sheet.name,
        type: "schematic",
        path: project.path,
        dirty: false,
      });
      setActiveTab(tabId);
    },
    children: [
      {
        label: `${sheet.symbols_count} components`,
        icon: <Component size={11} className="text-text-muted/50" />,
        onContextMenu: handleProjectContextMenu,
      },
      {
        label: `${sheet.wires_count} wires`,
        icon: <Cable size={11} className="text-text-muted/50" />,
        onContextMenu: handleProjectContextMenu,
      },
      {
        label: `${sheet.labels_count} labels`,
        icon: <Tag size={11} className="text-text-muted/50" />,
        onContextMenu: handleProjectContextMenu,
      },
    ],
  }));

  // Source Documents: schematics + PCB + BOM
  const sourceDocChildren: TreeNode[] = [
    ...sheetsChildren,
    ...(project.pcb_file
      ? [
          {
            label: project.pcb_file,
            icon: <Cpu size={12} className="text-success/70" />,
            onContextMenu: handleProjectContextMenu,
            onClick: () => {
              const tabId = `pcb-${project.path}:${project.pcb_file}`;
              openTab({
                id: tabId,
                name: project.pcb_file!,
                type: "pcb" as const,
                path: project.path,
                dirty: false,
              });
              setActiveTab(tabId);
            },
          },
        ]
      : []),
  ];

  const tree: TreeNode = {
    label: project.name,
    icon: <FolderClosed size={13} className="text-accent" />,
    expandedIcon: <FolderOpen size={13} className="text-accent" />,
    onContextMenu: handleProjectContextMenu,
    children: [
      {
        label: "Variants",
        icon: <FolderClosed size={12} className="text-text-muted/60" />,
        expandedIcon: <FolderOpen size={12} className="text-text-muted/60" />,
        isFolder: true,
        defaultExpanded: false,
        onContextMenu: handleProjectContextMenu,
        children: [],
      },
      {
        label: "Source Documents",
        icon: <FolderClosed size={12} className="text-warning/80" />,
        expandedIcon: <FolderOpen size={12} className="text-warning/80" />,
        isFolder: true,
        onContextMenu: handleProjectContextMenu,
        children: sourceDocChildren,
      },
      {
        label: "Settings",
        icon: <FolderClosed size={12} className="text-text-muted/60" />,
        expandedIcon: <FolderOpen size={12} className="text-text-muted/60" />,
        isFolder: true,
        defaultExpanded: false,
        onContextMenu: handleProjectContextMenu,
        children: [],
      },
      {
        label: "Libraries",
        icon: <FolderClosed size={12} className="text-purple-400/70" />,
        expandedIcon: <FolderOpen size={12} className="text-purple-400/70" />,
        isFolder: true,
        defaultExpanded: false,
        onContextMenu: handleProjectContextMenu,
        children: [],
      },
      {
        label: "Generated",
        icon: <FolderClosed size={12} className="text-text-muted/60" />,
        expandedIcon: <FolderOpen size={12} className="text-text-muted/60" />,
        isFolder: true,
        defaultExpanded: false,
        onContextMenu: handleProjectContextMenu,
        children: [],
      },
    ],
  };

  return (
    <div className="py-1 h-full" onContextMenu={(e) => { e.preventDefault(); handlePanelContextMenu(e); }}>
      <TreeItem node={tree} />
      {project.format === "kicad" && (
        <div className="mx-3 mt-3 px-2 py-1.5 text-[10px] text-text-muted/40 bg-bg-surface/30 rounded border border-border-subtle">
          Imported from KiCad
        </div>
      )}

      {/* Context menus */}
      {ctxMenu?.type === "panel" && (
        <PanelContextMenu x={ctxMenu.x} y={ctxMenu.y} onClose={closeCtx} recentProjects={recentProjects} />
      )}
      {ctxMenu?.type === "project" && (
        <ProjectContextMenu x={ctxMenu.x} y={ctxMenu.y} onClose={closeCtx}
          onExplore={handleExplore} projectName={project.name} />
      )}
    </div>
  );
}

// ═══════════════════════════════════════════════════════════════
// CONTEXT MENUS
// ═══════════════════════════════════════════════════════════════

function PanelContextMenu({ x, y, onClose, recentProjects }: {
  x: number; y: number; onClose: () => void; recentProjects: string[];
}) {
  const [showRecent, setShowRecent] = useState(false);
  return (
    <>
      <div className="fixed inset-0 z-[90]" onClick={onClose} onContextMenu={(e) => { e.preventDefault(); onClose(); }} />
      <div className="fixed z-[95] bg-bg-secondary border border-border-subtle rounded shadow-xl py-1 min-w-[200px]"
        style={{ left: x, top: y }}>
        <div className="relative" onMouseEnter={() => setShowRecent(true)} onMouseLeave={() => setShowRecent(false)}>
          <ContextMenuItem label="Recent Project Groups" hasSubmenu />
          {showRecent && recentProjects.length > 0 && (
            <div className="absolute left-full top-0 bg-bg-secondary border border-border-subtle rounded shadow-xl py-1 min-w-[250px]">
              {recentProjects.map((p, i) => (
                <ContextMenuItem key={i} label={`${i + 1}  ${p}`} onClick={onClose} />
              ))}
            </div>
          )}
        </div>
        <ContextMenuSep />
        <ContextMenuItem label="Add New Project..." icon={<FilePlus size={12} />} onClick={onClose} />
        <ContextMenuItem label="Add Existing Project..." icon={<FolderPlus size={12} />} onClick={onClose} />
        <ContextMenuSep />
        <ContextMenuItem label="Open Project Group..." icon={<FolderSearch size={12} />} onClick={onClose} />
        <ContextMenuItem label="Save Project Group" icon={<Save size={12} />} onClick={onClose} />
        <ContextMenuItem label="Rename..." onClick={onClose} />
        <ContextMenuItem label="Save All" disabled />
        <ContextMenuSep />
        <ContextMenuItem label="Explore" onClick={onClose} />
        <ContextMenuItem label="Refresh" icon={<RefreshCw size={12} />} onClick={onClose} />
      </div>
    </>
  );
}

function ProjectContextMenu({ x, y, onClose, onExplore, projectName }: {
  x: number; y: number; onClose: () => void; onExplore: () => void; projectName: string;
}) {
  const [sub, setSub] = useState<string | null>(null);
  return (
    <>
      <div className="fixed inset-0 z-[90]" onClick={onClose} onContextMenu={(e) => { e.preventDefault(); onClose(); }} />
      <div className="fixed z-[95] bg-bg-secondary border border-border-subtle rounded shadow-xl py-1 min-w-[260px]"
        style={{ left: Math.min(x, window.innerWidth - 280), top: Math.min(y, window.innerHeight - 500) }}>
        <ContextMenuItem label={`Validate PCB Project ${projectName}`} disabled />
        <ContextMenuSep />

        {/* Add New to Project — submenu */}
        <div className="relative" onMouseEnter={() => setSub("addNew")} onMouseLeave={() => setSub(null)}>
          <ContextMenuItem label="Add New to Project" hasSubmenu />
          {sub === "addNew" && (
            <div className="absolute left-full top-0 bg-bg-secondary border border-border-subtle rounded shadow-xl py-1 min-w-[180px] z-[100]">
              <ContextMenuItem label="Schematic" icon={<FileText size={12} className="text-warning/70" />} onClick={onClose} />
              <ContextMenuItem label="PCB" icon={<Cpu size={12} className="text-success/70" />} onClick={onClose} />
              <ContextMenuItem label="PCB3D" icon={<Cpu size={12} className="text-accent/70" />} onClick={onClose} disabled />
              <ContextMenuItem label="Draftsman Document" disabled />
              <ContextMenuSep />
              <ContextMenuItem label="Schematic Library" icon={<FileText size={12} className="text-accent" />} onClick={onClose} />
              <ContextMenuItem label="PCB Library" icon={<Cpu size={12} className="text-success" />} onClick={onClose} />
              <ContextMenuItem label="Pad Via Library" disabled />
              <ContextMenuSep />
              <ContextMenuItem label="CAM Document" disabled />
              <ContextMenuItem label="Output Job File" onClick={onClose} />
              <ContextMenuItem label="Database Link File" disabled />
            </div>
          )}
        </div>

        <ContextMenuItem label="Add Existing to Project..." onClick={onClose} />
        <ContextMenuItem label="Save to Server..." disabled />
        <ContextMenuItem label="Save" icon={<Save size={12} />} onClick={onClose} />
        <ContextMenuItem label="Make a copy..." disabled />
        <ContextMenuItem label="Rename..." onClick={onClose} />
        <ContextMenuSep />
        <ContextMenuItem label="Close Project Documents" onClick={onClose} />
        <ContextMenuItem label="Close Project" onClick={onClose} />
        <ContextMenuSep />
        <ContextMenuItem label="Explore" onClick={onExplore} />
        <ContextMenuItem label="Variants..." onClick={onClose} />
        <ContextMenuSep />

        {/* History & Version Control — submenu */}
        <div className="relative" onMouseEnter={() => setSub("history")} onMouseLeave={() => setSub(null)}>
          <ContextMenuItem label="History && Version Control" hasSubmenu />
          {sub === "history" && (
            <div className="absolute left-full top-0 bg-bg-secondary border border-border-subtle rounded shadow-xl py-1 min-w-[160px] z-[100]">
              <ContextMenuItem label="Show History" disabled />
              <ContextMenuItem label="Compare..." disabled />
            </div>
          )}
        </div>

        <ContextMenuSep />
        <ContextMenuItem label="Project Packager..." disabled />
        <ContextMenuItem label="Project Releaser..." disabled />
        <ContextMenuSep />
        <ContextMenuItem label="Show in Explorer" onClick={onExplore} />
        <ContextMenuItem label="Show in Web Browser" disabled />
        <ContextMenuSep />
        <ContextMenuItem label="Share..." disabled />
        <ContextMenuItem label="Project Options..." onClick={onClose} />
      </div>
    </>
  );
}
