import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Search, Package, Cpu, ChevronRight, Pencil, Copy, FilePlus, Star, FolderOpen, Menu, RefreshCw } from "lucide-react";
import { cn } from "@/lib/utils";
import { useSchematicStore } from "@/stores/schematic";
import { useLibraryEditorStore } from "@/stores/libraryEditor";
import type { LibSymbol, SymbolSearchResult, LibraryInfo } from "@/types";

/** Footprint data returned from Rust pcb_parser (snake_case field names) */
interface RustFootprintData {
  uuid: string;
  reference: string;
  value: string;
  footprint_id: string;
  position: { x: number; y: number };
  rotation: number;
  layer: string;
  locked: boolean;
  pads: RustPadData[];
  graphics: RustFpGraphic[];
}

interface RustPadData {
  uuid: string;
  number: string;
  pad_type: string;
  shape: string;
  position: { x: number; y: number };
  size: [number, number];
  drill?: { diameter: number; shape?: string };
  layers: string[];
  net?: { number: number; name: string };
  roundrect_ratio?: number;
}

interface RustFpGraphic {
  graphic_type: string;
  layer: string;
  width: number;
  start?: { x: number; y: number };
  end?: { x: number; y: number };
  center?: { x: number; y: number };
  mid?: { x: number; y: number };
  radius?: number;
  points: { x: number; y: number }[];
  text?: string;
  font_size?: number;
  position?: { x: number; y: number };
  rotation?: number;
  fill?: boolean;
}

/** Collapsible section header matching Altium's panel style */
function SectionHeader({ title, expanded, onToggle, className }: {
  title: string;
  expanded: boolean;
  onToggle: () => void;
  className?: string;
}) {
  return (
    <button
      className={cn(
        "w-full flex items-center gap-1.5 px-2 py-1 text-[10px] font-semibold text-text-secondary",
        "bg-bg-tertiary/80 border-b border-border-subtle/50 hover:bg-bg-tertiary transition-colors",
        className
      )}
      onClick={onToggle}
    >
      <span className="text-[8px] text-text-muted/60">{expanded ? "\u25BC" : "\u25B6"}</span>
      {title}
    </button>
  );
}

export function ComponentPanel() {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SymbolSearchResult[]>([]);
  const [libraries, setLibraries] = useState<LibraryInfo[]>([]);
  const [expandedLib, setExpandedLib] = useState<string | null>(null);
  const [libSymbols, setLibSymbols] = useState<SymbolSearchResult[]>([]);
  const [loading, setLoading] = useState(false);
  const [preview, setPreview] = useState<LibSymbol | null>(null);
  const [selectedResult, setSelectedResult] = useState<SymbolSearchResult | null>(null);
  const [showModelsSection, setShowModelsSection] = useState(true);
  const [detailsHeight, setDetailsHeight] = useState(280);
  const [menuOpen, setMenuOpen] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout>>(undefined);
  const menuRef = useRef<HTMLDivElement>(null);

  // Close menu on outside click
  useEffect(() => {
    if (!menuOpen) return;
    const handler = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setMenuOpen(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [menuOpen]);

  // Load library list on mount
  const fetchLibraries = useCallback(() => {
    invoke<LibraryInfo[]>("list_libraries")
      .then(setLibraries)
      .catch(() => {});
  }, []);

  useEffect(() => { fetchLibraries(); }, [fetchLibraries]);

  // Debounced search
  const search = useCallback((q: string) => {
    if (debounceRef.current) clearTimeout(debounceRef.current);
    if (q.length < 2) { setResults([]); return; }
    setLoading(true);
    debounceRef.current = setTimeout(async () => {
      try {
        const res = await invoke<SymbolSearchResult[]>("search_symbols", { query: q, limit: 80 });
        setResults(res);
      } catch { setResults([]); }
      finally { setLoading(false); }
    }, 250);
  }, []);

  useEffect(() => { search(query); }, [query, search]);

  // Expand library to show all symbols
  const libCacheRef = useRef<Record<string, SymbolSearchResult[]>>({});
  const expandLibrary = async (lib: LibraryInfo) => {
    if (expandedLib === lib.name) { setExpandedLib(null); return; }
    setExpandedLib(lib.name);
    // Use cache if available
    if (libCacheRef.current[lib.name]) {
      setLibSymbols(libCacheRef.current[lib.name]);
      return;
    }
    try {
      const res = await invoke<SymbolSearchResult[]>("list_library_symbols", { libraryName: lib.name });
      libCacheRef.current[lib.name] = res;
      setLibSymbols(res);
    } catch { setLibSymbols([]); }
  };

  const loadPreview = async (result: SymbolSearchResult) => {
    setSelectedResult(result);
    const lib = libraries.find((l) => l.name === result.library);
    if (!lib) return;
    try {
      const sym = await invoke<LibSymbol>("get_symbol", { libraryPath: lib.path, symbolId: result.symbol_id });
      setPreview(sym);
    } catch { setPreview(null); }
  };

  const editSymbol = () => {
    if (!preview || !selectedResult) return;
    const lib = libraries.find((l) => l.name === selectedResult.library);
    if (!lib) return;
    useLibraryEditorStore.getState().openSymbol(preview, lib.path, selectedResult.symbol_id);
  };

  const duplicateSymbol = () => {
    if (!preview || !selectedResult) return;
    const lib = libraries.find((l) => l.name === selectedResult.library);
    if (!lib) return;
    const cloned = structuredClone(preview);
    cloned.id = cloned.id + "_copy";
    useLibraryEditorStore.getState().openSymbol(cloned, lib.path, cloned.id);
  };

  const newSymbol = () => {
    const emptySymbol: LibSymbol = {
      id: "NewSymbol",
      graphics: [{ type: "Rectangle", start: { x: -2.54, y: -5.08 }, end: { x: 2.54, y: 5.08 }, width: 0.254, fill: false }],
      pins: [
        { pin_type: "passive", shape: "line", position: { x: -5.08, y: 2.54 }, rotation: 0, length: 2.54, name: "1", number: "1", name_visible: true, number_visible: true },
        { pin_type: "passive", shape: "line", position: { x: -5.08, y: -2.54 }, rotation: 0, length: 2.54, name: "2", number: "2", name_visible: true, number_visible: true },
      ],
      show_pin_numbers: true,
      show_pin_names: true,
      pin_name_offset: 1.016,
    };
    useLibraryEditorStore.getState().openSymbol(emptySymbol, "user_library.snxsym", "NewSymbol");
  };

  const placeComponent = async (result: SymbolSearchResult) => {
    let sym = preview;
    if (!sym || selectedResult?.symbol_id !== result.symbol_id) {
      const lib = libraries.find((l) => l.name === result.library);
      if (!lib) return;
      try {
        sym = await invoke<LibSymbol>("get_symbol", { libraryPath: lib.path, symbolId: result.symbol_id });
      } catch { return; }
    }
    useSchematicStore.getState().startPlacement(sym, result);
  };

  const isSearching = query.length >= 2;
  const displayResults = isSearching ? results : [];

  const [selectedLib, setSelectedLib] = useState<string>("all");

  // Filter results by selected library
  const filteredResults = selectedLib === "all" ? displayResults : displayResults.filter(r => r.library === selectedLib);

  const handleRefresh = () => {
    libCacheRef.current = {};
    fetchLibraries();
    if (query.length >= 2) search(query);
    setMenuOpen(false);
  };

  return (
    <div className="flex flex-col h-full">
      {/* Library selector dropdown with menu button */}
      <div className="flex items-center gap-1.5 px-2 py-1 border-b border-border-subtle">
        <FolderOpen size={12} className="text-accent/60 shrink-0" />
        <select value={selectedLib} onChange={(e) => setSelectedLib(e.target.value)}
          className="flex-1 bg-bg-surface border border-border-subtle rounded px-1.5 py-0.5 text-[10px] text-text-primary outline-none truncate">
          <option value="all">All Libraries</option>
          {libraries.map(lib => (
            <option key={lib.name} value={lib.name}>
              {lib.name}
            </option>
          ))}
        </select>
        {/* Menu button */}
        <div className="relative" ref={menuRef}>
          <button
            onClick={() => setMenuOpen(!menuOpen)}
            className="p-1 rounded text-text-muted/40 hover:text-text-secondary hover:bg-bg-hover transition-colors shrink-0"
            title="Panel options"
          >
            <Menu size={13} />
          </button>
          {menuOpen && (
            <div className="absolute right-0 top-full mt-0.5 z-50 bg-bg-surface border border-border-subtle rounded shadow-lg min-w-[200px] py-0.5">
              <button
                disabled
                className="w-full text-left px-3 py-1.5 text-[10px] text-text-muted/30 cursor-not-allowed"
              >
                Import Library...
              </button>
              <button
                onClick={() => { setShowModelsSection(!showModelsSection); setMenuOpen(false); }}
                className="w-full text-left px-3 py-1.5 text-[10px] text-text-secondary hover:bg-bg-hover transition-colors flex items-center gap-2"
              >
                <span className="w-3 text-center text-accent">{showModelsSection ? "\u2713" : ""}</span>
                Models
              </button>
              <div className="border-t border-border-subtle/40 my-0.5" />
              <button
                disabled
                className="w-full text-left px-3 py-1.5 text-[10px] text-text-muted/30 cursor-not-allowed"
              >
                Libraries Preferences...
              </button>
              <button
                disabled
                className="w-full text-left px-3 py-1.5 text-[10px] text-text-muted/30 cursor-not-allowed"
              >
                File-based Libraries Search...
              </button>
              <div className="border-t border-border-subtle/40 my-0.5" />
              <button
                onClick={handleRefresh}
                className="w-full text-left px-3 py-1.5 text-[10px] text-text-secondary hover:bg-bg-hover transition-colors flex items-center gap-2"
              >
                <RefreshCw size={10} className="shrink-0" />
                Refresh
                <span className="ml-auto text-text-muted/40 text-[9px]">F5</span>
              </button>
            </div>
          )}
        </div>
      </div>

      {/* Search bar */}
      <div className="flex items-center gap-2 px-2 py-1.5 border-b border-border-subtle">
        <Search size={13} className="text-text-muted/40 shrink-0" />
        <input
          ref={inputRef}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search..."
          className="flex-1 bg-transparent text-[11px] text-text-primary placeholder:text-text-muted/30 outline-none"
        />
        {loading && <div className="w-3 h-3 border-2 border-accent/40 border-t-accent rounded-full animate-spin" />}
        <button onClick={newSymbol} title="New Symbol"
          className="p-1 rounded text-text-muted/40 hover:text-accent hover:bg-accent/10 transition-colors shrink-0">
          <FilePlus size={13} />
        </button>
      </div>

      {/* "Drag a column header here to group" placeholder */}
      {isSearching && filteredResults.length > 0 && (
        <div className="px-2 py-[3px] bg-bg-tertiary/40 border-b border-border-subtle/40 text-center">
          <span className="text-[8px] text-text-muted/30 italic select-none">
            Drag a column header here to group by that column
          </span>
        </div>
      )}

      {/* Results table with sticky column headers */}
      {isSearching && filteredResults.length > 0 && (
        <div className="flex-1 overflow-auto min-h-0">
          <table className="w-full min-w-[380px] border-collapse">
            <thead className="sticky top-0 z-10">
              <tr className="bg-bg-tertiary border-b border-border-subtle/60">
                <th className="text-left px-2 py-[3px] text-[9px] text-text-muted/50 uppercase tracking-wider font-semibold border-r border-border-subtle/30 whitespace-nowrap">
                  Part Number
                </th>
                <th className="text-left px-2 py-[3px] text-[9px] text-text-muted/50 uppercase tracking-wider font-semibold border-r border-border-subtle/30 whitespace-nowrap w-[140px]">
                  Description
                </th>
                <th className="text-right px-2 py-[3px] text-[9px] text-text-muted/50 uppercase tracking-wider font-semibold whitespace-nowrap w-[90px]">
                  Library Ref
                </th>
              </tr>
            </thead>
            <tbody>
              {filteredResults.map((r) => (
                <tr
                  key={`${r.library}:${r.symbol_id}`}
                  draggable
                  onDragStart={(e) => {
                    e.dataTransfer.setData("application/signex-symbol", JSON.stringify(r));
                    e.dataTransfer.effectAllowed = "copy";
                  }}
                  className={cn(
                    "cursor-pointer transition-colors border-b border-border-subtle/20",
                    selectedResult?.symbol_id === r.symbol_id && selectedResult?.library === r.library
                      ? "bg-accent/10" : "hover:bg-bg-hover"
                  )}
                  onClick={() => loadPreview(r)}
                  onDoubleClick={() => placeComponent(r)}
                >
                  <td className="px-2 py-1">
                    <div className="flex items-center gap-1.5 min-w-0">
                      <Cpu size={11} className="shrink-0 text-accent/50" />
                      <span className="text-[10px] font-medium truncate text-text-primary">{r.symbol_id}</span>
                    </div>
                  </td>
                  <td className="px-2 py-1">
                    <span className="text-[10px] text-text-muted/60 truncate block">{r.description || "\u2014"}</span>
                  </td>
                  <td className="px-2 py-1 text-right">
                    <span className="text-[10px] text-text-muted/40 truncate block">{r.library}</span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {/* Results count */}
      {isSearching && filteredResults.length > 0 && (
        <div className="px-2 py-[3px] border-t border-border-subtle/50 bg-bg-tertiary/40 text-[9px] text-text-muted/50 shrink-0">
          Results: {filteredResults.length}
        </div>
      )}

      {/* No results or library tree (only when not searching with results) */}
      {isSearching && filteredResults.length === 0 && !loading && (
        <div className="flex-1 overflow-y-auto">
          <div className="text-center py-6 text-text-muted/40 text-[11px]">
            No results
            <div className="mt-2 text-accent/60 text-[10px]">Need more components?<br/>Find in Manufacturer Part Search</div>
          </div>
        </div>
      )}

      {!isSearching && (
        <div className="flex-1 overflow-y-auto">
          {libraries.length === 0 ? (
            <div className="text-center py-6 text-text-muted/40 text-[11px]">
              <Package size={24} className="mx-auto mb-2 opacity-20" />
              Loading libraries...
            </div>
          ) : (
            libraries.map((lib) => (
              <div key={lib.name}>
                <button
                  className="w-full flex items-center gap-1.5 px-2 py-1 text-left hover:bg-bg-hover transition-colors text-[11px]"
                  onClick={() => expandLibrary(lib)}
                >
                  <ChevronRight size={10} className={cn("text-text-muted/40 transition-transform", expandedLib === lib.name && "rotate-90")} />
                  <FolderOpen size={11} className="text-yellow-600/60 shrink-0" />
                  <span className="text-text-secondary truncate">{lib.name}</span>
                </button>
                {expandedLib === lib.name && libSymbols.map((r) => (
                  <button
                    key={r.symbol_id}
                    className={cn(
                      "w-full flex items-center gap-2 pl-7 pr-2 py-1 text-left transition-colors text-[10px]",
                      selectedResult?.symbol_id === r.symbol_id ? "bg-accent/10" : "hover:bg-bg-hover"
                    )}
                    onClick={() => loadPreview(r)}
                    onDoubleClick={() => placeComponent(r)}
                  >
                    <Cpu size={10} className="shrink-0 text-accent/40" />
                    <span className="text-text-primary truncate">{r.symbol_id}</span>
                    <span className="text-text-muted/30 ml-auto shrink-0">{r.pin_count}p</span>
                  </button>
                ))}
              </div>
            ))
          )}
        </div>
      )}

      {/* Resize handle + Details/Models — always at bottom (like Altium) */}
      {preview && selectedResult && (
        <>
          <div
            className="h-[5px] bg-border-subtle/30 hover:bg-accent/30 active:bg-accent/50 cursor-row-resize shrink-0 flex items-center justify-center"
            onMouseDown={(e) => {
              e.preventDefault();
              const startY = e.clientY;
              const startH = detailsHeight;
              const maxH = window.innerHeight - 200;
              const onMove = (ev: MouseEvent) => {
                const delta = startY - ev.clientY;
                setDetailsHeight(Math.max(60, Math.min(maxH, startH + delta)));
              };
              const onUp = () => {
                window.removeEventListener("mousemove", onMove);
                window.removeEventListener("mouseup", onUp);
              };
              window.addEventListener("mousemove", onMove);
              window.addEventListener("mouseup", onUp);
            }}
          >
            <div className="w-8 h-[2px] bg-border-subtle/60 rounded-full" />
          </div>
          <div className="overflow-y-auto shrink-0" style={{ height: detailsHeight }}>
            <ComponentDetailSections
              preview={preview}
              selectedResult={selectedResult}
              onEdit={editSymbol}
              onDuplicate={duplicateSymbol}
              onPlace={() => placeComponent(selectedResult)}
              showModels={showModelsSection}
            />
          </div>
        </>
      )}
    </div>
  );
}

/** Altium-style component detail sections with collapsible panels */
function ComponentDetailSections({
  preview,
  selectedResult,
  onEdit,
  onDuplicate,
  onPlace,
  showModels,
}: {
  preview: LibSymbol;
  selectedResult: SymbolSearchResult;
  onEdit: () => void;
  onDuplicate: () => void;
  onPlace: () => void;
  showModels: boolean;
}) {
  const [detailsOpen, setDetailsOpen] = useState(true);
  const [showAllDetails, setShowAllDetails] = useState(false);
  const [modelsOpen, setModelsOpen] = useState(true);
  const [referencesOpen, setReferencesOpen] = useState(false);
  const [partChoicesOpen, setPartChoicesOpen] = useState(false);

  const coreDetails = [
    { label: "Library Path", value: selectedResult.library },
    { label: "Library Ref", value: selectedResult.symbol_id },
    { label: "Description", value: selectedResult.description || "\u2014" },
    { label: "Keywords", value: selectedResult.keywords?.length ? selectedResult.keywords.join(", ") : "\u2014" },
    { label: "Prefix", value: selectedResult.reference_prefix || "?" },
    { label: "Pins", value: String(selectedResult.pin_count) },
  ];

  const extraDetails = [
    { label: "Manufacturer", value: "\u2014" },
    { label: "Part Number", value: selectedResult.symbol_id },
    { label: "PartId", value: "1" },
    { label: "Supplier 1", value: "\u2014" },
    { label: "Supplier Part No 1", value: "\u2014" },
  ];

  const visibleDetails = showAllDetails ? [...coreDetails, ...extraDetails] : coreDetails;

  return (
    <div className="border-b border-border-subtle bg-bg-surface/50">
      {/* Action bar */}
      <div className="px-2 py-1 flex items-center gap-1 border-b border-border-subtle/50">
        <div className="text-[10px] text-text-muted truncate flex-1">
          {selectedResult.symbol_id}
        </div>
        <button onClick={onEdit} title="Edit Symbol"
          className="p-1 rounded text-text-muted/40 hover:text-accent hover:bg-accent/10 transition-colors">
          <Pencil size={11} />
        </button>
        <button onClick={onDuplicate} title="Duplicate Symbol"
          className="p-1 rounded text-text-muted/40 hover:text-accent hover:bg-accent/10 transition-colors">
          <Copy size={11} />
        </button>
        <button
          onClick={onPlace}
          className="px-2 py-0.5 bg-accent/20 hover:bg-accent/30 text-accent rounded text-[10px] font-medium transition-colors"
        >
          Place
        </button>
      </div>

      {/* Details section */}
      <SectionHeader
        title={`Details  ${selectedResult.symbol_id}`}
        expanded={detailsOpen}
        onToggle={() => setDetailsOpen(!detailsOpen)}
      />
      {detailsOpen && (
        <div className="px-2 py-1.5 text-[10px] space-y-0.5 border-b border-border-subtle/30">
          {visibleDetails.map(row => (
            <div key={row.label} className="flex items-center gap-2 group">
              <span className="text-text-muted/50 w-[90px] shrink-0 truncate">{row.label}</span>
              <span className="text-text-secondary truncate flex-1" title={row.value}>{row.value}</span>
              <Star size={9} className="shrink-0 text-text-muted/20 opacity-0 group-hover:opacity-100 transition-opacity cursor-pointer hover:text-yellow-400" />
            </div>
          ))}
          <button
            onClick={() => setShowAllDetails(!showAllDetails)}
            className="text-[9px] text-accent/60 hover:text-accent mt-1 flex items-center gap-1 transition-colors"
          >
            {showAllDetails ? "Show Less \u25B2" : "Show More \u25BC"}
          </button>
        </div>
      )}

      {/* Models section (toggleable from menu) */}
      {showModels && (
        <>
          <SectionHeader
            title="Models"
            expanded={modelsOpen}
            onToggle={() => setModelsOpen(!modelsOpen)}
          />
          {modelsOpen && (
            <div className="border-b border-border-subtle/30">
              {/* Symbol preview — tall, cream background like Altium */}
              <div className="h-[200px] mx-2 mt-1.5 rounded border border-[#d4cba8]/30 overflow-hidden">
                <SymbolPreviewMini symbol={preview} />
              </div>
              <div className="px-2 py-1 text-[10px] text-text-muted/70 font-medium">
                {selectedResult.symbol_id}
              </div>

              {/* Footprint preview — dark background like Altium */}
              <div className="h-[200px] mx-2 mb-1 rounded overflow-hidden">
                <FootprintPreviewMini footprintId={selectedResult.footprint} />
              </div>
              <div className="px-2 pb-1.5 text-[10px] text-text-muted/50 font-medium truncate" title={selectedResult.footprint || "No footprint"}>
                {selectedResult.footprint
                  ? selectedResult.footprint.includes(":") ? selectedResult.footprint.split(":")[1] : selectedResult.footprint
                  : "No footprint assigned"}
              </div>
            </div>
          )}
        </>
      )}

      {/* References section */}
      <SectionHeader
        title="References"
        expanded={referencesOpen}
        onToggle={() => setReferencesOpen(!referencesOpen)}
      />
      {referencesOpen && (
        <div className="px-2 py-2 text-[10px] text-text-muted/40 border-b border-border-subtle/30">
          No datasheets linked
        </div>
      )}

      {/* Part Choices section */}
      <SectionHeader
        title="Part Choices"
        expanded={partChoicesOpen}
        onToggle={() => setPartChoicesOpen(!partChoicesOpen)}
      />
      {partChoicesOpen && (
        <div className="px-2 py-2 text-[10px] text-text-muted/40 border-b border-border-subtle/30">
          No part choices configured
        </div>
      )}
    </div>
  );
}

/** Layer color mapping for footprint preview */
const FP_LAYER_COLORS: Record<string, string> = {
  "F.Cu": "#ff4444",
  "B.Cu": "#4444ff",
  "F.SilkS": "#e0e040",
  "B.SilkS": "#808080",
  "F.Fab": "#aaaa44",
  "B.Fab": "#44aaaa",
  "F.CrtYd": "#c8c800",
  "B.CrtYd": "#00c8c8",
  "F.Mask": "#804080",
  "B.Mask": "#408080",
  "F.Paste": "#808040",
  "B.Paste": "#408040",
  "Edge.Cuts": "#c8c800",
};

function FootprintPreviewMini({ footprintId }: { footprintId: string }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [fpData, setFpData] = useState<RustFootprintData | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  // Load footprint data when footprintId changes
  useEffect(() => {
    setFpData(null);
    setError(null);
    if (!footprintId || !footprintId.includes(":")) {
      setError(footprintId ? "Invalid footprint reference" : "No footprint assigned");
      return;
    }
    setLoading(true);
    invoke<RustFootprintData>("get_footprint", { footprintId })
      .then((data) => { setFpData(data); setError(null); })
      .catch((e) => { setError(String(e)); setFpData(null); })
      .finally(() => setLoading(false));
  }, [footprintId]);

  // Render footprint on canvas
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const dpr = window.devicePixelRatio || 1;
    const w = canvas.clientWidth, h = canvas.clientHeight;
    canvas.width = w * dpr; canvas.height = h * dpr;
    ctx.scale(dpr, dpr);

    // Dark background
    ctx.fillStyle = "#1a1b2e";
    ctx.fillRect(0, 0, w, h);

    if (!fpData) return;

    // Calculate bounds from pads and graphics
    let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
    const expand = (x: number, y: number) => {
      minX = Math.min(minX, x); minY = Math.min(minY, y);
      maxX = Math.max(maxX, x); maxY = Math.max(maxY, y);
    };

    for (const pad of fpData.pads) {
      expand(pad.position.x - pad.size[0] / 2, pad.position.y - pad.size[1] / 2);
      expand(pad.position.x + pad.size[0] / 2, pad.position.y + pad.size[1] / 2);
    }
    for (const g of fpData.graphics) {
      if (g.start) expand(g.start.x, g.start.y);
      if (g.end) expand(g.end.x, g.end.y);
      if (g.center && g.radius != null) {
        expand(g.center.x - g.radius, g.center.y - g.radius);
        expand(g.center.x + g.radius, g.center.y + g.radius);
      }
      for (const pt of g.points) expand(pt.x, pt.y);
    }

    if (!isFinite(minX)) { minX = -2; minY = -2; maxX = 2; maxY = 2; }

    const pad = 0.5;
    const bw = maxX - minX + pad * 2, bh = maxY - minY + pad * 2;
    const scale = Math.min(w / bw, h / bh) * 0.85;
    const ox = (w - bw * scale) / 2 - (minX - pad) * scale;
    const oy = (h - bh * scale) / 2 - (minY - pad) * scale;

    ctx.save();
    ctx.translate(ox, oy);
    ctx.scale(scale, scale);

    // Render graphics (silkscreen, fab, courtyard) - back layers first, then front
    const layerOrder = ["B.Fab", "B.CrtYd", "B.SilkS", "F.Fab", "F.CrtYd", "F.SilkS"];
    const sortedGraphics = [...fpData.graphics].sort((a, b) => {
      const ai = layerOrder.indexOf(a.layer);
      const bi = layerOrder.indexOf(b.layer);
      return (ai === -1 ? 99 : ai) - (bi === -1 ? 99 : bi);
    });

    for (const g of sortedGraphics) {
      // Skip text for cleaner preview
      if (g.graphic_type === "text") continue;

      const color = FP_LAYER_COLORS[g.layer] || "#666666";
      ctx.strokeStyle = color;
      ctx.lineWidth = Math.max(g.width, 0.05);
      ctx.fillStyle = color;

      if (g.graphic_type === "line" && g.start && g.end) {
        ctx.beginPath();
        ctx.moveTo(g.start.x, g.start.y);
        ctx.lineTo(g.end.x, g.end.y);
        ctx.stroke();
      } else if (g.graphic_type === "rect" && g.start && g.end) {
        const rx = Math.min(g.start.x, g.end.x), ry = Math.min(g.start.y, g.end.y);
        const rw = Math.abs(g.end.x - g.start.x), rh = Math.abs(g.end.y - g.start.y);
        if (g.fill) {
          ctx.globalAlpha = 0.3;
          ctx.fillRect(rx, ry, rw, rh);
          ctx.globalAlpha = 1;
        }
        ctx.strokeRect(rx, ry, rw, rh);
      } else if (g.graphic_type === "circle" && g.center && g.radius != null) {
        ctx.beginPath();
        ctx.arc(g.center.x, g.center.y, g.radius, 0, Math.PI * 2);
        if (g.fill) {
          ctx.globalAlpha = 0.3;
          ctx.fill();
          ctx.globalAlpha = 1;
        }
        ctx.stroke();
      } else if (g.graphic_type === "arc" && g.start && g.end) {
        ctx.beginPath();
        if (g.mid) {
          // Three-point arc: approximate with quadratic curve
          ctx.moveTo(g.start.x, g.start.y);
          ctx.quadraticCurveTo(g.mid.x, g.mid.y, g.end.x, g.end.y);
        } else {
          ctx.moveTo(g.start.x, g.start.y);
          ctx.lineTo(g.end.x, g.end.y);
        }
        ctx.stroke();
      } else if (g.graphic_type === "poly" && g.points.length >= 2) {
        ctx.beginPath();
        ctx.moveTo(g.points[0].x, g.points[0].y);
        for (let i = 1; i < g.points.length; i++) ctx.lineTo(g.points[i].x, g.points[i].y);
        ctx.closePath();
        if (g.fill) {
          ctx.globalAlpha = 0.3;
          ctx.fill();
          ctx.globalAlpha = 1;
        }
        ctx.stroke();
      }
    }

    // Render pads on top
    for (const p of fpData.pads) {
      const isSmd = p.pad_type === "smd";
      const isTh = p.pad_type === "thru_hole";
      const px = p.position.x, py = p.position.y;
      const sw = p.size[0], sh = p.size[1];

      // Pad fill color
      if (isSmd) {
        ctx.fillStyle = "#cc3333"; // Red for front copper SMD
      } else if (isTh) {
        ctx.fillStyle = "#cc9933"; // Gold for through-hole
      } else {
        ctx.fillStyle = "#666666";
      }

      if (p.shape === "circle") {
        ctx.beginPath();
        ctx.arc(px, py, sw / 2, 0, Math.PI * 2);
        ctx.fill();
      } else if (p.shape === "roundrect") {
        const r = (p.roundrect_ratio ?? 0.25) * Math.min(sw, sh) / 2;
        drawRoundRect(ctx, px - sw / 2, py - sh / 2, sw, sh, r);
        ctx.fill();
      } else {
        // rect, oval, etc. — draw as rectangle
        ctx.fillRect(px - sw / 2, py - sh / 2, sw, sh);
      }

      // Draw drill hole for through-hole pads
      if (isTh && p.drill) {
        ctx.fillStyle = "#1a1b2e"; // Background color for hole
        ctx.beginPath();
        ctx.arc(px, py, p.drill.diameter / 2, 0, Math.PI * 2);
        ctx.fill();
      }

      // Pad number label
      if (p.number) {
        const fontSize = Math.min(sw, sh) * 0.5;
        if (fontSize * scale > 3) { // Only show if readable
          ctx.fillStyle = "#ffffff";
          ctx.font = `${fontSize}px sans-serif`;
          ctx.textAlign = "center";
          ctx.textBaseline = "middle";
          ctx.save();
          ctx.scale(1, -1); // Flip for text since we may have inverted Y
          // Note: coordinate system is not inverted here (PCB Y goes down)
          ctx.restore();
        }
      }
    }

    ctx.restore();
  }, [fpData]);

  if (loading) {
    return (
      <div className="w-full h-full bg-[#1a1b2e] border border-border-subtle/30 flex flex-col items-center justify-center">
        <div className="w-4 h-4 border-2 border-accent/40 border-t-accent rounded-full animate-spin mb-1" />
        <span className="text-[9px] text-text-muted/30">Loading footprint...</span>
      </div>
    );
  }

  if (error || !fpData) {
    return (
      <div className="w-full h-full bg-[#1a1b2e] border border-border-subtle/30 flex flex-col items-center justify-center relative">
        <Package size={20} className="text-text-muted/15 mb-1" />
        <span className="text-[9px] text-text-muted/30">
          {!footprintId ? "No footprint assigned" : error || "No footprint data"}
        </span>
        <button className="absolute bottom-1.5 left-1.5 px-2 py-0.5 text-[9px] text-text-muted/50 border border-border-subtle/50 rounded bg-bg-surface/50 hover:text-text-secondary hover:border-border-subtle transition-colors">
          2D
        </button>
      </div>
    );
  }

  return (
    <div className="w-full h-full relative">
      <canvas ref={canvasRef} className="w-full h-full" />
      <button className="absolute bottom-1.5 left-1.5 px-2 py-0.5 text-[9px] text-text-muted/50 border border-border-subtle/50 rounded bg-bg-surface/50 hover:text-text-secondary hover:border-border-subtle transition-colors">
        2D
      </button>
    </div>
  );
}

/** Draw a rounded rectangle path */
function drawRoundRect(ctx: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, r: number) {
  r = Math.min(r, w / 2, h / 2);
  ctx.beginPath();
  ctx.moveTo(x + r, y);
  ctx.lineTo(x + w - r, y);
  ctx.arcTo(x + w, y, x + w, y + r, r);
  ctx.lineTo(x + w, y + h - r);
  ctx.arcTo(x + w, y + h, x + w - r, y + h, r);
  ctx.lineTo(x + r, y + h);
  ctx.arcTo(x, y + h, x, y + h - r, r);
  ctx.lineTo(x, y + r);
  ctx.arcTo(x, y, x + r, y, r);
  ctx.closePath();
}

function SymbolPreviewMini({ symbol }: { symbol: LibSymbol }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const dpr = window.devicePixelRatio || 1;
    const w = canvas.clientWidth, h = canvas.clientHeight;
    canvas.width = w * dpr; canvas.height = h * dpr;
    ctx.scale(dpr, dpr);

    let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
    const expand = (x: number, y: number) => { minX = Math.min(minX, x); minY = Math.min(minY, y); maxX = Math.max(maxX, x); maxY = Math.max(maxY, y); };

    for (const g of symbol.graphics) {
      if (g.type === "Rectangle") { expand(g.start.x, g.start.y); expand(g.end.x, g.end.y); }
      else if (g.type === "Polyline") { for (const p of g.points) expand(p.x, p.y); }
      else if (g.type === "Circle") { expand(g.center.x - g.radius, g.center.y - g.radius); expand(g.center.x + g.radius, g.center.y + g.radius); }
    }
    for (const pin of symbol.pins) {
      expand(pin.position.x, pin.position.y);
      const rad = (pin.rotation * Math.PI) / 180;
      expand(pin.position.x + Math.cos(rad) * pin.length, pin.position.y + Math.sin(rad) * pin.length);
    }
    if (!isFinite(minX)) { minX = -5; minY = -5; maxX = 5; maxY = 5; }

    const pad = 1.5;
    const bw = maxX - minX + pad * 2, bh = maxY - minY + pad * 2;
    const scale = Math.min(w / bw, h / bh) * 0.85;
    const ox = (w - bw * scale) / 2 - (minX - pad) * scale;
    const oy = (h - bh * scale) / 2 - (minY - pad) * scale;

    // Light cream/yellow background like Altium's symbol preview
    ctx.fillStyle = "#f5f0dc"; ctx.fillRect(0, 0, w, h);
    // Subtle border inside
    ctx.strokeStyle = "#d4cba8"; ctx.lineWidth = 1;
    ctx.strokeRect(0.5, 0.5, w - 1, h - 1);

    ctx.save(); ctx.translate(ox, oy); ctx.scale(scale, -scale);

    // Dark body outline and fill on cream background
    ctx.strokeStyle = "#2a4080"; ctx.fillStyle = "#e8e0c0"; ctx.lineWidth = 0.15;
    for (const g of symbol.graphics) {
      if (g.type === "Polyline" && g.points.length >= 2) {
        ctx.beginPath(); ctx.moveTo(g.points[0].x, g.points[0].y);
        for (let i = 1; i < g.points.length; i++) ctx.lineTo(g.points[i].x, g.points[i].y);
        if (g.fill) ctx.fill(); ctx.stroke();
      } else if (g.type === "Rectangle") {
        const rx = Math.min(g.start.x, g.end.x), ry = Math.min(g.start.y, g.end.y);
        ctx.fillRect(rx, ry, Math.abs(g.end.x - g.start.x), Math.abs(g.end.y - g.start.y));
        ctx.strokeRect(rx, ry, Math.abs(g.end.x - g.start.x), Math.abs(g.end.y - g.start.y));
      } else if (g.type === "Circle") {
        ctx.beginPath(); ctx.arc(g.center.x, g.center.y, g.radius, 0, Math.PI * 2);
        if (g.fill) ctx.fill(); ctx.stroke();
      }
    }
    // Green pins on cream background
    ctx.strokeStyle = "#2e7d32"; ctx.lineWidth = 0.1;
    for (const pin of symbol.pins) {
      const rad = (pin.rotation * Math.PI) / 180;
      ctx.beginPath();
      ctx.moveTo(pin.position.x, pin.position.y);
      ctx.lineTo(pin.position.x + Math.cos(rad) * pin.length, pin.position.y + Math.sin(rad) * pin.length);
      ctx.stroke();
    }
    ctx.restore();
  }, [symbol]);

  return <canvas ref={canvasRef} className="w-full h-full" />;
}
