// Ootle Escrow — demo UI wiring. All deal logic lives in sim.js, which
// mirrors the Rust template rule for rule. This file only renders state,
// collects input, and surfaces the simulation's exact accept/reject messages.
import {
  PERSONAS,
  STATUS_PENDING,
  STATUS_DELIVERED,
  STATUS_DISPUTED,
  STATUS_RELEASED,
  STATUS_REFUNDED,
  STATUS_SPLIT,
  STATUS_NAMES,
  approveRelease,
  cancelDeal,
  cancelMilestone,
  claimExpiredRefund,
  createDeal,
  dealTotal,
  fmtTari,
  fundDeal,
  isComplete,
  markDelivered,
  openDispute,
  resolveDispute,
  sampleDeal,
  shortKey,
  vaultBalance,
} from "./sim.js";

const STORE_KEY = "ootle-escrow-v1";
const $ = (sel) => document.querySelector(sel);

let state = load() || freshState();
let view = "deals";
let openDealId = null;

function freshState() {
  const epoch = 1000;
  const sample = sampleDeal(epoch);
  return { epoch, signerId: "seller", deals: { [sample.id]: sample }, order: [sample.id] };
}

function load() {
  try {
    const raw = localStorage.getItem(STORE_KEY);
    if (!raw) return null;
    const s = JSON.parse(raw);
    if (!s.deals || !s.order) return null;
    return { epoch: s.epoch ?? 1000, signerId: "seller", deals: s.deals, order: s.order };
  } catch {
    return null;
  }
}

function persist() {
  localStorage.setItem(STORE_KEY, JSON.stringify({ epoch: state.epoch, deals: state.deals, order: state.order }));
}

function signerPubkey() {
  return PERSONAS[state.signerId].pubkey;
}

function esc(s) {
  return String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
}

function toast(msg, kind) {
  const el = $("#toast");
  el.textContent = msg;
  el.className = kind || "";
  el.style.display = "block";
  clearTimeout(toast._t);
  toast._t = setTimeout(() => (el.style.display = "none"), 4500);
}

function tryAction(label, fn) {
  try {
    fn();
    persist();
    render();
    toast("✓ " + label, "ok");
  } catch (e) {
    toast("✗ Rejected — " + e.message, "error");
    render();
  }
}

function dealLink(id) {
  return location.origin + location.pathname + "#/deal/" + id;
}

// ---------- routing ----------

function route() {
  const m = location.hash.match(/^#\/deal\/([\w-]+)$/);
  if (m && state.deals[m[1]]) {
    view = "deal";
    openDealId = m[1];
  } else if (view === "deal") {
    view = "deals";
    openDealId = null;
  }
  render();
}
window.addEventListener("hashchange", route);

// ---------- render ----------

function render() {
  $("#epochNow").textContent = state.epoch;
  renderRoles();
  document.querySelectorAll("nav button").forEach((b) => b.classList.toggle("active", b.dataset.view === view));
  for (const v of ["deals", "create", "tour", "how", "deal"]) {
    $("#view-" + v).style.display = v === view ? "" : "none";
  }
  if (view === "deals") renderDeals();
  if (view === "create") renderCreate();
  if (view === "tour") renderTour();
  if (view === "how") renderHow();
  if (view === "deal") renderDeal();
}

function renderRoles() {
  $("#roles").innerHTML = Object.values(PERSONAS)
    .map(
      (p) => `<div class="role ${p.id === state.signerId ? "active" : ""}" data-role="${p.id}">
        <b>${esc(p.name)}</b><small>${esc(p.role)}</small>
        <div class="key">${shortKey(p.pubkey)}</div>
      </div>`,
    )
    .join("");
  document.querySelectorAll(".role").forEach((el) =>
    el.addEventListener("click", () => {
      state.signerId = el.dataset.role;
      render();
    }),
  );
}

function statusPill(code) {
  const cls = STATUS_NAMES[code].toLowerCase();
  return `<span class="pill ${cls}">${STATUS_NAMES[code]}</span>`;
}

function dealStatus(d) {
  if (d.cancelled) return "Cancelled";
  if (isComplete(d)) return "Completed";
  if (!d.funded) return "Awaiting funding";
  return "In progress";
}

function renderDeals() {
  const rows = state.order
    .map((id) => state.deals[id])
    .filter(Boolean)
    .map(
      (d) => `<div class="dealrow">
        <div>
          <b><a href="#/deal/${d.id}">${esc(d.title)}</a></b>
          <div class="muted">${d.milestones.length} milestones · total ${fmtTari(dealTotal(d))} TARI · vault ${fmtTari(vaultBalance(d))} TARI</div>
          <div class="muted">Seller ${esc(nameOf(d.seller))} · Buyer ${esc(nameOf(d.buyer))}${d.arbiter ? ` · Arbiter ${esc(nameOf(d.arbiter))}` : " · no arbiter"}</div>
        </div>
        <div style="text-align:right">
          <div>${dealStatus(d)}</div>
          <div class="muted">created @ epoch ${d.createdEpoch}</div>
        </div>
      </div>`,
    )
    .join("");
  $("#view-deals").innerHTML = `
    <section class="card">
      <h2 style="margin-top:0">Milestone escrow for freelance work</h2>
      <p>The buyer funds the full deal into a vault. The seller delivers milestone by milestone; the buyer approves each one and the funds release. A neutral arbiter splits disputed milestones. Milestones nobody delivers by their deadline refund to the buyer. No custodian, no platform cut — the template holds the money and follows the rules.</p>
      <p class="muted">This page simulates the on-chain template in your browser. Switch roles above to play each side. Every rejection below uses the template's exact error message.</p>
      <div style="display:flex;gap:8px;flex-wrap:wrap">
        <button class="primary" id="btnNewSample">Load a fresh sample deal</button>
        <button id="btnReset">Reset demo (clear all deals)</button>
      </div>
    </section>
    <section class="card">
      <h3 style="margin-top:0">Your deals (${state.order.length})</h3>
      ${rows || '<p class="muted">No deals yet. Create one, or load the sample deal.</p>'}
    </section>
    <section class="card">
      <h3 style="margin-top:0">Simulated balances</h3>
      <p class="muted">Each persona starts the demo with play funds. On-chain, balances live in real accounts — the template never holds keys.</p>
      ${balanceTable()}
    </section>`;
  $("#btnNewSample").addEventListener("click", () => {
    const d = sampleDeal(state.epoch);
    state.deals[d.id] = d;
    state.order.unshift(d.id);
    persist();
    location.hash = "#/deal/" + d.id;
  });
  $("#btnReset").addEventListener("click", () => {
    state = freshState();
    persist();
    location.hash = "";
    view = "deals";
    render();
    toast("Demo reset — fresh sample deal loaded", "ok");
  });
}

function nameOf(pubkey) {
  return Object.values(PERSONAS).find((p) => p.pubkey === pubkey)?.name ?? shortKey(pubkey);
}

function balanceTable() {
  const totals = {};
  for (const d of Object.values(state.deals)) {
    for (const [k, v] of Object.entries(d.balances || {})) totals[k] = (totals[k] ?? 0) + v;
  }
  // Buyer starts with 10,000 TARI per deal in the sim; show the latest deal's balances instead.
  const latest = state.deals[state.order[0]];
  if (!latest) return "";
  return `<dl class="kv">${Object.entries(latest.balances || {})
    .map(([k, v]) => `<dt>${esc(nameOf(k))}</dt><dd>${fmtTari(v)} TARI</dd>`)
    .join("")}</dl>
    <div class="muted">Balances shown for the newest deal (${esc(latest.title)}).</div>`;
}

function renderDeal() {
  const d = state.deals[openDealId];
  if (!d) {
    view = "deals";
    return render();
  }
  const me = signerPubkey();
  const myRole = me === d.seller ? "seller" : me === d.buyer ? "buyer" : me === d.arbiter ? "arbiter" : "stranger";
  const milestones = d.milestones
    .map((m, i) => {
      const expired = state.epoch > m.deadlineEpoch;
      const actions = [];
      if (m.status === STATUS_PENDING) {
        if (d.funded) {
          actions.push(`<button data-act="deliver" data-i="${i}">Mark delivered (seller)</button>`);
          actions.push(`<button data-act="cancel-ms" data-i="${i}">Cancel &amp; refund (buyer)</button>`);
          if (expired) actions.push(`<button data-act="expire" data-i="${i}">Claim expired refund (anyone)</button>`);
        }
      }
      if (m.status === STATUS_DELIVERED) {
        actions.push(`<button class="primary" data-act="release" data-i="${i}">Approve &amp; pay seller (buyer)</button>`);
        actions.push(`<button data-act="dispute" data-i="${i}">Open dispute (buyer/seller)</button>`);
      }
      if (m.status === STATUS_DISPUTED) {
        actions.push(`<span class="muted">Arbiter splits ${fmtTari(m.amount)} TARI:</span>
          <input type="number" min="0" max="${m.amount}" value="${Math.floor(m.amount / 2)}" id="split-${i}" class="narrow" style="max-width:170px" placeholder="seller share (µT)">
          <button data-act="resolve" data-i="${i}">Resolve (arbiter)</button>`);
      }
      return `<div class="ms">
        <div class="head"><b>#${i + 1} ${esc(m.title)}</b>${statusPill(m.status)}</div>
        <div class="muted">${fmtTari(m.amount)} TARI · deadline epoch ${m.deadlineEpoch}${expired && m.status === STATUS_PENDING ? ' · <b style="color:var(--danger)">expired</b>' : ""}</div>
        ${actions.length ? `<div class="actions">${actions.join("")}</div>` : ""}
      </div>`;
    })
    .join("");

  $("#view-deal").innerHTML = `
    <section class="card">
      <p><a href="#" id="backLink">← All deals</a></p>
      <h2 style="margin:0 0 4px">${esc(d.title)}</h2>
      <div class="muted">Deal ${d.id} · ${dealStatus(d)} · you are acting as <b>${esc(PERSONAS[state.signerId].name)}</b> (${myRole} in this deal)</div>
      <dl class="kv" style="margin-top:12px">
        <dt>Seller</dt><dd>${esc(nameOf(d.seller))} <span class="mono">${shortKey(d.seller)}</span></dd>
        <dt>Buyer</dt><dd>${esc(nameOf(d.buyer))} <span class="mono">${shortKey(d.buyer)}</span></dd>
        <dt>Arbiter</dt><dd>${d.arbiter ? `${esc(nameOf(d.arbiter))} <span class="mono">${shortKey(d.arbiter)}</span>` : "none — disputes unavailable, expiry refund is the escape hatch"}</dd>
        <dt>Total</dt><dd>${fmtTari(dealTotal(d))} TARI</dd>
        <dt>Vault</dt><dd>${fmtTari(vaultBalance(d))} TARI locked</dd>
        <dt>Funded</dt><dd>${d.funded ? "yes" : "no"} · Cancelled: ${d.cancelled ? "yes" : "no"}</dd>
      </dl>
      <div style="display:flex;gap:8px;flex-wrap:wrap;margin-top:12px">
        ${!d.funded && !d.cancelled ? `<button class="primary" id="btnFund">Fund deal — exactly ${fmtTari(dealTotal(d))} TARI (buyer)</button>` : ""}
        ${!d.funded && !d.cancelled ? `<button id="btnCancelDeal" class="danger">Cancel deal (buyer/seller)</button>` : ""}
        <button id="btnCopyLink">Copy deal link</button>
      </div>
      ${!d.funded && !d.cancelled ? `<p class="muted">Try funding as the wrong role, or with the wrong amount in the guided tour — the template rejects it and nothing moves. On-chain, a rejected transaction costs no funds (only network fees may still apply).</p>` : ""}
    </section>
    <section class="card"><h3 style="margin-top:0">Milestones</h3>${milestones}</section>
    <section class="card">
      <h3 style="margin-top:0">Deal events</h3>
      <div class="events">${d.events.map((e) => `<div><code>${e.type}</code> <span class="muted">@ epoch ${e.epoch} — ${esc(JSON.stringify(e.detail))}</span></div>`).join("") || '<span class="muted">No events yet.</span>'}</div>
    </section>
    <section class="card">
      <h3 style="margin-top:0">Rejections to try</h3>
      <ul class="muted">
        <li>Fund the deal as Alya (seller) — only the buyer can fund.</li>
        <li>Mark a milestone delivered as Budi (buyer) — only the seller delivers.</li>
        <li>Approve a release before delivery — the milestone has not been delivered.</li>
        <li>Cancel a delivered milestone — delivered work can only be approved or disputed.</li>
        <li>Resolve a dispute as anyone but Citra (arbiter).</li>
        <li>Claim an expiry refund before the deadline — advance the epoch past it first.</li>
      </ul>
    </section>`;

  $("#backLink").addEventListener("click", (e) => {
    e.preventDefault();
    location.hash = "";
    view = "deals";
    render();
  });
  $("#btnCopyLink").addEventListener("click", async () => {
    try {
      await navigator.clipboard.writeText(dealLink(d.id));
      toast("Deal link copied", "ok");
    } catch {
      toast(dealLink(d.id), "ok");
    }
  });
  const fundBtn = $("#btnFund");
  if (fundBtn) {
    fundBtn.addEventListener("click", () =>
      tryAction(`Deal funded — ${fmtTari(dealTotal(d))} TARI locked in the vault`, () => {
        state.deals[d.id] = fundDeal(d, dealTotal(d), me, state.epoch);
      }),
    );
  }
  const cancelBtn = $("#btnCancelDeal");
  if (cancelBtn) {
    cancelBtn.addEventListener("click", () =>
      tryAction("Deal cancelled", () => {
        state.deals[d.id] = cancelDeal(d, me, state.epoch);
      }),
    );
  }
  document.querySelectorAll("[data-act]").forEach((btn) =>
    btn.addEventListener("click", () => {
      const i = Number(btn.dataset.i);
      const act = btn.dataset.act;
      if (act === "deliver")
        tryAction(`Milestone #${i + 1} marked delivered`, () => {
          state.deals[d.id] = markDelivered(d, i, me, state.epoch);
        });
      if (act === "release")
        tryAction(`Milestone #${i + 1} released — seller paid ${fmtTari(d.milestones[i].amount)} TARI`, () => {
          state.deals[d.id] = approveRelease(d, i, me, state.epoch);
        });
      if (act === "cancel-ms")
        tryAction(`Milestone #${i + 1} cancelled — buyer refunded`, () => {
          state.deals[d.id] = cancelMilestone(d, i, me, state.epoch);
        });
      if (act === "dispute")
        tryAction(`Milestone #${i + 1} disputed — waiting on the arbiter`, () => {
          state.deals[d.id] = openDispute(d, i, me, state.epoch);
        });
      if (act === "expire")
        tryAction(`Milestone #${i + 1} expired — buyer refunded`, () => {
          state.deals[d.id] = claimExpiredRefund(d, i, me, state.epoch);
        });
      if (act === "resolve") {
        const sellerAmount = Number($("#split-" + i).value);
        tryAction(`Dispute resolved — seller ${fmtTari(sellerAmount)} TARI, buyer ${fmtTari(d.milestones[i].amount - sellerAmount)} TARI`, () => {
          state.deals[d.id] = resolveDispute(d, i, sellerAmount, me, state.epoch);
        });
      }
    }),
  );
}

function renderCreate() {
  $("#view-create").innerHTML = `
    <section class="card">
      <h2 style="margin-top:0">Create a deal</h2>
      <p class="muted">You are creating as <b>${esc(PERSONAS[state.signerId].name)}</b>. On-chain, only the seller can create a deal naming themselves — switch to Alya (seller) to create, or try another role to see the rejection.</p>
      <label>Deal title</label>
      <input id="fTitle" value="Blog post + social graphics" maxlength="120">
      <div class="grid2">
        <div>
          <label>Buyer</label>
          <select id="fBuyer">
            <option value="${PERSONAS.buyer.pubkey}">Budi (client)</option>
            <option value="${PERSONAS.stranger.pubkey}">Dedi (stranger)</option>
          </select>
        </div>
        <div>
          <label>Arbiter (optional)</label>
          <select id="fArbiter">
            <option value="${PERSONAS.arbiter.pubkey}">Citra (neutral arbiter)</option>
            <option value="">No arbiter</option>
          </select>
        </div>
      </div>
      <label>Milestones (1–8, deadlines in epochs, current epoch ${state.epoch})</label>
      <div id="msRows"></div>
      <div style="display:flex;gap:8px;margin-top:10px">
        <button id="btnAddMs">+ Add milestone</button>
        <button class="primary" id="btnCreate">Create deal</button>
      </div>
      <p class="muted">Deadlines must be in the future and in order. Amounts are in TARI (whole or decimal).</p>
    </section>`;
  const rows = $("#msRows");
  const addRow = (title = "", amount = "", deadline = "") => {
    const div = document.createElement("div");
    div.className = "row";
    div.style.marginBottom = "8px";
    div.innerHTML = `<input placeholder="Milestone title" maxlength="120" value="${esc(title)}">
      <input type="number" min="0" step="any" placeholder="TARI" value="${esc(amount)}" class="narrow">
      <input type="number" min="0" step="1" placeholder="Deadline epoch" value="${esc(deadline)}" class="narrow">
      <button class="danger" style="flex:0 0 auto">✕</button>`;
    div.querySelector("button").addEventListener("click", () => {
      if (rows.children.length > 1) div.remove();
    });
    rows.appendChild(div);
  };
  addRow("First draft", "100", String(state.epoch + 12));
  addRow("Final delivery", "150", String(state.epoch + 24));
  $("#btnAddMs").addEventListener("click", () => {
    if (rows.children.length >= 8) return toast("A deal can have at most 8 milestones", "error");
    addRow("", "", String(state.epoch + 12 * (rows.children.length + 1)));
  });
  $("#btnCreate").addEventListener("click", () => {
    const milestones = [...rows.children].map((div) => {
      const [t, a, dl] = div.querySelectorAll("input");
      return { title: t.value.trim(), amount: Math.round(Number(a.value) * 1_000_000), deadlineEpoch: Number(dl.value) };
    });
    const arbiter = $("#fArbiter").value || null;
    tryAction("Deal created — share the link with the buyer", () => {
      const d = createDeal(
        {
          title: $("#fTitle").value.trim(),
          seller: PERSONAS.seller.pubkey,
          buyer: $("#fBuyer").value,
          arbiter,
          milestones,
        },
        signerPubkey(),
        state.epoch,
      );
      state.deals[d.id] = d;
      state.order.unshift(d.id);
      persist();
      location.hash = "#/deal/" + d.id;
    });
  });
}

function renderTour() {
  const d = state.deals[state.order[0]];
  const steps = d
    ? [
        ["Create or open the sample deal", true],
        ["Fund it as Budi (buyer) with exactly the total", d.funded],
        ["Try funding again — rejected: already funded", d.funded],
        ["Mark milestone #1 delivered as Alya (seller)", d.milestones[0]?.status >= STATUS_DELIVERED],
        ["Approve milestone #1 as Budi — Alya is paid", d.milestones[0]?.status === STATUS_RELEASED],
        ["Mark milestone #2 delivered, then dispute it as Budi", d.milestones[1]?.status >= STATUS_DISPUTED],
        ["Resolve the dispute as Citra (arbiter) with a split", d.milestones[1]?.status === STATUS_SPLIT],
        ["Cancel milestone #3 as Budi — refunded", d.milestones[2]?.status === STATUS_REFUNDED],
        ["Deal completes: vault is empty", isComplete(d)],
        ["Advance the epoch past a deadline on a fresh deal, then claim the expiry refund", false],
        ["Try any action as Dedi (stranger) — rejected", false],
        ["Create your own deal from the Create tab", state.order.length > 1],
      ]
    : [];
  $("#view-tour").innerHTML = `
    <section class="card">
      <h2 style="margin-top:0">Guided tour — about 4 minutes</h2>
      <p class="muted">Work through the newest deal (<b>${d ? esc(d.title) : "none yet"}</b>). Switch roles with the cards at the top of the page. Checkmarks update as you complete each step.</p>
      <div style="display:flex;gap:8px;flex-wrap:wrap;margin-bottom:12px">
        <button class="primary" id="btnTourStart">Start fresh tour deal</button>
        ${d ? `<button id="btnTourOpen">Open the tour deal</button>` : ""}
      </div>
      <ol class="checklist">${steps.map(([t, done]) => `<li class="${done ? "done" : ""}">${done ? "✓ " : "○ "}${t}</li>`).join("")}</ol>
      <p class="muted">The tour deliberately includes rejections: wrong role, double funding, and actions in the wrong state. On-chain each rejection is a rolled-back transaction — no funds move.</p>
    </section>`;
  $("#btnTourStart").addEventListener("click", () => {
    const nd = sampleDeal(state.epoch);
    state.deals[nd.id] = nd;
    state.order.unshift(nd.id);
    persist();
    location.hash = "#/deal/" + nd.id;
  });
  const open = $("#btnTourOpen");
  if (open) open.addEventListener("click", () => (location.hash = "#/deal/" + d.id));
}

function renderHow() {
  $("#view-how").innerHTML = `
    <section class="card">
      <h2 style="margin-top:0">How each click maps on-chain</h2>
      <p class="muted">The demo mirrors the Rust template (<code>template/src/lib.rs</code>). When the template is published, each row becomes one Ootle transaction calling that method with those arguments.</p>
      <table>
        <tr><th>Demo action</th><th>Template call</th><th>Who can call it</th></tr>
        <tr><td>Create deal</td><td><code>new(seller, buyer, arbiter, title, resource, titles, amounts, deadlines)</code></td><td>Seller (signer must equal seller)</td></tr>
        <tr><td>Fund deal</td><td><code>fund(payment: Bucket)</code> — exact total, else revert</td><td>Buyer</td></tr>
        <tr><td>Mark delivered</td><td><code>mark_delivered(index)</code></td><td>Seller, before deadline</td></tr>
        <tr><td>Approve &amp; pay</td><td><code>approve_release(index, seller_account)</code></td><td>Buyer, after delivery</td></tr>
        <tr><td>Cancel &amp; refund</td><td><code>cancel_milestone(index, buyer_account)</code></td><td>Buyer, pending only</td></tr>
        <tr><td>Open dispute</td><td><code>open_dispute(index)</code></td><td>Buyer or seller, delivered only</td></tr>
        <tr><td>Resolve dispute</td><td><code>resolve_dispute(index, seller_amount, seller_account, buyer_account)</code></td><td>Arbiter only</td></tr>
        <tr><td>Claim expired refund</td><td><code>claim_expired_refund(index, buyer_account)</code></td><td>Anyone; pays the buyer</td></tr>
        <tr><td>Cancel deal</td><td><code>cancel_deal()</code></td><td>Buyer or seller, before funding</td></tr>
        <tr><td>Views</td><td><code>get_info / get_parties / get_milestone / get_total / is_complete</code></td><td>Anyone (read-only)</td></tr>
      </table>
    </section>
    <section class="card">
      <h3 style="margin-top:0">Honest limits</h3>
      <ul>
        <li>The template cannot judge work quality — it enforces <i>who decides</i>, not <i>what is good</i>. The buyer approves; the arbiter breaks ties. Pick an arbiter both sides trust.</li>
        <li>Deal amounts and milestones are public on-chain (vault accounting). Hiding them would need a confidential/stealth design.</li>
        <li>The template is not published to Esmeralda or mainnet yet and is not audited. The demo is a simulation and says so.</li>
        <li>An arbiter who never rules leaves a disputed milestone locked. Choose an arbiter with a reputation — a future version can add a ruling deadline with a default split.</li>
      </ul>
    </section>`;
}

// ---------- top bar ----------

document.querySelectorAll("nav button").forEach((b) =>
  b.addEventListener("click", () => {
    view = b.dataset.view;
    openDealId = null;
    if (location.hash.startsWith("#/deal/")) history.pushState(null, "", location.pathname);
    render();
  }),
);
$("#btnEpoch1").addEventListener("click", () => {
  state.epoch += 1;
  persist();
  render();
});
$("#btnEpoch10").addEventListener("click", () => {
  state.epoch += 10;
  persist();
  render();
  toast("Epoch advanced to " + state.epoch + " — check for newly expired milestones", "ok");
});

route();
