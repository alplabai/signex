import { Info } from "lucide-react";

export function MessagesPanel() {
  return (
    <div className="flex items-center gap-2 p-3 text-xs text-text-muted">
      <Info size={14} />
      <span>Ready. Open a project to begin.</span>
    </div>
  );
}
