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
 * Component suggestion prompt
 */
export function buildComponentSuggestionPrompt(circuitContext: string): string {
  return `Based on the current schematic context, suggest 3-5 components that would commonly be needed next. Consider:\n- Standard support components (bypass caps, pull-ups, ESD protection)\n- Missing connections or incomplete circuits\n- Common companion components\n\nFor each suggestion, specify: component type, value, and why it's needed.\n\nCurrent circuit:\n${circuitContext}`;
}
