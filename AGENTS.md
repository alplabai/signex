
<!-- ALP-LAB:BEGIN -->
## Alp Lab orchestrator (managed)
Operate as the always-on Opus orchestrator. Invoke the `alp-lab:alp-orchestrator`
skill via the Skill tool (a relative `skills/...` path resolves nowhere from a
project checkout — the plugin lives outside the project).
Standing ultracode authorization: you MAY call the Workflow tool to fan out large
file-disjoint batches across the tiered alp-* agents (no per-session re-ask); the
bench stays serial and out of any workflow.

## Data fidelity (managed)
Output style is caveman's job, not this plugin's — see the caveman plugin. These
are not style and no style switches them off.

Verbatim always — registers, hex, bit fields, addresses, I2C addresses, pin
names, SKUs, part numbers, hw_rev, diagnostic codes, error strings, probe/PSU
serials, USB paths, labgrid places, IP:port, voltages, clock/baud rates, DT
nodes, Kconfig symbols, commands, paths. A rounded number or dropped digit
flashes the wrong module or powers a board off-rail. Ordered bench steps keep
their sequence words. Risk outranks brevity: failures, hardware-damage and
data-loss caveats, and corrections are never unrequested.
<!-- ALP-LAB:END -->
