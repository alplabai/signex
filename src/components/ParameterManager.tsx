import { useState, useMemo } from "react";
import { X, ArrowUpDown } from "lucide-react";
import { useSchematicStore } from "@/stores/schematic";
import { cn } from "@/lib/utils";

interface Props {
  open: boolean;
  onClose: () => void;
}

type SortDir = "asc" | "desc" | null;

export function ParameterManager({ open, onClose }: Props) {
  const data = useSchematicStore((s) => s.data);
  const [sortCol, setSortCol] = useState<string>("Reference");
  const [sortDir, setSortDir] = useState<SortDir>("asc");
  const [filterRow, setFilterRow] = useState<Record<string, string>>({});
  const [edits, setEdits] = useState<Map<string, Map<string, string>>>(new Map());
  const [editingCell, setEditingCell] = useState<{ uuid: string; col: string } | null>(null);

  // Build column list from all symbols
  const columns = useMemo(() => {
    if (!data) return [];
    const cols = new Set<string>(["Reference", "Value", "Footprint"]);
    for (const sym of data.symbols) {
      if (sym.is_power) continue;
      for (const key of Object.keys(sym.fields)) cols.add(key);
    }
    return Array.from(cols);
  }, [data]);

  // Build rows
  const rows = useMemo(() => {
    if (!data) return [];
    return data.symbols
      .filter((s) => !s.is_power)
      .map((sym) => {
        const row: Record<string, string> = {
          uuid: sym.uuid,
          Reference: sym.reference,
          Value: sym.value,
          Footprint: sym.footprint,
          ...sym.fields,
        };
        // Apply pending edits
        const symEdits = edits.get(sym.uuid);
        if (symEdits) {
          for (const [k, v] of symEdits) row[k] = v;
        }
        return row;
      });
  }, [data, edits]);

  // Filter and sort
  const filteredRows = useMemo(() => {
    let result = rows;
    for (const [col, filter] of Object.entries(filterRow)) {
      if (!filter) continue;
      const lower = filter.toLowerCase();
      result = result.filter((r) => (r[col] || "").toLowerCase().includes(lower));
    }
    if (sortCol && sortDir) {
      result = [...result].sort((a, b) => {
        const av = a[sortCol] || "", bv = b[sortCol] || "";
        const cmp = av.localeCompare(bv, undefined, { numeric: true });
        return sortDir === "asc" ? cmp : -cmp;
      });
    }
    return result;
  }, [rows, filterRow, sortCol, sortDir]);

  if (!open || !data) return null;

  const toggleSort = (col: string) => {
    if (sortCol === col) {
      setSortDir(sortDir === "asc" ? "desc" : sortDir === "desc" ? null : "asc");
    } else {
      setSortCol(col);
      setSortDir("asc");
    }
  };

  const setCellValue = (uuid: string, col: string, value: string) => {
    const newEdits = new Map(edits);
    if (!newEdits.has(uuid)) newEdits.set(uuid, new Map());
    newEdits.get(uuid)!.set(col, value);
    setEdits(newEdits);
  };

  const handleApply = () => {
    if (edits.size === 0) return;
    const store = useSchematicStore.getState();
    store.pushUndo();
    for (const [uuid, fields] of edits) {
      for (const [key, value] of fields) {
        if (key === "Reference") store.updateSymbolProp(uuid, "reference", value);
        else if (key === "Value") store.updateSymbolProp(uuid, "value", value);
        else if (key === "Footprint") store.updateSymbolProp(uuid, "footprint", value);
        else store.updateSymbolField(uuid, key, value);
      }
    }
    setEdits(new Map());
    onClose();
  };

  const dirtyCount = Array.from(edits.values()).reduce((sum, m) => sum + m.size, 0);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="bg-[#1e1e2e] border border-[#45475a] rounded-lg shadow-2xl w-[800px] max-h-[80vh] flex flex-col">
        <div className="flex items-center justify-between px-4 py-3 border-b border-[#45475a]">
          <h2 className="text-sm font-semibold text-[#cdd6f4]">Parameter Manager</h2>
          <button onClick={onClose} className="p-1 rounded hover:bg-[#313244] text-[#6c7086]"><X size={16} /></button>
        </div>

        <div className="flex-1 overflow-auto">
          <table className="w-full text-[11px] border-collapse">
            <thead className="sticky top-0 bg-[#313244] z-10">
              <tr>
                {columns.map((col) => (
                  <th key={col}
                    className="px-2 py-1.5 text-left text-[#a6adc8] font-semibold border-b border-[#45475a] cursor-pointer hover:bg-[#45475a]/50 select-none whitespace-nowrap"
                    onClick={() => toggleSort(col)}>
                    <span className="flex items-center gap-1">
                      {col}
                      {sortCol === col && sortDir && (
                        <ArrowUpDown size={10} className={cn("text-[#89b4fa]", sortDir === "desc" && "rotate-180")} />
                      )}
                    </span>
                  </th>
                ))}
              </tr>
              {/* Filter row */}
              <tr>
                {columns.map((col) => (
                  <td key={col} className="px-1 py-0.5 border-b border-[#45475a]">
                    <input
                      value={filterRow[col] || ""}
                      onChange={(e) => setFilterRow({ ...filterRow, [col]: e.target.value })}
                      onKeyDown={(e) => e.stopPropagation()}
                      placeholder="Filter..."
                      className="w-full bg-[#1e1e2e] border border-[#45475a]/50 rounded px-1.5 py-0.5 text-[10px] text-[#cdd6f4] outline-none focus:border-[#89b4fa] placeholder:text-[#6c7086]/40"
                    />
                  </td>
                ))}
              </tr>
            </thead>
            <tbody>
              {filteredRows.map((row) => (
                <tr key={row.uuid} className="hover:bg-[#313244]/50 border-b border-[#45475a]/30">
                  {columns.map((col) => {
                    const isEditing = editingCell?.uuid === row.uuid && editingCell?.col === col;
                    const isDirty = edits.get(row.uuid)?.has(col);
                    return (
                      <td key={col}
                        className={cn("px-2 py-0.5 font-mono text-[10px] cursor-text",
                          isDirty ? "text-[#a6e3a1] bg-[#a6e3a1]/5" : "text-[#cdd6f4]"
                        )}
                        onDoubleClick={() => setEditingCell({ uuid: row.uuid, col })}>
                        {isEditing ? (
                          <input
                            autoFocus
                            value={row[col] || ""}
                            onChange={(e) => setCellValue(row.uuid, col, e.target.value)}
                            onKeyDown={(e) => {
                              e.stopPropagation();
                              if (e.key === "Enter" || e.key === "Escape") setEditingCell(null);
                            }}
                            onBlur={() => setEditingCell(null)}
                            className="w-full bg-[#1e1e2e] border border-[#89b4fa] rounded px-1 py-0 text-[10px] font-mono text-[#cdd6f4] outline-none"
                          />
                        ) : (
                          <span className="truncate block">{row[col] || ""}</span>
                        )}
                      </td>
                    );
                  })}
                </tr>
              ))}
            </tbody>
          </table>
        </div>

        <div className="flex items-center justify-between px-4 py-3 border-t border-[#45475a]">
          <span className="text-[10px] text-[#6c7086]">
            {filteredRows.length} of {rows.length} components
            {dirtyCount > 0 && ` | ${dirtyCount} change${dirtyCount !== 1 ? "s" : ""}`}
          </span>
          <div className="flex gap-2">
            <button onClick={onClose}
              className="px-4 py-1.5 rounded text-xs bg-[#313244] text-[#a6adc8] hover:bg-[#45475a] transition-colors">
              Cancel
            </button>
            <button onClick={handleApply}
              className="px-4 py-1.5 rounded text-xs bg-[#89b4fa]/20 text-[#89b4fa] hover:bg-[#89b4fa]/30 transition-colors"
              disabled={dirtyCount === 0}>
              Apply Changes ({dirtyCount})
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
