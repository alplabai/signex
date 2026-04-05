import {
  FolderOpen,
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
  badge?: string;
  children?: TreeNode[];
  onClick?: () => void;
}

function TreeItem({ node, depth = 0 }: { node: TreeNode; depth?: number }) {
  const [expanded, setExpanded] = useState(true);
  const hasChildren = node.children && node.children.length > 0;

  return (
    <div>
      <button
        className={cn(
          "flex items-center gap-1.5 w-full py-[5px] text-[12px] hover:bg-bg-hover transition-colors text-left text-text-secondary hover:text-text-primary group"
        )}
        style={{ paddingLeft: `${depth * 14 + 10}px` }}
        onClick={() => {
          if (hasChildren) setExpanded(!expanded);
          node.onClick?.();
        }}
      >
        {hasChildren ? (
          expanded ? (
            <ChevronDown size={11} className="text-text-muted shrink-0" />
          ) : (
            <ChevronRight size={11} className="text-text-muted shrink-0" />
          )
        ) : (
          <span className="w-[11px]" />
        )}
        <span className="shrink-0">{node.icon}</span>
        <span className="truncate flex-1">{node.label}</span>
        {node.badge && (
          <span className="text-[10px] text-text-muted/50 pr-2 tabular-nums">
            {node.badge}
          </span>
        )}
      </button>
      {hasChildren && expanded && (
        <div>
          {node.children!.map((child, i) => (
            <TreeItem key={i} node={child} depth={depth + 1} />
          ))}
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

  const sheetsChildren: TreeNode[] = project.sheets.map((sheet) => ({
    label: sheet.name,
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

  const tree: TreeNode = {
    label: project.name,
    icon: <FolderOpen size={13} className="text-accent" />,
    children: [
      ...(sheetsChildren.length > 0
        ? [
            {
              label: `Schematics (${sheetsChildren.length})`,
              icon: <FileText size={13} className="text-warning" />,
              children: sheetsChildren,
            },
          ]
        : []),
      ...(project.pcb_file
        ? [
            {
              label: project.pcb_file,
              icon: <Cpu size={13} className="text-success" />,
            },
          ]
        : []),
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
