# AIL — AI Layer

<div align="center">

**"Vibe code fearlessly."**

*The development environment AI deserves.*

<p>
  <a href="./LICENSE"><img alt="License: BUSL-1.1" src="https://img.shields.io/badge/license-BUSL--1.1-0f766e?style=for-the-badge"></a>
  <a href="./crates/ail-runtime-py/LICENSE"><img alt="Runtime: MIT" src="https://img.shields.io/badge/runtime-MIT-16a34a?style=for-the-badge"></a>
  <img alt="Status: active development" src="https://img.shields.io/badge/status-active%20development-f59e0b?style=for-the-badge">
</p>

<p>
  <img alt="Rust core" src="https://img.shields.io/badge/Rust-core-b7410e?style=flat-square&logo=rust&logoColor=white">
  <img alt="Z3 proofs" src="https://img.shields.io/badge/Z3-proofs-2563eb?style=flat-square">
  <img alt="MCP integration" src="https://img.shields.io/badge/MCP-agent%20tools-7c3aed?style=flat-square">
  <img alt="Python emitter" src="https://img.shields.io/badge/Python-emitter-3776ab?style=flat-square&logo=python&logoColor=white">
  <img alt="TypeScript emitter" src="https://img.shields.io/badge/TypeScript-emitter-3178c6?style=flat-square&logo=typescript&logoColor=white">
  <img alt="Phase 14" src="https://img.shields.io/badge/Phase%2014-agent%20foundation-111827?style=flat-square">
</p>

</div>

> ⚠️ **Early development.** This repo is a work in progress — the pipeline is being built and tested incrementally. Nothing runs end-to-end yet. If you're here early, you're seeing the ideas before the product.

---

## The problem nobody is talking about

You tell your AI "never let balance go negative."

It works. The feature ships. Everyone's happy.

Three weeks later, you add a refund feature. Different conversation, different context window. The AI doesn't remember what you said three weeks ago. Neither does your codebase. A refund runs. Balance goes to `-$240`. You find out from a customer email.

This isn't a hypothetical. This is Tuesday.

---

## What if constraints couldn't be forgotten?

That's AIL.

AIL sits **between your AI and your code** as an invisible layer. You describe what your system does — in plain English, in layers, the way you'd explain it on a whiteboard. AIL turns that into a graph. Every constraint you write propagates automatically to every node that needs it. When your AI generates code, AIL verifies the output with a mathematical proof before a single line reaches your codebase.

Not tests. **Proofs.**

```
You write 5 constraints.
AIL verifies 16.          ← the other 11 were inferred automatically
```

When something's wrong, you don't get "tests failed." You get:

```
Counterexample found:
  sender.balance = 100, amount = 120
  → new_balance = -20
  Rule violated: balance must stay ≥ 0
```

The bug is caught **before the code exists.**

---

## What makes this different

Most tools try to make AI faster. AIL makes AI **correct**.

The core idea: your system's constraints live in the graph structure itself — not in your AI's memory, not in a comment, not in a doc nobody reads. The system computes them deterministically. Every time. You can't forget a constraint because it's not stored where forgetting is possible.

We call this **CIC — Constraint Inheritance Chain**. Four propagation rules:

- **DOWN** — constraints flow from parent to all children automatically
- **UP** — verified child facts become available to the parent
- **ACROSS** — what one step outputs, the next step knows about
- **DIAGONAL** — type constraints inject everywhere that type is used

Write a constraint once on `User`. Every function that touches a `User` inherits it. Automatically. Forever.

---

## Document ↔ Code — always in sync

Here's the other thing AIL solves that nobody else does.

Documentation lies. Not because people are lazy — because there's no structural link between what's written and what runs. You refactor the code. The doc doesn't know. Six months later a new engineer reads the doc and builds the wrong thing.

AIL enforces a two-way contract:

```
Spec  →  Code:   .ail compiles to verified Python / TypeScript / Rust
Code  →  Spec:   every generated line traces back to the exact node that produced it
```

The whiteboard **is** the system. The code is the output. They cannot diverge.

---

## How it works

```
You describe your system in .ail
         ↓
   AIL parses it into a graph
         ↓
   CIC propagates all constraints
         ↓
   Z3 (mathematical solver) proves everything holds
         ↓
   AIL emits Python / TypeScript / Rust + tests
         ↓
   You review code you already understand
```

A real example. You write:

```
transfer money from sender to receiver, amount
  before: sender.balance >= amount
  before: amount > 0
  after:  sender.balance = old(sender.balance) - amount
  after:  receiver.balance = old(receiver.balance) + amount

  check sender and receiver are different users
    otherwise raise SameAccountError

  deduct amount from sender
  add amount to receiver
  save transaction record
```

AIL generates verified Python — with runtime guards, type annotations, and tests already written. If the logic violates any constraint anywhere in your entire graph, you know before `git push`.

---

## What AIL is not

This matters because it's easy to get wrong:

- **Not a language you need to learn.** AI writes `.ail`. You read Python.
- **Not competing with Cursor or Copilot.** AIL runs underneath them as a correctness layer. Use both.
- **Not a runtime.** AIL verifies and generates, then disappears. Your code runs natively.
- **Not low-code.** AIL is for engineers building real systems — the constraint engine is Z3, not drag-and-drop.

---

## Architecture

```
ail/
├── crates/
│   ├── ail-graph/       PSSD graph + CIC engine + search + validation
│   ├── ail-types/       Type system + constraint expressions + type checker
│   ├── ail-contract/    Static checks + Z3 encoding + verification pipeline
│   ├── ail-text/        PEG parser (17 syntax patterns + synonyms) + renderer
│   ├── ail-emit/        Python / TypeScript / Rust generators + test gen
│   ├── ail-mcp/         MCP server — connect Claude, Cursor, any AI agent
│   ├── ail-cli/         CLI (verify, build, search, context)
│   └── ail-runtime-py/  Python runtime (pre/post/keep validators)
```

Pipeline enforced by Rust's type system:

```
AilGraph → ValidGraph → TypedGraph → VerifiedGraph → Python / TS / Rust
```

You cannot emit unverified code. The compiler won't let you.

---

## MCP integration

AIL speaks MCP. Drop it into your Claude or Cursor setup and your AI agent can read and write the graph directly.

```json
{
  "mcpServers": {
    "ail": {
      "command": "ail-mcp",
      "args": ["--project", "."]
    }
  }
}
```

Your AI now has tools to navigate your system's structure — and every action it takes is constraint-aware.

---

## Status

**v0.1 in active development.** Built in Rust. Verified by Z3. Designed to run under the AI tools you already use.

**Currently:** v3.0 — Phase 14 Agent Foundation

| Component | Status |
|-----------|--------|
| Graph + CIC engine | ✅ v1.0 |
| Type system | ✅ v1.0 |
| Z3 verification | ✅ v1.0 |
| PEG parser (17 patterns) | ✅ v1.0 |
| Python emitter | ✅ v1.0 |
| MCP read tools (5 tools) | ✅ v1.0 |
| SQLite backend | ✅ v2.0 |
| Path-sensitive CIC | ✅ v2.0 |
| TypeScript emitter | ✅ v2.0 |
| Embedding search | ✅ v2.0 |
| MCP write tools (5 tools) | ✅ v2.0 |
| Semantic coverage (SCFT) | ✅ v3.0 |
| **Agent Foundation** | 🔄 **in progress** |
| AIL IDE (visual canvas) | 🔜 v4.0 |

---

## Roadmap

| Version | Theme | Key Deliverables |
|---------|-------|-----------------|
| ~~**v1.0**~~ | ~~Core engine~~ | ~~Parse · Graph · CIC · Z3 · Python emit · MCP read~~ |
| ~~**v2.0**~~ | ~~Foundation~~ | ~~SQLite · TypeScript emit · Embedding search · Path-sensitive CIC · MCP write~~ |
| ~~**v3.0 / Phase 13**~~ | ~~Semantic intelligence~~ | ~~`ail coverage` · intent coverage scoring · SCFT Step 2~~ |
| 🔄 **v3.0 / Phase 14** | Agent foundation | `ail agent "task"` · LangGraph · Planner + Coder workers · Provider layer |
| **v4.0** | IDE & Full Agent | Tauri visual IDE · AI Chat on canvas · Agent runs on canvas · Sheaf consistency |
| **v5.0** | Intelligence | Entropy analysis · Interactive debug · Advanced agent workflows |
| **v6.0** | Runtime | Runtime tracing · `.ailmap` crash → node · Production monitoring |
| **v7.0+** | Scale & Ecosystem | Rust emitter · Collaboration · SDK · Plugin system · Full launch |

---

## Tech stack

Rust · Z3 · petgraph · pest · SQLite · MCP · Tauri (IDE, v1.0)

---

## License

**Business Source License 1.1 (`BUSL-1.1`)**

AIL core is source-available under BUSL-1.1. Non-production use is permitted.
Personal, academic, research, educational, evaluation, and non-commercial
open-source production use is also permitted. Commercial production use of AIL
core, hosted or managed AIL service use, or embedding AIL core into commercial
products requires a commercial license from PhanNhuong123.
Commercial licensing terms can start from the non-binding
[commercial license template](./COMMERCIAL_LICENSE_TEMPLATE.md); no commercial
license is granted until a separate agreement or order form is signed.

AIL runtime packages and generated runtime helpers are MIT-licensed immediately
for generated applications. Each BUSL-licensed core version converts to the MIT
License on **2030-04-19** or the fourth
anniversary of that version's first public distribution, whichever comes first.
Code generated by unmodified AIL tools from user-authored input is not part of
the Licensed Work.

See [LICENSE](./LICENSE).

---

<div align="center">

*If you've ever shipped a bug because your AI forgot something you told it —*
*this is being built for you.*

</div>
