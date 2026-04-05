import { Zap } from "lucide-react";

export function SignalPanel() {
  return (
    <div className="flex flex-col h-full">
      <div className="flex-1 flex flex-col items-center justify-center gap-3 p-6 text-text-muted text-xs">
        <Zap size={24} className="text-accent/40" />
        <span className="text-text-muted/50 font-medium">Signal</span>
        <span className="text-text-muted/30 text-[11px] text-center">
          AI-powered design assistant. Coming in Phase 1.
        </span>
      </div>
      <div className="border-t border-border-subtle p-2">
        <div className="flex gap-2">
          <input
            type="text"
            placeholder="Ask Signal about your design..."
            className="flex-1 bg-bg-surface border border-border-subtle rounded px-3 py-1.5 text-xs text-text-primary placeholder:text-text-muted/40 outline-none focus:border-accent/50"
            disabled
          />
          <button
            className="px-3 py-1.5 bg-accent/10 text-accent/40 rounded text-xs cursor-not-allowed"
            disabled
          >
            Send
          </button>
        </div>
      </div>
    </div>
  );
}
