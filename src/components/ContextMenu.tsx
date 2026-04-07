import { useEffect, useRef, useState } from "react";
import { cn } from "@/lib/utils";

export interface ContextMenuItem {
  label: string;
  shortcut?: string;
  action: () => void;
  separator?: boolean;
  disabled?: boolean;
  icon?: React.ReactNode;
  children?: ContextMenuItem[]; // Submenu items
}

interface Props {
  x: number;
  y: number;
  items: ContextMenuItem[];
  onClose: () => void;
}

function MenuItemRow({ item, onClose }: { item: ContextMenuItem; onClose: () => void }) {
  const [subOpen, setSubOpen] = useState(false);
  const closeTimer = useRef<ReturnType<typeof setTimeout>>(undefined);
  const hasChildren = item.children && item.children.length > 0;

  const openSub = () => {
    if (closeTimer.current) clearTimeout(closeTimer.current);
    setSubOpen(true);
  };
  const closeSub = () => {
    closeTimer.current = setTimeout(() => setSubOpen(false), 150);
  };

  return (
    <div className="relative"
      onMouseEnter={() => hasChildren && openSub()}
      onMouseLeave={() => hasChildren && closeSub()}>
      <button disabled={item.disabled && !hasChildren}
        className={cn(
          "w-full flex items-center px-3 py-[5px] text-[12px] text-left transition-colors gap-2",
          hasChildren && subOpen ? "bg-accent/15 text-text-primary" :
          item.disabled && !hasChildren
            ? "text-text-muted/40 cursor-default"
            : "text-text-secondary hover:bg-accent/15 hover:text-text-primary"
        )}
        onClick={() => { if (!hasChildren) { item.action(); onClose(); } }}>
        {item.icon && <span className="w-4 shrink-0 flex justify-center">{item.icon}</span>}
        <span className="flex-1">{item.label}</span>
        {item.shortcut && !hasChildren && (
          <span className="text-text-muted/50 ml-4 text-[10px] font-mono">{item.shortcut}</span>
        )}
        {hasChildren && (
          <span className="text-text-muted/50 ml-2 text-[9px]">&#9656;</span>
        )}
      </button>
      {hasChildren && subOpen && (
        <div
          className="absolute left-full top-0 min-w-[200px] bg-bg-surface border border-border rounded-lg shadow-2xl shadow-black/50 py-1 z-50 -ml-1"
          onMouseEnter={openSub}
          onMouseLeave={closeSub}>
          {item.children!.map((child, j) =>
            child.separator ? (
              <div key={j} className="h-px bg-border-subtle mx-3 my-1" />
            ) : (
              <MenuItemRow key={j} item={child} onClose={onClose} />
            )
          )}
        </div>
      )}
    </div>
  );
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

  const style: React.CSSProperties = {
    left: x,
    top: y,
    maxHeight: "80vh",
  };

  return (
    <div ref={ref} style={style}
      className="absolute z-50 min-w-[200px] bg-bg-surface border border-border rounded-lg shadow-2xl shadow-black/50 py-1 overflow-y-auto">
      {items.map((item, i) =>
        item.separator ? (
          <div key={i} className="h-px bg-border-subtle mx-3 my-1" />
        ) : (
          <MenuItemRow key={i} item={item} onClose={onClose} />
        )
      )}
    </div>
  );
}
