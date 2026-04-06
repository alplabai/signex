import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Search, X, Package, Cpu } from "lucide-react";
import { cn } from "@/lib/utils";
import { useSchematicStore } from "@/stores/schematic";
import type { LibSymbol, SymbolSearchResult, LibraryInfo } from "@/types";

interface ComponentSearchProps {
  open: boolean;
  onClose: () => void;
}

export function ComponentSearch({ open, onClose }: ComponentSearchProps) {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SymbolSearchResult[]>([]);
  const [libraries, setLibraries] = useState<LibraryInfo[]>([]);
  const [selectedIdx, setSelectedIdx] = useState(0);
  const [loading, setLoading] = useState(false);
  const [preview, setPreview] = useState<LibSymbol | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout>>(undefined);

  // Load library list on first open
  useEffect(() => {
    if (open && libraries.length === 0) {
      invoke<LibraryInfo[]>("list_libraries")
        .then(setLibraries)
        .catch(() => {});
    }
  }, [open, libraries.length]);

  // Focus input when dialog opens
  useEffect(() => {
    if (open) {
      setTimeout(() => inputRef.current?.focus(), 50);
      setQuery("");
      setResults([]);
      setSelectedIdx(0);
      setPreview(null);
    }
  }, [open]);

  // Debounced search
  const search = useCallback((q: string) => {
    if (debounceRef.current) clearTimeout(debounceRef.current);
    if (q.length < 2) {
      setResults([]);
      setPreview(null);
      return;
    }
    setLoading(true);
    debounceRef.current = setTimeout(async () => {
      try {
        const res = await invoke<SymbolSearchResult[]>("search_symbols", { query: q, limit: 50 });
        setResults(res);
        setSelectedIdx(0);
        if (res.length > 0) loadPreview(res[0]);
        else setPreview(null);
      } catch {
        setResults([]);
      } finally {
        setLoading(false);
      }
    }, 200);
  }, []);

  useEffect(() => { search(query); }, [query, search]);

  const loadPreview = async (result: SymbolSearchResult) => {
    const lib = libraries.find((l) => l.name === result.library);
    if (!lib) return;
    try {
      const sym = await invoke<LibSymbol>("get_symbol", {
        libraryPath: lib.path,
        symbolId: result.symbol_id,
      });
      setPreview(sym);
    } catch {
      setPreview(null);
    }
  };

  const placeSelected = async () => {
    const result = results[selectedIdx];
    if (!result) return;

    let sym = preview;
    if (!sym) {
      const lib = libraries.find((l) => l.name === result.library);
      if (!lib) return;
      try {
        sym = await invoke<LibSymbol>("get_symbol", {
          libraryPath: lib.path,
          symbolId: result.symbol_id,
        });
      } catch { return; }
    }

    useSchematicStore.getState().startPlacement(sym, result);
    onClose();
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      e.stopPropagation();
      onClose();
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      const next = Math.min(selectedIdx + 1, results.length - 1);
      setSelectedIdx(next);
      if (results[next]) loadPreview(results[next]);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      const next = Math.max(selectedIdx - 1, 0);
      setSelectedIdx(next);
      if (results[next]) loadPreview(results[next]);
    } else if (e.key === "Enter") {
      e.preventDefault();
      placeSelected();
    }
  };

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center pt-[10vh]" onClick={onClose}>
      <div className="absolute inset-0 bg-black/50" />
      <div
        className="relative w-[700px] max-h-[70vh] bg-bg-surface border border-border rounded-xl shadow-2xl shadow-black/60 flex flex-col overflow-hidden"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={handleKeyDown}
      >
        {/* Search header */}
        <div className="flex items-center gap-3 px-4 py-3 border-b border-border-subtle">
          <Search size={18} className="text-text-muted/60 shrink-0" />
          <input
            ref={inputRef}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search components... (e.g. resistor, capacitor, STM32)"
            className="flex-1 bg-transparent text-[13px] text-text-primary placeholder:text-text-muted/40 outline-none"
          />
          {loading && <div className="w-4 h-4 border-2 border-accent/40 border-t-accent rounded-full animate-spin" />}
          <button onClick={onClose} className="p-1 rounded hover:bg-bg-hover text-text-muted/50 hover:text-text-secondary">
            <X size={16} />
          </button>
        </div>

        <div className="flex flex-1 min-h-0">
          {/* Results list */}
          <div className="w-[380px] border-r border-border-subtle overflow-y-auto">
            {results.length === 0 && query.length >= 2 && !loading && (
              <div className="text-center py-10 text-text-muted/50 text-[12px]">No results found</div>
            )}
            {results.length === 0 && query.length < 2 && (
              <div className="text-center py-10 text-text-muted/40 text-[12px]">
                <Package size={32} className="mx-auto mb-3 opacity-30" />
                Type to search {libraries.length} libraries
              </div>
            )}
            {results.map((r, i) => (
              <button
                key={`${r.library}:${r.symbol_id}`}
                className={cn(
                  "w-full flex items-start gap-2.5 px-3 py-2 text-left transition-colors",
                  i === selectedIdx ? "bg-accent/15 text-text-primary" : "text-text-secondary hover:bg-bg-hover"
                )}
                onClick={() => {
                  setSelectedIdx(i);
                  loadPreview(r);
                }}
                onDoubleClick={placeSelected}
              >
                <Cpu size={14} className="mt-0.5 shrink-0 text-accent/60" />
                <div className="min-w-0 flex-1">
                  <div className="text-[12px] font-medium truncate">
                    <span className="text-text-muted/60">{r.library}:</span>{r.symbol_id}
                  </div>
                  {r.description && (
                    <div className="text-[11px] text-text-muted/60 truncate mt-0.5">{r.description}</div>
                  )}
                  <div className="flex gap-2 mt-1 text-[10px] text-text-muted/40">
                    <span>{r.reference_prefix}?</span>
                    <span>{r.pin_count} pins</span>
                  </div>
                </div>
              </button>
            ))}
          </div>

          {/* Preview pane */}
          <div className="flex-1 flex flex-col">
            {preview ? (
              <SymbolPreview symbol={preview} />
            ) : (
              <div className="flex-1 flex items-center justify-center text-text-muted/30 text-[11px]">
                Select a symbol to preview
              </div>
            )}

            {/* Place button */}
            {results.length > 0 && (
              <div className="px-4 py-3 border-t border-border-subtle flex items-center justify-between">
                <div className="text-[11px] text-text-muted/50">
                  Enter to place | R rotate | X/Y mirror
                </div>
                <button
                  onClick={placeSelected}
                  className="px-4 py-1.5 bg-accent/20 hover:bg-accent/30 text-accent rounded text-[12px] font-medium transition-colors"
                >
                  Place
                </button>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

/** Mini canvas to preview a LibSymbol's graphics */
function SymbolPreview({ symbol }: { symbol: LibSymbol }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const dpr = window.devicePixelRatio || 1;
    const w = canvas.clientWidth;
    const h = canvas.clientHeight;
    canvas.width = w * dpr;
    canvas.height = h * dpr;
    ctx.scale(dpr, dpr);

    // Calculate bounds
    let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
    const expand = (x: number, y: number) => {
      minX = Math.min(minX, x); minY = Math.min(minY, y);
      maxX = Math.max(maxX, x); maxY = Math.max(maxY, y);
    };

    for (const g of symbol.graphics) {
      if (g.type === "Rectangle") { expand(g.start.x, g.start.y); expand(g.end.x, g.end.y); }
      else if (g.type === "Polyline") { for (const p of g.points) expand(p.x, p.y); }
      else if (g.type === "Circle") { expand(g.center.x - g.radius, g.center.y - g.radius); expand(g.center.x + g.radius, g.center.y + g.radius); }
      else if (g.type === "Arc") { expand(g.start.x, g.start.y); expand(g.mid.x, g.mid.y); expand(g.end.x, g.end.y); }
    }
    for (const pin of symbol.pins) {
      expand(pin.position.x, pin.position.y);
      const rad = (pin.rotation * Math.PI) / 180;
      expand(pin.position.x + Math.cos(rad) * pin.length, pin.position.y + Math.sin(rad) * pin.length);
    }

    if (!isFinite(minX)) { minX = -5; minY = -5; maxX = 5; maxY = 5; }

    const pad = 2;
    const bw = maxX - minX + pad * 2;
    const bh = maxY - minY + pad * 2;
    const scale = Math.min(w / bw, h / bh) * 0.85;
    const ox = (w - bw * scale) / 2 - (minX - pad) * scale;
    const oy = (h - bh * scale) / 2 - (minY - pad) * scale;

    // Clear
    ctx.fillStyle = "#1a1b2e";
    ctx.fillRect(0, 0, w, h);

    ctx.save();
    ctx.translate(ox, oy);
    ctx.scale(scale, -scale); // Flip Y for symbol space

    // Draw graphics
    ctx.strokeStyle = "#9fa8da";
    ctx.fillStyle = "#1e2035";
    ctx.lineWidth = 0.15;

    for (const g of symbol.graphics) {
      switch (g.type) {
        case "Polyline": {
          if (g.points.length < 2) break;
          ctx.beginPath();
          ctx.moveTo(g.points[0].x, g.points[0].y);
          for (let i = 1; i < g.points.length; i++) ctx.lineTo(g.points[i].x, g.points[i].y);
          if (g.fill) ctx.fill();
          ctx.stroke();
          break;
        }
        case "Rectangle": {
          const rx = Math.min(g.start.x, g.end.x), ry = Math.min(g.start.y, g.end.y);
          const rw = Math.abs(g.end.x - g.start.x), rh = Math.abs(g.end.y - g.start.y);
          ctx.fillRect(rx, ry, rw, rh);
          ctx.strokeRect(rx, ry, rw, rh);
          break;
        }
        case "Circle": {
          ctx.beginPath();
          ctx.arc(g.center.x, g.center.y, g.radius, 0, Math.PI * 2);
          if (g.fill) ctx.fill();
          ctx.stroke();
          break;
        }
      }
    }

    // Draw pins
    ctx.strokeStyle = "#81c784";
    ctx.lineWidth = 0.1;
    for (const pin of symbol.pins) {
      const rad = (pin.rotation * Math.PI) / 180;
      const ex = pin.position.x + Math.cos(rad) * pin.length;
      const ey = pin.position.y + Math.sin(rad) * pin.length;
      ctx.beginPath();
      ctx.moveTo(pin.position.x, pin.position.y);
      ctx.lineTo(ex, ey);
      ctx.stroke();

      // Pin dot
      ctx.fillStyle = "#81c784";
      ctx.beginPath();
      ctx.arc(pin.position.x, pin.position.y, 0.15, 0, Math.PI * 2);
      ctx.fill();
    }

    ctx.restore();
  }, [symbol]);

  return (
    <div className="flex-1 min-h-[200px]">
      <canvas ref={canvasRef} className="w-full h-full" />
    </div>
  );
}
