import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Search, Package, Cpu, ChevronRight, Pencil, Copy, FilePlus } from "lucide-react";
import { cn } from "@/lib/utils";
import { useSchematicStore } from "@/stores/schematic";
import { useLibraryEditorStore } from "@/stores/libraryEditor";
import type { LibSymbol, SymbolSearchResult, LibraryInfo } from "@/types";

export function ComponentPanel() {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SymbolSearchResult[]>([]);
  const [libraries, setLibraries] = useState<LibraryInfo[]>([]);
  const [expandedLib, setExpandedLib] = useState<string | null>(null);
  const [libSymbols, setLibSymbols] = useState<SymbolSearchResult[]>([]);
  const [loading, setLoading] = useState(false);
  const [preview, setPreview] = useState<LibSymbol | null>(null);
  const [selectedResult, setSelectedResult] = useState<SymbolSearchResult | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout>>(undefined);

  // Load library list on mount
  useEffect(() => {
    invoke<LibraryInfo[]>("list_libraries")
      .then(setLibraries)
      .catch(() => {});
  }, []);

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
  const expandLibrary = async (lib: LibraryInfo) => {
    if (expandedLib === lib.name) { setExpandedLib(null); return; }
    setExpandedLib(lib.name);
    try {
      const res = await invoke<SymbolSearchResult[]>("search_symbols", { query: lib.name, limit: 500 });
      setLibSymbols(res.filter(r => r.library === lib.name));
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

  return (
    <div className="flex flex-col h-full">
      {/* Search bar */}
      <div className="flex items-center gap-2 px-2 py-1.5 border-b border-border-subtle">
        <Search size={13} className="text-text-muted/40 shrink-0" />
        <input
          ref={inputRef}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search components..."
          className="flex-1 bg-transparent text-[11px] text-text-primary placeholder:text-text-muted/30 outline-none"
        />
        {loading && <div className="w-3 h-3 border-2 border-accent/40 border-t-accent rounded-full animate-spin" />}
        <button onClick={newSymbol} title="New Symbol"
          className="p-1 rounded text-text-muted/40 hover:text-accent hover:bg-accent/10 transition-colors shrink-0">
          <FilePlus size={13} />
        </button>
      </div>

      {/* Preview + Details (Altium-style) */}
      {preview && selectedResult && (
        <div className="border-b border-border-subtle bg-bg-surface/50">
          {/* Symbol preview */}
          <div className="h-[100px]">
            <SymbolPreviewMini symbol={preview} />
          </div>
          {/* Action bar */}
          <div className="px-2 py-1 flex items-center gap-1 border-t border-border-subtle/50">
            <div className="text-[10px] text-text-muted truncate flex-1">
              {selectedResult.symbol_id}
            </div>
            <button onClick={editSymbol} title="Edit Symbol"
              className="p-1 rounded text-text-muted/40 hover:text-accent hover:bg-accent/10 transition-colors">
              <Pencil size={11} />
            </button>
            <button onClick={duplicateSymbol} title="Duplicate Symbol"
              className="p-1 rounded text-text-muted/40 hover:text-accent hover:bg-accent/10 transition-colors">
              <Copy size={11} />
            </button>
            <button
              onClick={() => placeComponent(selectedResult)}
              className="px-2 py-0.5 bg-accent/20 hover:bg-accent/30 text-accent rounded text-[10px] font-medium transition-colors"
            >
              Place
            </button>
          </div>
          {/* Details section (Altium-style) */}
          <div className="px-2 py-1.5 border-t border-border-subtle/50 text-[10px] space-y-0.5">
            <div className="text-[11px] font-semibold text-text-secondary mb-1">Details</div>
            {[
              { label: "Library Path", value: selectedResult.library },
              { label: "Library Ref", value: selectedResult.symbol_id },
              { label: "Description", value: selectedResult.description || "—" },
              { label: "Prefix", value: selectedResult.reference_prefix || "?" },
              { label: "Pins", value: String(selectedResult.pin_count) },
            ].map(row => (
              <div key={row.label} className="flex gap-2">
                <span className="text-text-muted/50 w-20 shrink-0">{row.label}</span>
                <span className="text-text-secondary truncate">{row.value}</span>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Results count */}
      {isSearching && displayResults.length > 0 && (
        <div className="px-2 py-1 border-b border-border-subtle/50 text-[10px] text-text-muted/50">
          Results: {displayResults.length}
        </div>
      )}

      {/* Results or library tree */}
      <div className="flex-1 overflow-y-auto">
        {isSearching ? (
          // Search results
          displayResults.length === 0 && !loading ? (
            <div className="text-center py-6 text-text-muted/40 text-[11px]">No results</div>
          ) : (
            displayResults.map((r) => (
              <button
                key={`${r.library}:${r.symbol_id}`}
                draggable
                onDragStart={(e) => {
                  e.dataTransfer.setData("application/signex-symbol", JSON.stringify(r));
                  e.dataTransfer.effectAllowed = "copy";
                }}
                className={cn(
                  "w-full flex items-start gap-2 px-2 py-1.5 text-left transition-colors border-b border-border-subtle/30",
                  selectedResult?.symbol_id === r.symbol_id && selectedResult?.library === r.library
                    ? "bg-accent/10" : "hover:bg-bg-hover"
                )}
                onClick={() => loadPreview(r)}
                onDoubleClick={() => placeComponent(r)}
              >
                <Cpu size={12} className="mt-0.5 shrink-0 text-accent/50" />
                <div className="min-w-0 flex-1">
                  <div className="text-[11px] font-medium truncate text-text-primary">{r.symbol_id}</div>
                  <div className="text-[10px] text-text-muted/50 truncate">{r.library} | {r.reference_prefix}? | {r.pin_count}p</div>
                  {r.description && <div className="text-[10px] text-text-muted/40 truncate">{r.description}</div>}
                </div>
              </button>
            ))
          )
        ) : (
          // Library tree
          libraries.length === 0 ? (
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
                  <span className="text-text-secondary truncate">{lib.name}</span>
                </button>
                {expandedLib === lib.name && libSymbols.map((r) => (
                  <button
                    key={r.symbol_id}
                    className={cn(
                      "w-full flex items-center gap-2 pl-6 pr-2 py-1 text-left transition-colors text-[10px]",
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
          )
        )}
      </div>
    </div>
  );
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

    ctx.fillStyle = "#1a1b2e"; ctx.fillRect(0, 0, w, h);
    ctx.save(); ctx.translate(ox, oy); ctx.scale(scale, -scale);

    ctx.strokeStyle = "#9fa8da"; ctx.fillStyle = "#1e2035"; ctx.lineWidth = 0.15;
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
    ctx.strokeStyle = "#81c784"; ctx.lineWidth = 0.1;
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
