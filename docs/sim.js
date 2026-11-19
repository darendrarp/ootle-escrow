// Ootle Escrow — browser simulation of the on-chain template rules.
//
// This module mirrors `template/src/lib.rs` exactly: same state machine, same
// validation order, same error messages. Every rule here has a matching engine
// test on the Rust side and a parity test in `sim.test.mjs`. The simulation
// runs entirely in the browser with no wallet and no network: it is a demo of
// the rules, not a connection to Tari.
//
// Amounts are integer micro-Tari (µT). 1 TARI = 1,000,000 µT.
// Time is measured in Ootle epochs (about 20 minutes on mainnet, about one
// hour on Esmeralda). The demo uses its own epoch counter starting at 1000.

export const STATUS_PENDING = 0;
export const STATUS_DELIVERED = 1;
export const STATUS_DISPUTED = 2;
export const STATUS_RELEASED = 3;
export const STATUS_REFUNDED = 4;
export const STATUS_SPLIT = 5;

export const STATUS_NAMES = {
  [STATUS_PENDING]: "Pending",
  [STATUS_DELIVERED]: "Delivered",
  [STATUS_DISPUTED]: "Disputed",
  [STATUS_RELEASED]: "Released",
  [STATUS_REFUNDED]: "Refunded",
  [STATUS_SPLIT]: "Split",
};

export const MAX_MILESTONES = 8;
export const MAX_TITLE_LEN = 120;

/** Demo personas. Public keys are fixed fake hex strings for the simulation. */
export const PERSONAS = {
  seller: {
    id: "seller",
    name: "Alya",
    role: "Seller (freelancer)",
    pubkey: "a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90",
  },
  buyer: {
    id: "buyer",
    name: "Budi",
    role: "Buyer (client)",
    pubkey: "b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90a1",
  },
  arbiter: {
    id: "arbiter",
    name: "Citra",
    role: "Arbiter (neutral)",
    pubkey: "c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2",
  },
  stranger: {
    id: "stranger",
    name: "Dedi",
    role: "Stranger (no role in the deal)",
    pubkey: "d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3",
  },
};

export function shortKey(pubkey) {
  return pubkey.slice(0, 6) + "…" + pubkey.slice(-4);
}

export function fmtTari(microTari) {
  return (microTari / 1_000_000).toLocaleString("en-US", {
    minimumFractionDigits: 0,
    maximumFractionDigits: 6,
  });
}

export function dealTotal(deal) {
  return deal.milestones.reduce((sum, m) => sum + m.amount, 0);
}

export function vaultBalance(deal) {
  if (!deal.funded) return 0;
  return deal.milestones
    .filter((m) => m.status === STATUS_PENDING || m.status === STATUS_DELIVERED || m.status === STATUS_DISPUTED)
    .reduce((sum, m) => sum + m.amount, 0);
}

export function isComplete(deal) {
  return (
    deal.funded &&
    deal.milestones.every(
      (m) => m.status === STATUS_RELEASED || m.status === STATUS_REFUNDED || m.status === STATUS_SPLIT,
    )
  );
}

function cloneDeal(deal) {
  return JSON.parse(JSON.stringify(deal));
}

function milestone(deal, index) {
  const m = deal.milestones[index];
  if (m === undefined) throw new Error("Unknown milestone");
  return m;
}

function pushEvent(deal, type, detail) {
  deal.events.push({ type, detail, epoch: deal._nowEpoch });
  if (type !== "Completed" && isComplete(deal)) {
    deal.events.push({ type: "Completed", detail: { title: deal.title, total: dealTotal(deal) }, epoch: deal._nowEpoch });
  }
}

// --- Actions. Each mirrors one template method, including panic messages. ---

/**
 * Create a deal. Mirrors `OotleEscrow::new`.
 * `spec` = { title, seller, buyer, arbiter|null, milestones: [{title, amount, deadlineEpoch}] }
 * `signer` must be the seller's pubkey. `nowEpoch` is the current epoch.
 */
export function createDeal(spec, signer, nowEpoch) {
  if (signer !== spec.seller) throw new Error("Only the seller can create this deal");
  if (spec.buyer === spec.seller) throw new Error("Buyer and seller must be different");
  if (spec.arbiter) {
    if (spec.arbiter === spec.seller) throw new Error("Arbiter must be different from the seller");
    if (spec.arbiter === spec.buyer) throw new Error("Arbiter must be different from the buyer");
  }
  if (!spec.title) throw new Error("Deal title must not be empty");
  if ([...spec.title].length > MAX_TITLE_LEN) throw new Error("Deal title must be at most 120 characters");
  if (spec.milestones.length === 0) throw new Error("A deal needs at least one milestone");
  if (spec.milestones.length > MAX_MILESTONES) throw new Error("A deal can have at most 8 milestones");
  let prevDeadline = 0;
  for (const m of spec.milestones) {
    if (!m.title) throw new Error("Milestone title must not be empty");
    if ([...m.title].length > MAX_TITLE_LEN) throw new Error("Milestone title must be at most 120 characters");
    if (!(m.amount > 0) || !Number.isInteger(m.amount)) throw new Error("Milestone amount must be greater than zero");
    if (!(m.deadlineEpoch > nowEpoch)) throw new Error("Milestone deadline must be in the future");
    if (!(m.deadlineEpoch >= prevDeadline)) throw new Error("Milestone deadlines must be in order");
    prevDeadline = m.deadlineEpoch;
  }
  const deal = {
    id: "deal-" + Math.random().toString(36).slice(2, 10),
    title: spec.title,
    seller: spec.seller,
    buyer: spec.buyer,
    arbiter: spec.arbiter || null,
    resource: "TARI",
    milestones: spec.milestones.map((m) => ({ ...m, status: STATUS_PENDING })),
    funded: false,
    cancelled: false,
    createdEpoch: nowEpoch,
    events: [],
    _nowEpoch: nowEpoch,
    // Simulated account balances (µT) for the demo personas.
    balances: { [spec.seller]: 0, [spec.buyer]: 10_000_000_000, ...(spec.arbiter ? { [spec.arbiter]: 0 } : {}) },
  };
  pushEvent(deal, "DealCreated", {
    title: deal.title,
    total: dealTotal(deal),
    milestones: deal.milestones.length,
  });
  return deal;
}

/** Fund the deal in full. Mirrors `fund`. `amount` must equal the deal total exactly. */
export function fundDeal(deal, amount, signer, nowEpoch) {
  const d = cloneDeal(deal);
  d._nowEpoch = nowEpoch;
  if (signer !== d.buyer) throw new Error("Only the buyer can fund this deal");
  if (d.cancelled) throw new Error("Deal was cancelled");
  if (d.funded) throw new Error("Deal is already funded");
  if (amount !== dealTotal(d)) throw new Error("Payment must equal the deal total exactly");
  if (!d.milestones.every((m) => m.deadlineEpoch > nowEpoch))
    throw new Error("A milestone deadline has passed; recreate the deal");
  if ((d.balances[d.buyer] ?? 0) < amount) throw new Error("Buyer balance is too low (simulation only)");
  d.balances[d.buyer] -= amount;
  d.funded = true;
  pushEvent(d, "Funded", { total: amount });
  return d;
}

/** Mark a milestone delivered. Mirrors `mark_delivered`. Seller only. */
export function markDelivered(deal, index, signer, nowEpoch) {
  const d = cloneDeal(deal);
  d._nowEpoch = nowEpoch;
  if (signer !== d.seller) throw new Error("Only the seller can mark a milestone delivered");
  if (!d.funded) throw new Error("Deal is not funded yet");
  const m = milestone(d, index);
  if (m.status !== STATUS_PENDING) throw new Error("Milestone is not pending");
  if (!(nowEpoch <= m.deadlineEpoch)) throw new Error("Milestone deadline has passed");
  m.status = STATUS_DELIVERED;
  pushEvent(d, "Delivered", { index });
  return d;
}

/** Approve delivered work and pay the seller. Mirrors `approve_release`. Buyer only. */
export function approveRelease(deal, index, signer, nowEpoch) {
  const d = cloneDeal(deal);
  d._nowEpoch = nowEpoch;
  if (signer !== d.buyer) throw new Error("Only the buyer can approve a release");
  const m = milestone(d, index);
  if (m.status !== STATUS_DELIVERED) throw new Error("Milestone has not been delivered");
  m.status = STATUS_RELEASED;
  d.balances[d.seller] = (d.balances[d.seller] ?? 0) + m.amount;
  pushEvent(d, "Released", { index, amount: m.amount });
  return d;
}

/** Cancel a pending milestone and refund the buyer. Mirrors `cancel_milestone`. Buyer only. */
export function cancelMilestone(deal, index, signer, nowEpoch) {
  const d = cloneDeal(deal);
  d._nowEpoch = nowEpoch;
  if (signer !== d.buyer) throw new Error("Only the buyer can cancel a milestone");
  if (!d.funded) throw new Error("Deal is not funded yet");
  const m = milestone(d, index);
  if (m.status !== STATUS_PENDING) throw new Error("Only a pending milestone can be cancelled");
  m.status = STATUS_REFUNDED;
  d.balances[d.buyer] = (d.balances[d.buyer] ?? 0) + m.amount;
  pushEvent(d, "Refunded", { index, amount: m.amount, reason: "cancelled" });
  return d;
}

/** Escalate a delivered milestone to the arbiter. Mirrors `open_dispute`. Buyer or seller. */
export function openDispute(deal, index, signer, nowEpoch) {
  const d = cloneDeal(deal);
  d._nowEpoch = nowEpoch;
  if (signer !== d.buyer && signer !== d.seller)
    throw new Error("Only the buyer or the seller can open a dispute");
  if (!d.arbiter) throw new Error("This deal has no arbiter");
  const m = milestone(d, index);
  if (m.status !== STATUS_DELIVERED) throw new Error("Only a delivered milestone can be disputed");
  m.status = STATUS_DISPUTED;
  pushEvent(d, "Disputed", { index, by: signer });
  return d;
}

/**
 * Resolve a disputed milestone. Mirrors `resolve_dispute`. Arbiter only.
 * `sellerAmount` goes to the seller, the rest to the buyer.
 */
export function resolveDispute(deal, index, sellerAmount, signer, nowEpoch) {
  const d = cloneDeal(deal);
  d._nowEpoch = nowEpoch;
  if (!d.arbiter) throw new Error("This deal has no arbiter");
  if (signer !== d.arbiter) throw new Error("Only the arbiter can resolve a dispute");
  const m = milestone(d, index);
  if (m.status !== STATUS_DISPUTED) throw new Error("Milestone is not under dispute");
  if (!(sellerAmount <= m.amount)) throw new Error("Seller share exceeds the milestone amount");
  if (!Number.isInteger(sellerAmount) || sellerAmount < 0)
    throw new Error("Seller share must be a non-negative amount");
  const buyerAmount = m.amount - sellerAmount;
  m.status =
    sellerAmount === m.amount ? STATUS_RELEASED : sellerAmount === 0 ? STATUS_REFUNDED : STATUS_SPLIT;
  d.balances[d.seller] = (d.balances[d.seller] ?? 0) + sellerAmount;
  d.balances[d.buyer] = (d.balances[d.buyer] ?? 0) + buyerAmount;
  pushEvent(d, "Resolved", { index, sellerAmount, buyerAmount, status: m.status });
  return d;
}

/** Refund a milestone nobody delivered by its deadline. Mirrors `claim_expired_refund`. Anyone. */
export function claimExpiredRefund(deal, index, signer, nowEpoch) {
  const d = cloneDeal(deal);
  d._nowEpoch = nowEpoch;
  if (!d.funded) throw new Error("Deal is not funded yet");
  const m = milestone(d, index);
  if (m.status !== STATUS_PENDING) throw new Error("Only a pending milestone can be refunded as expired");
  if (!(nowEpoch > m.deadlineEpoch)) throw new Error("Milestone has not expired yet");
  m.status = STATUS_REFUNDED;
  d.balances[d.buyer] = (d.balances[d.buyer] ?? 0) + m.amount;
  pushEvent(d, "Refunded", { index, amount: m.amount, reason: "expired" });
  return d;
}

/** Cancel the whole deal before funding. Mirrors `cancel_deal`. Buyer or seller. */
export function cancelDeal(deal, signer, nowEpoch) {
  const d = cloneDeal(deal);
  d._nowEpoch = nowEpoch;
  if (signer !== d.buyer && signer !== d.seller)
    throw new Error("Only the buyer or the seller can cancel the deal");
  if (d.funded) throw new Error("Cannot cancel a funded deal");
  if (d.cancelled) throw new Error("Deal is already cancelled");
  d.cancelled = true;
  pushEvent(d, "Cancelled", { title: d.title });
  return d;
}

/** One demo deal, pre-filled so judges can start clicking immediately. */
export function sampleDeal(nowEpoch) {
  return createDeal(
    {
      title: "Logo + landing page for Tari Café",
      seller: PERSONAS.seller.pubkey,
      buyer: PERSONAS.buyer.pubkey,
      arbiter: PERSONAS.arbiter.pubkey,
      milestones: [
        { title: "Logo concepts (3 options)", amount: 150_000_000, deadlineEpoch: nowEpoch + 12 },
        { title: "Landing page design", amount: 250_000_000, deadlineEpoch: nowEpoch + 24 },
        { title: "Final files + revisions", amount: 100_000_000, deadlineEpoch: nowEpoch + 36 },
      ],
    },
    PERSONAS.seller.pubkey,
    nowEpoch,
  );
}
