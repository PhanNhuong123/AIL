# AIL — AI Layer

<div align="center">

**"Vibe code fearlessly."**

*The development environment AI deserves.*

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

| Component | Status |
|-----------|--------|
| Graph + CIC engine | ✅ |
| Type system | ✅ |
| Z3 verification | ✅ |
| PEG parser (17 patterns) | ✅ |
| Python emitter | 🔄 in progress |
| TypeScript emitter | 🔜 v0.2 |
| SQLite backend | 🔜 v0.2 |
| MCP write tools | 🔜 v0.2 |
| AIL IDE (visual canvas) | 🔜 v1.0 |

---

## Roadmap

| Version | Direction |
|---------|-----------|
| **v0.1** | CLI · Python codegen · CIC · Z3 · MCP read |
| **v0.2** | TypeScript · SQLite backend · semantic search · MCP write |
| **v0.3** | Rust codegen · VSCode extension · live sync |
| **v1.0** | AIL IDE — visual canvas where the whiteboard *is* the system |

---

## Tech stack

Rust · Z3 · petgraph · pest · SQLite · MCP · Tauri (IDE, v1.0)

---

## License

**Business Source License 1.1**

Free for personal use and non-commercial research. Commercial use requires a license. Converts to MIT after 4 years.

See [LICENSE](./LICENSE).

---

<div align="center">

*If you've ever shipped a bug because your AI forgot something you told it —*
*this is being built for you.*

</div>
