import {
  FolderOpen,
  FileText,
  Cpu,
  ChevronRight,
  ChevronDown,
} from "lucide-react";
import { useState } from "react";
import { cn } from "@/lib/utils";
import { useProjectStore } from "@/stores/project";

interface TreeNode {
  label: string;
  icon: React.ReactNode;
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
          "flex items-center gap-1.5 w-full py-[5px] text-[12px] hover:bg-bg-hover transition-colors text-left text-text-secondary hover:text-text-primary"
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
        <span className="truncate">{node.label}</span>
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

  if (!project) {
    return (
      <div className="flex flex-col items-center justify-center h-full text-text-muted text-xs gap-3 p-6">
        <FolderOpen size={28} className="text-text-muted/20" />
        <span className="text-text-muted/50">No project open</span>
        <span className="text-text-muted/30 text-[11px]">
          Ctrl+O to open
        </span>
      </div>
    );
  }

  const tree: TreeNode = {
    label: project.name,
    icon: <FolderOpen size={13} className="text-accent" />,
    children: [
      {
        label: "Schematics",
        icon: <FileText size={13} className="text-warning" />,
        children: project.schematics.map((s) => ({
          label: s,
          icon: <FileText size={12} className="text-warning/60" />,
        })),
      },
      ...(project.pcb
        ? [
            {
              label: project.pcb,
              icon: <Cpu size={13} className="text-success" />,
            },
          ]
        : []),
    ],
  };

  return (
    <div className="py-1">
      <TreeItem node={tree} />
    </div>
  );
}
