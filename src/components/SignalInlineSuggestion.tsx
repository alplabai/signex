import { useState, useEffect, useRef } from "react";
import { Zap, Loader2, X } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { useSchematicStore } from "@/stores/schematic";
import { useEditorStore } from "@/stores/editor";
import { useSignalStore } from "@/stores/signal";
import { buildRichContext } from "@/lib/signalContext";

interface Props {
  targetUuid: string;
  targetType: "symbol" | "wire" | "label" | "net";
  targetInfo: string; // e.g., "R1 = 10k" or "Net: SDA"
  position: { x: number; y: number }; // Screen position for popover
  onClose: () => void;
}

export function SignalInlineSuggestion({ targetUuid, targetType, targetInfo, position, onClose }: Props) {
  const [suggestion, setSuggestion] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const popoverRef = useRef<HTMLDivElement>(null);

  const apiKeySet = useSignalStore((s) => s.apiKeySet);

  useEffect(() => {
    if (!apiKeySet) return;

    const fetchSuggestion = async () => {
      setLoading(true);
      setError(null);

      try {
        const data = useSchematicStore.getState().data;
        const selectedIds = useSchematicStore.getState().selectedIds;
        const ercMarkers = useEditorStore.getState().ercMarkers;
        const model = useSignalStore.getState().model;

        if (!data) throw new Error("No schematic loaded");

        const detailedContext = buildRichContext(data, selectedIds, ercMarkers);

        const prompt = targetType === "symbol"
          ? `Quick analysis of component ${targetInfo}. Any concerns about value, footprint, or connections? One paragraph max.`
          : targetType === "label" || targetType === "net"
          ? `Quick analysis of net/label "${targetInfo}". Any signal integrity or connectivity concerns? One paragraph max.`
          : `Quick analysis of ${targetType} "${targetInfo}". Any concerns? One paragraph max.`;

        const context = {
          component_count: data.symbols.filter((s) => !s.is_power).length,
          wire_count: data.wires.length,
          net_count: data.labels.length,
          selected_components: [],
          erc_errors: ercMarkers.filter((m) => m.severity === "error").length,
          erc_warnings: ercMarkers.filter((m) => m.severity === "warning").length,
          paper_size: data.paper_size,
          title: data.title_block?.title || "",
          detailed_context: detailedContext,
          design_brief: useSignalStore.getState().designBrief || null,
        };

        const response = await invoke<{ message: string; usage: { input_tokens: number; output_tokens: number } }>(
          "signal_chat",
          {
            messages: [{ role: "user", content: prompt }],
            context,
            model,
            imageBase64: null,
          }
        );

        setSuggestion(response.message);
        useSignalStore.getState().addTokens(response.usage.input_tokens, response.usage.output_tokens);
        useSignalStore.getState().addCost(response.usage.input_tokens, response.usage.output_tokens);
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      } finally {
        setLoading(false);
      }
    };

    fetchSuggestion();
  }, [targetUuid, targetType, targetInfo, apiKeySet]);

  // Click outside to close
  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (popoverRef.current && !popoverRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [onClose]);

  if (!apiKeySet) return null;

  return (
    <div
      ref={popoverRef}
      className="fixed z-[100] bg-[#1e1e2e] border border-[#45475a] rounded-lg shadow-2xl p-3 max-w-[300px] text-xs"
      style={{ left: position.x, top: position.y }}
    >
      <div className="flex items-center justify-between mb-2">
        <div className="flex items-center gap-1.5 text-accent">
          <Zap size={12} />
          <span className="text-[10px] font-semibold uppercase tracking-wider">Signal</span>
        </div>
        <button onClick={onClose} className="p-0.5 rounded hover:bg-[#313244] text-[#6c7086]">
          <X size={12} />
        </button>
      </div>

      <div className="text-[10px] text-[#a6adc8] mb-2 font-mono truncate">
        {targetInfo}
      </div>

      {loading && (
        <div className="flex items-center gap-2 text-accent/60 py-2">
          <Loader2 size={12} className="animate-spin" />
          <span className="text-[10px]">Analyzing...</span>
        </div>
      )}

      {error && (
        <div className="text-[10px] text-red-400 py-1">
          {error}
        </div>
      )}

      {suggestion && (
        <div className="text-[11px] text-[#cdd6f4] leading-relaxed whitespace-pre-wrap">
          {suggestion}
        </div>
      )}
    </div>
  );
}
