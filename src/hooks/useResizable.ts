import { useCallback, useRef, useEffect } from "react";

type Direction = "horizontal" | "vertical";

interface UseResizableOptions {
  direction: Direction;
  onResize: (size: number) => void;
  min?: number;
  max?: number;
  reverse?: boolean;
}

export function useResizable({
  direction,
  onResize,
  min = 150,
  max = 600,
  reverse = false,
}: UseResizableOptions) {
  const startPos = useRef(0);
  const startSize = useRef(0);
  const listeners = useRef<{
    move: (e: MouseEvent) => void;
    up: () => void;
  } | null>(null);

  const cleanup = useCallback(() => {
    if (listeners.current) {
      document.removeEventListener("mousemove", listeners.current.move);
      document.removeEventListener("mouseup", listeners.current.up);
      listeners.current = null;
    }
    document.body.style.cursor = "";
    document.body.style.userSelect = "";
  }, []);

  useEffect(() => cleanup, [cleanup]);

  const onMouseDown = useCallback(
    (e: React.MouseEvent, currentSize: number) => {
      e.preventDefault();
      cleanup();

      startPos.current = direction === "horizontal" ? e.clientX : e.clientY;
      startSize.current = currentSize;

      const onMouseMove = (ev: MouseEvent) => {
        const pos = direction === "horizontal" ? ev.clientX : ev.clientY;
        const delta = reverse
          ? startPos.current - pos
          : pos - startPos.current;
        const newSize = Math.min(max, Math.max(min, startSize.current + delta));
        onResize(newSize);
      };

      const onMouseUp = () => {
        cleanup();
      };

      listeners.current = { move: onMouseMove, up: onMouseUp };
      document.addEventListener("mousemove", onMouseMove);
      document.addEventListener("mouseup", onMouseUp);
      document.body.style.cursor =
        direction === "horizontal" ? "col-resize" : "row-resize";
      document.body.style.userSelect = "none";
    },
    [direction, onResize, min, max, reverse, cleanup]
  );

  return { onMouseDown };
}
