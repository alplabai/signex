# Hexagonal architecture, DDD and the signex AppCore design: a sourced reference

Prepared 2026-11-13 for epic #2326 and the ADR-0001 amendment (A2, A4, D1, D5, D8, Part C). It reviews the first draft of that amendment; the ADR text that ships already takes its recommendations into account.

**Method and clean-room note.** Only articles, public book summaries, official documentation and blog posts were read. No source code of any project was read, and no GPL project material. Licences are given for every code-bearing project cited; where GitHub's licence detection and the project's own text differ, both are given. Numbered references are at the end. Text marked *(analysis)* is a judgement with no source.

---

## 1. Hexagonal architecture (ports and adapters)

### 1.1 Cockburn's original definition

- **Intent.** Cockburn's report (HaT Technical Report 2005.02, dated 2005-09-04, v0.9) states the goal as letting an application "equally be driven by users, programs, automated test or batch scripts", and as developing and testing it apart from its eventual devices and databases [1]. The symmetry is the point. A GUI is one driver among several, not the privileged one.
- **Ports and adapters.** A port is a "purposeful conversation" whose protocol is an API. An adapter "converts the API definition to the signals needed by that device". Several adapters can sit on one port, for example a GUI, a test harness or a mock database [1].
- **Driving and driven.** A primary actor drives the application. A secondary actor is one the application drives, to get answers or to send notifications. Cockburn draws primary ports and adapters on the left or top, and secondary ones on the right or bottom [1].
- **Inside vs outside.** The rule is that inside code must not leak to the outside. The key asymmetry is inside versus outside, not left versus right [1].
- **Testing guidance.**
  - On primary ports, FIT tables and fixtures act as the test adapter that drives the application from a script.
  - On secondary ports, an in-memory mock (for example of the database) is the test adapter.
  - Automated builds run against the mock, and the test and real databases are swapped in later [1].

### 1.2 Common misreadings

| Misreading | What the sources say |
|---|---|
| "Hexagonal is three layers: domain, ports, adapters." | Garrido de Paz, Cockburn's 2024 co-author: "the pattern says nothing about layers" [2]. Cockburn says the hexagon shape exists to get away from a one-dimensional layered picture, and that the number six is not significant [1]. |
| "There is one port per external system." | Cockburn favours "two, three or four ports", chosen largely by intuition [1]. |
| "There must be one port per use case." | Garrido de Paz treats this as one option among several. A port can group many use cases or few. Other options are one port per use case (SRP), or a command bus with one handler per use case, with command and query ports split CQRS-style [2]. |
| "The hexagon prescribes the inside." | It does not. Layers, feature components or DDD tactical patterns are all allowed [2]. |
| "More ports means more decoupling." | Petrov (2026) reports that fine-grained ports produce "constructor explosion, test-double bloat, wiring noise", and he recommends roughly 4–8 coarse ports [3]. This is a practitioner's opinion, not Cockburn's. |

### 1.3 Onion, Clean and Explicit Architecture

| Style | Same as hexagonal | Different |
|---|---|---|
| **Onion** (Palermo, 2008) [4] | Dependencies point inward: code "cannot depend on layers further out from the core". Inner layers define interfaces and outer layers implement them. "The database is not the center." | Concentric layers with the domain model at the centre, so layering is explicit. It is object-model first ("puts objects before all others"). |
| **Clean Architecture** (Martin, 2012) [5] | The Dependency Rule: "source code dependencies can only point inwards". | Four named circles (Entities, Use Cases, Interface Adapters, Frameworks and Drivers), though "there's no rule that says you must always have just these four". Use cases hold the application-specific business rules. Martin presents it as a merger of Hexagonal, Onion, DCI and BCE. |
| **Explicit Architecture** (Graça, 2017) [6] | Ports belong inside, adapters outside. Primary adapters "translate whatever comes from a delivery mechanism into a method call in the Application Core". | It combines DDD layers inside the hexagon: the Domain Model at the centre, then domain services, then application services or command handlers. Components cut across those layers. It also separates domain events from application events. |

---

## 2. Domain-Driven Design: Evans and Vernon

Evans's own *DDD Reference* (2015, CC BY 4.0; its pattern summaries are CC BY 3.0) is the primary text used here [7].

### 2.1 Aggregates and consistency boundaries

- **Evans** [7]:
  - Cluster entities and value objects into aggregates, each with one root. Outside objects hold references only to the root.
  - The root, or "some designated framework mechanism", enforces the invariants.
  - "Use the same aggregate boundaries to govern transactions and distribution. Within an aggregate boundary, apply consistency rules synchronously. Across boundaries, handle updates asynchronously."
- **Vernon** (*Effective Aggregate Design*, 2011) [8][9]:
  - "Rule: Model True Invariants In Consistency Boundaries". An invariant "is a business rule that must always be consistent", in the transactional sense.
  - "Rule: Design Small Aggregates".
  - "Rule: Reference Other Aggregates By Identity".
  - "Rule: Use Eventual Consistency Outside the Boundary".
  - Only one aggregate is modified per transaction.
  - Vernon allows a principled exception. With **user-aggregate affinity**, where only one user works on a given set of aggregates at a time, "sometimes it works out well to modify multiple aggregate instances in one transaction" [9].

### 2.2 Domain services vs application services

- **Domain service** (Evans): when a significant domain process "is not a natural responsibility of an entity or value object", it becomes a standalone service operation, named in the ubiquitous language [7].
- **Application layer**: Fowler quotes Evans: "This layer is kept thin. It does not contain business rules or knowledge, but only coordinates tasks and delegates work" [10]. Graça puts it more bluntly: "domain logic should stay out of the application layer!" [6]. Vernon's application service "resolve[s] dependencies", loads aggregates and runs the transaction, and does not license changing two aggregates in one request [9].
- **Layered Architecture** (Evans): isolate the domain model "from the user interface, application, and infrastructure code". Evans names Hexagonal Architecture as a pattern that "may serve as well or better" [7].

### 2.3 Anemic domain model, transaction script, and data-plus-engine

- **Transaction Script** (Fowler, PoEAA 2002) "organizes business logic by procedures where each procedure handles a single request from the presentation" [11].
- **Anemic Domain Model** (Fowler, 2003): objects that are "little more than bags of getters and setters". "By pulling all the behavior out into services, however, you essentially end up with Transaction Scripts" [10].
- **When data plus functions is legitimate.** Seemann (2016) argues that good functional design "is equivalent to the Ports and Adapters architecture": a big core of pure functions and a shell of IO [12]. In that style data and behaviour are split by design. *(analysis)* What makes a model anemic is not where the data sits but where the rules end up. If the rules live in domain functions (the engine) and not in application services, the model is a functional domain model, not an anemic one. It degrades into a transaction script only when the per-request procedures, here the application handlers, start to hold the rules.

### 2.4 Domain events

- **Evans** [7]: model domain activity "as a series of discrete events". Domain events are "distinct from system events that reflect activity within the software itself".
- **Vernon** (2011): "An aggregate command method publishes a domain event that is in time delivered to one or more asynchronous subscribers", through an in-process singleton `DomainEventPublisher` [9].
- **Immediate vs deferred dispatch.**
  - Dahan's static `DomainEvents.Raise` dispatches immediately. Microsoft's .NET architecture guide notes that this "makes testing and debugging more difficult" [13].
  - The deferred alternative is Bogard's "A better domain events pattern" (2014): collect events on the entity and dispatch them just before or just after commit [13][14].
- **Domain vs integration events** (Microsoft guide): domain events stay in-process. Integration events cross process boundaries and should only be sent after a successful persist [13].
- **Domain vs application events** (Graça): domain events "live" in the domain model, while application events "represent some outcome of a use case" [6].

*(analysis)* Returning events as part of the call's result, which is what signex plans, is the deferred pattern taken to its end: no dispatcher, no subscribers, and the caller decides what happens.

### 2.5 Bounded contexts and shared kernel

- **Shared Kernel** (Evans) is a relationship *between bounded contexts or teams*: "Designate with an explicit boundary some subset of the domain model that the teams agree to share. Keep this kernel small". Changes to it need "consultation with the other team" [7].
- Graça likewise asks for a shared kernel "as minimal as possible" [6].

---

## 3. Combining hexagonal and DDD

Graça's synthesis [6] is the standard map:

1. The domain model sits at the centre and depends on nothing.
2. Domain services surround the domain model and know nothing of the application layer.
3. Application services or command handlers "unfold a use case".
4. Ports are interfaces owned by the core.
5. Adapters sit outside: primary adapters call into the core, and secondary adapters implement the core's ports.
6. Dependencies point inward, which is the same Dependency Rule as Martin's [5] and Palermo's [4].

Hexagonal says nothing about how the inside is organised [2]. DDD supplies that organisation.

---

## 4. Functional core, effects as data, TEA, sans-IO

| Idea | Source | Relation to hexagonal |
|---|---|---|
| **Functional core, imperative shell** | Bernhardt, "Boundaries" (SCNA 2012), and the DAS screencast of that name. Simple values "as the boundaries between components and subsystems" [15]. | The values crossing the boundary play the part of ports. Seemann makes the equivalence explicit [12]. |
| **The Elm Architecture: `Cmd` / `Sub`** | Programs "send `Cmd` and `Sub` values to the runtime system", which performs the work [16]. | An effect is data handed to a runtime, which acts as a single driven adapter. |
| **iced `Task`** (MIT) | "A set of concurrent actions to be performed by the iced runtime"; `Task::perform` runs a future and maps its output into a message [17]. | iced's version of `Cmd`. It holds an opaque future, so it is not inspectable data. |
| **Crux** (redbadger/crux, **Apache-2.0** per GitHub) | "Side effects such as calling an API are captured as values and executed by the Shell." The bridge has `update`, `resolve` and `view`. The core compiles to Wasm and must stay side-effect-free [18]. | A direct precedent for signex's `execute` / `complete` / `query`, with Crux's `resolve` playing the part of signex's `complete`. |
| **sans-IO** | Cannon (2016): a protocol implementation with "no code that does any form of network I/O", built from "synchronous functions returning synchronous results". Its benefits are reuse across all flow-control styles and "no risk of non-determinism in the tests" [19]. | The same move applied to protocols: bytes in, events out. |

*(analysis)* "Commands return events and effects, and the shell performs IO" is the intersection of all four ideas. In hexagonal terms, the effect enum *is* the driven port, and `perform` is its one production adapter.

---

## 5. Desktop and GUI apps that share one core with headless front ends

### 5.1 Patterns

- **Presentation Model** (Fowler, 2004): "Represent the state and behavior of the presentation independently of the GUI controls". Its stated benefit is testing without the UI [20].
- **IO-free analysis core behind an API-boundary crate** (rust-analyzer; GitHub reports Apache-2.0):
  - "The analyzer keeps all this input data in memory and never does any IO".
  - "Only parts touching LSP are allowed to do IO".
  - `ide` "is an **API Boundary**", built from "POD types with public fields".
  - `base-db` "doesn't know about file system and file paths".
  - The vfs crates supply "consistent snapshots" fed by the client [21].
- **Remote UIs over RPC** (Neovim; mixed Apache-2.0 and Vim licence; only the user docs were read):
  - UIs are "external client processes communicating with Nvim over the RPC API".
  - Each UI attaches with `nvim_ui_attach`.
  - The core sends one "redraw" notification holding a batch of ordered events to every attached UI [22].
- **Process-split editor, a cautionary case** (xi-editor; Levien's 2020 retrospective):
  - "the process separation between front-end and core was not a good idea".
  - "adding async made everything more complicated".
  - Scrolling through a front-end cache "took months to get it right" [23].

### 5.2 Pitfalls *(analysis, informed by [20][23])*

1. **UI state leaking into the core.** Selection, focus, hover, the active tab and the active tool are presentation state. If the core stores them, every headless driver has to fake them, which is exactly the `gui_only` / `missing_headless_operand` heuristics signex has today.
2. **Selection ownership.** If the GUI owns selection, every selection-shaped command needs explicit target operands, and the GUI has to prune its selection whenever an event removes or renames items.
3. **Undo across front ends.** With two drivers editing one document, the policy for "whose step does Ctrl+Z undo?" must be written down. Recording the origin of each step is the minimum.
4. **Shadow copies.** xi's front-end cache is the warning [23]. Derived caches must be invalidated by events, never treated as state.

### 5.3 Event streams to remote clients without an in-process bus

- Neovim's shape is the one to copy: the serving adapter batches ordered events per attached client and pushes them as one notification [22].
- The Microsoft guide's integration-event rule applies: send only after the change has committed [13].
- The core does not need to know that subscribers exist.

---

## 6. Anti-patterns and failure modes

| Failure | Description | Source |
|---|---|---|
| Ports for everything | Dozens of fine-grained ports bring constructor explosion and test-double bloat. | [3] |
| Layer worship | Treating the hexagon as layers, or adding layers the problem doesn't need. | [1][2] |
| Anemic or transaction-script application services | Rules creep into per-request procedures. | [10][11] |
| Event-bus spaghetti | Immediate static dispatch hides control flow and hampers tests and debugging. | [13] |
| Distributed monolith in process | The process split and async of xi brought races and months of cache protocol work for little gain. | [23] |
| Large aggregates | Large-cluster aggregates bring transactional failures. | [8][9] |

---

## 7. Rust-specific considerations

### 7.1 Ownership and borrowing

*(analysis)* The design fits Rust's ownership model well:

- `update(&mut self)` calls `self.core.execute(..)` while holding `&mut self.core`.
- `view(&self)` reads `&self.core`. iced's `view` takes `&self`, so reading the core by reference costs nothing.

The friction is the projector. A method `Signex::project(&mut self, ..)` borrows all of `Signex`. A free function `project(ui: &mut UiState, core: &AppCore, events: &[CoreEvent])` uses disjoint field borrows and compiles cleanly. Prefer free functions over `&mut self` methods at the adapter seam.

### 7.2 Trait ports vs an enum of effects

| | `dyn Trait` port | Generic `AppCore<P: Ports>` | `enum Effect` + `perform` |
|---|---|---|---|
| Dispatch | Dynamic; inlining is blocked [24] | Static, through monomorphisation [24] | A `match` in the adapter |
| Compile time | Low | Grows with each instantiation; the type parameter spreads to every type that holds the core | Low |
| Object safety | Must satisfy dyn compatibility [24] | n/a | n/a |
| Testability | Needs a mock implementation | Needs a mock type | Assert on the returned `Vec<Effect>` and feed back any `EffectResult`, with no mocks |
| wasm32 | IO runs inside the core | IO runs inside the core | The core never runs IO |
| Openness | Open: plugins can add implementations | Open | **Closed**: a new IO kind means a new variant and a new `match` arm |
| Multi-step flows | Straight-line code | Straight-line code | An explicit state machine across `complete` calls |

Effects-as-data is the idiomatic Rust choice for a core that must not run IO. The protocol crates follow it:

- `quinn-proto` (quinn: GitHub reports Apache-2.0; the README lists Apache-2.0 and MIT) is a "deterministic state machine ... which performs no I/O internally" [25].
- rustls (Apache-2.0 OR ISC OR MIT) "doesn't make or accept TCP connections, or do DNS, or read or write files". The application pumps bytes through `read_tls` / `write_tls` [26].

No hyper/h2 sans-IO discussion could be verified, so this report does not cite one.

**Cost.** The cost lands on multi-step flows. Save, then git commit, then restore becomes a state machine keyed by effect ids, and those transitions are exactly the persistence invariants the epic marks high-risk.

### 7.3 `async` in a sync core

Cannon's definition [19] and Crux's `resolve` [18] both keep the core synchronous. This matches the Rust guidance of keeping tokio at the edge, which ADR D8 already follows. *(analysis)* Not every `spawn_blocking` site is IO. CPU-heavy pure work, such as a full ERC run or raster export, belongs off the UI thread, but it is compute, not an effect. Calling it an `Effect` muddles the port and adds `complete` round-trips that are not needed.

### 7.4 Errors across ports

- The anyhow README says to use anyhow "if you don't care what error type your functions return... common in application code", and thiserror "if you are a library that wants to design your own dedicated error type(s)" (anyhow: Apache-2.0 or MIT) [27].
- *(analysis)* The core and `signex-commands` are libraries, so they need typed errors (thiserror), and those errors must serialise for the wire (D11) with i18n keys (D6). Only binaries such as the CLI `main` should use anyhow.
- The epic's `query -> Result<Value, String>` throws the error type away at the port. It should return a typed error.

### 7.5 Crate boundaries as enforcement

rust-analyzer uses crate boundaries as API boundaries, and only one crate knows LSP and JSON [21]. *(analysis)* A crate boundary enforces the *direction* of dependencies. It does not stop an adapter from mutating the core's state through `pub` fields. The real guarantee that "documents change only through `execute`" comes from **private fields on `AppCore` plus `&`-only accessors**. A `pub` field or a `&mut` getter turns the port into a suggestion.

### 7.6 Large structs, moves and the stack

- `Box::new` is documented as "Allocates memory on the heap and then places `x` into it" [28].
- Large values can be built on the stack first and then copied, especially in debug builds. Stable Rust has no placement-new. The workarounds are:
  - `Box::new_uninit` (stable since 1.82) followed by `write` and `assume_init`;
  - `Box::write` (1.87), where the copy "may" be elided;
  - building with `Vec` or iterators [28][29].
- Clippy's `large_stack_frames` and `large_stack_arrays` lints exist but are allow-by-default [29].

*(analysis)* For signex, whose 1 MB Windows main-stack overflow is tracked as #2325:

- Give the core `fn new(..) -> Box<Self>`.
- Box its big fields individually.
- Never pass `AppCore` or `CoreOutput` by value through deep call chains.
- Enable `large_stack_frames` on the core crate.
- Keep `Effect` variants small with `Box` or `Arc` payloads, and serialise documents to bytes inside the core before they enter `WriteDocument`.

### 7.7 Send and Sync

*(analysis)* If the agent socket thread called `AppCore` directly, `AppCore` would need to be `Send` and sit behind a `Mutex`. The UI thread and the agent would then contend for it, and the single-writer MVU loop would break.

ADR D8 already prescribes the right shape: the worker sends over a channel, and a `Subscription` turns it into a `Message`. If attach requests enter `update` that way, `AppCore` needs neither `Send` nor `Sync`. Only `Effect` and `EffectResult` must be `Send`, so that `perform` can run on the blocking pool. The CLI owns its `Box<AppCore>` on one thread.

### 7.8 Precedents and licences

| Project | Licence | What it shows | What was read |
|---|---|---|---|
| Crux | Apache-2.0 | A shared core, effects as values, shells | README [18] |
| rust-analyzer | GitHub reports Apache-2.0 | An IO-free core and an API-boundary crate | architecture doc [21] |
| quinn-proto | GitHub reports Apache-2.0; README lists MIT too | A sans-IO state machine | README [25] |
| rustls | Apache-2.0 OR ISC OR MIT | Caller-driven IO | docs.rs [26] |
| iced | MIT | `Task` as a runtime-performed action | docs.rs [17] |
| helix | MPL-2.0 | not used; no architecture doc read | — |

### 7.9 What Rust makes cheap and what it makes expensive

*(analysis)*

- **Cheap:**
  - enums with exhaustive `match` for commands, events and effects;
  - borrow-checked single-writer mutation;
  - crate-enforced dependency direction;
  - `wasm32` and clippy as mechanical gates.
- **Expensive:**
  - multi-step async flows written as explicit state machines;
  - large moves and debug-build stack use;
  - open extension, because plugins cannot add enum variants;
  - the field-borrow choreography around `&mut self` methods;
  - churn in a large codebase, where renames that tools cannot do safely end up done by hand.

---

## 8. Check against signex

Verdict scale: **C** = consistent with the literature, **P** = partly consistent, **I** = inconsistent.

> **Status against the shipped ADR.** The rows below judge the *first draft* of the amendment. The shipped ADR-0001 resolves:
> - row 3a: `PersonGuard` is replaced by plain data in the arguments;
> - row 4: the host lives outside the core and is pluggable per platform;
> - row 5: attach requests enter through the GUI loop;
> - row 9: the A2 wording now calls the engine the aggregate's commands and `AppCore` the application layer.
>
> Rows 2, 8, 10 and 11 still apply, and the ADR addresses them as listed in its A4 and D5.

| # | Element | Verdict | Why | Recommendation | Counter-argument: when the recommendation is wrong for signex |
|---|---|---|---|---|---|
| 1 | One `AppCore` crate as the application layer over the engines | **C** | Matches Evans's thin application layer [7][10], Graça's application services [6] and Cockburn's app-inside-the-hexagon [1]. | Keep D2-A: an in-place module plus a boundary gate, and lift to a crate last. | A crate is enforcement, not architecture [2]. If the CLI could stop booting the GUI another way, a module with private fields gives roughly 90% of the benefit for two maintainers. Lift only when the CLI must drop `signex-app`. |
| 2 | Documents as aggregates, mutated only through the engine command path | **P** | It fits Evans's root and transaction rule [7]. Two gaps: (a) per ADR A2, only schematics go through `signex-engine` today, while symbol, footprint and Component rows use app-local history; (b) ECO, Update PCB, back-annotation and hierarchy renames change several documents at once, which breaks Vernon's one-aggregate-per-transaction rule [9]. | State in A4 that cross-document commands are one undo group, justified by Vernon's user-aggregate affinity (one person per desktop session) [9]. Record that the aggregate rule covers schematics only until #661's `History<S>` slice lands. | Attach mode puts a second actor on the same documents, which weakens affinity. `PersonGuard` is what keeps the exception honest. |
| 3 | `execute` / `complete` / `query` as driving ports | **C** | These are coarse ports in line with Cockburn's "two, three or four" [1]. They follow the command-bus and CQRS-split option [2] and mirror Crux's `update` / `resolve` / `view` [18]. | Make `query` typed (§7.4). Note that `complete` is the return leg of the driven side, as in Crux. | — |
| 3a | `guard: &dyn PersonGuard` | **P** | A callback from the core into the GUI is a hidden driven port, and it breaks "values at the boundary" [15]. | Pass a value, such as a `BusyTargets` snapshot, and drop the trait. | A value can go stale between snapshot and use. On one UI thread nothing changes in between, so staleness is harmless. |
| 4 | `Effect` plus one free `perform()` | **C** in kind, **P** in placement | Matches Elm `Cmd` [16], Crux [18] and sans-IO [19]. However, a single `perform` with `std::fs` cannot be the web adapter's `perform`, so `perform` is an *adapter*, not part of the core. In Cockburn's terms there is one port and several adapters [1]. | ADR A4 should say that `perform` lives outside the core crate, for example in an IO crate used by the desktop adapters, and that each platform supplies its own. Add an effect id to `Effect` / `EffectResult`, and require `complete` to handle results for closed or re-keyed documents. Do not turn pure CPU jobs into Effects (§7.3). Settle A3.2: a tool trait such as `Simulator` is called *from* `perform`, not from the core. | Converting all 181 sites is where most of the epic's risk sits (§10). |
| 5 | Core events returned to the caller, no bus | **C** | This is the deferred-dispatch pattern taken to its end [13][14] and the Elm and Crux shape [16][18]. It avoids immediate-dispatch debugging pain [13]. It differs from Vernon's in-process `DomainEventPublisher` [9], but Vernon publishes for *eventual* consistency across aggregates, which a single-user desktop app does not need. | Name them **application events** in Graça's sense [6], not domain events. **Gap:** D12 says "the GUI projector consumes the events of its own calls". In attach mode an agent edits the *running window*. A4 must state that in-process attach requests enter through `Subscription` → `Message` → `update` (ADR D8), so the GUI makes the call and sees its events, and the socket thread never touches `AppCore` (§7.7). | — |
| 6 | The GUI owns selection, focus and clipboard, and passes explicit operands | **C** | This is the Presentation Model [20] and is what makes headless drivers symmetric [1]. | Land the `Targets` operand first (the epic already says so). The projector must prune selection on `DocumentClosed` and on item removal. Write down an undo policy for agent steps in attach mode (§5.2). | Commands such as "select connected" or "next free designator" need core queries, so the core needs a query surface, not selection state. |
| 7 | GUI, CLI and agent socket as adapters | **C** | Cockburn's symmetric drivers [1]. Neovim's batched push to remote UIs is the model for the forwarding adapter [22]. | The forwarding adapter batches events, versions them (D11), and sends only after commit, following the integration-event rule [13]. | — |
| 8 | The wasm32 constraint (D10, ADR D5) | **P** (the gate is weaker than claimed) | No hexagonal source requires it. It is enforcement, like rust-analyzer's "never does IO" invariant [21]. **The ADR's D5 claim that a wasm32 build stops "file system or thread call[s]" is false.** On `wasm32-unknown-unknown`, `std::fs` compiles and "always return[s] errors" at run time, and `std::thread::spawn` compiles and "will panic" [30]. The build catches native dependencies (libgit2, mio and the like), not `std::fs`. | Add a `clippy.toml` with `disallowed-methods` and `disallowed-types` for `std::fs::*`, `std::thread::spawn`, `std::process`, `std::net` and `tokio::*` on the core and domain crates [31]. Keep the wasm32 check, but describe it as "no native-only dependencies". | YAGNI: no web front end is scheduled. The clippy gate alone gives the IO-free guarantee now. The wasm32 job can wait until slice S-IO actually begins. |
| 9 | ADR A2, "the command engine is our application layer", and the A4 amendment | **I** (as the text now stands) | The amended A2 paragraph names `AppCore` as the application layer, but **line 56, "The command engine *is* our 'application layer'", and Part C (:226), "the command/engine pattern already gives us an application layer", still stand**, and the A4 table lists the engines as domain services. In DDD terms `signex-engine`'s `Command::*` is the domain's operation set, playing the part of the aggregate root's methods [7], not an application layer [10]. | Rewrite line 56: "The command engine is the domain's operation set: the methods of the document aggregate in DDD terms. The application layer is `AppCore` (A4)." Strike the matching clause in Part C, so that the amendment does not keep two definitions side by side. | — |
| 10 | Risk that command handlers moved into `AppCore` become a transaction script | **P** (a real risk) | Fowler's definition of Transaction Script is literally "each procedure handles a single request from the presentation" [11]. Per-command handlers *are* procedures, and that is fine for an application layer as long as they hold no rules [6][10]. The risk is that defaults, designator assignment, connectivity or placement maths drift into handlers. | Add a review rule and a ratchet. A core handler only (a) resolves operands, (b) calls engine or domain functions, (c) groups the undo, and (d) assembles events and effects. A loop over document items, or any geometry arithmetic, in a handler is a smell that moves into an engine. Extend `check_layer_ratchet.sh` to count such patterns under `app/core/`. | A ratchet on heuristics produces false positives. Review discipline may be enough for two maintainers. |
| 11 | `signex-types` as the "shared kernel" | **P** | Evans's shared kernel is a small subset of the model shared *between bounded contexts or teams*, changed only after consultation [7]. `signex-types` is the whole model's data, shared inside one team. | Either call it "the common domain types", or keep "shared kernel" only for the schematic-to-PCB contract of A3.1 (`Netlist`), which is a genuine context seam. | — |

---

## 9. Skeptic's notes: where orthodoxy over-engineers signex

- **One process, one user, two maintainers.** Vernon's small-aggregate and eventual-consistency rules exist for scalability and locks [9]. Microsoft's guide calls real messaging "overkill" for in-process events [13]. None of that pressure applies here. *(analysis)* The epic is right not to add a bus. It should resist adding anything else justified only by "DDD says so".
- **Ports count.** The three driving entry points plus one effect enum are already minimal [1][3]. Adding trait ports per subsystem, such as a `GitPort` or a `LibraryPort`, would recreate the over-abstraction Petrov describes [3].
- **Effects for all 181 sites.** This is the most expensive slice, and it buys mostly the web option. xi's experience says async seams cost more than expected [23]. *(analysis)* Convert only the persistence and git effects, where the invariants live. Leave pure compute on its existing `Task` until a web front end is real.
- **A crate is not the goal.** Cockburn's goal is symmetric drivers and testability without devices [1]. For signex, that means a CLI that does not build UI, and parity by structure. Both are reached once the CLI calls the core module, before any crate is lifted.
- **The cheapest alternative**, for comparison. Keep `Signex`, make `UiState` lazy so the headless boot does not construct it, and box the large fields. That fixes #2325 and the build cost of the CLI. It does **not** remove the parity heuristics (`gui_only`, `honest_status`), so it is a stopgap, not a substitute.

---

## 10. Risks of adopting this design

1. **Persistence invariants turn into state machines.** Dirty marks, save ordering, the quit barrier and pending restores move across `complete` boundaries. A dropped transition silently loses work. The epic already flags slices 2, 3 and 5 as the high-risk ones.
2. **Churn against open work.** The epic counts 2,755 `ui_state.` and 5,386 `document_state.` references. Long-lived strangler phases breed dual paths.
3. **The closed effect enum vs plugins.** S-P plugins cannot add IO kinds without a core change. Decide before S-P whether plugins get a generic `PluginEffect` variant.
4. **Event granularity.** If `DocumentChanged { patch }` is too coarse, the projector over-invalidates and drags get slow. If it is too fine, events flood. Add a benchmark before slice 2 lands.
5. **The transaction-script drift** described in item 10 of §8.
6. **A false sense of IO-freedom** from the wasm32 gate (item 8 of §8).
7. **Attach-mode concurrency**, if the socket is ever allowed to call the core off the UI thread (§7.7).

---

## 11. Minimum viable version

*(analysis)* The smallest design that still meets the epic's stated goals: no UI built by the CLI, structural parity, and a testable core.

1. **`signex-commands` crate** (slice 1): the catalog, operands and outcome, plus the wire types. Without it the CLI cannot leave `signex-app`.
2. **An `app/core/` module with private fields.** It exposes `execute` / `query` and returns `CoreOutput { events, effects }`. Read-only accessors only. Guarded by the boundary script and a clippy `disallowed-methods` list instead of a wasm32 job.
3. **Effects only for persistence**: read, write, stat, git and journal, with effect ids. Everything else stays a `Task` around IO-free domain calls.
4. **Explicit operands plus the `Targets` kind.** No selection in the core. Attach requests enter through `Subscription` → `update`.
5. **The CLI uses the core module** through a thin crate split only when a dependency cut is needed (slice 9). The web front end (S-IO and the wasm32 job), the full effect conversion (5e) and the plugin host are deferred until each is scheduled.

This keeps every structural benefit Cockburn's intent asks for [1] and drops the parts that pay off only for a web or plugin future.

---

## References

1. Alistair Cockburn, "Hexagonal Architecture (Ports & Adapters)", HaT Technical Report 2005.02, 2005-09-04. https://alistair.cockburn.us/hexagonal-architecture/
2. Juan Manuel Garrido de Paz, "Hexagonal Architecture – Ports and Adapters Pattern", 2018-08-29. https://jmgarridopaz.github.io/content/hexagonalarchitecture
3. Alexander Petrov, "Ports & Adapters: death by a thousand ports", 2026-01-26. https://world.hey.com/apetrov/ports-adapters-death-by-a-thousand-ports-8b42afcf
4. Jeffrey Palermo, "The Onion Architecture: part 1", 2008. https://jeffreypalermo.com/2008/07/the-onion-architecture-part-1/
5. Robert C. Martin, "The Clean Architecture", 2012-08-13. https://blog.cleancoder.com/uncle-bob/2012/08/13/the-clean-architecture.html
6. Herberto Graça, "DDD, Hexagonal, Onion, Clean, CQRS, … How I put it all together", 2017-11-16. https://herbertograca.com/2017/11/16/explicit-architecture-01-ddd-hexagonal-onion-clean-cqrs-how-i-put-it-all-together/
7. Eric Evans, *DDD Reference*, 2015 (CC BY 4.0; pattern summaries CC BY 3.0). https://www.domainlanguage.com/ddd/reference/ (PDF: https://www.domainlanguage.com/wp-content/uploads/2016/05/DDD_Reference_2015-03.pdf)
8. Vaughn Vernon, "Effective Aggregate Design, Part I", 2011. https://www.dddcommunity.org/wp-content/uploads/files/pdf_articles/Vernon_2011_1.pdf
9. Vaughn Vernon, "Effective Aggregate Design, Part II: Making Aggregates Work Together", 2011. https://www.dddcommunity.org/wp-content/uploads/files/pdf_articles/Vernon_2011_2.pdf
10. Martin Fowler, "AnemicDomainModel", 2003-11-25 (quotes Evans on the application layer). https://martinfowler.com/bliki/AnemicDomainModel.html
11. Martin Fowler, "Transaction Script", *PoEAA*, 2002. https://martinfowler.com/eaaCatalog/transactionScript.html
12. Mark Seemann, "Functional architecture is Ports and Adapters", 2016-03-18. https://blog.ploeh.dk/2016/03/18/functional-architecture-is-ports-and-adapters/
13. Microsoft, ".NET Microservices Architecture: Domain events: design and implementation", 2018 (updated 2024). https://learn.microsoft.com/en-us/dotnet/architecture/microservices/microservice-ddd-cqrs-patterns/domain-events-design-implementation
14. Jimmy Bogard, "A better domain events pattern", 2014-05-13 (direct fetch returned 404; content verified through [13]). https://lostechies.com/jimmybogard/2014/05/13/a-better-domain-events-pattern
15. Gary Bernhardt, "Boundaries", SCNA 2012, and the screencast "Functional Core, Imperative Shell". https://www.destroyallsoftware.com/talks/boundaries
16. Evan Czaplicki et al., *An Introduction to Elm*, "Commands and Subscriptions". https://guide.elm-lang.org/effects/
17. iced 0.14 docs, `Task` (MIT). https://docs.rs/iced/0.14.0/iced/struct.Task.html
18. Red Badger, Crux README (Apache-2.0). https://github.com/redbadger/crux
19. Brett Cannon et al., "Writing I/O-Free (Sans-I/O) Protocol Implementations", 2016. https://sans-io.readthedocs.io/how-to-sans-io.html
20. Martin Fowler, "Presentation Model", 2004-07-19. https://martinfowler.com/eaaDev/PresentationModel.html
21. rust-analyzer, "Architecture" (GitHub reports Apache-2.0). https://rust-analyzer.github.io/book/contributing/architecture.html
22. Neovim docs, "UI events" (`api-ui-events`). https://neovim.io/doc/user/api-ui-events/
23. Raph Levien, "xi-editor retrospective", 2020-06-27. https://raphlinus.github.io/xi/2020/06/27/xi-retrospective.html
24. *The Rust Programming Language*, "Using Trait Objects to Abstract over Shared Behavior". https://doc.rust-lang.org/book/ch18-02-trait-objects.html
25. quinn README (GitHub reports Apache-2.0; the README lists Apache-2.0 and MIT). https://github.com/quinn-rs/quinn
26. rustls crate docs (Apache-2.0 OR ISC OR MIT). https://docs.rs/rustls/latest/rustls/
27. David Tolnay, anyhow README (Apache-2.0 or MIT). https://github.com/dtolnay/anyhow
28. Rust std docs, `Box` (`new`, `new_uninit` 1.82, `write` 1.87). https://doc.rust-lang.org/std/boxed/struct.Box.html
29. Rust Internals, "Returning a large type into a `Box` without copies", 2025-07. https://internals.rust-lang.org/t/returning-a-large-type-into-a-box-without-copies/23224
30. rustc book, "wasm32-unknown-unknown" platform support. https://doc.rust-lang.org/rustc/platform-support/wasm32-unknown-unknown.html
31. Clippy, "Lint configuration": `disallowed-methods`, `disallowed-types`. https://doc.rust-lang.org/clippy/lint_configuration.html

Related but not cited: the Rust Book chapter on `Send` and `Sync` (https://doc.rust-lang.org/book/ch16-04-extensible-concurrency-sync-and-send.html), for the §7.7 background.
