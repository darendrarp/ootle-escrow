// Parity tests: the browser simulation must behave exactly like the Rust
// template (`template/src/lib.rs`). Same scenarios, same error messages.
// Run: node --test docs/sim.test.mjs
import test from "node:test";
import assert from "node:assert/strict";
import {
  PERSONAS,
  STATUS_PENDING,
  STATUS_DELIVERED,
  STATUS_DISPUTED,
  STATUS_RELEASED,
  STATUS_REFUNDED,
  STATUS_SPLIT,
  approveRelease,
  cancelDeal,
  cancelMilestone,
  claimExpiredRefund,
  createDeal,
  dealTotal,
  fundDeal,
  isComplete,
  markDelivered,
  openDispute,
  resolveDispute,
  sampleDeal,
  vaultBalance,
} from "./sim.js";

const S = PERSONAS.seller.pubkey;
const B = PERSONAS.buyer.pubkey;
const A = PERSONAS.arbiter.pubkey;
const X = PERSONAS.stranger.pubkey;
const NOW = 1000;

const spec = (over = {}) => ({
  title: "Website build",
  seller: S,
  buyer: B,
  arbiter: A,
  milestones: [
    { title: "Design", amount: 100_000_000, deadlineEpoch: 1010 },
    { title: "Build", amount: 200_000_000, deadlineEpoch: 1020 },
  ],
  ...over,
});

const funded = () => fundDeal(createDeal(spec(), S, NOW), 300_000_000, B, NOW);

test("create: happy path, totals and initial state", () => {
  const d = createDeal(spec(), S, NOW);
  assert.equal(dealTotal(d), 300_000_000);
  assert.equal(vaultBalanceSafe(d), 0);
  assert.equal(d.funded, false);
  assert.equal(d.cancelled, false);
  assert.equal(d.milestones.length, 2);
  assert.ok(d.milestones.every((m) => m.status === STATUS_PENDING));
  assert.equal(d.events[0].type, "DealCreated");
});

function vaultBalanceSafe(d) {
  return vaultBalance(d);
}

test("create: only the seller can create", () => {
  assert.throws(() => createDeal(spec(), B, NOW), /Only the seller can create this deal/);
  assert.throws(() => createDeal(spec(), X, NOW), /Only the seller can create this deal/);
});

test("create: rejects bad parties and titles", () => {
  assert.throws(() => createDeal(spec({ buyer: S }), S, NOW), /Buyer and seller must be different/);
  assert.throws(() => createDeal(spec({ arbiter: S }), S, NOW), /Arbiter must be different from the seller/);
  assert.throws(() => createDeal(spec({ arbiter: B }), S, NOW), /Arbiter must be different from the buyer/);
  assert.throws(() => createDeal(spec({ title: "" }), S, NOW), /Deal title must not be empty/);
  assert.throws(() => createDeal(spec({ title: "x".repeat(121) }), S, NOW), /at most 120 characters/);
});

test("create: rejects bad milestone lists", () => {
  assert.throws(() => createDeal(spec({ milestones: [] }), S, NOW), /at least one milestone/);
  const nine = Array.from({ length: 9 }, (_, i) => ({
    title: `M${i}`,
    amount: 1_000_000,
    deadlineEpoch: 1010 + i,
  }));
  assert.throws(() => createDeal(spec({ milestones: nine }), S, NOW), /at most 8 milestones/);
  assert.throws(
    () => createDeal(spec({ milestones: [{ title: "", amount: 1, deadlineEpoch: 1010 }] }), S, NOW),
    /Milestone title must not be empty/,
  );
  assert.throws(
    () => createDeal(spec({ milestones: [{ title: "M", amount: 0, deadlineEpoch: 1010 }] }), S, NOW),
    /Milestone amount must be greater than zero/,
  );
  assert.throws(
    () => createDeal(spec({ milestones: [{ title: "M", amount: 1, deadlineEpoch: 1000 }] }), S, NOW),
    /Milestone deadline must be in the future/,
  );
  assert.throws(
    () =>
      createDeal(
        spec({
          milestones: [
            { title: "A", amount: 1, deadlineEpoch: 1020 },
            { title: "B", amount: 1, deadlineEpoch: 1010 },
          ],
        }),
        S,
        NOW,
      ),
    /Milestone deadlines must be in order/,
  );
});

test("fund: exact amount only, buyer only, once", () => {
  const d = createDeal(spec(), S, NOW);
  assert.throws(() => fundDeal(d, 300_000_000, S, NOW), /Only the buyer can fund this deal/);
  assert.throws(() => fundDeal(d, 299_999_999, B, NOW), /Payment must equal the deal total exactly/);
  assert.throws(() => fundDeal(d, 300_000_001, B, NOW), /Payment must equal the deal total exactly/);
  const f = fundDeal(d, 300_000_000, B, NOW);
  assert.equal(f.funded, true);
  assert.equal(vaultBalance(f), 300_000_000);
  assert.equal(f.balances[B], 10_000_000_000 - 300_000_000);
  assert.throws(() => fundDeal(f, 300_000_000, B, NOW), /Deal is already funded/);
});

test("fund: refuses a deal with a passed deadline", () => {
  const d = createDeal(spec(), S, NOW);
  assert.throws(() => fundDeal(d, 300_000_000, B, 1015), /A milestone deadline has passed/);
});

test("deliver: seller only, funded, pending, before deadline", () => {
  const d = createDeal(spec(), S, NOW);
  assert.throws(() => markDelivered(d, 0, S, NOW), /Deal is not funded yet/);
  const f = funded();
  assert.throws(() => markDelivered(f, 0, B, NOW), /Only the seller can mark a milestone delivered/);
  assert.throws(() => markDelivered(f, 9, S, NOW), /Unknown milestone/);
  const d1 = markDelivered(f, 0, S, NOW);
  assert.equal(d1.milestones[0].status, STATUS_DELIVERED);
  assert.throws(() => markDelivered(d1, 0, S, NOW), /Milestone is not pending/);
  assert.throws(() => markDelivered(f, 0, S, 1011), /Milestone deadline has passed/);
  // Delivering exactly at the deadline epoch is allowed.
  assert.equal(markDelivered(f, 0, S, 1010).milestones[0].status, STATUS_DELIVERED);
});

test("release: buyer approves delivered work, seller is paid", () => {
  const f = funded();
  assert.throws(() => approveRelease(f, 0, B, NOW), /Milestone has not been delivered/);
  const d1 = markDelivered(f, 0, S, NOW);
  assert.throws(() => approveRelease(d1, 0, S, NOW), /Only the buyer can approve a release/);
  const r = approveRelease(d1, 0, B, NOW);
  assert.equal(r.milestones[0].status, STATUS_RELEASED);
  assert.equal(r.balances[S], 100_000_000);
  assert.equal(vaultBalance(r), 200_000_000);
  assert.throws(() => approveRelease(r, 0, B, NOW), /Milestone has not been delivered/);
});

test("cancel milestone: pending only, refunds buyer", () => {
  const f = funded();
  const c = cancelMilestone(f, 1, B, NOW);
  assert.equal(c.milestones[1].status, STATUS_REFUNDED);
  assert.equal(c.balances[B], 10_000_000_000 - 300_000_000 + 200_000_000);
  assert.equal(vaultBalance(c), 100_000_000);
  const d1 = markDelivered(f, 0, S, NOW);
  assert.throws(() => cancelMilestone(d1, 0, B, NOW), /Only a pending milestone can be cancelled/);
  assert.throws(() => cancelMilestone(f, 0, S, NOW), /Only the buyer can cancel a milestone/);
});

test("dispute: open by buyer or seller, needs arbiter and delivered status", () => {
  const f = funded();
  assert.throws(() => openDispute(f, 0, B, NOW), /Only a delivered milestone can be disputed/);
  const d1 = markDelivered(f, 0, S, NOW);
  assert.throws(() => openDispute(d1, 0, X, NOW), /Only the buyer or the seller can open a dispute/);
  const noArb = fundDeal(createDeal(spec({ arbiter: null }), S, NOW), 300_000_000, B, NOW);
  const noArbDelivered = markDelivered(noArb, 0, S, NOW);
  assert.throws(() => openDispute(noArbDelivered, 0, B, NOW), /This deal has no arbiter/);
  const disp = openDispute(d1, 0, B, NOW);
  assert.equal(disp.milestones[0].status, STATUS_DISPUTED);
  assert.throws(() => openDispute(disp, 0, S, NOW), /Only a delivered milestone can be disputed/);
});

test("resolve: arbiter splits, exact balances, terminal states", () => {
  const f = funded();
  const d1 = markDelivered(f, 0, S, NOW);
  const disp = openDispute(d1, 0, B, NOW);
  assert.throws(() => resolveDispute(disp, 0, 60_000_000, B, NOW), /Only the arbiter can resolve a dispute/);
  assert.throws(
    () => resolveDispute(disp, 0, 100_000_001, A, NOW),
    /Seller share exceeds the milestone amount/,
  );
  assert.throws(() => resolveDispute(d1, 0, 60_000_000, A, NOW), /Milestone is not under dispute/);
  const r = resolveDispute(disp, 0, 60_000_000, A, NOW);
  assert.equal(r.milestones[0].status, STATUS_SPLIT);
  assert.equal(r.balances[S], 60_000_000);
  assert.equal(r.balances[B], 10_000_000_000 - 300_000_000 + 40_000_000);
  assert.equal(vaultBalance(r), 200_000_000);
  // Full split to the seller reads as Released; full to the buyer as Refunded.
  const disp2 = openDispute(markDelivered(funded(), 1, S, NOW), 1, S, NOW);
  assert.equal(resolveDispute(disp2, 1, 200_000_000, A, NOW).milestones[1].status, STATUS_RELEASED);
  const disp3 = openDispute(markDelivered(funded(), 1, S, NOW), 1, B, NOW);
  assert.equal(resolveDispute(disp3, 1, 0, A, NOW).milestones[1].status, STATUS_REFUNDED);
});

test("expiry: pending milestones refund after deadline; delivered never auto-refunds", () => {
  const f = funded();
  assert.throws(() => claimExpiredRefund(f, 0, X, 1010), /Milestone has not expired yet/);
  const r = claimExpiredRefund(f, 0, X, 1011);
  assert.equal(r.milestones[0].status, STATUS_REFUNDED);
  assert.equal(r.balances[B], 10_000_000_000 - 300_000_000 + 100_000_000);
  const d1 = markDelivered(funded(), 0, S, NOW);
  assert.throws(
    () => claimExpiredRefund(d1, 0, B, 9999),
    /Only a pending milestone can be refunded as expired/,
  );
});

test("cancel deal: pre-funding only, buyer or seller", () => {
  const d = createDeal(spec(), S, NOW);
  assert.throws(() => cancelDeal(d, X, NOW), /Only the buyer or the seller can cancel the deal/);
  const c = cancelDeal(d, S, NOW);
  assert.equal(c.cancelled, true);
  assert.throws(() => fundDeal(c, 300_000_000, B, NOW), /Deal was cancelled/);
  assert.throws(() => cancelDeal(c, B, NOW), /Deal is already cancelled/);
  assert.throws(() => cancelDeal(funded(), B, NOW), /Cannot cancel a funded deal/);
});

test("full lifecycle: release + refund + split settles the vault to zero", () => {
  let d = createDeal(
    spec({
      milestones: [
        { title: "A", amount: 100_000_000, deadlineEpoch: 1010 },
        { title: "B", amount: 100_000_000, deadlineEpoch: 1020 },
        { title: "C", amount: 100_000_000, deadlineEpoch: 1030 },
      ],
    }),
    S,
    NOW,
  );
  d = fundDeal(d, 300_000_000, B, NOW);
  d = markDelivered(d, 0, S, NOW);
  d = approveRelease(d, 0, B, NOW);
  d = cancelMilestone(d, 1, B, NOW);
  d = markDelivered(d, 2, S, NOW);
  d = openDispute(d, 2, B, NOW);
  d = resolveDispute(d, 2, 30_000_000, A, NOW);
  assert.equal(vaultBalance(d), 0);
  assert.equal(isComplete(d), true);
  assert.equal(d.balances[S], 130_000_000);
  assert.equal(d.balances[B], 10_000_000_000 - 300_000_000 + 170_000_000);
  assert.ok(d.events.some((e) => e.type === "Completed"));
});

test("sample deal is valid and fundable", () => {
  const d = sampleDeal(NOW);
  assert.equal(dealTotal(d), 500_000_000);
  assert.equal(fundDeal(d, 500_000_000, B, NOW).funded, true);
});
