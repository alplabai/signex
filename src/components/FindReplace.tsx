import { useState, useRef, useEffect, useCallback } from "react";
import { Search, Replace, X, ChevronDown, ChevronUp } from "lucide-react";
import { useSchematicStore } from "@/stores/schematic";
import { cn } from "@/lib/utils";

interface FindReplaceProps {
  open: boolean;
  onClose: () => void;
  showReplace?: boolean;
}

interface Match {
  uuid: string;
  type: "symbol" | "label";
  field: string;
  text: string;
}

export function FindReplace({ open, onClose, showReplace: initialReplace }: FindReplaceProps) {
  const [query, setQuery] = useState("");
  const [replacement, setReplacement] = useState("");
  const [showReplace, setShowReplace] = useState(initialReplace ?? false);
  const [matches, setMatches] = useState<Match[]>([]);
  const [currentIdx, setCurrentIdx] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (open) {
      setTimeout(() => inputRef.current?.focus(), 50);
    }
  }, [open]);

  const doSearch = useCallback((q: string) => {
    if (q.length === 0) { setMatches([]); return; }
    const data = useSchematicStore.getState().data;
    if (!data) { setMatches([]); return; }
    const lower = q.toLowerCase();
    const found: Match[] = [];

    for (const sym of data.symbols) {
      if (sym.reference.toLowerCase().includes(lower))
        found.push({ uuid: sym.uuid, type: "symbol", field: "reference", text: sym.reference });
      if (sym.value.toLowerCase().includes(lower))
        found.push({ uuid: sym.uuid, type: "symbol", field: "value", text: sym.value });
    }
    for (const label of data.labels) {
      if (label.text.toLowerCase().includes(lower))
        found.push({ uuid: label.uuid, type: "label", field: "text", text: label.text });
    }

    setMatches(found);
    setCurrentIdx(0);
    if (found.length > 0) {
      useSchematicStore.getState().select(found[0].uuid);
    }
  }, []);

  useEffect(() => { doSearch(query); }, [query, doSearch]);

  const navigate = (dir: 1 | -1) => {
    if (matches.length === 0) return;
    const next = (currentIdx + dir + matches.length) % matches.length;
    setCurrentIdx(next);
    useSchematicStore.getState().select(matches[next].uuid);
  };

  const replaceOne = () => {
    if (matches.length === 0 || !replacement) return;
    const m = matches[currentIdx];
    const store = useSchematicStore.getState();
    if (m.type === "symbol") {
      store.updateSymbolProp(m.uuid, m.field, m.text.replace(new RegExp(query, "i"), replacement));
    } else {
      store.updateLabelProp(m.uuid, m.field, m.text.replace(new RegExp(query, "i"), replacement));
    }
    doSearch(query);
  };

  const replaceAll = () => {
    if (matches.length === 0 || !replacement) return;
    const store = useSchematicStore.getState();
    for (const m of matches) {
      if (m.type === "symbol") {
        store.updateSymbolProp(m.uuid, m.field, m.text.replace(new RegExp(query, "gi"), replacement));
      } else {
        store.updateLabelProp(m.uuid, m.field, m.text.replace(new RegExp(query, "gi"), replacement));
      }
    }
    doSearch(query);
  };

  if (!open) return null;

  return (
    <div className="absolute top-12 right-3 z-50 bg-bg-surface border border-border rounded-lg shadow-xl shadow-black/40 w-[340px]">
      <div className="flex items-center gap-2 px-3 py-2">
        <Search size={14} className="text-text-muted/50 shrink-0" />
        <input
          ref={inputRef}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Find..."
          className="flex-1 bg-transparent text-[12px] text-text-primary placeholder:text-text-muted/40 outline-none"
          onKeyDown={(e) => {
            if (e.key === "Escape") onClose();
            if (e.key === "Enter") navigate(e.shiftKey ? -1 : 1);
            if (e.key === "h" && e.ctrlKey) { e.preventDefault(); setShowReplace(!showReplace); }
            e.stopPropagation();
          }}
        />
        <span className="text-[10px] text-text-muted/50 shrink-0">
          {matches.length > 0 ? `${currentIdx + 1}/${matches.length}` : "0"}
        </span>
        <button onClick={() => navigate(-1)} className="p-0.5 hover:bg-bg-hover rounded text-text-muted/50" title="Previous (Shift+Enter)">
          <ChevronUp size={14} />
        </button>
        <button onClick={() => navigate(1)} className="p-0.5 hover:bg-bg-hover rounded text-text-muted/50" title="Next (Enter)">
          <ChevronDown size={14} />
        </button>
        <button onClick={() => setShowReplace(!showReplace)} className={cn("p-0.5 rounded", showReplace ? "text-accent" : "text-text-muted/50 hover:bg-bg-hover")} title="Toggle Replace (Ctrl+H)">
          <Replace size={14} />
        </button>
        <button onClick={onClose} className="p-0.5 hover:bg-bg-hover rounded text-text-muted/50">
          <X size={14} />
        </button>
      </div>

      {showReplace && (
        <div className="flex items-center gap-2 px-3 pb-2">
          <Replace size={14} className="text-text-muted/50 shrink-0" />
          <input
            value={replacement}
            onChange={(e) => setReplacement(e.target.value)}
            placeholder="Replace with..."
            className="flex-1 bg-transparent text-[12px] text-text-primary placeholder:text-text-muted/40 outline-none"
            onKeyDown={(e) => {
              if (e.key === "Escape") onClose();
              if (e.key === "Enter") replaceOne();
              e.stopPropagation();
            }}
          />
          <button onClick={replaceOne} className="text-[10px] px-2 py-0.5 hover:bg-bg-hover rounded text-text-muted/60" title="Replace">
            1
          </button>
          <button onClick={replaceAll} className="text-[10px] px-2 py-0.5 hover:bg-bg-hover rounded text-text-muted/60" title="Replace All">
            All
          </button>
        </div>
      )}
    </div>
  );
}
