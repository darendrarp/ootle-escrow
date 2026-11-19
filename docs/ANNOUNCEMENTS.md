# Announcement drafts — Ootle Escrow

The contest requires the entry to be announced on **Bluesky, Reddit, Medium,
or YouTube** (NOT X). The owner posts; paste the published link into
`SUBMISSION.md` and the forum post. Pick ONE platform (one announcement link
is required; more is fine).

---

## Bluesky (short post)

> I built Ootle Escrow for the Tari November Build Contest (MVP) — milestone
> escrow for freelance work on the Ootle.
>
> Client funds lock in a vault, release milestone by milestone as work is
> approved. Arbiter splits disputes. Undelivered milestones refund after
> expiry. No custodian.
>
> Try it (no wallet): https://darendrarp.github.io/ootle-escrow/
> Code (MIT): https://github.com/darendrarp/ootle-escrow

## Reddit (r/tari — title + body)

**Title:** Ootle Escrow — milestone escrow for freelance work on Tari Ootle (November Build Contest: MVP)

**Body:**

Hiring a freelancer in XTM has a trust problem: pay upfront and they might
vanish; deliver first and the client might never pay.

Ootle Escrow is my entry for the November Build Contest (MVP). The buyer
funds the full deal into a template vault, and funds release milestone by
milestone as the buyer approves delivered work. A neutral arbiter splits
disputed milestones, and milestones nobody delivers by their deadline refund
to the buyer. No custodian, no platform cut.

- Demo (walletless, no funds): https://darendrarp.github.io/ootle-escrow/
- Code (MIT): https://github.com/darendrarp/ootle-escrow

What's built: a Rust `#[template]` (10 methods, 7 views, 9 event types),
engine tests against the real Ootle engine covering every rule with passing
and rejection paths, a full lifecycle test that settles the vault to zero,
and a browser demo that mirrors the template's rules exactly (15 parity
tests). Full mechanism spec + threat model in `docs/MECHANISM.md`.

Honest limits: the template isn't published to Esmeralda/mainnet yet and
isn't audited — the demo is a clearly labelled simulation. It can't judge
work quality (it enforces *who decides*, not *what is good*), amounts are
public, and an arbiter who never rules leaves a disputed milestone locked
(a ruling deadline is the planned v2 fix).

Feedback welcome — especially from anyone who hires or freelances in XTM.

## Medium (article outline — expand to ~600 words)

**Title:** Ootle Escrow: milestone escrow for freelance work on Tari Ootle

1. The trust gap in freelance crypto payments (pay-first vs deliver-first).
2. The mechanism in one paragraph + the state machine diagram from
   `docs/MECHANISM.md`.
3. Walkthrough: create → fund → deliver → approve → dispute → split →
   expiry refund (screenshots from the demo).
4. Design decisions: signer-checked authorization, no resource created (no
   owner-bypass trap), exact-amount funding, delivered work never
   auto-refunds, arbiter can only split.
5. What it doesn't do (honest limits) + the mainnet path.
6. Links: demo, repo, contest thread.

## YouTube (description for the demo video)

Ootle Escrow — milestone escrow for freelance work on Tari Ootle. Entry for
the Tari November Build Contest (MVP).

The client locks the full payment in a smart contract; it releases milestone
by milestone as work is approved. Arbiter splits disputes. Undelivered
milestones refund after expiry. No custodian, no platform cut.

- Try it (no wallet): https://darendrarp.github.io/ootle-escrow/
- Code (MIT): https://github.com/darendrarp/ootle-escrow
- Contest: https://community.tari.com/t/november-build-contest-thread-mvp/520

Script for the video is in the repo (`VIDEO_SCRIPT.md`). The demo is a
browser simulation of the Rust template, which is tested against the real
Ootle engine.
