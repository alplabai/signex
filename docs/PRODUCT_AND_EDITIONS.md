# Signex — Product and Editions

> **Status:** Product packaging and business model reference.
> **Audience:** Anyone making decisions about features, pricing, or licensing.
> **Companion to:** `MASTER_PLAN.md` (product thesis), `ARCHITECTURE.md` (how
> the code is structured), `ROADMAP.md` (when things ship).

This document describes how Signex is packaged, priced, and licensed. It does
not describe how features are implemented — that is `ARCHITECTURE.md`'s job.
It does not describe when features ship — that is `ROADMAP.md`'s job. It
describes what a user gets, what they pay for, and why the split is drawn
where it is.

---

## 1. The Two Editions

Signex ships as two editions from a single codebase:

- **Signex Community** — free, open source, fully functional EDA editor
- **Signex Pro** — subscription-based, adds Signal AI and live collaboration

Both editions share the same editor core. They open the same files. A project
created in Community opens without modification in Pro, and a project from
Pro opens in Community (minus the Pro-only artifacts, which are stored
separately and do not pollute the KiCad files).

**There is no "trial version" of Pro, and no feature in Community that is
artificially crippled to push upgrades.** Community is the real product.
Pro is Community plus two specific capabilities that require infrastructure
Community users do not need.

---

## 2. Why This Split and Not a Different One

We considered several alternatives before landing on this split. Recording the
reasoning so we don't quietly drift away from it.

### 2.1. Why not feature-gate the PCB editor?

Keeping PCB editing in Pro would be an obvious monetization path. We are not
doing it because:

- It would make Community unable to replace KiCad for anyone doing real work
- It would destroy the "KiCad-compatible editor" positioning — an editor that
  can only read half of a KiCad project is not compatible
- KiCad's PCB editor is free; gating ours behind a paywall would push users
  back to KiCad instead of toward Signex
- Open hardware culture, which is most of our likely user base, is actively
  hostile to gated core features

The core editor — schematic and PCB — is Community. This is non-negotiable.

### 2.2. Why not feature-gate simulation?

Simulation is a natural Pro candidate: it is specialized, it is valuable to
professionals, and the tooling (ngspice, OpenEMS, Elmer) is heavy. We are not
doing it because:

- The simulators themselves are free and open source; gating UI access to
  already-free tools feels hostile
- Academic and hobbyist users simulate constantly and would be the worst
  audience to paywall
- Simulation is already in every competing free tool (KiCad ships ngspice
  integration)

Simulation is Community, when it eventually ships.

### 2.3. Why is Signal AI in Pro?

AI assistance is in Pro for one concrete reason: **it costs us money per use.**
Every Signal AI request routes through Anthropic's API, and those requests have
a per-token cost that we pay regardless of whether the user pays us. A free
tier of Signal AI would be a direct subsidy from paying users to non-paying
users, with no upper bound.

Pro subscriptions include Signal AI usage at no per-token cost to the user.
Alp Lab absorbs the API costs and amortizes them across the subscriber base.
This only works if there is a subscriber base.

### 2.4. Why is collaboration in Pro?

Live collaboration is in Pro for the same reason as AI: **it costs us money
per use.** Supabase Realtime, Storage, Postgres, and Auth all bill per project,
per user, per byte, per connection. Unlike AI, collaboration's infrastructure
cost scales slowly with individual users but predictably with team adoption —
which is exactly the audience most willing and able to pay.

Pro subscriptions include unlimited collaboration on the user's projects, with
cloud storage of reasonable size and team-sized presence limits.

### 2.5. What we are explicitly not doing

- No ad-supported Community tier
- No "upgrade nag" popups in Community
- No telemetry that feeds upsell targeting
- No feature removed from Community to push users to Pro
- No Community-only bugs that "happen to" be fixed in Pro
- No license key required to use Community offline indefinitely

Community is supposed to be a product we would be proud to ship on its own.
Pro is supposed to be worth the money on its own merits. Anything else
corrodes trust in both.

---

## 3. Signex Community

### 3.1. What Community Is

Signex Community is a full-featured, desktop EDA editor. It is what most users
will ever need. It is released under **GPL-3.0** and its source code is fully
open.

### 3.2. License

**GPL-3.0.** Chosen over more permissive options deliberately:

- KiCad itself is GPL-3.0; using the same license reduces friction for users
  and contributors coming from that ecosystem
- GPL-3.0 prevents a competitor from forking Community, adding proprietary
  features, and releasing a closed product that undercuts both Signex Pro and
  Signex Community
- GPL-3.0 does not prevent commercial use; companies can use Community
  internally for any purpose, including producing closed-source hardware
  designs
- The distinction between "using the tool to make your thing" and "distributing
  a modified version of the tool" is clear and well-understood in this industry

### 3.3. Price

**Free. Forever. Full stop.**

No registration, no account, no telemetry, no license check, no online
activation. Downloading the installer and running it is sufficient. The
application works offline indefinitely.

### 3.4. What Community Includes

Everything that is not explicitly listed as Pro. In v1.0 terms, this means
the entire editor:

- Schematic capture (v1.0)
- PCB editing, routing, DRC (v2.0)
- Export to all industry formats (Gerber, ODB++, PDF, BOM, STEP)
- Library browsing and editing
- KiCad file compatibility
- Altium-inspired UX, keyboard shortcuts, panels
- All built-in themes
- Simulation — SPICE, OpenEMS, Elmer (post-v2.0)
- 3D PCB viewer (post-v2.0)
- Plugin system, when it ships (post-v2.0)

If a feature is about *editing designs*, it is in Community.

### 3.5. Support

- **Public issue tracker** on GitHub — community-supported
- **Community forum / Discord** — user-to-user help
- No guaranteed response time, no support SLA
- Documentation is open and editable

This is the standard open-source support model. It is honest about what it is.

---

## 4. Signex Pro

### 4.1. What Pro Is

Signex Pro is Signex Community plus two specific capabilities that require
backend infrastructure:

1. **Signal AI** — a design copilot integrated into the editor
2. **Live collaboration** — real-time co-editing of schematic and PCB

Pro is proprietary software. It is a separate binary, built from the same
workspace with the `pro` Cargo feature enabled. The Pro-only crates are not
GPL; they are proprietary and shipped under Signex's commercial license.

### 4.2. License

**Proprietary subscription.** The Pro binary is not open source and not
redistributable. A Pro license grants the user (or organization) the right to
use the Pro binary and the associated backend services for the duration of
their subscription.

Community users can read all of Signex's public source code, including the
engine, the editor, the renderer, and the Community UI. They cannot read the
Pro-only crates (`signex-signal`, `signex-collab`) because those are
proprietary. This is a clean split: the core editor is open, the added
services are not.

### 4.3. Price

**To be determined.** Pricing is deferred until we have a v1.0 product and
actual users. Locking in a price before the product exists is how SaaS
pricing gets stuck at numbers that make no sense later.

Working assumptions for planning:

- Monthly subscription, roughly in the range of other professional EDA
  subscriptions (Altium, OrCAD) but significantly lower
- Annual subscription at a meaningful discount
- Team/organization tier for multi-seat deployment
- Educational pricing, probably free for verified students and academic
  institutions
- No per-token, per-minute, or per-project metering from the user's side —
  Pro is flat-rate

**What we will not do:**

- No usage-based billing that makes users nervous about running a simulation
- No tiered subscriptions within Pro ("Pro Lite," "Pro Plus," etc.) — Pro is
  Pro, one price, everything included
- No hidden "enterprise" tier that unlocks features kept out of the main Pro
  tier

Pricing is revisited annually based on costs and adoption.

### 4.4. What Pro Adds

#### 4.4.1. Signal AI

A design copilot that runs inside the editor, integrated with the design:

- **Chat interface** — streaming conversation with Claude, in a dockable panel
- **Design context** — Signal AI automatically has access to the current
  schematic, PCB, ERC/DRC results, and simulation data; the user does not have
  to paste anything
- **Tool use** — Signal AI can take actions: place components, draw wires,
  fix ERC violations, run simulations, export reports. Every action is undoable.
- **Visual context** — Signal AI can see rendered views of the design when
  useful (for spatial questions like "why is this layout cramped?")
- **Circuit templates** — Signal AI can place complete sub-circuits (buck
  converter, LDO, filter, etc.) from natural-language descriptions
- **Design review** — on request, Signal AI produces a structured review of
  the design with severity-ranked findings

Signal AI routes through Alp Lab's managed gateway. The user does not supply
an API key. Usage is included in the subscription with reasonable fair-use
limits; if someone is clearly running an automated farm of requests, we will
contact them, not cut them off silently.

#### 4.4.2. Live Collaboration

Real-time co-editing of Signex projects:

- **Multiple editors on one project** — two, three, or more engineers working
  on the same schematic or PCB at the same time
- **Per-user cursors** — see where everyone is on the canvas, with name and
  color
- **Presence** — online status, follow-mode (ride along with a collaborator's
  viewport)
- **Locking** — optional exclusive locks on sheets, PCB regions, copper layers,
  or specific nets, for avoiding routing conflicts
- **Comments** — pin comments to specific canvas locations, reply threads,
  resolve/unresolve
- **Review workflow** — request review, diff overlay, approve or request
  changes
- **Version history** — every change attributed to an author, browsable and
  diffable
- **Offline support** — work offline, queue changes locally, sync on reconnect
- **Cloud storage** — projects stored in the user's workspace, accessible from
  any Pro install

The backend is Supabase. The user does not see this detail and does not need
to manage any infrastructure. Alp Lab operates the Supabase project on behalf
of subscribers.

### 4.5. Support

Pro subscribers get:

- Email and chat support with guaranteed response times (specifics TBD)
- Priority consideration for bug fixes affecting Pro features
- Access to Alp Lab engineers for design-related questions within reason
- Onboarding assistance for teams

---

## 5. Feature Gating Strategy

Pro features are gated at **compile time**, not at runtime. A Community binary
cannot possibly access Pro features because the code for them is not compiled
in. A Pro binary additionally performs runtime license validation for the
backend services but the feature code itself is present in both code paths
only when the `pro` feature is enabled.

### 5.1. Cargo Feature Flags

```toml
# crates/signex-app/Cargo.toml

[features]
default = []
pro = ["signex-signal", "signex-collab"]

[dependencies]
# Core dependencies always present
signex-engine = { path = "../signex-engine" }
signex-model  = { path = "../signex-model" }
# ... etc

# Pro-only dependencies, behind the feature flag
signex-signal = { path = "../signex-signal", optional = true }
signex-collab = { path = "../signex-collab", optional = true }
```

Pro crates are optional dependencies. When `--features pro` is absent from the
build, they are not compiled, not linked, and not present in the binary.

### 5.2. In-Source Gating

Pro-specific code paths in `signex-app` are gated with `#[cfg(feature = "pro")]`:

```rust
// Example: the application's menu construction
fn build_menu(&self) -> Menu {
    let mut menu = Menu::new();
    menu.add(file_menu());
    menu.add(edit_menu());
    menu.add(view_menu());

    #[cfg(feature = "pro")]
    menu.add(signal_ai_menu());

    #[cfg(feature = "pro")]
    menu.add(collaboration_menu());

    menu
}
```

In Community builds, these blocks disappear entirely. The menu items do not
exist; clicking where they would be is not even possible.

### 5.3. No Runtime Flags for Feature Presence

We do not ship a single binary that contains both editions with a runtime flag
determining which "mode" to run in. That approach has several failure modes:

- Reverse engineering to flip the flag locally is trivial
- The binary is larger for no good reason
- Community users technically receive proprietary code even though they can't
  run it, which complicates the GPL separation
- A single bug in the flag check compromises the entire separation

Compile-time gating makes the split cryptographically boring: the code
simply is not there.

### 5.4. License Validation (Pro Only)

When Pro features attempt to connect to Alp Lab's backend (Signal AI gateway,
Supabase), they present a license token. The token:

- Is issued at subscription time
- Validates against Alp Lab's license server on first use and periodically
  thereafter
- Caches validation locally for a 7-day offline grace period
- Is tied to the user account, not to a machine, so users can install on
  multiple devices

License validation is a network check, not a DRM system. It determines whether
the backend will serve requests. It does not prevent the Pro binary from
running locally in offline mode for functionality that doesn't require the
backend (of which there is none, by design — everything Pro adds is a backend
service).

### 5.5. Downgrade Behavior

A Pro user whose subscription lapses:

- Can still open and edit all their projects (the editor is fully functional
  locally)
- Loses access to Signal AI (gateway rejects requests after grace period)
- Loses access to cloud collaboration (Supabase connections are rejected)
- Cloud-stored projects remain available for download for a defined window
  (specifics in terms of service)
- Local project files are entirely unaffected — they are KiCad files, which
  open in KiCad or in Signex Community without any Pro binary present

This is important. We never hold a user's design hostage because their
subscription lapsed. The design is a KiCad project on their hard drive and
opens in any KiCad-compatible tool forever.

---

## 6. What Is Stored Where

Understanding what lives locally vs. in the cloud matters for privacy,
portability, and subscription behavior.

| Data                       | Community      | Pro (non-collab) | Pro (collab)   |
|----------------------------|----------------|------------------|----------------|
| Project files (.kicad_*)   | Local          | Local            | Cloud + local  |
| Library files              | Local          | Local            | Local          |
| Signal AI conversations    | N/A            | Local (opt cloud)| Local + cloud  |
| Signal AI design context   | N/A            | Sent to gateway  | Sent to gateway|
| Collaboration edit log     | N/A            | N/A              | Cloud          |
| Collaboration locks/state  | N/A            | N/A              | Cloud          |
| User settings              | Local          | Local            | Local + cloud  |
| License state              | N/A            | Local cache      | Local cache    |

**Privacy principles:**

- Design data is not transmitted anywhere without an explicit, local user
  action (opening a collaboration session, asking Signal AI a question)
- Signal AI requests include design context because that is the feature;
  requests are not logged beyond what Anthropic's API retention terms require
- Collaboration data lives in the user's Supabase workspace; other
  subscribers cannot see it
- We do not train models on user designs. We do not sell usage data. We do not
  have advertising.

---

## 7. Open Product Questions

These are questions that will be resolved closer to launch.

### 7.1. Actual Pro Pricing

Waiting until v1.0 ships and we have real users. Current working number is in
the range of $20–$50/month individual, with team pricing tiered. This is a
placeholder.

### 7.2. Educational Access

The likely policy is free Pro for verified students and faculty at accredited
institutions, with automatic verification through a service like GitHub
Education or SheerID. Details TBD.

### 7.3. Open-Source Project Sponsorship

A plausible policy is free Pro for maintainers of sufficiently popular open
hardware projects, as a form of community contribution. Not committed to yet.

### 7.4. On-Premise / Self-Hosted Pro

Some organizations will ask to run Supabase and the Signal AI gateway
themselves, for data governance reasons. This is technically possible (Supabase
is self-hostable; the Signal AI gateway is a thin layer over Anthropic's API
that could be deployed privately). Whether we offer this as a product, and at
what price, is deferred.

### 7.5. Non-Subscription Pro

The question of whether to offer perpetual-license Pro (one-time payment, no
updates after a period) comes up in every EDA product discussion. We are not
planning to offer it because the backend costs are ongoing, and a
perpetual-license model cannot fund them. If we change our mind, it would be
for specific constrained use cases (e.g., air-gapped installations) rather
than a general-audience offering.

---

## 8. Rules for Changing This Document

This document represents promises to users. Changes that *add* Pro features
are product development. Changes that *move existing features into Pro* are
breaking promises and require a very strong reason.

- Moving a feature from Community to Pro is a last resort and requires
  explicit justification
- Moving a feature from Pro to Community is always allowed
- Changing pricing is always allowed (for new subscribers; existing
  subscribers are grandfathered for a reasonable period)
- Changing license terms requires clear communication and advance notice
- Feature lists in this document are aspirational until they ship; "included
  in Pro" means "planned for Pro," not "available now"

When in doubt, err toward giving users more, not less. Trust in a tool like
this compounds over years. Most decisions that feel like short-term wins are
long-term losses.
