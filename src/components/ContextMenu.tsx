import { useEffect, useRef } from "react";
import { cn } from "@/lib/utils";

export interface ContextMenuItem {
  label: string;
  shortcut?: string;
  action: () => void;
  separator?: boolean;
  disabled?: boolean;
}

interface Props {
  x: number;
  y: number;
  items: ContextMenuItem[];
  onClose: () => void;
}

export function ContextMenu({ x, y, items, onClose }: Props) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose();
    };
    const keyHandler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("mousedown", handler);
    document.addEventListener("keydown", keyHandler);
    return () => {
      document.removeEventListener("mousedown", handler);
      document.removeEventListener("keydown", keyHandler);
    };
  }, [onClose]);

  // Adjust position to stay within viewport
  const style: React.CSSProperties = {
    left: x,
    top: y,
    maxHeight: "80vh",
  };

  return (
    <div ref={ref} style={style}
      className="absolute z-50 min-w-[180px] bg-bg-surface border border-border rounded-lg shadow-2xl shadow-black/50 py-1 overflow-y-auto">
      {items.map((item, i) =>
        item.separator ? (
          <div key={i} className="h-px bg-border-subtle mx-3 my-1" />
        ) : (
          <button key={i} disabled={item.disabled}
            className={cn(
              "w-full flex items-center justify-between px-3 py-[4px] text-[12px] text-left transition-colors",
              item.disabled
                ? "text-text-muted/40 cursor-default"
                : "text-text-secondary hover:bg-accent/15 hover:text-text-primary"
            )}
            onClick={() => { item.action(); onClose(); }}>
            <span>{item.label}</span>
            {item.shortcut && (
              <span className="text-text-muted/50 ml-6 text-[10px] font-mono">{item.shortcut}</span>
            )}
          </button>
        )
      )}
    </div>
  );
}
