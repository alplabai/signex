import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";

// Global error handler — shows errors on screen since F12 may not work
window.onerror = (msg, source, line, col, error) => {
  const el = document.getElementById("root");
  if (el) {
    el.innerHTML = `<pre style="color:#e8667a;background:#1a1b2e;padding:40px;font-family:monospace;font-size:14px;white-space:pre-wrap;">
UNCAUGHT ERROR:
${msg}
at ${source}:${line}:${col}
${error?.stack || ""}
    </pre>`;
  }
};

window.onunhandledrejection = (event) => {
  const el = document.getElementById("root");
  if (el) {
    el.innerHTML = `<pre style="color:#e8667a;background:#1a1b2e;padding:40px;font-family:monospace;font-size:14px;white-space:pre-wrap;">
UNHANDLED PROMISE REJECTION:
${event.reason}
${event.reason?.stack || ""}
    </pre>`;
  }
};

class ErrorBoundary extends React.Component<
  { children: React.ReactNode },
  { error: Error | null }
> {
  state: { error: Error | null } = { error: null };

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  render() {
    if (this.state.error) {
      return (
        <pre
          style={{
            color: "#e8667a",
            background: "#1a1b2e",
            padding: 40,
            fontFamily: "monospace",
            fontSize: 14,
            whiteSpace: "pre-wrap",
          }}
        >
          {`REACT RENDER ERROR:\n${this.state.error.message}\n\n${this.state.error.stack}`}
        </pre>
      );
    }
    return this.props.children;
  }
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <ErrorBoundary>
    <App />
  </ErrorBoundary>
);
