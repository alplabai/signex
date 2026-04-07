import { useState, useRef, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Zap, Send, Trash2, Key, AlertCircle, Cpu, FileSearch, Loader2 } from "lucide-react";
import { useSignalStore } from "@/stores/signal";
import { useSchematicStore } from "@/stores/schematic";
import { useEditorStore } from "@/stores/editor";
import { cn } from "@/lib/utils";
import type { SignalMessage } from "@/stores/signal";

function buildContext() {
  const data = useSchematicStore.getState().data;
  const selectedIds = useSchematicStore.getState().selectedIds;
  const ercMarkers = useEditorStore.getState().ercMarkers;

  if (!data) {
    return {
      component_count: 0, wire_count: 0, net_count: 0,
      selected_components: [], erc_errors: 0, erc_warnings: 0,
      paper_size: "A4", title: "",
    };
  }

  const selectedComponents = data.symbols
    .filter((s) => selectedIds.has(s.uuid) && !s.is_power)
    .map((s) => ({
      reference: s.reference,
      value: s.value,
      footprint: s.footprint,
      lib_id: s.lib_id,
    }));

  return {
    component_count: data.symbols.filter((s) => !s.is_power).length,
    wire_count: data.wires.length,
    net_count: data.labels.length,
    selected_components: selectedComponents,
    erc_errors: ercMarkers.filter((m) => m.severity === "error").length,
    erc_warnings: ercMarkers.filter((m) => m.severity === "warning").length,
    paper_size: data.paper_size,
    title: data.title_block?.title || "",
  };
}

export function SignalPanel() {
  const messages = useSignalStore((s) => s.messages);
  const apiKeySet = useSignalStore((s) => s.apiKeySet);
  const isLoading = useSignalStore((s) => s.isLoading);
  const totalTokens = useSignalStore((s) => s.totalTokens);
  const [input, setInput] = useState("");
  const [showKeyInput, setShowKeyInput] = useState(false);
  const [apiKey, setApiKey] = useState("");
  const scrollRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  // Check API key on mount
  useEffect(() => {
    invoke<boolean>("has_api_key").then((has) => {
      useSignalStore.getState().setApiKeySet(has);
    });
  }, []);

  // Auto-scroll to bottom
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages]);

  const saveApiKey = async () => {
    try {
      await invoke("set_api_key", { key: apiKey });
      useSignalStore.getState().setApiKeySet(true);
      setShowKeyInput(false);
      setApiKey("");
    } catch (e) {
      console.error("Failed to set API key:", e);
    }
  };

  const sendMessage = async () => {
    const text = input.trim();
    if (!text || isLoading) return;

    setInput("");
    const store = useSignalStore.getState();

    // Add user message
    store.addMessage({ role: "user", content: text });

    // Add loading placeholder
    const loadingId = crypto.randomUUID();
    store.addMessage({ role: "assistant", content: "", loading: true, id: loadingId } as SignalMessage);
    store.setLoading(true);

    try {
      const chatMessages = [...store.messages.filter((m) => !m.loading), { role: "user", content: text }]
        .map((m) => ({ role: m.role, content: m.content }));

      const context = buildContext();
      const response = await invoke<{ message: string; usage: { input_tokens: number; output_tokens: number } }>(
        "signal_chat", { messages: chatMessages, context }
      );

      store.updateMessage(loadingId, {
        content: response.message,
        loading: false,
        usage: response.usage,
      });
      store.addTokens(response.usage.input_tokens, response.usage.output_tokens);
    } catch (e) {
      store.updateMessage(loadingId, {
        content: `Error: ${e instanceof Error ? e.message : String(e)}`,
        loading: false,
        role: "system",
      });
    } finally {
      store.setLoading(false);
    }
  };

  const runDesignReview = async () => {
    const store = useSignalStore.getState();
    store.addMessage({ role: "user", content: "Review my schematic design" });

    const loadingId = crypto.randomUUID();
    store.addMessage({ role: "assistant", content: "", loading: true, id: loadingId } as SignalMessage);
    store.setLoading(true);

    try {
      const context = buildContext();
      const response = await invoke<{ message: string; usage: { input_tokens: number; output_tokens: number } }>(
        "signal_review", { context }
      );
      store.updateMessage(loadingId, {
        content: response.message,
        loading: false,
        usage: response.usage,
      });
      store.addTokens(response.usage.input_tokens, response.usage.output_tokens);
    } catch (e) {
      store.updateMessage(loadingId, {
        content: `Error: ${e instanceof Error ? e.message : String(e)}`,
        loading: false,
        role: "system",
      });
    } finally {
      store.setLoading(false);
    }
  };

  // No API key state
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

  // API key input state
  if (showKeyInput) {
    return (
      <div className="flex flex-col h-full items-center justify-center gap-3 p-6 text-xs">
        <Key size={20} className="text-accent/60" />
        <span className="text-text-secondary font-semibold">Anthropic API Key</span>
        <input
          type="password"
          value={apiKey}
          onChange={(e) => setApiKey(e.target.value)}
          onKeyDown={(e) => { e.stopPropagation(); if (e.key === "Enter") saveApiKey(); }}
          placeholder="sk-ant-..."
          className="w-full max-w-[280px] bg-bg-surface border border-border-subtle rounded px-3 py-2 text-[11px] font-mono text-text-primary outline-none focus:border-accent"
        />
        <div className="flex gap-2">
          <button onClick={() => setShowKeyInput(false)}
            className="px-3 py-1.5 rounded text-[11px] bg-bg-hover text-text-muted hover:text-text-secondary transition-colors">
            Cancel
          </button>
          <button onClick={saveApiKey} disabled={!apiKey.startsWith("sk-")}
            className="px-3 py-1.5 rounded text-[11px] bg-accent/20 text-accent hover:bg-accent/30 transition-colors disabled:opacity-40">
            Save
          </button>
        </div>
        <span className="text-text-muted/30 text-[10px] text-center max-w-[250px]">
          Key is stored in memory only. It's never saved to disk or sent anywhere except the Anthropic API.
        </span>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full text-xs">
      {/* Toolbar */}
      <div className="flex items-center gap-1.5 px-3 py-1.5 border-b border-border-subtle shrink-0">
        <Zap size={12} className="text-accent" />
        <span className="text-[10px] font-semibold text-accent uppercase tracking-wider">Signal</span>
        <div className="flex-1" />
        <button onClick={runDesignReview} disabled={isLoading} title="Design Review"
          className="p-1 rounded text-text-muted/50 hover:text-accent hover:bg-accent/10 transition-colors disabled:opacity-30">
          <FileSearch size={13} />
        </button>
        <button onClick={() => useSignalStore.getState().clearHistory()} title="Clear History"
          className="p-1 rounded text-text-muted/50 hover:text-error hover:bg-error/10 transition-colors">
          <Trash2 size={13} />
        </button>
        <button onClick={() => setShowKeyInput(true)} title="Change API Key"
          className="p-1 rounded text-text-muted/50 hover:text-text-secondary transition-colors">
          <Key size={13} />
        </button>
        {totalTokens > 0 && (
          <span className="text-[9px] text-text-muted/30 font-mono tabular-nums">
            {(totalTokens / 1000).toFixed(1)}k
          </span>
        )}
      </div>

      {/* Messages */}
      <div ref={scrollRef} className="flex-1 overflow-y-auto p-3 space-y-3">
        {messages.length === 0 && (
          <div className="flex flex-col items-center justify-center h-full gap-3 text-text-muted/30">
            <Zap size={20} />
            <span className="text-[11px]">Ask Signal about your design</span>
            <div className="flex flex-wrap gap-1.5 justify-center max-w-[250px]">
              {["Review my design", "Suggest bypass caps", "Fix ERC errors", "Component alternatives"].map((q) => (
                <button key={q} onClick={() => { setInput(q); inputRef.current?.focus(); }}
                  className="px-2 py-1 rounded bg-bg-surface border border-border-subtle text-[10px] text-text-muted hover:text-accent hover:border-accent/30 transition-colors">
                  {q}
                </button>
              ))}
            </div>
          </div>
        )}

        {messages.map((msg) => (
          <MessageBubble key={msg.id} message={msg} />
        ))}
      </div>

      {/* Input */}
      <div className="border-t border-border-subtle p-2 shrink-0">
        <div className="flex gap-2">
          <input
            ref={inputRef}
            type="text"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => {
              e.stopPropagation();
              if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); sendMessage(); }
            }}
            placeholder="Ask Signal about your design..."
            className="flex-1 bg-bg-surface border border-border-subtle rounded px-3 py-1.5 text-[11px] text-text-primary placeholder:text-text-muted/40 outline-none focus:border-accent/50"
            disabled={isLoading}
          />
          <button onClick={sendMessage} disabled={isLoading || !input.trim()}
            className={cn(
              "px-3 py-1.5 rounded text-[11px] transition-colors flex items-center gap-1",
              isLoading ? "bg-accent/10 text-accent/40" : "bg-accent/20 text-accent hover:bg-accent/30"
            )}>
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

  if (message.loading) {
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
        isUser
          ? "bg-accent/15 text-text-primary"
          : isSystem
            ? "bg-error/10 text-error border border-error/20"
            : "bg-bg-surface border border-border-subtle text-text-secondary"
      )}>
        {isSystem && (
          <div className="flex items-center gap-1 mb-1 text-[10px] text-error/70">
            <AlertCircle size={10} /> Error
          </div>
        )}
        {!isUser && !isSystem && (
          <div className="flex items-center gap-1 mb-1 text-[10px] text-accent/50">
            <Cpu size={10} /> Signal
          </div>
        )}
        <div className="whitespace-pre-wrap break-words">
          {renderMarkdown(message.content)}
        </div>
      </div>
      {message.usage && (
        <span className="text-[9px] text-text-muted/20 font-mono px-1">
          {message.usage.input_tokens + message.usage.output_tokens} tokens
        </span>
      )}
    </div>
  );
}

/**
 * Minimal markdown rendering — bold, inline code, code blocks, lists
 */
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
        parts.push(
          <pre key={`code-${codeKey++}`} className="bg-bg-primary rounded p-2 my-1 text-[10px] font-mono overflow-x-auto border border-border-subtle">
            {codeContent}
          </pre>
        );
        codeContent = "";
        inCodeBlock = false;
      } else {
        inCodeBlock = true;
      }
      continue;
    }

    if (inCodeBlock) {
      codeContent += (codeContent ? "\n" : "") + line;
      continue;
    }

    // Headers
    if (line.startsWith("### ")) {
      parts.push(<div key={i} className="font-semibold text-text-primary mt-2 mb-0.5">{line.slice(4)}</div>);
    } else if (line.startsWith("## ")) {
      parts.push(<div key={i} className="font-bold text-text-primary mt-2 mb-0.5">{line.slice(3)}</div>);
    } else if (line.startsWith("# ")) {
      parts.push(<div key={i} className="font-bold text-text-primary text-xs mt-2 mb-1">{line.slice(2)}</div>);
    } else if (line.startsWith("- ") || line.startsWith("* ")) {
      parts.push(<div key={i} className="pl-3">{"\u2022 "}{formatInline(line.slice(2))}</div>);
    } else if (/^\d+\.\s/.test(line)) {
      const match = line.match(/^(\d+\.)\s(.*)$/);
      if (match) parts.push(<div key={i} className="pl-3">{match[1]} {formatInline(match[2])}</div>);
    } else if (line.trim() === "") {
      parts.push(<div key={i} className="h-1" />);
    } else {
      parts.push(<div key={i}>{formatInline(line)}</div>);
    }
  }

  return <>{parts}</>;
}

function formatInline(text: string): React.ReactNode {
  // Bold, inline code
  const parts: React.ReactNode[] = [];
  let remaining = text;
  let key = 0;

  while (remaining.length > 0) {
    // Inline code
    const codeMatch = remaining.match(/^(.*?)`([^`]+)`(.*)$/);
    if (codeMatch) {
      if (codeMatch[1]) parts.push(formatBold(codeMatch[1], key++));
      parts.push(
        <code key={`ic-${key++}`} className="bg-bg-primary px-1 py-0.5 rounded text-accent font-mono text-[10px]">
          {codeMatch[2]}
        </code>
      );
      remaining = codeMatch[3];
      continue;
    }
    parts.push(formatBold(remaining, key++));
    break;
  }
  return <>{parts}</>;
}

function formatBold(text: string, key: number): React.ReactNode {
  const parts: React.ReactNode[] = [];
  const boldRegex = /\*\*(.+?)\*\*/g;
  let lastIndex = 0;
  let match;

  while ((match = boldRegex.exec(text)) !== null) {
    if (match.index > lastIndex) parts.push(text.slice(lastIndex, match.index));
    parts.push(<strong key={`b-${key}-${match.index}`}>{match[1]}</strong>);
    lastIndex = match.index + match[0].length;
  }
  if (lastIndex < text.length) parts.push(text.slice(lastIndex));
  if (parts.length === 0) return text;
  return <>{parts}</>;
}
