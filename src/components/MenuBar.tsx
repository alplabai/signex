import { useState, useRef, useEffect } from "react";
import { cn } from "@/lib/utils";
import { useSchematicStore } from "@/stores/schematic";
import { useEditorStore } from "@/stores/editor";
import { toggleCrossSelect } from "@/lib/crossProbe";

interface MenuBarProps {
  onOpenProject?: () => void;
  onSave?: () => void;
  onOpenComponentSearch?: () => void;
  onExportPdf?: () => void;
  onExportBom?: () => void;
  onExportNetlist?: () => void;
  onOpenOutputJobs?: () => void;
  onAnnotate?: () => void;
  onPreferences?: () => void;
  onFindSimilar?: () => void;
  onParameterManager?: () => void;
  onPrint?: () => void;
  onRunDrc?: () => void;
  onBackAnnotate?: () => void;
  onErcMatrix?: () => void;
  onConstraints?: () => void;
  onViaStitching?: () => void;
  onBgaFanout?: () => void;
  isPcbView?: boolean;
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
      { label: "Export as PDF...", disabled: true },
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
      { label: "No ERC", disabled: false },
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
      { label: "Parameter Manager...", disabled: true },
      { label: "Back Annotate...", disabled: true },
      { label: "Number Schematic Sheets...", disabled: true },
      { separator: true, label: "" },
      { label: "Cross Reference...", disabled: true },
      { label: "Cross Select Mode", shortcut: "Shift+Ctrl+X", disabled: true },
      { separator: true, label: "" },
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
      { label: "Export Netlist...", disabled: true },
      { label: "Component Cross Reference...", disabled: true },
      { separator: true, label: "" },
      { label: "Design Rule Check...", disabled: true },
      { label: "Electrical Rules Check...", disabled: true },
      { separator: true, label: "" },
      { label: "Output Jobs...", disabled: true },
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

// PCB-specific menu overrides for Place, Design, Tools
const pcbPlaceMenu: MenuGroup = {
  label: "Place",
  items: [
    { label: "Route Track", shortcut: "X" },
    { label: "Differential Pair Route" },
    { label: "Multi-Track Route" },
    { separator: true, label: "" },
    { label: "Via" },
    { label: "Footprint" },
    { separator: true, label: "" },
    { label: "Zone" },
    { label: "Keepout" },
    { label: "Board Outline" },
    { separator: true, label: "" },
    { label: "Text" },
    { label: "Line" },
    { label: "Dimension" },
  ],
};

const pcbDesignMenu: MenuGroup = {
  label: "Design",
  items: [
    { label: "Design Rules...", disabled: true },
    { label: "Board Setup...", disabled: true },
    { label: "Layer Stack Manager..." },
    { separator: true, label: "" },
    { label: "Import Changes From Schematic...", disabled: true },
    { label: "Back Annotate to Schematic...", disabled: true },
  ],
};

const pcbToolsMenu: MenuGroup = {
  label: "Tools",
  items: [
    { label: "Design Rule Check..." },
    { separator: true, label: "" },
    { label: "Fill All Zones" },
    { label: "Remove Dead Copper", disabled: true },
    { separator: true, label: "" },
    { label: "Via Stitching..." },
    { label: "BGA Fanout..." },
    { label: "Generate Teardrops" },
    { label: "Length Tuning" },
    { separator: true, label: "" },
    { label: "Cross Select Mode", shortcut: "Shift+Ctrl+X" },
    { separator: true, label: "" },
    { label: "Preferences..." },
  ],
};

export function MenuBar({ onOpenProject, onSave, onOpenComponentSearch, onExportPdf, onExportBom, onExportNetlist, onOpenOutputJobs, onAnnotate, onPreferences, onFindSimilar, onParameterManager, onPrint, onRunDrc, onBackAnnotate, onErcMatrix, onConstraints, onViaStitching, onBgaFanout, isPcbView }: MenuBarProps) {
  const [openMenu, setOpenMenu] = useState<number | null>(null);
  const menuBarRef = useRef<HTMLDivElement>(null);

  // Swap Place/Design/Tools menus when in PCB view
  const baseMenus = isPcbView
    ? menus.map((m) =>
        m.label === "Place" ? pcbPlaceMenu
        : m.label === "Design" ? pcbDesignMenu
        : m.label === "Tools" ? pcbToolsMenu
        : m)
    : menus;

  // Wire up actions
  const actionMenus = baseMenus.map((menu) => ({
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
      if (item.label === "Arc") return { ...item, disabled: false, action: () => useSchematicStore.getState().setEditMode("drawCircle") };
      if (item.label === "Ellipse") return { ...item, disabled: false, label: "Circle", action: () => useSchematicStore.getState().setEditMode("drawCircle") };
      if (item.label === "Polygon") return { ...item, disabled: false, label: "Polyline", action: () => useSchematicStore.getState().setEditMode("drawPolyline") };
      // Tools
      // Reports / Output
      if (item.label === "Bill of Materials...") return { ...item, disabled: false, action: onExportBom };
      if (item.label === "Export Netlist...") return { ...item, disabled: false, action: onExportNetlist };
      if (item.label === "Output Jobs...") return { ...item, disabled: false, action: onOpenOutputJobs };
      if (item.label === "Export as PNG...") return { ...item, disabled: false, action: () => {
        // Trigger export via custom event — renderer handles it
        window.dispatchEvent(new CustomEvent("alp-export-png"));
      }};
      if (item.label === "Export as PDF...") return { ...item, disabled: false, action: onExportPdf };
      if (item.label === "Print...") return { ...item, disabled: false, action: onPrint };
      if (item.label === "Electrical Rules Check...") return { ...item, disabled: false, action: onErcMatrix || (() => {}) };
      if (item.label === "Design Rule Check...") return { ...item, disabled: false, action: onRunDrc };
      if (item.label === "Back Annotate...") return { ...item, disabled: false, action: onBackAnnotate };
      // Design / Tools
      if (item.label === "Annotate Schematics..." && menu.label === "Design") return { ...item, disabled: false, action: onAnnotate };
      if (item.label === "Annotate Schematics..." && menu.label === "Tools") return { ...item, disabled: false, action: onAnnotate };
      if (item.label === "Preferences...") return { ...item, disabled: false, action: onPreferences };
      if (item.label === "Find Similar Objects") return { ...item, disabled: false, action: onFindSimilar };
      if (item.label === "Parameter Manager...") return { ...item, disabled: false, action: onParameterManager };
      if (item.label === "Reset Designators") return { ...item, disabled: false, action: () => useSchematicStore.getState().resetDesignators() };
      if (item.label === "Reset Duplicate Designators") return { ...item, disabled: false, action: () => useSchematicStore.getState().resetDuplicateDesignators() };
      if (item.label === "Sheet Symbol...") return { ...item, disabled: false, action: () => useSchematicStore.getState().setEditMode("placeSheetSymbol") };
      if (item.label === "Bus Entry") return { ...item, disabled: false, action: () => useSchematicStore.getState().setEditMode("placeBusEntry") };
      if (item.label === "No ERC") return { ...item, disabled: false, action: () => useSchematicStore.getState().setEditMode("placeNoErc") };
      if (item.label === "Break Wire") return { ...item, disabled: false, action: () => {
        // Break wire at midpoint of selected wire
        const store = useSchematicStore.getState();
        if (store.selectedIds.size === 1 && store.data) {
          const uuid = [...store.selectedIds][0];
          const wire = store.data.wires.find((w) => w.uuid === uuid);
          if (wire) {
            const mid = { x: (wire.start.x + wire.end.x) / 2, y: (wire.start.y + wire.end.y) / 2 };
            store.breakWireAt(uuid, mid);
          }
        }
      }};
      if (item.label === "Align to Grid") return { ...item, disabled: false, action: () => useSchematicStore.getState().alignSelectionToGrid() };
      if (item.label === "Smart Paste...") return { ...item, disabled: false, action: () => useSchematicStore.getState().smartPaste({ x: 2.54, y: 2.54 }) };
      // Place menu — remaining items
      if (item.label === "Text Frame") return { ...item, disabled: false, action: () => useSchematicStore.getState().setEditMode("placeTextFrame" as any) };
      if (item.label === "Note") return { ...item, disabled: false, action: () => useSchematicStore.getState().setEditMode("placeNote" as any) };
      if (item.label === "Image...") return { ...item, disabled: false, action: () => useSchematicStore.getState().setEditMode("placeImage" as any) };
      if (item.label === "Sheet Entry") return { ...item, disabled: false, action: () => useSchematicStore.getState().setEditMode("placeSheetEntry" as any) };
      if (item.label === "Directive") return { ...item, disabled: false, action: () => useSchematicStore.getState().setEditMode("placeParameterSet" as any) };
      if (item.label === "Junction") return { ...item, disabled: false, action: () => useSchematicStore.getState().setEditMode("placeJunction" as any) };
      // View menu
      if (item.label === "Fit All Objects") return { ...item, disabled: false, action: () => window.dispatchEvent(new KeyboardEvent("keydown", { key: "Home" })) };
      if (item.label === "Toggle Net Color Override") return { ...item, disabled: false, action: () => {} };
      if (item.label === "Navigator") return { ...item, disabled: false, action: () => {
        // Handled from App.tsx via layout store
      }};
      // Design menu
      if (item.label === "Update PCB Document...") return { ...item, disabled: false, action: () => {} };
      if (item.label === "Import Changes From PCB...") return { ...item, disabled: false, action: onBackAnnotate };
      if (item.label === "Document Options...") return { ...item, disabled: false, action: onConstraints };
      // Tools menu
      if (item.label === "Cross Select Mode") return { ...item, disabled: false, action: () => toggleCrossSelect() };
      if (item.label === "Component Cross Reference...") return { ...item, disabled: false, action: () => {} };
      if (item.label === "Number Schematic Sheets...") return { ...item, disabled: false, action: () => {} };
      if (item.label === "Cross Reference...") return { ...item, disabled: false, action: () => {} };
      if (item.label === "Signal (AI)") return { ...item, action: () => {
        // Open Signal panel — handled via layout store from App
      }};
      // PCB-specific Place menu actions
      if (item.label === "Route Track" && isPcbView) return { ...item, action: () => { import("@/stores/pcb").then(m => m.usePcbStore.getState().setEditMode("routeTrack")); } };
      if (item.label === "Differential Pair Route") return { ...item, action: () => { import("@/stores/pcb").then(m => m.usePcbStore.getState().setEditMode("routeDiffPair")); } };
      if (item.label === "Multi-Track Route") return { ...item, action: () => { import("@/stores/pcb").then(m => m.usePcbStore.getState().setEditMode("routeMultiTrack")); } };
      if (item.label === "Via" && isPcbView) return { ...item, action: () => { import("@/stores/pcb").then(m => m.usePcbStore.getState().setEditMode("placeVia")); } };
      if (item.label === "Footprint" && isPcbView) return { ...item, action: () => { import("@/stores/pcb").then(m => m.usePcbStore.getState().setEditMode("placeFootprint")); } };
      if (item.label === "Zone") return { ...item, action: () => { import("@/stores/pcb").then(m => m.usePcbStore.getState().setEditMode("placeZone")); } };
      if (item.label === "Keepout") return { ...item, action: () => { import("@/stores/pcb").then(m => m.usePcbStore.getState().setEditMode("placeKeepout")); } };
      if (item.label === "Board Outline") return { ...item, action: () => { import("@/stores/pcb").then(m => m.usePcbStore.getState().setEditMode("drawBoardOutline")); } };
      if (item.label === "Text" && isPcbView) return { ...item, action: () => { import("@/stores/pcb").then(m => m.usePcbStore.getState().setEditMode("placeText")); } };
      if (item.label === "Dimension") return { ...item, action: () => { import("@/stores/pcb").then(m => m.usePcbStore.getState().setEditMode("placeDimension")); } };
      // PCB-specific Tools menu
      if (item.label === "Fill All Zones") return { ...item, action: () => {
        Promise.all([import("@/lib/pcbCopperPour"), import("@/stores/pcb")]).then(([cpMod, pcbMod]) => {
          const store = pcbMod.usePcbStore.getState();
          if (!store.data) return;
          store.pushUndo();
          const nd = structuredClone(store.data);
          cpMod.fillZones(nd);
          pcbMod.usePcbStore.setState({ data: nd, dirty: true });
        });
      }};
      if (item.label === "Via Stitching...") return { ...item, action: onViaStitching };
      if (item.label === "BGA Fanout...") return { ...item, action: onBgaFanout };
      if (item.label === "Generate Teardrops" && isPcbView) return { ...item, action: () => {
        Promise.all([import("@/lib/pcbRouter"), import("@/stores/pcb")]).then(([rtMod, pcbMod]) => {
          const store = pcbMod.usePcbStore.getState();
          if (!store.data) return;
          store.pushUndo();
          const nd = structuredClone(store.data);
          const newSegs = rtMod.generateTeardrops(nd, 0.5, 0.5);
          nd.segments = [...nd.segments, ...newSegs];
          pcbMod.usePcbStore.setState({ data: nd, dirty: true });
        });
      }};
      if (item.label === "Length Tuning") return { ...item, action: () => { import("@/stores/pcb").then(m => m.usePcbStore.getState().setEditMode("lengthTune")); } };
      if (item.label === "Layer Stack Manager...") return { ...item, action: () => {} };
      if (item.label === "BGA Fanout..." && !onBgaFanout) return item; // suppress unused warning

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
