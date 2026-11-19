# Demo video script — Ootle Escrow (target: 2 minutes)

Record the browser demo at https://darendrarp.github.io/ootle-escrow/
with a fresh sample deal (Deals → "Load a fresh sample deal"). Narrate in
English. No face cam needed — screen + voiceover is enough.

## 0:00–0:15 — The problem (show the Deals page)

> "Hiring a freelancer in crypto has a trust problem. Pay upfront, and they
> might vanish. Deliver first, and the client might never pay. Ootle Escrow
> fixes it on Tari Ootle: the client locks the full payment in a smart
> contract, and it releases milestone by milestone — only when the work is
> approved."

## 0:15–0:40 — The deal (open the sample deal)

> "Here's a deal: logo and landing page for Tari Café, 500 TARI across three
> milestones. Alya is the freelancer, Budi the client, Citra the neutral
> arbiter. Nothing is funded yet — the vault is empty."

Show the deal page: parties, milestones, vault 0 TARI.

## 0:40–1:00 — Funding (switch to Budi, click Fund)

> "Budi funds the deal with exactly 500 TARI. The template rejects any other
> amount — no partial payments, no change to lose. Now the money is locked.
> Alya can't touch it, Budi can't take it back. Only the milestone rules can
> move it."

Click "Fund deal". Show vault 500 TARI, Funded event.

## 1:00–1:25 — Deliver and approve (Alya delivers #1, Budi approves)

> "Alya delivers the logo concepts. Budi reviews and approves — 150 TARI
> releases to Alya instantly, in the same transaction. One down, two to go."

Switch roles, mark delivered, approve. Show Released event + balances.

## 1:25–1:50 — Dispute and split (deliver #2, dispute, Citra splits)

> "Milestone two is disputed — Budi isn't happy with the design. Citra the
> arbiter reviews and splits it: 60% to Alya for the work done, 40% back to
> Budi. The arbiter can't steal anything — a ruling only splits one disputed
> milestone between the two sides."

Deliver #2 as Alya, dispute as Budi, resolve as Citra with a split. Show the
Split status.

## 1:50–2:05 — Expiry refund (advance epoch, claim refund)

> "And if a milestone is never delivered, anyone can trigger the refund once
> its deadline passes. The money always goes back to the buyer."

Advance the epoch past milestone #3's deadline, claim the expired refund.
Show vault at zero, deal Completed.

## 2:05–2:15 — Close (show the repo)

> "Ootle Escrow: milestone escrow for freelance work on Tari Ootle. The Rust
> template is tested against the real Ootle engine, the code is MIT licensed,
> and the demo runs entirely in your browser. Links below."

Show github.com/darendrarp/ootle-escrow.

## Recording notes

- Use a clean browser profile so localStorage starts fresh; the sample deal
  loads automatically on first visit.
- Keep the mouse slow over the role cards when switching — the role switch is
  the core interaction.
- If a click is rejected (wrong role), leave it in: the rejection messages are
  part of the demo. Or cut to the correct role.
- Captions: enable auto-captions; the script above can be pasted as the video
  description.
