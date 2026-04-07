/**
 * Circuit template prompts for Signal AI.
 * When selected, these are sent as user messages with instructions
 * for Claude to use tool calls to build the circuit.
 */

export interface CircuitTemplate {
  name: string;
  description: string;
  prompt: string;
}

export const CIRCUIT_TEMPLATES: CircuitTemplate[] = [
  {
    name: "LDO Regulator",
    description: "Linear voltage regulator with input/output caps",
    prompt: "Create an LDO voltage regulator circuit. Use the add_component, add_wire, and add_net_label tools to build it. Include:\n- LDO regulator IC (e.g., AMS1117-3.3)\n- Input capacitor (10uF electrolytic)\n- Output capacitor (10uF + 100nF ceramic)\n- Input and output net labels (VIN, 3V3)\n- Ground connections\nPlace components in a clean left-to-right layout starting at x=100, y=100 with ~20mm spacing.",
  },
  {
    name: "Decoupling Caps",
    description: "Standard bypass capacitor arrangement",
    prompt: "Add standard decoupling capacitors near the selected component (or at x=150, y=100 if nothing selected). Use add_component to place:\n- 100nF ceramic cap close to the IC\n- 10uF bulk cap nearby\nAdd GND net labels on the bottom pins. Place them vertically with ~5mm spacing.",
  },
  {
    name: "Pull-up Resistors",
    description: "I2C or GPIO pull-up network",
    prompt: "Create a pull-up resistor network for I2C. Use add_component and add_net_label tools:\n- Two 4.7k pull-up resistors (R_PU_SDA, R_PU_SCL)\n- Net labels: SDA, SCL, 3V3\nPlace at x=120, y=80. Arrange vertically with the resistors connected to 3V3 on top and signal lines below.",
  },
  {
    name: "Op-Amp Buffer",
    description: "Unity-gain voltage follower",
    prompt: "Create an op-amp unity-gain buffer circuit. Use tools to place:\n- Op-amp IC (e.g., LM358)\n- Input and output connections\n- Power supply bypass cap (100nF)\n- VCC and GND net labels\nConnect output to inverting input for unity gain. Place at x=130, y=100.",
  },
  {
    name: "RC Filter",
    description: "Low-pass RC filter",
    prompt: "Create a simple RC low-pass filter. Use tools:\n- One resistor (10k)\n- One capacitor (100nF)\n- Input and output net labels\n- GND connection\nCutoff frequency ~160Hz. Place at x=110, y=100 in series configuration.",
  },
  {
    name: "Power Header",
    description: "Power input connector with protection",
    prompt: "Create a power input section. Use tools to add:\n- 2-pin header connector for power input\n- Schottky diode for reverse polarity protection\n- Bulk capacitor (100uF)\n- Ceramic capacitor (100nF)\n- VIN and GND net labels\nPlace at x=80, y=100.",
  },
];

/**
 * BOM optimization prompt
 */
export const BOM_OPTIMIZE_PROMPT = "Analyze the current schematic's Bill of Materials. Look at all components and suggest:\n\n1. **Consolidation**: Are there similar values that could be standardized? (e.g., using all 10k resistors instead of 9.1k and 10k)\n2. **Missing components**: Are there bypass caps or pull-ups that should be added?\n3. **Value corrections**: Any obviously wrong values for common circuits?\n4. **Package standardization**: Could packages be standardized (e.g., all 0402 or all 0603)?\n\nUse the set_component_value tool to apply any recommended changes. Be conservative — only change values that are clearly improvements.";

/**
 * Component suggestion prompt — hardware-specific
 */
export function buildComponentSuggestionPrompt(circuitContext: string): string {
  return `Based on the current schematic, suggest 3-5 components that are commonly needed. Prioritize:\n\n1. **Power integrity**: Missing bypass caps (100nF per IC power pin, 10uF per rail), bulk caps, ferrite beads\n2. **Signal integrity**: Missing pull-ups (I2C: 4.7k, SPI CS: 10k), termination resistors, series resistors for protection\n3. **Protection**: TVS diodes on connectors, ESD protection on USB/Ethernet, reverse polarity protection\n4. **Testability**: Test points on critical signals, debug headers\n5. **Missing passives**: Decoupling, filtering, biasing\n\nFor each suggestion give: specific part number, value with tolerance, package size, and placement rationale.\n\nCurrent circuit:\n${circuitContext}`;
}

/**
 * Design review prompt — deep hardware analysis
 */
export const DESIGN_REVIEW_PROMPT = `Review this schematic for production readiness. Check systematically:

**Power:**
- Every IC has 100nF ceramic bypass cap within 2mm of power pin?
- Bulk capacitors (10-47uF) on each power rail?
- Correct voltage ratings on all caps (2x operating voltage minimum)?
- Power-good/enable sequencing correct?
- Current ratings adequate for expected load?

**Signal Integrity:**
- I2C: pull-ups present, value appropriate for bus speed and capacitance?
- SPI: CS pull-ups, MISO tri-state handling?
- UART: series resistors for protection?
- Clock signals: termination if trace > lambda/10?
- Reset pins: RC filter + ESD protection?

**Protection:**
- ESD protection on all external connectors?
- Reverse polarity protection on power input?
- Over-current protection (PTC fuse or electronic)?
- Voltage clamping on ADC inputs?

**Manufacturing:**
- All components have footprints assigned?
- Unannotated designators?
- Single-source components that need alternates?
- Standard package sizes (prefer 0402/0603)?

Provide a prioritized punch list with severity (Critical/Important/Nice-to-have).`;

/**
 * ERC fix prompt — context-aware
 */
export function buildErcFixPrompt(violation: string): string {
  return `ERC violation: "${violation}"

Explain:
1. **Root cause** — what's electrically wrong
2. **Impact** — what happens if unfixed (signal corruption, damage, intermittent failure)
3. **Fix** — specific component/connection to add/change, with values
4. **Prevention** — design practice to avoid this in future

Be concise. If a component needs adding, specify exact value and placement.`;
}

/**
 * PCB review prompt — layout-specific
 */
export const PCB_REVIEW_PROMPT = `Review this PCB layout for manufacturing readiness:

**Routing:**
- Minimum trace width adequate for current? (0.15mm = ~0.5A, 0.25mm = ~1A, 0.5mm = ~2A for 1oz Cu)
- High-current paths wide enough? Power traces especially.
- Differential pairs matched in length and gap?
- No acute angles in traces (prefer 45-degree or curved)?
- Return paths unbroken under high-speed signals?

**Power:**
- Power planes solid and uninterrupted?
- Bypass caps placed close to IC power pins?
- Via stitching between ground planes?
- Thermal relief on high-current connections?

**Manufacturing:**
- All pads have adequate annular ring?
- Solder mask clearances correct?
- Silkscreen not overlapping pads?
- Board outline closed and clean?
- Drill sizes standard for fab house?

**EMC:**
- High-speed signals routed on inner layers with reference planes?
- No traces crossing split planes?
- Crystal traces short and guarded?
- Antenna keep-out zones respected?

Provide actionable items with location references.`;
