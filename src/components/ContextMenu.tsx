import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
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
  const rowRef = useRef<HTMLDivElement>(null);
  const hasChildren = item.children && item.children.length > 0;

  const openSub = () => {
    if (closeTimer.current) clearTimeout(closeTimer.current);
    setSubOpen(true);
  };
  const closeSub = () => {
    closeTimer.current = setTimeout(() => setSubOpen(false), 200);
  };

  // Get screen position for submenu portal
  const getSubPos = () => {
    if (!rowRef.current) return { x: 0, y: 0 };
    const rect = rowRef.current.getBoundingClientRect();
    return { x: rect.right - 2, y: rect.top };
  };

  return (
    <div ref={rowRef} className="relative"
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
      {hasChildren && subOpen && createPortal(
        <div
          data-ctx-sub="true"
          className="fixed min-w-[200px] bg-bg-surface border border-border rounded-lg shadow-2xl shadow-black/50 py-1 z-[200]"
          style={{ left: getSubPos().x, top: getSubPos().y }}
          onMouseEnter={openSub}
          onMouseLeave={closeSub}>
          {item.children!.map((child, j) =>
            child.separator ? (
              <div key={j} className="h-px bg-border-subtle mx-3 my-1" />
            ) : (
              <MenuItemRow key={j} item={child} onClose={onClose} />
            )
          )}
        </div>,
        document.body
      )}
    </div>
  );
}

export function ContextMenu({ x, y, items, onClose }: Props) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      // Check if click is inside any submenu portal
      const target = e.target as HTMLElement;
      if (target.closest("[data-ctx-sub]")) return;
      if (ref.current && !ref.current.contains(target)) onClose();
    };
    const keyHandler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    // Delay to avoid closing immediately on the right-click that opened us
    const timer = setTimeout(() => {
      document.addEventListener("mousedown", handler);
    }, 50);
    document.addEventListener("keydown", keyHandler);
    return () => {
      clearTimeout(timer);
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
      className="absolute z-[150] min-w-[200px] bg-bg-surface border border-border rounded-lg shadow-2xl shadow-black/50 py-1 overflow-y-auto">
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
