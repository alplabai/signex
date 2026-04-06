import { useState, useRef, useEffect } from "react";
import { cn } from "@/lib/utils";
import { useSchematicStore } from "@/stores/schematic";
import { useEditorStore } from "@/stores/editor";

interface MenuBarProps {
  onOpenProject?: () => void;
  onSave?: () => void;
  onOpenComponentSearch?: () => void;
}

interface MenuItem {
  id?: string;
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
      { id: "file-open", label: "Open Project...", shortcut: "Ctrl+O" },
      { separator: true, label: "" },
      { label: "Save", shortcut: "Ctrl+S", disabled: true },
      { label: "Save As...", shortcut: "Ctrl+Alt+S", disabled: true },
      { separator: true, label: "" },
      { label: "Export as PNG...", disabled: true },
      { label: "Print...", shortcut: "Ctrl+P", disabled: true },
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
      { label: "Smart Paste...", shortcut: "Shift+Ctrl+V", disabled: true },
      { label: "Duplicate", shortcut: "Ctrl+D", disabled: true },
      { label: "Delete", shortcut: "Del", disabled: true },
      { separator: true, label: "" },
      { label: "Select All", shortcut: "Ctrl+A", disabled: true },
      { label: "Find...", shortcut: "Ctrl+F", disabled: true },
      { label: "Find and Replace...", shortcut: "Ctrl+H", disabled: true },
      { label: "Find Similar Objects", shortcut: "Shift+F", disabled: true },
      { separator: true, label: "" },
      { label: "Break Wire", disabled: true },
      { separator: true, label: "" },
      { label: "Align Left", shortcut: "Shift+Ctrl+L", disabled: true },
      { label: "Align Right", shortcut: "Shift+Ctrl+R", disabled: true },
      { label: "Align Top", shortcut: "Shift+Ctrl+T", disabled: true },
      { label: "Align Bottom", shortcut: "Shift+Ctrl+B", disabled: true },
      { label: "Distribute Horizontally", shortcut: "Shift+Ctrl+H", disabled: true },
      { label: "Distribute Vertically", disabled: true },
      { label: "Align to Grid", shortcut: "Shift+Ctrl+D", disabled: true },
    ],
  },
  {
    label: "View",
    items: [
      { label: "Fit Document", shortcut: "Home" },
      { label: "Fit All Objects", disabled: true },
      { separator: true, label: "" },
      { label: "Zoom In", shortcut: "PgUp" },
      { label: "Zoom Out", shortcut: "PgDn" },
      { label: "Zoom Last", disabled: true },
      { separator: true, label: "" },
      { label: "Toggle Units", shortcut: "Ctrl+Q" },
      { label: "Toggle Net Color Override", shortcut: "F5", disabled: true },
      { separator: true, label: "" },
      { label: "Set Snap Grid", shortcut: "G" },
      { label: "Toggle Visible Grid", shortcut: "Shift+Ctrl+G" },
      { label: "Toggle Electrical Grid", shortcut: "Shift+E", disabled: true },
      { separator: true, label: "" },
      { label: "Properties Panel", shortcut: "F11" },
      { label: "Components Panel" },
      { label: "Messages Panel" },
      { label: "Navigator", disabled: true },
      { separator: true, label: "" },
      { label: "Refresh", shortcut: "End" },
    ],
  },
  {
    label: "Place",
    items: [
      { label: "Part...", shortcut: "P, P", disabled: true },
      { separator: true, label: "" },
      { label: "Wire", shortcut: "P, W", disabled: true },
      { label: "Bus", shortcut: "P, B", disabled: true },
      { label: "Bus Entry", disabled: true },
      { separator: true, label: "" },
      { label: "Net Label", shortcut: "P, N", disabled: true },
      { label: "Power Port", shortcut: "P, O", disabled: true },
      { label: "Port", shortcut: "P, R", disabled: true },
      { label: "No Connect", shortcut: "P, X", disabled: true },
      { label: "Junction", shortcut: "P, J", disabled: true },
      { separator: true, label: "" },
      { label: "Sheet Symbol...", disabled: true },
      { label: "Sheet Entry", disabled: true },
      { separator: true, label: "" },
      { label: "Text String", shortcut: "P, T", disabled: true },
      { label: "Text Frame", disabled: true },
      { label: "Note", disabled: true },
      { separator: true, label: "" },
      { label: "Line", shortcut: "P, L", disabled: true },
      { label: "Rectangle", disabled: true },
      { label: "Arc", disabled: true },
      { label: "Ellipse", disabled: true },
      { label: "Polygon", disabled: true },
      { label: "Image...", disabled: true },
      { separator: true, label: "" },
      { label: "No ERC", disabled: true },
      { label: "Directive", disabled: true },
    ],
  },
  {
    label: "Design",
    items: [
      { label: "Update PCB Document...", disabled: true },
      { label: "Import Changes From PCB...", disabled: true },
      { separator: true, label: "" },
      { label: "Create Sheet Symbol From Sheet", disabled: true },
      { label: "Create Sheet From Sheet Symbol", disabled: true },
      { separator: true, label: "" },
      { label: "Annotate Schematics...", shortcut: "T, A", disabled: true },
      { label: "Reset Designators", disabled: true },
      { label: "Reset Duplicate Designators", disabled: true },
      { separator: true, label: "" },
      { label: "Sheet Templates", disabled: true },
      { label: "Document Options...", disabled: true },
    ],
  },
  {
    label: "Tools",
    items: [
      { label: "Annotate Schematics...", shortcut: "T, A", disabled: true },
      { label: "Back Annotate...", disabled: true },
      { label: "Number Schematic Sheets...", disabled: true },
      { separator: true, label: "" },
      { label: "Cross Reference...", disabled: true },
      { label: "Cross Select Mode", shortcut: "Shift+Ctrl+X", disabled: true },
      { separator: true, label: "" },
      { label: "Measure Distance", shortcut: "Ctrl+M", disabled: true },
      { separator: true, label: "" },
      { label: "Preferences...", disabled: true },
      { separator: true, label: "" },
      { label: "Signal (AI)", shortcut: "Ctrl+Shift+A" },
    ],
  },
  {
    label: "Reports",
    items: [
      { label: "Bill of Materials...", disabled: true },
      { label: "Component Cross Reference...", disabled: true },
      { separator: true, label: "" },
      { label: "Design Rule Check...", disabled: true },
      { label: "Electrical Rules Check...", disabled: true },
    ],
  },
  {
    label: "Help",
    items: [
      { label: "Documentation" },
      { label: "Keyboard Shortcuts (F1)" },
      { separator: true, label: "" },
      { label: "About Signex" },
    ],
  },
];

export function MenuBar({ onOpenProject, onSave, onOpenComponentSearch }: MenuBarProps) {
  const [openMenu, setOpenMenu] = useState<number | null>(null);
  const menuBarRef = useRef<HTMLDivElement>(null);

  // Wire up actions
  const actionMenus = menus.map((menu) => ({
    ...menu,
    items: menu.items.map((item) => {
      if (item.id === "file-open") return { ...item, action: onOpenProject };

      // File
      if (item.label === "Save") return { ...item, disabled: false, action: onSave };
      // Edit
      if (item.label === "Undo") return { ...item, disabled: false, action: () => useSchematicStore.getState().undo() };
      if (item.label === "Redo") return { ...item, disabled: false, action: () => useSchematicStore.getState().redo() };
      if (item.label === "Cut") return { ...item, disabled: false, action: () => { useSchematicStore.getState().copySelected(); useSchematicStore.getState().deleteSelected(); } };
      if (item.label === "Copy") return { ...item, disabled: false, action: () => useSchematicStore.getState().copySelected() };
      if (item.label === "Paste") return { ...item, disabled: false, action: () => useSchematicStore.getState().pasteClipboard({ x: 5, y: 5 }) };
      if (item.label === "Duplicate") return { ...item, disabled: false, action: () => useSchematicStore.getState().duplicateSelected() };
      if (item.label === "Delete") return { ...item, disabled: false, action: () => useSchematicStore.getState().deleteSelected() };
      if (item.label === "Select All") return { ...item, disabled: false, action: () => useSchematicStore.getState().selectAll() };
      if (item.label === "Find...") return { ...item, disabled: false, action: () => window.dispatchEvent(new KeyboardEvent("keydown", { key: "f", ctrlKey: true })) };
      if (item.label === "Find and Replace...") return { ...item, disabled: false, action: () => window.dispatchEvent(new KeyboardEvent("keydown", { key: "h", ctrlKey: true })) };
      // Edit > Align
      if (item.label === "Align Left") return { ...item, disabled: false, action: () => useSchematicStore.getState().alignSelected("left") };
      if (item.label === "Align Right") return { ...item, disabled: false, action: () => useSchematicStore.getState().alignSelected("right") };
      if (item.label === "Align Top") return { ...item, disabled: false, action: () => useSchematicStore.getState().alignSelected("top") };
      if (item.label === "Align Bottom") return { ...item, disabled: false, action: () => useSchematicStore.getState().alignSelected("bottom") };
      if (item.label === "Distribute Horizontally") return { ...item, disabled: false, action: () => useSchematicStore.getState().distributeSelected("horizontal") };
      if (item.label === "Distribute Vertically") return { ...item, disabled: false, action: () => useSchematicStore.getState().distributeSelected("vertical") };
      // View
      if (item.label === "Toggle Visible Grid") return { ...item, action: () => useEditorStore.getState().toggleGrid() };
      if (item.label === "Set Snap Grid") return { ...item, action: () => window.dispatchEvent(new KeyboardEvent("keydown", { key: "g" })) };
      if (item.label === "Fit Document") return { ...item, action: () => window.dispatchEvent(new KeyboardEvent("keydown", { key: "Home" })) };
      if (item.label === "Toggle Units") return { ...item, action: () => {
        const u = useEditorStore.getState().statusBar.units;
        useEditorStore.getState().updateStatusBar({ units: u === "mm" ? "mil" : u === "mil" ? "inch" : "mm" });
      }};
      // Place
      if (item.label === "Wire") return { ...item, disabled: false, action: () => useSchematicStore.getState().setEditMode("drawWire") };
      if (item.label === "Part...") return { ...item, disabled: false, action: onOpenComponentSearch };
      if (item.label === "Net Label") return { ...item, disabled: false, action: () => useSchematicStore.getState().setEditMode("placeLabel") };
      if (item.label === "Power Port") return { ...item, disabled: false, action: () => useSchematicStore.getState().setEditMode("placePower") };
      if (item.label === "No Connect") return { ...item, disabled: false, action: () => useSchematicStore.getState().setEditMode("placeNoConnect") };
      // Junction is auto-only (Altium behavior) — no manual placement mode
      if (item.label === "Port") return { ...item, disabled: false, action: () => useSchematicStore.getState().setEditMode("placePort") };
      if (item.label === "Text String") return { ...item, disabled: false, action: () => useSchematicStore.getState().setEditMode("placeText") };
      if (item.label === "Bus" && menu.label === "Place") return { ...item, disabled: false, action: () => useSchematicStore.getState().setEditMode("drawBus") };
      if (item.label === "Line") return { ...item, disabled: false, action: () => useSchematicStore.getState().setEditMode("drawLine") };
      if (item.label === "Rectangle" && menu.label === "Place") return { ...item, disabled: false, action: () => useSchematicStore.getState().setEditMode("drawRect") };
      // Tools
      if (item.label === "Measure Distance") return { ...item, disabled: false, action: () => useSchematicStore.getState().setEditMode("measure") };
      // Reports / Output
      if (item.label === "Bill of Materials...") return { ...item, disabled: false, action: async () => {
        const data = useSchematicStore.getState().data;
        if (!data) return;
        try {
          const { invoke } = await import("@tauri-apps/api/core");
          const csv = await invoke<string>("generate_bom", { data });
          // Download as file
          const blob = new Blob([csv], { type: "text/csv" });
          const url = URL.createObjectURL(blob);
          const a = document.createElement("a"); a.href = url; a.download = "bom.csv"; a.click();
          URL.revokeObjectURL(url);
        } catch (e) { console.error("BOM generation failed:", e); }
      }};
      if (item.label === "Export as PNG...") return { ...item, disabled: false, action: () => {
        // Trigger export via custom event — renderer handles it
        window.dispatchEvent(new CustomEvent("alp-export-png"));
      }};
      if (item.label === "Electrical Rules Check...") return { ...item, disabled: false, action: () => {
        /* ERC runs from Messages panel */
      }};
      // Design / Tools
      if (item.label === "Annotate Schematics..." && menu.label === "Design") return { ...item, disabled: false, action: () => useSchematicStore.getState().annotateAll() };
      if (item.label === "Annotate Schematics..." && menu.label === "Tools") return { ...item, disabled: false, action: () => useSchematicStore.getState().annotateAll() };

      return item;
    }),
  }));

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
      className="flex h-8 items-center bg-bg-tertiary border-b border-border-subtle px-1 text-[12.5px]"
    >
      {actionMenus.map((menu, idx) => (
        <div key={menu.label} className="relative">
          <button
            className={cn(
              "px-3 py-1.5 rounded hover:bg-bg-hover hover:text-text-primary text-text-secondary transition-colors",
              openMenu === idx && "bg-bg-hover text-text-primary"
            )}
            onClick={() => setOpenMenu(openMenu === idx ? null : idx)}
            onMouseEnter={() => openMenu !== null && setOpenMenu(idx)}
          >
            {menu.label}
          </button>
          {openMenu === idx && (
            <div className="absolute left-0 top-full mt-0.5 min-w-[260px] bg-bg-surface border border-border rounded-lg shadow-2xl shadow-black/40 z-50 py-1.5 max-h-[80vh] overflow-y-auto">
              {menu.items.map((item, iIdx) =>
                item.separator ? (
                  <div key={iIdx} className="h-px bg-border-subtle mx-3 my-1.5" />
                ) : (
                  <button
                    key={iIdx}
                    disabled={item.disabled}
                    className={cn(
                      "w-full flex items-center justify-between px-4 py-[5px] text-[12.5px] text-left transition-colors",
                      item.disabled
                        ? "text-text-muted/50 cursor-default"
                        : "text-text-secondary hover:bg-accent/15 hover:text-text-primary"
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
                      <span className="text-text-muted/60 ml-8 text-[11px] font-mono">
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
