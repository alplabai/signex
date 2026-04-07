import { useState, useRef, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Zap, Send, Trash2, Key, AlertCircle, Cpu, FileSearch, Loader2,
  Download, Camera, ChevronDown, ChevronRight,
} from "lucide-react";
import { useSignalStore } from "@/stores/signal";
import { useSchematicStore } from "@/stores/schematic";
import { useEditorStore } from "@/stores/editor";
import { buildRichContext, captureSchematicScreenshot, estimateCost } from "@/lib/signalContext";
import { cn } from "@/lib/utils";
import type { SignalMessage } from "@/stores/signal";

function buildContext() {
  const data = useSchematicStore.getState().data;
  const selectedIds = useSchematicStore.getState().selectedIds;
  const ercMarkers = useEditorStore.getState().ercMarkers;
  const designBrief = useSignalStore.getState().designBrief;

  if (!data) {
    return {
      component_count: 0, wire_count: 0, net_count: 0,
      selected_components: [] as { reference: string; value: string; footprint: string; lib_id: string }[],
      erc_errors: 0, erc_warnings: 0, paper_size: "A4", title: "",
      detailed_context: null as string | null,
      design_brief: designBrief || null,
    };
  }

  const selectedComponents = data.symbols
    .filter((s) => selectedIds.has(s.uuid) && !s.is_power)
    .map((s) => ({ reference: s.reference, value: s.value, footprint: s.footprint, lib_id: s.lib_id }));

  const detailedContext = buildRichContext(data, selectedIds, ercMarkers);

  return {
    component_count: data.symbols.filter((s) => !s.is_power).length,
    wire_count: data.wires.length,
    net_count: data.labels.length,
    selected_components: selectedComponents,
    erc_errors: ercMarkers.filter((m) => m.severity === "error").length,
    erc_warnings: ercMarkers.filter((m) => m.severity === "warning").length,
    paper_size: data.paper_size,
    title: data.title_block?.title || "",
    detailed_context: detailedContext,
    design_brief: designBrief || null,
  };
}

export function SignalPanel() {
  const messages = useSignalStore((s) => s.messages);
  const apiKeySet = useSignalStore((s) => s.apiKeySet);
  const isLoading = useSignalStore((s) => s.isLoading);
  const totalTokens = useSignalStore((s) => s.totalTokens);
  const sessionCost = useSignalStore((s) => s.sessionCost);
  const model = useSignalStore((s) => s.model);
  const designBrief = useSignalStore((s) => s.designBrief);
  const [input, setInput] = useState("");
  const [showKeyInput, setShowKeyInput] = useState(false);
  const [apiKey, setApiKey] = useState("");
  const [showBrief, setShowBrief] = useState(false);
  const [includeScreenshot, setIncludeScreenshot] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    invoke<boolean>("has_api_key").then((has) => useSignalStore.getState().setApiKeySet(has));
  }, []);

  useEffect(() => {
    if (scrollRef.current) scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
  }, [messages]);

  const saveApiKey = async () => {
    try {
      await invoke("set_api_key", { key: apiKey });
      useSignalStore.getState().setApiKeySet(true);
      setShowKeyInput(false);
      setApiKey("");
    } catch (e) { console.error("Failed to set API key:", e); }
  };

  const sendMessage = async () => {
    const text = input.trim();
    if (!text || isLoading) return;
    setInput("");
    const store = useSignalStore.getState();

    store.addMessage({ role: "user", content: text });

    const msgId = crypto.randomUUID();
    store.addMessage({ role: "assistant", content: "", loading: true, id: msgId } as SignalMessage);
    store.setLoading(true);

    // Start stream listener before sending
    await store.startStreamListener(msgId);

    try {
      const chatMessages = [...store.messages.filter((m) => !m.loading), { role: "user", content: text }]
        .map((m) => ({ role: m.role, content: m.content }));

      const context = buildContext();

      // Capture screenshot if enabled
      let imageBase64: string | null = null;
      if (includeScreenshot) {
        const data = useSchematicStore.getState().data;
        if (data) imageBase64 = captureSchematicScreenshot(data);
      }

      await invoke("signal_chat_stream", {
        messageId: msgId,
        messages: chatMessages,
        context,
        model: store.model,
        imageBase64,
      });
    } catch (e) {
      store.updateMessage(msgId, {
        content: `Error: ${e instanceof Error ? e.message : String(e)}`,
        loading: false,
        role: "system",
      });
      store.setLoading(false);
    }
  };

  const runDesignReview = async () => {
    const store = useSignalStore.getState();
    store.addMessage({ role: "user", content: "Review my schematic design" });
    const msgId = crypto.randomUUID();
    store.addMessage({ role: "assistant", content: "", loading: true, id: msgId } as SignalMessage);
    store.setLoading(true);
    await store.startStreamListener(msgId);

    try {
      const context = buildContext();
      const chatMessages = [{ role: "user", content: "Review this schematic design. Check for missing bypass capacitors, incorrect pull-up/pull-down values, power rail issues, and signal integrity concerns. Provide a brief, actionable review." }];
      await invoke("signal_chat_stream", {
        messageId: msgId, messages: chatMessages, context,
        model: store.model, imageBase64: null,
      });
    } catch (e) {
      store.updateMessage(msgId, { content: `Error: ${e}`, loading: false, role: "system" });
      store.setLoading(false);
    }
  };

  const exportChat = () => {
    const lines = messages.map((m) => {
      const role = m.role === "user" ? "**You**" : m.role === "assistant" ? "**Signal**" : "**Error**";
      return `### ${role}\n\n${m.content}\n`;
    });
    const md = `# Signal AI Chat\n\n_Exported ${new Date().toLocaleString()}_\n\n---\n\n${lines.join("\n---\n\n")}\n\n---\n_Tokens: ${totalTokens} | Cost: $${sessionCost.toFixed(4)}_\n`;
    const blob = new Blob([md], { type: "text/markdown" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url; a.download = "signal-chat.md"; a.click();
    URL.revokeObjectURL(url);
  };

  // No API key
  if (!apiKeySet && !showKeyInput) {
    return (
      <div className="flex flex-col h-full items-center justify-center gap-4 p-6 text-xs">
        <Zap size={28} className="text-accent/60" />
        <span className="text-text-secondary font-semibold text-sm">Signal AI</span>
        <span className="text-text-muted/50 text-center text-[11px] max-w-[200px]">
          AI-powered design assistant. Enter your Anthropic API key to get started.
        </span>
        <button onClick={() => setShowKeyInput(true)}
          className="flex items-center gap-2 px-4 py-2 rounded bg-accent/20 text-accent hover:bg-accent/30 transition-colors text-[11px] font-medium">
          <Key size={14} /> Set API Key
        </button>
      </div>
    );
  }

  if (showKeyInput) {
    return (
      <div className="flex flex-col h-full items-center justify-center gap-3 p-6 text-xs">
        <Key size={20} className="text-accent/60" />
        <span className="text-text-secondary font-semibold">Anthropic API Key</span>
        <input type="password" value={apiKey}
          onChange={(e) => setApiKey(e.target.value)}
          onKeyDown={(e) => { e.stopPropagation(); if (e.key === "Enter") saveApiKey(); }}
          placeholder="sk-ant-..."
          className="w-full max-w-[280px] bg-bg-surface border border-border-subtle rounded px-3 py-2 text-[11px] font-mono text-text-primary outline-none focus:border-accent" />
        <div className="flex gap-2">
          <button onClick={() => setShowKeyInput(false)} className="px-3 py-1.5 rounded text-[11px] bg-bg-hover text-text-muted">Cancel</button>
          <button onClick={saveApiKey} disabled={!apiKey.startsWith("sk-")}
            className="px-3 py-1.5 rounded text-[11px] bg-accent/20 text-accent hover:bg-accent/30 disabled:opacity-40">Save</button>
        </div>
        <span className="text-text-muted/30 text-[10px] text-center max-w-[250px]">
          Key is stored in memory only. Never saved to disk.
        </span>
      </div>
    );
  }

  const estCost = estimateCost(
    Math.ceil((input.length + 2000) / 4), // rough input estimate
    500, // assume ~500 output tokens
    model
  );

  return (
    <div className="flex flex-col h-full text-xs">
      {/* Toolbar */}
      <div className="flex items-center gap-1 px-3 py-1.5 border-b border-border-subtle shrink-0">
        <Zap size={12} className="text-accent" />
        <span className="text-[10px] font-semibold text-accent uppercase tracking-wider">Signal</span>
        <div className="flex-1" />

        {/* Model selector */}
        <select value={model} onChange={(e) => useSignalStore.getState().setModel(e.target.value)}
          className="bg-transparent border border-border-subtle rounded px-1.5 py-0.5 text-[9px] text-text-muted outline-none focus:border-accent">
          <option value="claude-sonnet-4-20250514">Sonnet 4</option>
          <option value="claude-opus-4-20250514">Opus 4</option>
        </select>

        <button onClick={() => setIncludeScreenshot(!includeScreenshot)} title={includeScreenshot ? "Screenshot ON" : "Screenshot OFF"}
          className={cn("p-1 rounded transition-colors", includeScreenshot ? "text-accent bg-accent/10" : "text-text-muted/50 hover:text-text-secondary")}>
          <Camera size={13} />
        </button>
        <button onClick={runDesignReview} disabled={isLoading} title="Design Review"
          className="p-1 rounded text-text-muted/50 hover:text-accent hover:bg-accent/10 transition-colors disabled:opacity-30">
          <FileSearch size={13} />
        </button>
        <button onClick={exportChat} title="Export Chat" disabled={messages.length === 0}
          className="p-1 rounded text-text-muted/50 hover:text-text-secondary transition-colors disabled:opacity-30">
          <Download size={13} />
        </button>
        <button onClick={() => useSignalStore.getState().clearHistory()} title="Clear"
          className="p-1 rounded text-text-muted/50 hover:text-error hover:bg-error/10 transition-colors">
          <Trash2 size={13} />
        </button>
        <button onClick={() => setShowKeyInput(true)} title="API Key"
          className="p-1 rounded text-text-muted/50 hover:text-text-secondary transition-colors">
          <Key size={13} />
        </button>

        {/* Cost display */}
        {sessionCost > 0 && (
          <span className="text-[9px] text-text-muted/30 font-mono tabular-nums" title={`${totalTokens} tokens`}>
            ${sessionCost.toFixed(3)}
          </span>
        )}
      </div>

      {/* Design Brief (collapsible) */}
      <div className="border-b border-border-subtle">
        <button onClick={() => setShowBrief(!showBrief)}
          className="w-full flex items-center gap-1 px-3 py-1 text-[10px] text-text-muted/50 hover:text-text-muted transition-colors">
          {showBrief ? <ChevronDown size={10} /> : <ChevronRight size={10} />}
          Design Brief {designBrief && <span className="text-accent/50">*</span>}
        </button>
        {showBrief && (
          <div className="px-3 pb-2">
            <textarea
              value={designBrief}
              onChange={(e) => useSignalStore.getState().setDesignBrief(e.target.value)}
              onKeyDown={(e) => e.stopPropagation()}
              placeholder="Describe your design intent... (e.g., USB-C PD board with STM32, 5V/3A output)"
              className="w-full h-16 bg-bg-surface border border-border-subtle rounded px-2 py-1.5 text-[10px] text-text-primary placeholder:text-text-muted/30 outline-none focus:border-accent/50 resize-none"
            />
          </div>
        )}
      </div>

      {/* Messages */}
      <div ref={scrollRef} className="flex-1 overflow-y-auto p-3 space-y-3">
        {messages.length === 0 && (
          <div className="flex flex-col items-center justify-center h-full gap-3 text-text-muted/30">
            <Zap size={20} />
            <span className="text-[11px]">Ask Signal about your design</span>
            <div className="flex flex-wrap gap-1.5 justify-center max-w-[280px]">
              {["Review my design", "Suggest bypass caps", "Fix ERC errors", "Component alternatives", "Create LDO circuit", "Optimize BOM"].map((q) => (
                <button key={q} onClick={() => { setInput(q); inputRef.current?.focus(); }}
                  className="px-2 py-1 rounded bg-bg-surface border border-border-subtle text-[10px] text-text-muted hover:text-accent hover:border-accent/30 transition-colors">
                  {q}
                </button>
              ))}
            </div>
          </div>
        )}
        {messages.map((msg) => <MessageBubble key={msg.id} message={msg} />)}
      </div>

      {/* Input */}
      <div className="border-t border-border-subtle p-2 shrink-0">
        <div className="flex gap-2">
          <input ref={inputRef} type="text" value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => { e.stopPropagation(); if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); sendMessage(); } }}
            placeholder="Ask Signal..."
            className="flex-1 bg-bg-surface border border-border-subtle rounded px-3 py-1.5 text-[11px] text-text-primary placeholder:text-text-muted/40 outline-none focus:border-accent/50"
            disabled={isLoading} />
          <button onClick={sendMessage} disabled={isLoading || !input.trim()}
            title={input.trim() ? `~$${estCost.toFixed(4)}` : ""}
            className={cn("px-3 py-1.5 rounded text-[11px] transition-colors flex items-center gap-1",
              isLoading ? "bg-accent/10 text-accent/40" : "bg-accent/20 text-accent hover:bg-accent/30")}>
            {isLoading ? <Loader2 size={12} className="animate-spin" /> : <Send size={12} />}
          </button>
        </div>
      </div>
    </div>
  );
}

function MessageBubble({ message }: { message: SignalMessage }) {
  const isUser = message.role === "user";
  const isSystem = message.role === "system";

  if (message.loading && !message.content) {
    return (
      <div className="flex items-center gap-2 text-accent/60">
        <Loader2 size={14} className="animate-spin" />
        <span className="text-[11px]">Signal is thinking...</span>
      </div>
    );
  }

  return (
    <div className={cn("flex flex-col gap-1", isUser ? "items-end" : "items-start")}>
      <div className={cn(
        "rounded-lg px-3 py-2 max-w-[90%] text-[11px] leading-relaxed",
        isUser ? "bg-accent/15 text-text-primary"
          : isSystem ? "bg-error/10 text-error border border-error/20"
          : "bg-bg-surface border border-border-subtle text-text-secondary"
      )}>
        {isSystem && <div className="flex items-center gap-1 mb-1 text-[10px] text-error/70"><AlertCircle size={10} /> Error</div>}
        {!isUser && !isSystem && <div className="flex items-center gap-1 mb-1 text-[10px] text-accent/50"><Cpu size={10} /> Signal</div>}
        <div className="whitespace-pre-wrap break-words">{renderMarkdown(message.content)}</div>
        {message.loading && <Loader2 size={10} className="animate-spin text-accent/40 mt-1" />}
        {message.toolCalls && message.toolCalls.length > 0 && (
          <div className="mt-2 space-y-1">
            {message.toolCalls.map((tc) => (
              <div key={tc.id} className="flex items-center gap-1.5 text-[9px] text-accent/60 bg-accent/5 rounded px-2 py-0.5">
                <Zap size={8} /> {tc.name}({JSON.stringify(tc.input).slice(0, 60)}...)
              </div>
            ))}
          </div>
        )}
      </div>
      {message.usage && (
        <span className="text-[9px] text-text-muted/20 font-mono px-1">
          {message.usage.input_tokens + message.usage.output_tokens} tok
        </span>
      )}
    </div>
  );
}

// --- Markdown renderer ---

function renderMarkdown(text: string): React.ReactNode {
  if (!text) return null;
  const parts: React.ReactNode[] = [];
  const lines = text.split("\n");
  let inCodeBlock = false;
  let codeContent = "";
  let codeKey = 0;

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (line.startsWith("```")) {
      if (inCodeBlock) {
        parts.push(<pre key={`code-${codeKey++}`} className="bg-bg-primary rounded p-2 my-1 text-[10px] font-mono overflow-x-auto border border-border-subtle">{codeContent}</pre>);
        codeContent = "";
        inCodeBlock = false;
      } else { inCodeBlock = true; }
      continue;
    }
    if (inCodeBlock) { codeContent += (codeContent ? "\n" : "") + line; continue; }
    if (line.startsWith("### ")) { parts.push(<div key={i} className="font-semibold text-text-primary mt-2 mb-0.5">{line.slice(4)}</div>); }
    else if (line.startsWith("## ")) { parts.push(<div key={i} className="font-bold text-text-primary mt-2 mb-0.5">{line.slice(3)}</div>); }
    else if (line.startsWith("# ")) { parts.push(<div key={i} className="font-bold text-text-primary text-xs mt-2 mb-1">{line.slice(2)}</div>); }
    else if (line.startsWith("- ") || line.startsWith("* ")) { parts.push(<div key={i} className="pl-3">{"\u2022 "}{formatInline(line.slice(2))}</div>); }
    else if (/^\d+\.\s/.test(line)) { const m = line.match(/^(\d+\.)\s(.*)$/); if (m) parts.push(<div key={i} className="pl-3">{m[1]} {formatInline(m[2])}</div>); }
    else if (line.trim() === "") { parts.push(<div key={i} className="h-1" />); }
    else { parts.push(<div key={i}>{formatInline(line)}</div>); }
  }
  return <>{parts}</>;
}

function formatInline(text: string): React.ReactNode {
  const parts: React.ReactNode[] = [];
  let remaining = text;
  let key = 0;
  while (remaining.length > 0) {
    const codeMatch = remaining.match(/^(.*?)`([^`]+)`(.*)$/);
    if (codeMatch) {
      if (codeMatch[1]) parts.push(formatBold(codeMatch[1], key++));
      parts.push(<code key={`ic-${key++}`} className="bg-bg-primary px-1 py-0.5 rounded text-accent font-mono text-[10px]">{codeMatch[2]}</code>);
      remaining = codeMatch[3]; continue;
    }
    parts.push(formatBold(remaining, key++));
    break;
  }
  return <>{parts}</>;
}

function formatBold(text: string, key: number): React.ReactNode {
  const parts: React.ReactNode[] = [];
  const re = /\*\*(.+?)\*\*/g;
  let last = 0, match;
  while ((match = re.exec(text)) !== null) {
    if (match.index > last) parts.push(text.slice(last, match.index));
    parts.push(<strong key={`b-${key}-${match.index}`}>{match[1]}</strong>);
    last = match.index + match[0].length;
  }
  if (last < text.length) parts.push(text.slice(last));
  return parts.length === 0 ? text : <>{parts}</>;
}
