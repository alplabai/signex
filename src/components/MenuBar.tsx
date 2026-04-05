import { useState, useRef, useEffect } from "react";
import { cn } from "@/lib/utils";

interface MenuItem {
  label: string;
  shortcut?: string;
  action?: () => void;
  separator?: boolean;
  disabled?: boolean;
}

interface MenuGroup {
  label: string;
  items: MenuItem[];
}

const menus: MenuGroup[] = [
  {
    label: "File",
    items: [
      { label: "New Project...", shortcut: "Ctrl+N" },
      { label: "Open Project...", shortcut: "Ctrl+O" },
      { separator: true, label: "" },
      { label: "Save", shortcut: "Ctrl+S", disabled: true },
      { label: "Save As...", shortcut: "Ctrl+Shift+S", disabled: true },
      { separator: true, label: "" },
      { label: "Recent Projects", disabled: true },
      { separator: true, label: "" },
      { label: "Exit", shortcut: "Alt+F4" },
    ],
  },
  {
    label: "Edit",
    items: [
      { label: "Undo", shortcut: "Ctrl+Z", disabled: true },
      { label: "Redo", shortcut: "Ctrl+Y", disabled: true },
      { separator: true, label: "" },
      { label: "Cut", shortcut: "Ctrl+X", disabled: true },
      { label: "Copy", shortcut: "Ctrl+C", disabled: true },
      { label: "Paste", shortcut: "Ctrl+V", disabled: true },
      { label: "Delete", shortcut: "Del", disabled: true },
      { separator: true, label: "" },
      { label: "Select All", shortcut: "Ctrl+A", disabled: true },
      { label: "Find...", shortcut: "Ctrl+F", disabled: true },
    ],
  },
  {
    label: "View",
    items: [
      { label: "Zoom In", shortcut: "Ctrl++" },
      { label: "Zoom Out", shortcut: "Ctrl+-" },
      { label: "Fit to View", shortcut: "Home" },
      { label: "Zoom 1:1", shortcut: "Ctrl+1" },
      { separator: true, label: "" },
      { label: "Toggle Grid", shortcut: "G" },
      { label: "Toggle Snap", shortcut: "Shift+G" },
      { separator: true, label: "" },
      { label: "Projects Panel" },
      { label: "Properties Panel" },
      { label: "AI Copilot" },
    ],
  },
  {
    label: "Place",
    items: [
      { label: "Component...", shortcut: "P, C", disabled: true },
      { label: "Wire", shortcut: "P, W", disabled: true },
      { label: "Bus", shortcut: "P, B", disabled: true },
      { label: "Net Label", shortcut: "P, L", disabled: true },
      { label: "Power Port", shortcut: "P, P", disabled: true },
      { label: "No Connect", shortcut: "P, X", disabled: true },
    ],
  },
  {
    label: "Route",
    items: [
      { label: "Interactive Route", shortcut: "X", disabled: true },
      { label: "Differential Pair", shortcut: "D", disabled: true },
      { label: "Length Tuning", disabled: true },
      { separator: true, label: "" },
      { label: "Fanout", disabled: true },
      { label: "Teardrops", disabled: true },
    ],
  },
  {
    label: "Design",
    items: [
      { label: "Run ERC...", disabled: true },
      { label: "Run DRC...", disabled: true },
      { separator: true, label: "" },
      { label: "Design Rules...", disabled: true },
      { label: "Layer Stack Manager...", disabled: true },
      { separator: true, label: "" },
      { label: "Annotate Schematic", disabled: true },
      { label: "Cross-Probe", shortcut: "Ctrl+Shift+X", disabled: true },
    ],
  },
  {
    label: "Output",
    items: [
      { label: "Generate BOM...", disabled: true },
      { label: "Generate Gerbers...", disabled: true },
      { label: "Assembly Drawings...", disabled: true },
      { separator: true, label: "" },
      { label: "Output Jobs...", disabled: true },
      { label: "Release...", disabled: true },
    ],
  },
  {
    label: "Tools",
    items: [
      { label: "Library Manager...", disabled: true },
      { label: "Simulation...", disabled: true },
      { label: "Supply Chain Search...", disabled: true },
      { separator: true, label: "" },
      { label: "AI Copilot", shortcut: "Ctrl+Shift+A" },
      { label: "Command Palette", shortcut: "Ctrl+K" },
    ],
  },
  {
    label: "Help",
    items: [
      { label: "Documentation" },
      { label: "Keyboard Shortcuts" },
      { separator: true, label: "" },
      { label: "About Alp EDA" },
    ],
  },
];

export function MenuBar() {
  const [openMenu, setOpenMenu] = useState<number | null>(null);
  const menuBarRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (menuBarRef.current && !menuBarRef.current.contains(e.target as Node)) {
        setOpenMenu(null);
      }
    }
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  return (
    <div
      ref={menuBarRef}
      className="flex h-7 items-center bg-bg-secondary border-b border-border px-1 text-xs"
    >
      {menus.map((menu, idx) => (
        <div key={menu.label} className="relative">
          <button
            className={cn(
              "px-2.5 py-1 rounded-sm hover:bg-bg-hover transition-colors",
              openMenu === idx && "bg-bg-hover"
            )}
            onClick={() => setOpenMenu(openMenu === idx ? null : idx)}
            onMouseEnter={() => openMenu !== null && setOpenMenu(idx)}
          >
            {menu.label}
          </button>
          {openMenu === idx && (
            <div className="absolute left-0 top-full mt-0.5 min-w-[220px] bg-bg-surface border border-border rounded-md shadow-xl z-50 py-1">
              {menu.items.map((item, iIdx) =>
                item.separator ? (
                  <div key={iIdx} className="h-px bg-border mx-2 my-1" />
                ) : (
                  <button
                    key={iIdx}
                    disabled={item.disabled}
                    className={cn(
                      "w-full flex items-center justify-between px-3 py-1.5 text-left hover:bg-bg-hover transition-colors",
                      item.disabled && "opacity-40 cursor-default"
                    )}
                    onClick={() => {
                      if (!item.disabled) {
                        item.action?.();
                        setOpenMenu(null);
                      }
                    }}
                  >
                    <span>{item.label}</span>
                    {item.shortcut && (
                      <span className="text-text-muted ml-6 text-[11px]">
                        {item.shortcut}
                      </span>
                    )}
                  </button>
                )
              )}
            </div>
          )}
        </div>
      ))}
    </div>
  );
}
