import { create } from "zustand";
import { persist } from "zustand/middleware";

export interface SignalMessage {
  id: string;
  role: "user" | "assistant" | "system";
  content: string;
  timestamp: number;
  usage?: { input_tokens: number; output_tokens: number };
  loading?: boolean;
}

interface SignalState {
  messages: SignalMessage[];
  apiKeySet: boolean;
  isLoading: boolean;
  totalTokens: number;

  addMessage: (msg: Omit<SignalMessage, "id" | "timestamp">) => void;
  updateMessage: (id: string, updates: Partial<SignalMessage>) => void;
  removeMessage: (id: string) => void;
  clearHistory: () => void;
  setApiKeySet: (v: boolean) => void;
  setLoading: (v: boolean) => void;
  addTokens: (input: number, output: number) => void;
}

export const useSignalStore = create<SignalState>()(
  persist(
    (set) => ({
      messages: [],
      apiKeySet: false,
      isLoading: false,
      totalTokens: 0,

      addMessage: (msg) =>
        set((s) => ({
          messages: [
            ...s.messages,
            { ...msg, id: crypto.randomUUID(), timestamp: Date.now() },
          ],
        })),

      updateMessage: (id, updates) =>
        set((s) => ({
          messages: s.messages.map((m) =>
            m.id === id ? { ...m, ...updates } : m
          ),
        })),

      removeMessage: (id) =>
        set((s) => ({ messages: s.messages.filter((m) => m.id !== id) })),

      clearHistory: () => set({ messages: [], totalTokens: 0 }),

      setApiKeySet: (v) => set({ apiKeySet: v }),

      setLoading: (v) => set({ isLoading: v }),

      addTokens: (input, output) =>
        set((s) => ({ totalTokens: s.totalTokens + input + output })),
    }),
    {
      name: "signex-signal",
      partialize: (state) => ({
        messages: state.messages.filter((m) => !m.loading),
        apiKeySet: state.apiKeySet,
        totalTokens: state.totalTokens,
      }),
    }
  )
);
