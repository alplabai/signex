import { create } from "zustand";
import { persist } from "zustand/middleware";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface SignalMessage {
  id: string;
  role: "user" | "assistant" | "system";
  content: string;
  timestamp: number;
  usage?: { input_tokens: number; output_tokens: number };
  loading?: boolean;
  toolCalls?: { id: string; name: string; input: Record<string, unknown> }[];
}

// Module-level listener registry — not in store state to avoid persistence issues
const activeListeners = new Map<string, UnlistenFn[]>();

function cleanupListeners(messageId: string) {
  const fns = activeListeners.get(messageId);
  if (fns) {
    for (const fn of fns) fn();
    activeListeners.delete(messageId);
  }
}

function cleanupAllListeners() {
  for (const [id] of activeListeners) cleanupListeners(id);
}

interface SignalState {
  messages: SignalMessage[];
  apiKeySet: boolean;
  isLoading: boolean;
  totalTokens: number;
  sessionCost: number;
  model: string;
  designBrief: string;

  addMessage: (msg: Omit<SignalMessage, "id" | "timestamp"> & { id?: string }) => void;
  updateMessage: (id: string, updates: Partial<SignalMessage>) => void;
  removeMessage: (id: string) => void;
  clearHistory: () => void;
  setApiKeySet: (v: boolean) => void;
  setLoading: (v: boolean) => void;
  addTokens: (input: number, output: number) => void;
  addCost: (input: number, output: number) => void;
  setModel: (model: string) => void;
  setDesignBrief: (brief: string) => void;
  startStreamListener: (messageId: string) => Promise<void>;
  cancelStream: (messageId: string) => void;
}

export const useSignalStore = create<SignalState>()(
  persist(
    (set, get) => ({
      messages: [],
      apiKeySet: false,
      isLoading: false,
      totalTokens: 0,
      sessionCost: 0,
      model: "claude-sonnet-4-20250514",
      designBrief: "",

      addMessage: (msg) =>
        set((s) => ({
          messages: [
            ...s.messages,
            { ...msg, id: msg.id || crypto.randomUUID(), timestamp: Date.now() } as SignalMessage,
          ],
        })),

      updateMessage: (id, updates) =>
        set((s) => ({
          messages: s.messages.map((m) => (m.id === id ? { ...m, ...updates } : m)),
        })),

      removeMessage: (id) =>
        set((s) => ({ messages: s.messages.filter((m) => m.id !== id) })),

      clearHistory: () => {
        cleanupAllListeners();
        set({ messages: [], totalTokens: 0, sessionCost: 0, isLoading: false });
      },

      setApiKeySet: (v) => set({ apiKeySet: v }),
      setLoading: (v) => set({ isLoading: v }),
      setModel: (model) => set({ model }),
      setDesignBrief: (brief) => set({ designBrief: brief }),

      addTokens: (input, output) =>
        set((s) => ({ totalTokens: s.totalTokens + input + output })),

      addCost: (input, output) => {
        const model = get().model;
        const isOpus = model.includes("opus");
        const cost = isOpus
          ? (input * 15 + output * 75) / 1_000_000
          : (input * 3 + output * 15) / 1_000_000;
        set((s) => ({ sessionCost: s.sessionCost + cost }));
      },

      startStreamListener: async (messageId) => {
        cleanupListeners(messageId);
        const unlisteners: UnlistenFn[] = [];

        const u1 = await listen<{ text: string; message_id: string }>(
          "signal:stream-delta",
          (event) => {
            if (event.payload.message_id === messageId) {
              set((s) => ({
                messages: s.messages.map((m) =>
                  m.id === messageId
                    ? { ...m, content: (m.content || "") + event.payload.text }
                    : m
                ),
              }));
            }
          }
        );
        unlisteners.push(u1);

        const u2 = await listen<{
          message_id: string;
          usage: { input_tokens: number; output_tokens: number };
          tool_calls: { id: string; name: string; input: Record<string, unknown> }[];
          stop_reason: string;
        }>("signal:stream-done", (event) => {
          if (event.payload.message_id === messageId) {
            const s = useSignalStore.getState();
            s.updateMessage(messageId, {
              loading: false,
              usage: event.payload.usage,
              toolCalls: event.payload.tool_calls.length > 0 ? event.payload.tool_calls : undefined,
            });
            s.addTokens(event.payload.usage.input_tokens, event.payload.usage.output_tokens);
            s.addCost(event.payload.usage.input_tokens, event.payload.usage.output_tokens);
            s.setLoading(false);
            cleanupListeners(messageId);
          }
        });
        unlisteners.push(u2);

        const u3 = await listen<{ message_id: string; error: string }>(
          "signal:stream-error",
          (event) => {
            if (event.payload.message_id === messageId) {
              const s = useSignalStore.getState();
              s.updateMessage(messageId, {
                content: `Error: ${event.payload.error}`,
                loading: false,
                role: "system",
              });
              s.setLoading(false);
              cleanupListeners(messageId);
            }
          }
        );
        unlisteners.push(u3);

        activeListeners.set(messageId, unlisteners);
      },

      cancelStream: (messageId) => {
        cleanupListeners(messageId);
        const s = get();
        const msg = s.messages.find((m) => m.id === messageId && m.loading);
        if (msg) {
          s.updateMessage(messageId, {
            loading: false,
            content: msg.content || "(cancelled)",
          });
          s.setLoading(false);
        }
      },
    }),
    {
      name: "signex-signal",
      partialize: (state) => ({
        messages: state.messages.filter((m) => !m.loading).slice(-100),
        apiKeySet: state.apiKeySet,
        totalTokens: state.totalTokens,
        sessionCost: state.sessionCost,
        model: state.model,
        designBrief: state.designBrief,
      }),
    }
  )
);
