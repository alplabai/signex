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
          "flex items-center gap-1.5 w-full px-2 py-1 text-xs hover:bg-bg-hover transition-colors text-left"
        )}
        style={{ paddingLeft: `${depth * 16 + 8}px` }}
        onClick={() => {
          if (hasChildren) setExpanded(!expanded);
          node.onClick?.();
        }}
      >
        {hasChildren ? (
          expanded ? (
            <ChevronDown size={12} className="text-text-muted shrink-0" />
          ) : (
            <ChevronRight size={12} className="text-text-muted shrink-0" />
          )
        ) : (
          <span className="w-3" />
        )}
        <span className="text-text-muted shrink-0">{node.icon}</span>
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
      <div className="flex flex-col items-center justify-center h-full text-text-muted text-xs gap-3 p-4">
        <FolderOpen size={32} className="opacity-30" />
        <span>No project open</span>
        <span className="text-text-muted/60">
          File &gt; Open Project (Ctrl+O)
        </span>
      </div>
    );
  }

  const tree: TreeNode = {
    label: project.name,
    icon: <FolderOpen size={14} className="text-accent" />,
    children: [
      {
        label: "Schematics",
        icon: <FileText size={14} />,
        children: project.schematics.map((s) => ({
          label: s,
          icon: <FileText size={13} />,
        })),
      },
      ...(project.pcb
        ? [
            {
              label: project.pcb,
              icon: <Cpu size={14} />,
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
