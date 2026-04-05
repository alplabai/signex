import { CheckCircle2 } from "lucide-react";

export function MessagesPanel() {
  return (
    <div className="flex items-center gap-2.5 px-4 py-3 text-xs">
      <CheckCircle2 size={13} className="text-success/60" />
      <span className="text-text-muted">Ready</span>
      <span className="text-text-muted/40">|</span>
      <span className="text-text-muted/40">Open a project to begin designing</span>
    </div>
  );
}
