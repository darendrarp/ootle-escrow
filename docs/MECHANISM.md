# Ootle Escrow — Mechanism Specification

Milestone escrow for freelance work on Tari Ootle. Version 1.0 — written for
the Tari November Build Contest (MVP), November 2026.

## 1. The problem

A client hiring a freelancer in XTM has no trustless way to pay. Pay upfront
and the freelancer may vanish; deliver first and the client may never pay.
Existing Ootle entries cover adjacent ground — OotleInvoice pays an invoice
directly with no protection either way, TariBill adds a simple invoice escrow
as a roadmap item — but none of them implements milestone escrow: money locked
upfront, released piece by piece as work is approved, with a neutral third
party to break ties.

Ootle Escrow is that product. It is usable the day the template is published:
a freelancer creates a deal link, the client funds it, and both sides transact
through the template instead of trusting each other.

## 2. Parties

| Party | Identified by | Powers |
|---|---|---|
| Seller (freelancer) | public key | Creates the deal; marks milestones delivered; opens disputes; cancels pre-funding |
| Buyer (client) | public key | Funds the deal; approves releases; cancels pending milestones; opens disputes; cancels pre-funding |
| Arbiter (optional) | public key | Resolves disputed milestones by splitting them |
| Anyone | — | Claims an expired refund (funds always go to the buyer); reads all views |

Parties are identified by their transaction signer public key
(`CallerContext::transaction_signer_public_key()`), checked in code on every
method. There are no admin keys, no owner powers, and no upgrade path: the
component's access rules are `allow_all` on every method precisely because
authorization lives in the method bodies, where the rules are explicit and
testable. The template creates no resource, so the "owner bypasses access
rules" trap cannot apply — there is nothing to mint, freeze, or recall.

## 3. State machine

One deal is one component. Its vault holds the deal funds in the resource
named at creation (TARI in the demo).

```
                    ┌─────────────┐
                    │  Unfunded   │  cancel_deal (buyer/seller)
                    └──────┬──────┘
                           │ fund (buyer, exact total)
                           ▼
                    ┌─────────────┐
                    │   Funded    │
                    └──────┬──────┘
                           │ per milestone:
                           ▼
   PENDING ──mark_delivered──▶ DELIVERED ──approve_release──▶ RELEASED (terminal)
      │                           │  ▲
      │ cancel_milestone          │  │ open_dispute (buyer/seller)
      │ (buyer)                   │  │
      ▼                           ▼  │
   REFUNDED (terminal)       DISPUTED ──resolve_dispute (arbiter)──▶ RELEASED / SPLIT / REFUNDED
      ▲                                                              (terminal)
      │ claim_expired_refund (anyone, after deadline)
      └──────────────────────────────────────────────────────────────┘
```

Per-milestone status codes: 0 pending, 1 delivered, 2 disputed, 3 released,
4 refunded, 5 split. A deal is complete when every milestone is released,
refunded, or split; completion emits a `Completed` event.

## 4. Rules (each enforced by the template, each with an engine test)

### 4.1 Creation (`new`)

- Only the seller can create a deal naming themselves (signer must equal the
  seller key), so nobody can open deals impersonating someone else.
- Buyer ≠ seller; arbiter (if any) ≠ buyer and ≠ seller.
- Deal title and milestone titles: 1–120 characters.
- 1–8 milestones; the titles/amounts/deadlines lists must have equal length.
- Every milestone amount > 0; every deadline in the future; deadlines in
  non-decreasing order (milestones are sequential work).
- The vault is created empty for the deal resource; nothing is funded yet.

### 4.2 Funding (`fund`)

- Buyer only, exactly once, before any deadline passes.
- The payment bucket must be the deal resource and must equal the deal total
  **exactly**. Over- or under-payment panics and the whole transaction rolls
  back — the buyer is never part-charged and there is no change to lose.
- After funding, the full value sits in the component vault. The seller cannot
  touch it; only the milestone rules below can move it.

### 4.3 Delivery and release (`mark_delivered`, `approve_release`)

- The seller marks a milestone delivered while it is pending and its deadline
  has not passed (delivery exactly at the deadline epoch is allowed).
- The buyer approves delivered work; the template withdraws that milestone's
  amount and deposits it into the seller's account in the same transaction.
- The buyer cannot approve undelivered work, and cannot approve twice.

### 4.4 Cancellation (`cancel_milestone`, `cancel_deal`)

- The buyer can cancel a *pending* milestone and be refunded immediately —
  scope cuts happen. Delivered work cannot be cancelled silently: the buyer
  must approve it or dispute it.
- Before funding, buyer or seller can cancel the whole deal. After funding, a
  deal can only end milestone by milestone — the buyer cannot claw back money
  for work already delivered.

### 4.5 Disputes (`open_dispute`, `resolve_dispute`)

- Either side can escalate a delivered milestone to the arbiter: the buyer
  disputing quality, or the seller disputing a buyer who goes silent after
  delivery.
- Deals without an arbiter cannot open disputes; their escape hatch is the
  expiry refund (§4.6) for undelivered work.
- The arbiter — and only the arbiter — resolves by choosing a seller share
  between zero and the full amount; the rest returns to the buyer. Full share
  reads as Released, zero as Refunded, anything between as Split.
- The arbiter cannot steal: a resolution only moves one disputed milestone's
  funds, only to the two deal accounts, and only adds up to exactly that
  amount.

### 4.6 Expiry (`claim_expired_refund`)

- A milestone nobody delivered by its deadline can be refunded to the buyer
  by *anyone* once the deadline epoch has passed. The caller needs no role
  because the funds can only go to the buyer — anyone can do the buyer the
  favour of triggering it.
- **Delivered work never auto-refunds.** If the buyer stalls after a delivery,
  the seller's remedy is the dispute path, not a race against the clock.

### 4.7 Views and events

- Read-only views: `get_info`, `get_parties`, `get_resource`,
  `get_created_epoch`, `get_milestone`, `get_total`, `is_complete`.
- Events (topic `OotleEscrow.<Name>`): `DealCreated`, `Funded`, `Delivered`,
  `Released`, `Refunded` (reason: `cancelled` | `expired`), `Disputed`,
  `Resolved`, `Cancelled`, `Completed`. Indexers can rebuild any deal's
  history from events alone.

## 5. Money safety

- **Conservation.** Every payout withdraws from the vault and deposits into an
  account in the same transaction. The vault balance always equals the sum of
  unsettled milestones; the full-lifecycle test asserts the vault ends at zero.
- **No partial funding.** Exact-amount funding means the vault never holds an
  ambiguous balance.
- **No double payment.** Each milestone transitions to a terminal state in the
  same call that pays it; every paying method asserts the pre-state first.
- **Zero-amount payouts are skipped**, not withdrawn, so a 0/100 arbiter split
  never attempts an empty-bucket deposit.
- **Panic = rollback.** Any failed check aborts the whole transaction; no
  funds move on a rejected call.

## 6. Threat model

| Threat | Mitigation | Residual risk |
|---|---|---|
| Impersonated seller opens fake deals | Creator must sign as the seller | None for creation; a scammer can still pose as a *real* seller off-chain — users must verify identities out of band, as with any invoice |
| Buyer funds, seller never delivers | Expiry refund returns the money | Funds are locked until the deadline; pick realistic deadlines |
| Seller delivers, buyer never approves | Seller opens a dispute; arbiter rules | Needs an arbiter; without one, delivered funds sit until the parties agree off-chain |
| Buyer approves then claims non-delivery | Approval is final by design; disputes must precede approval | Buyer must inspect before approving — the UI says so |
| Malicious arbiter | Can only split one disputed milestone between the two parties; cannot take funds or touch other milestones | A biased arbiter can still rule unfairly — pick an arbiter both sides trust; reputation is the defence |
| Arbiter never rules | — | Disputed milestone stays locked. **Known limitation:** v1 has no ruling deadline. A future version can add one with a default split |
| Griefing via tiny milestones / spam deals | 8-milestone cap; each deal costs its own funding tx | Spam deals cost the spammer network fees; no on-chain registry to pollute |
| Front-running / reordering | All state transitions assert pre-state; a raced call simply fails | None identified |
| Template owner backdoor | Template creates no resource; no owner-only methods exist | None by construction |

## 7. What the template deliberately does not do

- **Judge work quality.** The chain cannot see whether a logo is good. It
  enforces *who decides* (buyer approves, arbiter breaks ties), not *what is
  good*. This is stated in the app, not buried.
- **Hide amounts.** Deal totals, milestones, and payouts are public
  (vault accounting). Confidential amounts would need a stealth design and are
  out of scope for v1.
- **Force delivery.** No code can make a human do the work. The mechanism
  bounds the loss (unreleased funds return) and prices dishonesty (the
  counterparty's time), it does not eliminate it.
- **Recurring billing / subscriptions.** Out of scope; milestone escrow is the
  product.

## 8. Verification

- `template/`: Rust template + engine tests (`cargo test`) — every rule in §4
  has a passing-path and a rejection-path test against the real Ootle engine.
- `docs/sim.test.mjs`: `node --test` parity tests — the browser simulation
  reproduces the template's rules and exact error messages.
- `cargo build --target wasm32-unknown-unknown --release` produces the
  publishable WASM (see README for the measured size).
- The demo at `docs/index.html` is a walletless simulation: no keys, no
  network, no funds. It is labelled as such on every screen.

## 9. Mainnet path (not yet done)

Publishing needs a Tari wallet holding XTM for the publish fee (the fee grows
with WASM size; the README records the measured binary size). Steps: build the
release WASM, publish via the wallet web UI ("Publish Template") or
`tari publish`, then wire the published template address into the demo's live
mode and re-verify the full lifecycle on Esmeralda first, mainnet second.
Until then, no on-chain claims are made.
