# Ootle Escrow — milestone escrow for freelance work on Tari Ootle

> Entry for the **Tari November Build Contest: MVP** (November 2026).
> All code is new, written for this round. MIT licensed.

A client hiring a freelancer in XTM has no trustless way to pay: pay upfront
and the freelancer may vanish; deliver first and the client may never pay.
**Ootle Escrow** closes the gap. The buyer funds the full deal into a template
vault, and funds release milestone by milestone as the buyer approves
delivered work. A neutral arbiter splits disputed milestones, and milestones
nobody delivers by their deadline refund to the buyer. No custodian, no
platform cut, no account to create.

**Try it now (no wallet, no funds):** https://darendrarp.github.io/ootle-escrow/

## What it is

- **Seller** creates a deal: buyer, optional arbiter, 1–8 milestones with a
  price and a deadline epoch each. Shares the deal link.
- **Buyer** funds the deal with exactly the total — anything else is rejected
  and nothing moves.
- **Seller** marks each milestone delivered; **buyer** approves and the funds
  release to the seller's account in the same transaction.
- **Either side** can escalate delivered work to the **arbiter**, who splits
  the milestone between seller and buyer.
- **Anyone** can trigger the refund of a milestone nobody delivered by its
  deadline — the money always goes back to the buyer.
- **Delivered work is never auto-refunded.** If the buyer goes silent after a
  delivery, the seller's remedy is the dispute path.

## What is new here

November already has invoice payment (OotleInvoice, TariBill) — but invoices
pay directly, with no protection either way, and TariBill's escrow is a
roadmap item. Ootle Escrow is a different product: **locked funds + milestone
release + arbiter splits + expiry refunds**, with the full lifecycle tested
against the real Ootle engine. It is the piece a freelancer actually needs
before taking XTM work from a stranger.

## Layout

```
ootle-escrow/
├── template/            Rust Ootle template (tari_template_lib 0.40)
│   ├── src/lib.rs       OotleEscrow component: 10 methods + 7 views
│   └── tests/test.rs    Engine tests (tari_template_test_tooling 0.53)
├── docs/                GitHub Pages demo (no build step, no dependencies)
│   ├── index.html       Demo app shell
│   ├── app.js           UI wiring (roles, routing, toasts)
│   ├── sim.js           Simulation mirroring the template rule for rule
│   ├── sim.test.mjs     Parity tests (node --test)
│   └── MECHANISM.md     Full mechanism spec + threat model
├── SUBMISSION.md        Forum submission draft (needs Tari address + announcement link)
├── VIDEO_SCRIPT.md      2-minute demo video script
└── LICENSE              MIT
```

## Verify it (for judges)

```bash
# 1. Engine tests — the template against the real Ootle engine, no network needed
cd template && cargo test
# 22 tests, all passing (31s on a 2-core VM)

# 2. Simulation parity tests — the demo reproduces the template's rules exactly
node --test docs/sim.test.mjs
# 15 tests, all passing

# 3. Build the publishable WASM
cargo build --target wasm32-unknown-unknown --release
# → target/wasm32-unknown-unknown/release/ootle_escrow.wasm (156.4 KiB;
#    the wallet daemon runs wasm-opt over it before estimating the publish fee)

# 4. Open the demo and follow the 4-minute guided tour (Tour tab),
#    including the deliberate rejections.
```

## Template API

| Method | Who | What |
|---|---|---|
| `new(seller, buyer, arbiter?, title, resource, titles, amounts, deadlines)` | Seller | Creates the deal (signer must be the seller) |
| `fund(payment: Bucket)` | Buyer | Funds the exact total, once |
| `mark_delivered(index)` | Seller | Marks work delivered, before the deadline |
| `approve_release(index, seller_account)` | Buyer | Pays the milestone to the seller |
| `cancel_milestone(index, buyer_account)` | Buyer | Cancels pending work, refunds the buyer |
| `open_dispute(index)` | Buyer or seller | Escalates delivered work to the arbiter |
| `resolve_dispute(index, seller_amount, seller_account, buyer_account)` | Arbiter | Splits a disputed milestone |
| `claim_expired_refund(index, buyer_account)` | Anyone | Refunds an undelivered, expired milestone to the buyer |
| `cancel_deal()` | Buyer or seller | Cancels before funding |
| `get_info / get_parties / get_resource / get_created_epoch / get_milestone / get_total / is_complete` | Anyone | Read-only views |

Milestone states: `0` pending, `1` delivered, `2` disputed, `3` released,
`4` refunded, `5` split. Events (`OotleEscrow.<Name>`): `DealCreated`,
`Funded`, `Delivered`, `Released`, `Refunded`, `Disputed`, `Resolved`,
`Cancelled`, `Completed`.

Authorization is enforced in code against the transaction signer; the
template creates no resource, so the "owner bypasses access rules" trap
cannot apply. Full spec, invariants, and threat model:
[`docs/MECHANISM.md`](docs/MECHANISM.md).

## Honest limits

- **Not published yet.** The template has not been published to Esmeralda or
  mainnet, and the demo is a browser simulation — labelled as such on every
  screen. No on-chain claims are made. Publishing needs a Tari wallet with XTM
  for the publish fee; see `docs/MECHANISM.md` §9 for the path.
- **Not audited.** Do not lock real funds until the template is published and
  reviewed.
- **The template cannot judge work quality.** It enforces *who decides*, not
  *what is good*. Pick an arbiter both sides trust.
- **Amounts are public** (vault accounting). Hiding them needs a
  confidential/stealth design, out of scope for v1.
- **An arbiter who never rules leaves a disputed milestone locked.** A ruling
  deadline with a default split is the planned v2 fix.
- **No code can force a human to work.** The mechanism bounds the loss to
  unreleased funds; it does not eliminate dishonesty.

## Development notes

- Built with an AI coding agent, as the contest rules explicitly allow
  (moderator confirmation, October 2026). The idea, direction, and review are
  the submitter's; the code is original and written for this round.
- Rust edition 2024, `tari_template_lib` 0.40, `tari_template_test_tooling`
  0.53. The release profile (`opt-level='s'`, LTO, `panic='abort'`, stripped)
  follows the official guide to keep the publish fee low.
- The web demo has no build step and no runtime dependencies: static HTML +
  vanilla JS, served from `docs/` via GitHub Pages.

## License

MIT — see [`LICENSE`](LICENSE).
