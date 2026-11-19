# Forum submission draft — DO NOT POST WITHOUT THE MISSING ITEMS

Post this as ONE reply to the official thread:
https://community.tari.com/t/november-build-contest-thread-mvp/520

## Still needed before posting (owner actions)

- [ ] **Tari payment address** — the owner's Tari address for winnings.
      Replace `TODO_TARI_PAYMENT_ADDRESS` below. Do not invent one.
- [ ] **Announcement link** — a public post on Bluesky, Reddit, Medium, or
      YouTube announcing the entry (NOT X). Drafts are in
      `docs/ANNOUNCEMENTS.md`; the owner posts and pastes the link here.

(Numbers verified 19 Nov 2026: 22 engine tests green, 15 parity tests green,
release WASM 156.4 KiB.)

---

## Ootle Escrow — milestone escrow for freelance work on Tari Ootle

**Code (MIT):** https://github.com/darendrarp/ootle-escrow

**MVP app (walletless demo, no funds needed):** https://darendrarp.github.io/ootle-escrow/

**Announcement:** TODO_ANNOUNCEMENT_LINK

**Tari payment address:** `TODO_TARI_PAYMENT_ADDRESS`

**What it is.** A client hiring a freelancer in XTM has no trustless way to
pay: pay upfront and the freelancer may vanish; deliver first and the client
may never pay. Ootle Escrow closes the gap. The buyer funds the full deal into
a template vault, and funds release milestone by milestone as the buyer
approves delivered work. A neutral arbiter splits disputed milestones, and
milestones nobody delivers by their deadline refund to the buyer. No
custodian, no platform cut, no account to create.

**Why it is an MVP you could launch today.** A freelancer creates a deal
(title, buyer, optional arbiter, 1–8 priced milestones with deadline epochs),
shares the link, and the client funds it with exactly the total — anything
else is rejected and nothing moves. Each milestone follows one path: deliver →
approve → paid, or deliver → dispute → arbiter split, or never delivered →
expiry refund. The demo walks the whole lifecycle in about four minutes,
including the deliberate rejections.

**What is new here.** November already has invoice payment (OotleInvoice,
TariBill) — but invoices pay directly, with no protection either way, and
TariBill's escrow is a roadmap item. Ootle Escrow is a different product:
locked funds + milestone release + arbiter splits + expiry refunds, with the
full lifecycle tested against the real Ootle engine.

**What is built and tested.**

- Rust `#[template]` on `tari_template_lib` 0.40: `new / fund /
  mark_delivered / approve_release / cancel_milestone / open_dispute /
  resolve_dispute / claim_expired_refund / cancel_deal`, plus 7 read-only
  views and 9 event types for indexers.
- 22 engine tests against the real Ootle engine
  (`tari_template_test_tooling` 0.53), all passing: every rule has a passing
  path and a rejection path — wrong role, double funding, wrong amount, late
  delivery, double release, silent cancel of delivered work, non-arbiter
  resolution, over-amount splits, early expiry claims — plus a full lifecycle
  test that settles the vault to exactly zero.
- 15 simulation parity tests (`node --test docs/sim.test.mjs`): the browser
  demo reproduces the template's rules and exact error messages.
- Release WASM: 156.4 KiB
  (`cargo build --target wasm32-unknown-unknown --release`), built with the
  official size profile (`opt-level='s'`, LTO, `panic='abort'`, stripped);
  the wallet daemon runs wasm-opt over it before estimating the publish fee.
- Full mechanism spec and threat model in `docs/MECHANISM.md`.

**Design points the judges may want to check.**

- Authorization is enforced in code against the transaction signer; the
  template creates no resource, so the "owner bypasses access rules" trap
  cannot apply.
- Delivered work is never auto-refunded — if the buyer goes silent after a
  delivery, the seller escalates to the arbiter instead of racing the clock.
- The arbiter cannot steal: a resolution only moves one disputed milestone's
  funds, only to the two deal accounts, and only adds up to exactly that
  amount.
- Funding requires the exact total, so the vault never holds an ambiguous
  balance; any failed call panics and the whole transaction rolls back.

**Honest limits.** The template has not been published to Esmeralda or
mainnet yet and is not audited; the demo is a clearly labelled browser
simulation, and no on-chain claims are made. The template cannot judge work
quality — it enforces *who decides*, not *what is good*. Amounts are public
(vault accounting). An arbiter who never rules leaves a disputed milestone
locked; a ruling deadline with a default split is the planned v2 fix.

Built with an AI coding agent under the submitter's direction, as the rules
allow. All code is new for this round.
