import { useState } from "react";
import { Scissors, Plus, Trash2, Copy } from "lucide-react";
import { useSchematicStore } from "@/stores/schematic";

interface Snippet {
  id: string;
  name: string;
  description: string;
  data: string; // JSON serialized clipboard data
  timestamp: number;
}

const STORAGE_KEY = "signex-snippets";

function loadSnippets(): Snippet[] {
  try {
    return JSON.parse(localStorage.getItem(STORAGE_KEY) || "[]");
  } catch { return []; }
}

function saveSnippets(snippets: Snippet[]) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(snippets));
}

export function SnippetsPanel() {
  const [snippets, setSnippets] = useState<Snippet[]>(loadSnippets);
  const [editingName, setEditingName] = useState<string | null>(null);

  const createSnippet = () => {
    const store = useSchematicStore.getState();
    if (!store.data || store.selectedIds.size === 0) return;

    // Copy selected elements as snippet
    store.copySelected();
    const clipboard = useSchematicStore.getState().clipboard;
    if (!clipboard) return;

    const snippet: Snippet = {
      id: crypto.randomUUID(),
      name: `Snippet ${snippets.length + 1}`,
      description: `${store.selectedIds.size} objects`,
      data: JSON.stringify(clipboard),
      timestamp: Date.now(),
    };

    const updated = [...snippets, snippet];
    setSnippets(updated);
    saveSnippets(updated);
  };

  const deleteSnippet = (id: string) => {
    const updated = snippets.filter((s) => s.id !== id);
    setSnippets(updated);
    saveSnippets(updated);
  };

  const placeSnippet = (snippet: Snippet) => {
    try {
      const clipboard = JSON.parse(snippet.data);
      useSchematicStore.setState({ clipboard });
      useSchematicStore.getState().pasteClipboard({ x: 5, y: 5 });
    } catch (e) {
      console.error("Failed to place snippet:", e);
    }
  };

  const renameSnippet = (id: string, name: string) => {
    const updated = snippets.map((s) => s.id === id ? { ...s, name } : s);
    setSnippets(updated);
    saveSnippets(updated);
    setEditingName(null);
  };

  return (
    <div className="text-xs h-full flex flex-col">
      <div className="flex items-center justify-between px-3 py-2 border-b border-border-subtle shrink-0">
        <span className="text-[11px] font-semibold text-text-secondary">Snippets</span>
        <button onClick={createSnippet} title="Create snippet from selection"
          className="flex items-center gap-1 text-[10px] text-accent hover:text-accent/80 transition-colors">
          <Plus size={12} /> Save
        </button>
      </div>

      <div className="flex-1 overflow-y-auto">
        {snippets.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full gap-2 text-text-muted/30 p-4">
            <Scissors size={20} />
            <span className="text-[11px] text-center">No snippets saved. Select objects and click Save.</span>
          </div>
        ) : (
          snippets.map((snippet) => (
            <div key={snippet.id}
              className="flex items-center gap-2 px-3 py-1.5 border-b border-border-subtle/30 hover:bg-bg-hover/50 transition-colors">
              <Scissors size={12} className="text-accent/50 shrink-0" />
              <div className="flex-1 min-w-0">
                {editingName === snippet.id ? (
                  <input autoFocus value={snippet.name}
                    onChange={(e) => {
                      const updated = snippets.map((s) => s.id === snippet.id ? { ...s, name: e.target.value } : s);
                      setSnippets(updated);
                    }}
                    onBlur={() => renameSnippet(snippet.id, snippet.name)}
                    onKeyDown={(e) => {
                      e.stopPropagation();
                      if (e.key === "Enter") renameSnippet(snippet.id, snippet.name);
                    }}
                    className="w-full bg-bg-surface border border-accent rounded px-1 py-0 text-[10px] outline-none" />
                ) : (
                  <div className="text-[11px] text-text-primary truncate cursor-pointer"
                    onClick={() => setEditingName(snippet.id)}>{snippet.name}</div>
                )}
                <div className="text-[9px] text-text-muted/40">{snippet.description}</div>
              </div>
              <button onClick={() => placeSnippet(snippet)} title="Place snippet"
                className="p-0.5 rounded text-text-muted/40 hover:text-accent transition-colors">
                <Copy size={11} />
              </button>
              <button onClick={() => deleteSnippet(snippet.id)} title="Delete"
                className="p-0.5 rounded text-text-muted/40 hover:text-error transition-colors">
                <Trash2 size={11} />
              </button>
            </div>
          ))
        )}
      </div>
    </div>
  );
}
