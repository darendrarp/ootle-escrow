// Ootle Escrow — milestone escrow for freelance work on Tari Ootle.
//
// A client hiring a freelancer in XTM faces a trust gap: pay upfront and risk
// no delivery, or deliver first and risk no payment. Ootle Escrow closes it:
// the buyer funds the full deal value into this component's vault, and funds
// are released milestone by milestone as the buyer approves delivered work.
// A neutral arbiter named at creation resolves disputes by splitting a
// milestone's funds, and milestones nobody delivers by their deadline can be
// refunded to the buyer by anyone once expired.
//
// Design notes:
// - Authorization is enforced in code against the transaction signer, the same
//   pattern used by other Ootle templates: every method is `allow_all` at the
//   access-rule layer and each method asserts the caller's role itself. The
//   component is ownerless (`OwnerRule::None`), so nobody can rewrite the
//   access rules after creation.
// - This template creates no resource, so the "owner bypasses access rules"
//   trap cannot apply: there is nothing to mint, freeze, or recall.
// - Amounts are exact. `fund` requires the payment to equal the deal total to
//   the micro-Tari; anything else panics and the whole transaction rolls back,
//   so the buyer is never part-charged.
// - Delivered work is never auto-refunded. If the buyer goes silent after a
//   delivery, the seller escalates to the arbiter via `open_dispute`.
// - All deadlines are Ootle epochs. On mainnet an epoch is about 20 minutes;
//   on Esmeralda about one hour.

use tari_template_lib::prelude::*;

#[template]
mod ootle_escrow {
    use super::*;

    /// Maximum number of milestones in one deal. Keeps component state small
    /// and every deal readable at a glance.
    const MAX_MILESTONES: usize = 8;
    /// Maximum length of the deal title and of each milestone title.
    const MAX_TITLE_LEN: usize = 120;

    /// Milestone lifecycle states, stored as a `u8` code.
    /// Work not yet delivered. Can be cancelled by the buyer, or refunded by
    /// anyone once the deadline passes.
    pub const STATUS_PENDING: u8 = 0;
    /// Seller marked the work delivered. Buyer approves, or either side opens
    /// a dispute.
    pub const STATUS_DELIVERED: u8 = 1;
    /// Under arbitration. Only the arbiter can move it, via `resolve_dispute`.
    pub const STATUS_DISPUTED: u8 = 2;
    /// Paid in full to the seller. Terminal.
    pub const STATUS_RELEASED: u8 = 3;
    /// Returned in full to the buyer. Terminal.
    pub const STATUS_REFUNDED: u8 = 4;
    /// Split between seller and buyer by the arbiter. Terminal.
    pub const STATUS_SPLIT: u8 = 5;

    /// A milestone escrow deal between one buyer, one seller, and an optional arbiter.
    ///
    /// NOTE: this must be the first `pub struct` in the module — the template
    /// macro treats the first struct as the component.
    pub struct OotleEscrow {
        seller: RistrettoPublicKeyBytes,
        buyer: RistrettoPublicKeyBytes,
        arbiter: Option<RistrettoPublicKeyBytes>,
        title: String,
        resource: ResourceAddress,
        vault: Vault,
        milestones: Vec<Milestone>,
        funded: bool,
        cancelled: bool,
        created_epoch: u64,
    }

    /// One milestone of a deal: a chunk of work with its own price and deadline.
    pub struct Milestone {
        title: String,
        amount: Amount,
        deadline_epoch: u64,
        status: u8,
    }

    impl OotleEscrow {
        /// Creates a deal. Callable by the seller only: the transaction signer
        /// must be the `seller` key, so nobody can open a deal impersonating
        /// someone else.
        ///
        /// Milestones are passed as three parallel lists (titles, amounts,
        /// deadlines) which must all have the same length. Deadlines must be
        /// in the future and in non-decreasing order.
        #[allow(clippy::too_many_arguments)]
        pub fn new(
            seller: RistrettoPublicKeyBytes,
            buyer: RistrettoPublicKeyBytes,
            arbiter: Option<RistrettoPublicKeyBytes>,
            title: String,
            resource: ResourceAddress,
            titles: Vec<String>,
            amounts: Vec<Amount>,
            deadlines: Vec<u64>,
        ) -> Component<Self> {
            let signer = CallerContext::transaction_signer_public_key();
            assert_eq!(signer, seller, "Only the seller can create this deal");
            assert_ne!(buyer, seller, "Buyer and seller must be different");
            if let Some(a) = arbiter.as_ref() {
                assert_ne!(*a, seller, "Arbiter must be different from the seller");
                assert_ne!(*a, buyer, "Arbiter must be different from the buyer");
            }
            assert!(!title.is_empty(), "Deal title must not be empty");
            assert!(
                title.chars().count() <= MAX_TITLE_LEN,
                "Deal title must be at most 120 characters"
            );
            assert!(!titles.is_empty(), "A deal needs at least one milestone");
            assert!(
                titles.len() <= MAX_MILESTONES,
                "A deal can have at most 8 milestones"
            );
            assert_eq!(
                titles.len(),
                amounts.len(),
                "Milestone titles and amounts must have the same length"
            );
            assert_eq!(
                titles.len(),
                deadlines.len(),
                "Milestone titles and deadlines must have the same length"
            );

            let now = Consensus::current_epoch();
            let mut milestones = Vec::with_capacity(titles.len());
            let mut prev_deadline = 0u64;
            for i in 0..titles.len() {
                let m_title = &titles[i];
                assert!(!m_title.is_empty(), "Milestone title must not be empty");
                assert!(
                    m_title.chars().count() <= MAX_TITLE_LEN,
                    "Milestone title must be at most 120 characters"
                );
                assert!(
                    amounts[i].is_positive(),
                    "Milestone amount must be greater than zero"
                );
                assert!(
                    deadlines[i] > now,
                    "Milestone deadline must be in the future"
                );
                assert!(
                    deadlines[i] >= prev_deadline,
                    "Milestone deadlines must be in order"
                );
                prev_deadline = deadlines[i];
                milestones.push(Milestone {
                    title: m_title.clone(),
                    amount: amounts[i],
                    deadline_epoch: deadlines[i],
                    status: STATUS_PENDING,
                });
            }

            let total = milestones
                .iter()
                .fold(Amount::new(0), |sum, m| sum + m.amount);

            // Every method is public at the access-rule layer; each method
            // enforces its own role by checking the transaction signer.
            let access_rules = ComponentAccessRules::new()
                .method("fund", rule![allow_all])
                .method("mark_delivered", rule![allow_all])
                .method("approve_release", rule![allow_all])
                .method("cancel_milestone", rule![allow_all])
                .method("open_dispute", rule![allow_all])
                .method("resolve_dispute", rule![allow_all])
                .method("claim_expired_refund", rule![allow_all])
                .method("cancel_deal", rule![allow_all])
                .method("get_info", rule![allow_all])
                .method("get_parties", rule![allow_all])
                .method("get_resource", rule![allow_all])
                .method("get_created_epoch", rule![allow_all])
                .method("get_milestone", rule![allow_all])
                .method("get_total", rule![allow_all])
                .method("is_complete", rule![allow_all]);

            emit_event(
                "DealCreated",
                metadata![
                    "title" => title.clone(),
                    "seller" => seller.to_string(),
                    "buyer" => buyer.to_string(),
                    "total" => total.to_string(),
                    "milestones" => milestones.len().to_string(),
                ],
            );

            Component::new(Self {
                seller,
                buyer,
                arbiter,
                title,
                resource,
                vault: Vault::new_empty(resource),
                milestones,
                funded: false,
                cancelled: false,
                created_epoch: now,
            })
            .with_access_rules(access_rules)
            // Ownerless: nobody — not even the seller who created the deal —
            // can change the access rules or claim owner powers over it. The
            // rules above are the whole deal, and they cannot be rewritten.
            .with_owner_rule(OwnerRule::None)
            .create()
        }

        /// Funds the deal in full. Buyer only, exactly once, with a payment
        /// that equals the deal total exactly. Any other amount panics and the
        /// transaction rolls back, so the buyer is never part-charged.
        pub fn fund(&mut self, payment: Bucket) {
            let signer = CallerContext::transaction_signer_public_key();
            assert_eq!(signer, self.buyer, "Only the buyer can fund this deal");
            assert!(!self.cancelled, "Deal was cancelled");
            assert!(!self.funded, "Deal is already funded");
            assert_eq!(
                payment.resource_address(),
                self.resource,
                "Payment must be in the deal resource"
            );
            assert_eq!(
                payment.amount(),
                self.total(),
                "Payment must equal the deal total exactly"
            );
            // Refuse to lock funds into a deal whose work is already late.
            let now = Consensus::current_epoch();
            assert!(
                self.milestones.iter().all(|m| m.deadline_epoch > now),
                "A milestone deadline has passed; recreate the deal"
            );
            let total = payment.amount();
            self.vault.deposit(payment);
            self.funded = true;
            emit_event(
                "Funded",
                metadata!["total" => total.to_string(), "buyer" => self.buyer.to_string()],
            );
        }

        /// Marks a milestone's work as delivered. Seller only, while the
        /// milestone is pending and its deadline has not passed.
        pub fn mark_delivered(&mut self, index: u32) {
            let signer = CallerContext::transaction_signer_public_key();
            assert_eq!(
                signer, self.seller,
                "Only the seller can mark a milestone delivered"
            );
            assert!(self.funded, "Deal is not funded yet");
            let milestone = self.milestone_mut(index);
            assert_eq!(milestone.status, STATUS_PENDING, "Milestone is not pending");
            assert!(
                Consensus::current_epoch() <= milestone.deadline_epoch,
                "Milestone deadline has passed"
            );
            milestone.status = STATUS_DELIVERED;
            emit_event("Delivered", metadata!["index" => index.to_string()]);
        }

        /// Approves delivered work and pays the milestone amount to the
        /// seller's account. Buyer only.
        pub fn approve_release(&mut self, index: u32, seller_account: ComponentAddress) {
            let signer = CallerContext::transaction_signer_public_key();
            assert_eq!(signer, self.buyer, "Only the buyer can approve a release");
            let amount = {
                let milestone = self.milestone_mut(index);
                assert_eq!(
                    milestone.status, STATUS_DELIVERED,
                    "Milestone has not been delivered"
                );
                milestone.status = STATUS_RELEASED;
                milestone.amount
            };
            self.pay(seller_account, amount);
            emit_event(
                "Released",
                metadata!["index" => index.to_string(), "amount" => amount.to_string()],
            );
            self.emit_completed_if_done();
        }

        /// Cancels a milestone that has not been delivered yet and refunds its
        /// amount to the buyer's account. Buyer only. Delivered work cannot be
        /// cancelled silently: approve it, or dispute it.
        pub fn cancel_milestone(&mut self, index: u32, buyer_account: ComponentAddress) {
            let signer = CallerContext::transaction_signer_public_key();
            assert_eq!(signer, self.buyer, "Only the buyer can cancel a milestone");
            assert!(self.funded, "Deal is not funded yet");
            let amount = {
                let milestone = self.milestone_mut(index);
                assert_eq!(
                    milestone.status, STATUS_PENDING,
                    "Only a pending milestone can be cancelled"
                );
                milestone.status = STATUS_REFUNDED;
                milestone.amount
            };
            self.pay(buyer_account, amount);
            emit_event(
                "Refunded",
                metadata![
                    "index" => index.to_string(),
                    "amount" => amount.to_string(),
                    "reason" => "cancelled",
                ],
            );
            self.emit_completed_if_done();
        }

        /// Escalates a delivered milestone to the arbiter. Buyer or seller.
        /// Requires the deal to have an arbiter.
        pub fn open_dispute(&mut self, index: u32) {
            let signer = CallerContext::transaction_signer_public_key();
            assert!(
                signer == self.buyer || signer == self.seller,
                "Only the buyer or the seller can open a dispute"
            );
            assert!(self.arbiter.is_some(), "This deal has no arbiter");
            let milestone = self.milestone_mut(index);
            assert_eq!(
                milestone.status, STATUS_DELIVERED,
                "Only a delivered milestone can be disputed"
            );
            milestone.status = STATUS_DISPUTED;
            emit_event(
                "Disputed",
                metadata!["index" => index.to_string(), "by" => signer.to_string()],
            );
        }

        /// Resolves a disputed milestone. Arbiter only. Splits the milestone
        /// amount: `seller_amount` to the seller, the rest to the buyer.
        pub fn resolve_dispute(
            &mut self,
            index: u32,
            seller_amount: Amount,
            seller_account: ComponentAddress,
            buyer_account: ComponentAddress,
        ) {
            let signer = CallerContext::transaction_signer_public_key();
            let arbiter = self.arbiter.expect("This deal has no arbiter");
            assert_eq!(signer, arbiter, "Only the arbiter can resolve a dispute");
            let (amount, status) = {
                let milestone = self.milestone_mut(index);
                assert_eq!(
                    milestone.status, STATUS_DISPUTED,
                    "Milestone is not under dispute"
                );
                assert!(
                    seller_amount <= milestone.amount,
                    "Seller share exceeds the milestone amount"
                );
                let status = if seller_amount == milestone.amount {
                    STATUS_RELEASED
                } else if seller_amount.is_zero() {
                    STATUS_REFUNDED
                } else {
                    STATUS_SPLIT
                };
                milestone.status = status;
                (milestone.amount, status)
            };
            let buyer_amount = amount - seller_amount;
            self.pay(seller_account, seller_amount);
            self.pay(buyer_account, buyer_amount);
            emit_event(
                "Resolved",
                metadata![
                    "index" => index.to_string(),
                    "seller_amount" => seller_amount.to_string(),
                    "buyer_amount" => buyer_amount.to_string(),
                    "status" => status.to_string(),
                ],
            );
            self.emit_completed_if_done();
        }

        /// Refunds a milestone nobody delivered by its deadline. Anyone can
        /// call it; the funds always go to the buyer's account. Only pending
        /// milestones expire — delivered work is never auto-refunded.
        pub fn claim_expired_refund(&mut self, index: u32, buyer_account: ComponentAddress) {
            assert!(self.funded, "Deal is not funded yet");
            let amount = {
                let milestone = self.milestone_mut(index);
                assert_eq!(
                    milestone.status, STATUS_PENDING,
                    "Only a pending milestone can be refunded as expired"
                );
                assert!(
                    Consensus::current_epoch() > milestone.deadline_epoch,
                    "Milestone has not expired yet"
                );
                milestone.status = STATUS_REFUNDED;
                milestone.amount
            };
            self.pay(buyer_account, amount);
            emit_event(
                "Refunded",
                metadata![
                    "index" => index.to_string(),
                    "amount" => amount.to_string(),
                    "reason" => "expired",
                ],
            );
            self.emit_completed_if_done();
        }

        /// Cancels the whole deal before it is funded. Buyer or seller. Once
        /// funded, a deal can only end milestone by milestone.
        pub fn cancel_deal(&mut self) {
            let signer = CallerContext::transaction_signer_public_key();
            assert!(
                signer == self.buyer || signer == self.seller,
                "Only the buyer or the seller can cancel the deal"
            );
            assert!(!self.funded, "Cannot cancel a funded deal");
            assert!(!self.cancelled, "Deal is already cancelled");
            self.cancelled = true;
            emit_event("Cancelled", metadata!["title" => self.title.clone()]);
        }

        /// (title, total, vault balance, funded, cancelled, milestone count).
        pub fn get_info(&self) -> (String, Amount, Amount, bool, bool, u32) {
            (
                self.title.clone(),
                self.total(),
                self.vault.balance(),
                self.funded,
                self.cancelled,
                self.milestones.len() as u32,
            )
        }

        /// (seller, buyer, arbiter or None).
        pub fn get_parties(
            &self,
        ) -> (
            RistrettoPublicKeyBytes,
            RistrettoPublicKeyBytes,
            Option<RistrettoPublicKeyBytes>,
        ) {
            (self.seller, self.buyer, self.arbiter)
        }

        pub fn get_resource(&self) -> ResourceAddress {
            self.resource
        }

        pub fn get_created_epoch(&self) -> u64 {
            self.created_epoch
        }

        /// (title, amount, deadline epoch, status code) for one milestone.
        /// Status codes: 0 pending, 1 delivered, 2 disputed, 3 released,
        /// 4 refunded, 5 split.
        pub fn get_milestone(&self, index: u32) -> (String, Amount, u64, u8) {
            let m = self.milestone(index);
            (m.title.clone(), m.amount, m.deadline_epoch, m.status)
        }

        pub fn get_total(&self) -> Amount {
            self.total()
        }

        /// True once every milestone is released, refunded, or split — and the
        /// deal was funded. An unfunded deal is never complete.
        pub fn is_complete(&self) -> bool {
            self.funded
                && self
                    .milestones
                    .iter()
                    .all(|m| matches!(m.status, STATUS_RELEASED | STATUS_REFUNDED | STATUS_SPLIT))
        }

        fn total(&self) -> Amount {
            self.milestones
                .iter()
                .fold(Amount::new(0), |sum, m| sum + m.amount)
        }

        fn milestone(&self, index: u32) -> &Milestone {
            self.milestones
                .get(index as usize)
                .expect("Unknown milestone")
        }

        fn milestone_mut(&mut self, index: u32) -> &mut Milestone {
            self.milestones
                .get_mut(index as usize)
                .expect("Unknown milestone")
        }

        /// Withdraws `amount` from the vault and deposits it into `account`.
        /// Zero-amount payouts are skipped: withdrawing zero is pointless and
        /// some account implementations reject empty buckets.
        fn pay(&mut self, account: ComponentAddress, amount: Amount) {
            if amount.is_zero() {
                return;
            }
            let bucket = self.vault.withdraw(amount);
            ComponentManager::get(account).invoke("deposit", args![bucket]);
        }

        fn emit_completed_if_done(&self) {
            if self.is_complete() {
                emit_event(
                    "Completed",
                    metadata![
                        "title" => self.title.clone(),
                        "total" => self.total().to_string(),
                    ],
                );
            }
        }
    }
}
