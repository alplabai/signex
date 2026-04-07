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
} from "lucide-react";
import { useState } from "react";
import { cn } from "@/lib/utils";
import { useProjectStore } from "@/stores/project";

interface TreeNode {
  label: string;
  icon: React.ReactNode;
  expandedIcon?: React.ReactNode;
  badge?: string;
  children?: TreeNode[];
  onClick?: () => void;
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

export function ProjectPanel() {
  const project = useProjectStore((s) => s.project);
  const openTab = useProjectStore((s) => s.openTab);
  const setActiveTab = useProjectStore((s) => s.setActiveTab);

  if (!project) {
    return (
      <div className="flex flex-col items-center justify-center h-full text-text-muted text-xs gap-3 p-6">
        <FolderOpen size={28} className="text-text-muted/20" />
        <span className="text-text-muted/50">No project open</span>
        <span className="text-text-muted/30 text-[11px]">Ctrl+O to open</span>
      </div>
    );
  }

  const sheetsChildren: TreeNode[] = project.sheets.map((sheet, idx) => ({
    label: `[${idx + 1}] ${sheet.name}`,
    icon: <FileText size={12} className="text-warning/70" />,
    badge: `${sheet.symbols_count}c ${sheet.wires_count}w`,
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
      },
      {
        label: `${sheet.wires_count} wires`,
        icon: <Cable size={11} className="text-text-muted/50" />,
      },
      {
        label: `${sheet.labels_count} labels`,
        icon: <Tag size={11} className="text-text-muted/50" />,
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
    children: [
      {
        label: "Variants",
        icon: <FolderClosed size={12} className="text-text-muted/60" />,
        expandedIcon: <FolderOpen size={12} className="text-text-muted/60" />,
        isFolder: true,
        defaultExpanded: false,
        children: [],
      },
      {
        label: "Source Documents",
        icon: <FolderClosed size={12} className="text-warning/80" />,
        expandedIcon: <FolderOpen size={12} className="text-warning/80" />,
        isFolder: true,
        children: sourceDocChildren,
      },
      {
        label: "Settings",
        icon: <FolderClosed size={12} className="text-text-muted/60" />,
        expandedIcon: <FolderOpen size={12} className="text-text-muted/60" />,
        isFolder: true,
        defaultExpanded: false,
        children: [],
      },
      {
        label: "Libraries",
        icon: <FolderClosed size={12} className="text-purple-400/70" />,
        expandedIcon: <FolderOpen size={12} className="text-purple-400/70" />,
        isFolder: true,
        defaultExpanded: false,
        children: [],
      },
      {
        label: "Generated",
        icon: <FolderClosed size={12} className="text-text-muted/60" />,
        expandedIcon: <FolderOpen size={12} className="text-text-muted/60" />,
        isFolder: true,
        defaultExpanded: false,
        children: [],
      },
    ],
  };

  return (
    <div className="py-1">
      <TreeItem node={tree} />
      {project.format === "kicad" && (
        <div className="mx-3 mt-3 px-2 py-1.5 text-[10px] text-text-muted/40 bg-bg-surface/30 rounded border border-border-subtle">
          Imported from KiCad
        </div>
      )}
    </div>
  );
}
