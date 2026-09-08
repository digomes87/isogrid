# Architecture Decision Records

Short notes on choices that are expensive to reverse: what was decided, why, and
what it costs.

An ADR is not documentation of how the code works — that belongs in rustdoc and
[`ARCHITECTURE.md`](../../ARCHITECTURE.md). An ADR exists so that someone
reading the code in a year, and disagreeing with it, can find out whether the
alternative they have in mind was already considered and rejected.

Records are immutable once merged. If a decision changes, write a new record
that supersedes the old one and mark the old one as superseded.

| # | Decision | Status |
| --- | --- | --- |
| [0001](0001-two-crates.md) | Split the engine and the game into two crates | Accepted |
| [0002](0002-integer-fixed-timestep.md) | Fixed timestep with an integer accumulator | Accepted |
| [0003](0003-bundled-rng.md) | Bundle a generator instead of depending on `rand` | Accepted |

## Template

```markdown
# NNNN. Title in the imperative

**Status:** Proposed | Accepted | Superseded by [NNNN](NNNN-....md)
**Date:** YYYY-MM-DD

## Context

What is the situation? What forces are in play?

## Decision

What was decided, in one or two sentences.

## Consequences

What this makes easy, what it makes hard, and what has to be true for it to
keep being the right call.

## Alternatives considered

What else was on the table, and why it lost.
```
