import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Library, RefreshCw, ToggleLeft, ToggleRight, Search } from "lucide-react";
import { cn } from "@/lib/utils";

interface LibInfo {
  name: string;
  path: string;
  symbolCount: number | null;
}

const DISABLED_KEY = "signex-disabled-libs";

function getDisabledLibs(): Set<string> {
  try {
    return new Set(JSON.parse(localStorage.getItem(DISABLED_KEY) || "[]"));
  } catch {
    return new Set();
  }
}

function setDisabledLibs(s: Set<string>) {
  localStorage.setItem(DISABLED_KEY, JSON.stringify([...s]));
}

export function LibraryPanel() {
  const [libs, setLibs] = useState<LibInfo[]>([]);
  const [disabled, setDisabled] = useState<Set<string>>(getDisabledLibs);
  const [loading, setLoading] = useState(false);
  const [search, setSearch] = useState("");

  const loadLibraries = useCallback(async () => {
    setLoading(true);
    try {
      const names: string[] = await invoke("list_libraries");
      const infos: LibInfo[] = names.map(name => ({ name, path: "", symbolCount: null }));
      setLibs(infos);

      // Fetch symbol counts in background
      for (const info of infos) {
        try {
          const symbols: any[] = await invoke("list_library_symbols", { libraryName: info.name });
          setLibs(prev => prev.map(l => l.name === info.name ? { ...l, symbolCount: symbols.length } : l));
        } catch { /* library might fail to parse */ }
      }
    } catch {
      setLibs([]);
    }
    setLoading(false);
  }, []);

  useEffect(() => { loadLibraries(); }, [loadLibraries]);

  const toggleLib = useCallback((name: string) => {
    setDisabled(prev => {
      const next = new Set(prev);
      if (next.has(name)) next.delete(name);
      else next.add(name);
      setDisabledLibs(next);
      return next;
    });
  }, []);

  const q = search.toLowerCase();
  const filtered = q ? libs.filter(l => l.name.toLowerCase().includes(q)) : libs;
  const enabledCount = libs.filter(l => !disabled.has(l.name)).length;
  const totalSymbols = libs.reduce((sum, l) => sum + (l.symbolCount ?? 0), 0);

  return (
    <div className="text-xs flex flex-col h-full select-none">
      {/* Header */}
      <div className="px-2 py-1.5 border-b border-border-subtle bg-bg-surface/80 shrink-0 flex items-center gap-2">
        <Library size={12} className="text-accent shrink-0" />
        <span className="text-[11px] font-semibold text-text-secondary">Libraries</span>
        <span className="ml-auto text-[10px] text-text-muted/50">{enabledCount}/{libs.length}</span>
        <button
          onClick={loadLibraries}
          className={cn("p-0.5 rounded hover:bg-bg-hover text-text-muted/50 hover:text-accent", loading && "animate-spin")}
          title="Refresh"
        >
          <RefreshCw size={11} />
        </button>
      </div>

      {/* Search */}
      <div className="px-2 py-1 border-b border-border-subtle/50 flex items-center gap-1.5">
        <Search size={10} className="text-text-muted/40 shrink-0" />
        <input
          type="text"
          value={search}
          onChange={e => setSearch(e.target.value)}
          placeholder="Filter libraries..."
          className="flex-1 bg-transparent text-[10px] text-text-primary placeholder:text-text-muted/30 outline-none"
        />
      </div>

      {/* Library list */}
      <div className="flex-1 overflow-y-auto">
        {loading && libs.length === 0 ? (
          <div className="p-4 text-[10px] text-text-muted/40 text-center">Loading libraries...</div>
        ) : filtered.length === 0 ? (
          <div className="p-4 text-[10px] text-text-muted/40 text-center">
            {libs.length === 0 ? "No libraries found" : "No matches"}
          </div>
        ) : (
          filtered.map(lib => {
            const isDisabled = disabled.has(lib.name);
            return (
              <div
                key={lib.name}
                className={cn(
                  "flex items-center gap-2 px-2 py-1 border-b border-border-subtle/20 hover:bg-bg-hover/50 transition-colors",
                  isDisabled && "opacity-40",
                )}
              >
                <button
                  onClick={() => toggleLib(lib.name)}
                  className="shrink-0 text-text-muted/60 hover:text-accent"
                  title={isDisabled ? "Enable" : "Disable"}
                >
                  {isDisabled
                    ? <ToggleLeft size={14} className="text-text-muted/40" />
                    : <ToggleRight size={14} className="text-accent" />
                  }
                </button>
                <div className="flex-1 min-w-0">
                  <div className="text-[10px] text-text-primary truncate">{lib.name}</div>
                </div>
                <span className="text-[9px] text-text-muted/50 tabular-nums shrink-0">
                  {lib.symbolCount !== null ? `${lib.symbolCount} sym` : "\u2026"}
                </span>
              </div>
            );
          })
        )}
      </div>

      {/* Stats footer */}
      <div className="px-3 py-1 border-t border-border-subtle text-[10px] text-text-muted/50 shrink-0 flex gap-3">
        <span>{libs.length} libraries</span>
        <span>{totalSymbols} symbols</span>
        <span>{enabledCount} enabled</span>
      </div>
    </div>
  );
}
