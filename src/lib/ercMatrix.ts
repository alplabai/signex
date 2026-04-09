/**
 * ERC Connection Matrix — defines violation severity for pin-to-pin connections.
 *
 * Altium/KiCad convention:
 * - Each cell defines what happens when pin type A connects to pin type B
 * - "ok" = valid connection, no report
 * - "warning" = suspicious but not necessarily wrong
 * - "error" = likely wiring mistake
 *
 * Pin types from KiCad: input, output, bidirectional, passive,
 * tri_state, open_collector, open_emitter, power_in, power_out, unconnected, free
 */

export type PinType =
  | "input" | "output" | "bidirectional" | "passive"
  | "tri_state" | "open_collector" | "open_emitter"
  | "power_in" | "power_out" | "unconnected" | "free"
  | "unspecified";

export type ErcSeverity = "ok" | "warning" | "error";

// Connection matrix: [row pin type][column pin type] → severity
// Matrix is symmetric: check(A,B) = check(B,A)
const matrix: Record<string, Record<string, ErcSeverity>> = {
  input: {
    input: "ok",           // Two inputs on same net is fine
    output: "ok",          // Output drives input — ideal
    bidirectional: "ok",   // Bidirectional can drive input
    passive: "ok",         // Passive connects to anything
    tri_state: "ok",       // Tri-state can drive input
    open_collector: "ok",  // OC can drive input
    open_emitter: "ok",    // OE can drive input
    power_in: "ok",        // Power input is like a load
    power_out: "ok",       // Power output drives input
    unconnected: "error",  // Unconnected pin connected to input
    free: "ok",
    unspecified: "ok",
  },
  output: {
    input: "ok",
    output: "error",       // TWO OUTPUTS = conflict
    bidirectional: "warning", // Output vs bidirectional — might contend
    passive: "ok",
    tri_state: "error",    // Output vs tri-state = conflict
    open_collector: "error", // OC and output = conflict
    open_emitter: "error", // OE and output = conflict
    power_in: "ok",
    power_out: "error",    // Two power sources = conflict
    unconnected: "error",
    free: "ok",
    unspecified: "warning",
  },
  bidirectional: {
    input: "ok",
    output: "warning",
    bidirectional: "ok",   // Two bidirectional = fine (I2C bus)
    passive: "ok",
    tri_state: "ok",
    open_collector: "ok",
    open_emitter: "ok",
    power_in: "ok",
    power_out: "warning",
    unconnected: "error",
    free: "ok",
    unspecified: "ok",
  },
  passive: {
    input: "ok",
    output: "ok",
    bidirectional: "ok",
    passive: "ok",         // Passive-to-passive = always fine (resistor network)
    tri_state: "ok",
    open_collector: "ok",
    open_emitter: "ok",
    power_in: "ok",
    power_out: "ok",
    unconnected: "error",
    free: "ok",
    unspecified: "ok",
  },
  tri_state: {
    input: "ok",
    output: "error",
    bidirectional: "ok",
    passive: "ok",
    tri_state: "ok",       // Multiple tri-state = fine (bus)
    open_collector: "warning",
    open_emitter: "warning",
    power_in: "ok",
    power_out: "error",
    unconnected: "error",
    free: "ok",
    unspecified: "warning",
  },
  open_collector: {
    input: "ok",
    output: "error",
    bidirectional: "ok",
    passive: "ok",
    tri_state: "warning",
    open_collector: "ok",  // Multiple OC = wired-AND (valid)
    open_emitter: "warning",
    power_in: "ok",
    power_out: "error",
    unconnected: "error",
    free: "ok",
    unspecified: "warning",
  },
  open_emitter: {
    input: "ok",
    output: "error",
    bidirectional: "ok",
    passive: "ok",
    tri_state: "warning",
    open_collector: "warning",
    open_emitter: "ok",    // Multiple OE = wired-OR (valid)
    power_in: "ok",
    power_out: "error",
    unconnected: "error",
    free: "ok",
    unspecified: "warning",
  },
  power_in: {
    input: "ok",
    output: "ok",
    bidirectional: "ok",
    passive: "ok",
    tri_state: "ok",
    open_collector: "ok",
    open_emitter: "ok",
    power_in: "ok",        // Multiple power consumers = fine
    power_out: "ok",       // Power supply to load = ideal
    unconnected: "error",
    free: "ok",
    unspecified: "ok",
  },
  power_out: {
    input: "ok",
    output: "error",
    bidirectional: "warning",
    passive: "ok",
    tri_state: "error",
    open_collector: "error",
    open_emitter: "error",
    power_in: "ok",
    power_out: "error",    // Two power supplies = conflict
    unconnected: "error",
    free: "ok",
    unspecified: "warning",
  },
  unconnected: {
    input: "error",
    output: "error",
    bidirectional: "error",
    passive: "error",
    tri_state: "error",
    open_collector: "error",
    open_emitter: "error",
    power_in: "error",
    power_out: "error",
    unconnected: "ok",     // Two unconnected = fine (both unused)
    free: "ok",
    unspecified: "error",
  },
  free: {
    input: "ok",
    output: "ok",
    bidirectional: "ok",
    passive: "ok",
    tri_state: "ok",
    open_collector: "ok",
    open_emitter: "ok",
    power_in: "ok",
    power_out: "ok",
    unconnected: "ok",
    free: "ok",
    unspecified: "ok",
  },
  unspecified: {
    input: "ok",
    output: "warning",
    bidirectional: "ok",
    passive: "ok",
    tri_state: "warning",
    open_collector: "warning",
    open_emitter: "warning",
    power_in: "ok",
    power_out: "warning",
    unconnected: "error",
    free: "ok",
    unspecified: "ok",
  },
};

/**
 * Check the connection matrix for two pin types.
 * Returns the worst severity found.
 */
export function checkPinConnection(typeA: string, typeB: string): ErcSeverity {
  const a = normalizeType(typeA);
  const b = normalizeType(typeB);
  return matrix[a]?.[b] ?? matrix[b]?.[a] ?? "warning";
}

/** Normalize KiCad pin type strings to our matrix keys */
function normalizeType(t: string): string {
  const lower = t.toLowerCase().replace(/\s+/g, "_");
  // KiCad uses various spellings
  if (lower === "power_output" || lower === "power output") return "power_out";
  if (lower === "power_input" || lower === "power input") return "power_in";
  if (lower === "tristate" || lower === "3state" || lower === "three_state") return "tri_state";
  if (lower === "opencollector" || lower === "open collector") return "open_collector";
  if (lower === "openemitter" || lower === "open emitter") return "open_emitter";
  if (lower === "bidi") return "bidirectional";
  if (lower === "line" || lower === "") return "passive"; // Default for unlabeled pins
  return lower;
}
