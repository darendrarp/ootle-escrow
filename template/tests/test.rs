// Ootle Escrow — engine tests.
//
// Every test runs against the real Ootle execution engine in-process
// (tari_template_test_tooling): no network, no wallet, no funds. Each rule in
// docs/MECHANISM.md has a passing-path test and a rejection-path test, and the
// full lifecycle test asserts the vault settles to exactly zero.
//
// Amounts are micro-Tari. A funded test account holds 1,000,000,000 µT
// (1,000 TARI) from the XTR faucet, so deal totals stay well under that.
// The harness starts at epoch 0; deadlines below use epochs 10/20/30 and the
// epoch is moved with `set_epoch` (via the CurrentEpoch virtual substate).

use tari_template_lib::prelude::{
    Amount, ComponentAddress, NonFungibleAddress, ResourceAddress, RistrettoPublicKeyBytes,
    TemplateAddress,
};
use tari_template_test_tooling::{
    TemplateTest,
    byte_type::ToByteType,
    crypto::RistrettoSecretKey,
    engine_types::virtual_substate::{VirtualSubstate, VirtualSubstateId},
    support::assert_error::assert_reject_reason,
    transaction::{args, builder::named_args::NamedArg},
};

const TEMPLATE_NAME: &str = "OotleEscrow";

// Milestone status codes (must match template/src/lib.rs).
const PENDING: u8 = 0;
const DELIVERED: u8 = 1;
const DISPUTED: u8 = 2;
const RELEASED: u8 = 3;
const REFUNDED: u8 = 4;
const SPLIT: u8 = 5;

const M1: u128 = 100_000_000; // 100 TARI
const M2: u128 = 200_000_000; // 200 TARI
const TOTAL: u128 = M1 + M2; // 300 TARI

struct Party {
    account: ComponentAddress,
    proof: NonFungibleAddress,
    secret: RistrettoSecretKey,
    pubkey: RistrettoPublicKeyBytes,
}

struct World {
    test: TemplateTest,
    template: TemplateAddress,
    seller: Party,
    buyer: Party,
    arbiter: Party,
    stranger: Party,
    resource: ResourceAddress,
}

fn make_party(test: &mut TemplateTest) -> Party {
    let (account, proof, secret, pubkey) = test.create_funded_account_with_keypair();
    Party {
        account,
        proof,
        secret,
        pubkey: pubkey.to_byte_type(),
    }
}

fn setup() -> World {
    let mut test = TemplateTest::my_crate();
    let template = test.get_template_address(TEMPLATE_NAME);
    let seller = make_party(&mut test);
    let buyer = make_party(&mut test);
    let arbiter = make_party(&mut test);
    let stranger = make_party(&mut test);
    // The resource the faucet funds accounts with — read from the buyer's
    // account rather than assumed, so the tests work whatever it is.
    let account = test
        .read_only_state_store()
        .get_account(buyer.account)
        .unwrap();
    let (resource, _) = account.vaults().iter().next().unwrap();
    let resource = *resource;
    World {
        test,
        template,
        seller,
        buyer,
        arbiter,
        stranger,
        resource,
    }
}

fn set_epoch(world: &mut World, epoch: u64) {
    world.test.set_virtual_substate(
        VirtualSubstateId::CurrentEpoch,
        VirtualSubstate::CurrentEpoch(epoch),
    );
}

/// The arguments a `new` call needs, in order.
type DealArgs = (
    RistrettoPublicKeyBytes,
    RistrettoPublicKeyBytes,
    Option<RistrettoPublicKeyBytes>,
    String,
    ResourceAddress,
    Vec<String>,
    Vec<Amount>,
    Vec<u64>,
);

fn deal_args(world: &World, arbiter: Option<RistrettoPublicKeyBytes>) -> DealArgs {
    (
        world.seller.pubkey,
        world.buyer.pubkey,
        arbiter,
        "Website build".to_string(),
        world.resource,
        vec!["Design".to_string(), "Build".to_string()],
        vec![Amount::new(M1), Amount::new(M2)],
        vec![10, 20],
    )
}

/// Creates the standard two-milestone deal (100 + 200 TARI, deadlines 10/20)
/// and returns its component address.
fn create_deal(world: &mut World) -> ComponentAddress {
    create_deal_with_arbiter(world, Some(world.arbiter.pubkey))
}

fn create_deal_with_arbiter(
    world: &mut World,
    arbiter: Option<RistrettoPublicKeyBytes>,
) -> ComponentAddress {
    let (seller, buyer, arb, title, resource, titles, amounts, deadlines) =
        deal_args(world, arbiter);
    world.test.execute_expect_success(
        world
            .test
            .transaction()
            .call_function(
                world.template,
                "new",
                args![
                    seller, buyer, arb, title, resource, titles, amounts, deadlines
                ],
            )
            .build_and_seal(&world.seller.secret),
        vec![],
    );
    find_deal(world)
}

fn find_deal(world: &World) -> ComponentAddress {
    world
        .test
        .read_only_state_store()
        .get_components_by_template_address(world.template)
        .unwrap()
        .remove(0)
        .0
}

/// Funds `deal` with `amount` from the buyer's account.
fn fund(world: &mut World, deal: ComponentAddress, amount: u128) {
    let resource = world.resource;
    world.test.execute_expect_success(
        world
            .test
            .transaction()
            .call_method(
                world.buyer.account,
                "withdraw",
                args![resource, Amount::new(amount)],
            )
            .put_last_instruction_output_on_workspace("payment")
            .call_method(deal, "fund", args![Workspace("payment")])
            .build_and_seal(&world.buyer.secret),
        vec![world.buyer.proof.clone()],
    );
}

fn funded_deal(world: &mut World) -> ComponentAddress {
    let deal = create_deal(world);
    fund(world, deal, TOTAL);
    deal
}

fn balance(world: &mut World, who: Who) -> Amount {
    let account = match who {
        Who::Seller => world.seller.account,
        Who::Buyer => world.buyer.account,
        Who::Arbiter => world.arbiter.account,
        Who::Stranger => world.stranger.account,
    };
    let resource = world.resource;
    world.test.execute_expect_success(
        world
            .test
            .transaction()
            .call_method(account, "balance", args![resource])
            .build_and_seal(world.test.secret_key()),
        vec![],
    );
    world.test.expect_return(0)
}

fn get_info(
    world: &mut World,
    deal: ComponentAddress,
) -> (String, Amount, Amount, bool, bool, u32) {
    world.test.execute_expect_success(
        world
            .test
            .transaction()
            .call_method(deal, "get_info", args![])
            .build_and_seal(world.test.secret_key()),
        vec![],
    );
    world.test.expect_return(0)
}

fn get_milestone(
    world: &mut World,
    deal: ComponentAddress,
    index: u32,
) -> (String, Amount, u64, u8) {
    world.test.execute_expect_success(
        world
            .test
            .transaction()
            .call_method(deal, "get_milestone", args![index])
            .build_and_seal(world.test.secret_key()),
        vec![],
    );
    world.test.expect_return(0)
}

fn is_complete(world: &mut World, deal: ComponentAddress) -> bool {
    world.test.execute_expect_success(
        world
            .test
            .transaction()
            .call_method(deal, "is_complete", args![])
            .build_and_seal(world.test.secret_key()),
        vec![],
    );
    world.test.expect_return(0)
}

#[derive(Clone, Copy)]
enum Who {
    Seller,
    Buyer,
    Arbiter,
    Stranger,
}

fn secret_of(world: &World, who: Who) -> RistrettoSecretKey {
    match who {
        Who::Seller => world.seller.secret.clone(),
        Who::Buyer => world.buyer.secret.clone(),
        Who::Arbiter => world.arbiter.secret.clone(),
        Who::Stranger => world.stranger.secret.clone(),
    }
}

fn call_as(
    world: &mut World,
    deal: ComponentAddress,
    method: &str,
    method_args: Vec<NamedArg>,
    who: Who,
) {
    let secret = secret_of(world, who);
    world.test.execute_expect_success(
        world
            .test
            .transaction()
            .call_method(deal, method, method_args)
            .build_and_seal(&secret),
        vec![],
    );
}

#[test]
fn create_deal_happy_path() {
    let mut world = setup();
    let deal = create_deal(&mut world);
    let (title, total, vault, funded, cancelled, count) = get_info(&mut world, deal);
    assert_eq!(title, "Website build");
    assert_eq!(total, Amount::new(TOTAL));
    assert_eq!(vault, Amount::new(0));
    assert!(!funded);
    assert!(!cancelled);
    assert_eq!(count, 2);
    let (m_title, m_amount, m_deadline, m_status) = get_milestone(&mut world, deal, 0);
    assert_eq!(m_title, "Design");
    assert_eq!(m_amount, Amount::new(M1));
    assert_eq!(m_deadline, 10);
    assert_eq!(m_status, PENDING);
    assert!(!is_complete(&mut world, deal));
}

#[test]
fn create_rejects_non_seller_signer() {
    let mut world = setup();
    let (seller, buyer, arb, title, resource, titles, amounts, deadlines) =
        deal_args(&world, Some(world.arbiter.pubkey));
    let reason = world.test.execute_expect_failure(
        world
            .test
            .transaction()
            .call_function(
                world.template,
                "new",
                args![
                    seller, buyer, arb, title, resource, titles, amounts, deadlines
                ],
            )
            .build_and_seal(&world.buyer.secret),
        vec![],
    );
    assert_reject_reason(reason, "Only the seller can create this deal");
}

#[test]
fn create_rejects_bad_parties_and_titles() {
    let mut world = setup();
    // Buyer == seller.
    let reason = world.test.execute_expect_failure(
        world
            .test
            .transaction()
            .call_function(
                world.template,
                "new",
                args![
                    world.seller.pubkey,
                    world.seller.pubkey,
                    Some(world.arbiter.pubkey),
                    "T".to_string(),
                    world.resource,
                    vec!["M".to_string()],
                    vec![Amount::new(1)],
                    vec![10u64],
                ],
            )
            .build_and_seal(&world.seller.secret),
        vec![],
    );
    assert_reject_reason(reason, "Buyer and seller must be different");

    // Arbiter == buyer.
    let reason = world.test.execute_expect_failure(
        world
            .test
            .transaction()
            .call_function(
                world.template,
                "new",
                args![
                    world.seller.pubkey,
                    world.buyer.pubkey,
                    Some(world.buyer.pubkey),
                    "T".to_string(),
                    world.resource,
                    vec!["M".to_string()],
                    vec![Amount::new(1)],
                    vec![10u64],
                ],
            )
            .build_and_seal(&world.seller.secret),
        vec![],
    );
    assert_reject_reason(reason, "Arbiter must be different from the buyer");

    // Empty deal title.
    let reason = world.test.execute_expect_failure(
        world
            .test
            .transaction()
            .call_function(
                world.template,
                "new",
                args![
                    world.seller.pubkey,
                    world.buyer.pubkey,
                    Some(world.arbiter.pubkey),
                    "".to_string(),
                    world.resource,
                    vec!["M".to_string()],
                    vec![Amount::new(1)],
                    vec![10u64],
                ],
            )
            .build_and_seal(&world.seller.secret),
        vec![],
    );
    assert_reject_reason(reason, "Deal title must not be empty");

    // 121-character title.
    let reason = world.test.execute_expect_failure(
        world
            .test
            .transaction()
            .call_function(
                world.template,
                "new",
                args![
                    world.seller.pubkey,
                    world.buyer.pubkey,
                    Some(world.arbiter.pubkey),
                    "x".repeat(121),
                    world.resource,
                    vec!["M".to_string()],
                    vec![Amount::new(1)],
                    vec![10u64],
                ],
            )
            .build_and_seal(&world.seller.secret),
        vec![],
    );
    assert_reject_reason(reason, "Deal title must be at most 120 characters");
}

#[test]
fn create_rejects_bad_milestones() {
    let mut world = setup();
    let seller = world.seller.pubkey;
    let buyer = world.buyer.pubkey;
    let arbiter = Some(world.arbiter.pubkey);
    let resource = world.resource;

    // Zero milestones.
    let reason = world.test.execute_expect_failure(
        world
            .test
            .transaction()
            .call_function(
                world.template,
                "new",
                args![
                    seller,
                    buyer,
                    arbiter,
                    "T".to_string(),
                    resource,
                    Vec::<String>::new(),
                    Vec::<Amount>::new(),
                    Vec::<u64>::new(),
                ],
            )
            .build_and_seal(&world.seller.secret),
        vec![],
    );
    assert_reject_reason(reason, "A deal needs at least one milestone");

    // Nine milestones.
    let reason = world.test.execute_expect_failure(
        world
            .test
            .transaction()
            .call_function(
                world.template,
                "new",
                args![
                    seller,
                    buyer,
                    arbiter,
                    "T".to_string(),
                    resource,
                    vec!["M".to_string(); 9],
                    vec![Amount::new(1); 9],
                    (10u64..19).collect::<Vec<_>>(),
                ],
            )
            .build_and_seal(&world.seller.secret),
        vec![],
    );
    assert_reject_reason(reason, "A deal can have at most 8 milestones");

    // Zero amount.
    let reason = world.test.execute_expect_failure(
        world
            .test
            .transaction()
            .call_function(
                world.template,
                "new",
                args![
                    seller,
                    buyer,
                    arbiter,
                    "T".to_string(),
                    resource,
                    vec!["M".to_string()],
                    vec![Amount::new(0)],
                    vec![10u64],
                ],
            )
            .build_and_seal(&world.seller.secret),
        vec![],
    );
    assert_reject_reason(reason, "Milestone amount must be greater than zero");

    // Deadline in the past (genesis epoch is 0, so 0 is not in the future).
    let reason = world.test.execute_expect_failure(
        world
            .test
            .transaction()
            .call_function(
                world.template,
                "new",
                args![
                    seller,
                    buyer,
                    arbiter,
                    "T".to_string(),
                    resource,
                    vec!["M".to_string()],
                    vec![Amount::new(1)],
                    vec![0u64],
                ],
            )
            .build_and_seal(&world.seller.secret),
        vec![],
    );
    assert_reject_reason(reason, "Milestone deadline must be in the future");

    // Out-of-order deadlines.
    let reason = world.test.execute_expect_failure(
        world
            .test
            .transaction()
            .call_function(
                world.template,
                "new",
                args![
                    seller,
                    buyer,
                    arbiter,
                    "T".to_string(),
                    resource,
                    vec!["A".to_string(), "B".to_string()],
                    vec![Amount::new(1), Amount::new(1)],
                    vec![20u64, 10u64],
                ],
            )
            .build_and_seal(&world.seller.secret),
        vec![],
    );
    assert_reject_reason(reason, "Milestone deadlines must be in order");

    // Mismatched list lengths.
    let reason = world.test.execute_expect_failure(
        world
            .test
            .transaction()
            .call_function(
                world.template,
                "new",
                args![
                    seller,
                    buyer,
                    arbiter,
                    "T".to_string(),
                    resource,
                    vec!["A".to_string(), "B".to_string()],
                    vec![Amount::new(1)],
                    vec![10u64, 20u64],
                ],
            )
            .build_and_seal(&world.seller.secret),
        vec![],
    );
    assert_reject_reason(
        reason,
        "Milestone titles and amounts must have the same length",
    );
}

#[test]
fn fund_happy_path() {
    let mut world = setup();
    let deal = create_deal(&mut world);
    let buyer_before = balance(&mut world, Who::Buyer);
    fund(&mut world, deal, TOTAL);
    let (_, total, vault, funded, _, _) = get_info(&mut world, deal);
    assert_eq!(total, Amount::new(TOTAL));
    assert_eq!(vault, Amount::new(TOTAL));
    assert!(funded);
    assert_eq!(
        balance(&mut world, Who::Buyer),
        buyer_before - Amount::new(TOTAL)
    );
}

#[test]
fn fund_rejects_wrong_amount_and_keeps_balances() {
    let mut world = setup();
    let deal = create_deal(&mut world);
    let buyer_before = balance(&mut world, Who::Buyer);

    for bad_amount in [TOTAL - 1, TOTAL + 1] {
        let resource = world.resource;
        let reason = world.test.execute_expect_failure(
            world
                .test
                .transaction()
                .call_method(
                    world.buyer.account,
                    "withdraw",
                    args![resource, Amount::new(bad_amount)],
                )
                .put_last_instruction_output_on_workspace("payment")
                .call_method(deal, "fund", args![Workspace("payment")])
                .build_and_seal(&world.buyer.secret),
            vec![world.buyer.proof.clone()],
        );
        assert_reject_reason(reason, "Payment must equal the deal total exactly");
    }
    // Nothing moved: the failed transactions rolled back.
    assert_eq!(balance(&mut world, Who::Buyer), buyer_before);
    let (_, _, vault, funded, _, _) = get_info(&mut world, deal);
    assert_eq!(vault, Amount::new(0));
    assert!(!funded);
}

#[test]
fn fund_rejects_non_buyer_and_double_funding() {
    let mut world = setup();
    let deal = create_deal(&mut world);

    // The seller cannot fund their own deal.
    let resource = world.resource;
    let reason = world.test.execute_expect_failure(
        world
            .test
            .transaction()
            .call_method(
                world.seller.account,
                "withdraw",
                args![resource, Amount::new(TOTAL)],
            )
            .put_last_instruction_output_on_workspace("payment")
            .call_method(deal, "fund", args![Workspace("payment")])
            .build_and_seal(&world.seller.secret),
        vec![world.seller.proof.clone()],
    );
    assert_reject_reason(reason, "Only the buyer can fund this deal");

    fund(&mut world, deal, TOTAL);

    // Twice is rejected.
    let reason = world.test.execute_expect_failure(
        world
            .test
            .transaction()
            .call_method(
                world.buyer.account,
                "withdraw",
                args![resource, Amount::new(TOTAL)],
            )
            .put_last_instruction_output_on_workspace("payment")
            .call_method(deal, "fund", args![Workspace("payment")])
            .build_and_seal(&world.buyer.secret),
        vec![world.buyer.proof.clone()],
    );
    assert_reject_reason(reason, "Deal is already funded");
}

#[test]
fn fund_rejects_passed_deadline() {
    let mut world = setup();
    let deal = create_deal(&mut world);
    set_epoch(&mut world, 11); // past milestone #1's deadline of 10
    let resource = world.resource;
    let reason = world.test.execute_expect_failure(
        world
            .test
            .transaction()
            .call_method(
                world.buyer.account,
                "withdraw",
                args![resource, Amount::new(TOTAL)],
            )
            .put_last_instruction_output_on_workspace("payment")
            .call_method(deal, "fund", args![Workspace("payment")])
            .build_and_seal(&world.buyer.secret),
        vec![world.buyer.proof.clone()],
    );
    assert_reject_reason(reason, "A milestone deadline has passed; recreate the deal");
}

#[test]
fn deliver_and_release_happy_path() {
    let mut world = setup();
    let deal = funded_deal(&mut world);
    let seller_before = balance(&mut world, Who::Seller);

    call_as(&mut world, deal, "mark_delivered", args![0u32], Who::Seller);
    assert_eq!(get_milestone(&mut world, deal, 0).3, DELIVERED);

    let seller_account = world.seller.account;
    call_as(
        &mut world,
        deal,
        "approve_release",
        args![0u32, seller_account],
        Who::Buyer,
    );
    assert_eq!(get_milestone(&mut world, deal, 0).3, RELEASED);
    assert_eq!(
        balance(&mut world, Who::Seller),
        seller_before + Amount::new(M1)
    );
    let (_, _, vault, _, _, _) = get_info(&mut world, deal);
    assert_eq!(vault, Amount::new(M2));
}

#[test]
fn deliver_rejects_wrong_state_and_wrong_signer() {
    let mut world = setup();
    // Unfunded.
    let deal = create_deal(&mut world);
    let reason = world.test.execute_expect_failure(
        world
            .test
            .transaction()
            .call_method(deal, "mark_delivered", args![0u32])
            .build_and_seal(&world.seller.secret),
        vec![],
    );
    assert_reject_reason(reason, "Deal is not funded yet");

    let deal = funded_deal(&mut world);
    // Non-seller.
    let reason = world.test.execute_expect_failure(
        world
            .test
            .transaction()
            .call_method(deal, "mark_delivered", args![0u32])
            .build_and_seal(&world.buyer.secret),
        vec![],
    );
    assert_reject_reason(reason, "Only the seller can mark a milestone delivered");

    // Unknown milestone.
    let reason = world.test.execute_expect_failure(
        world
            .test
            .transaction()
            .call_method(deal, "mark_delivered", args![9u32])
            .build_and_seal(&world.seller.secret),
        vec![],
    );
    assert_reject_reason(reason, "Unknown milestone");

    // Deliver twice.
    call_as(&mut world, deal, "mark_delivered", args![0u32], Who::Seller);
    let reason = world.test.execute_expect_failure(
        world
            .test
            .transaction()
            .call_method(deal, "mark_delivered", args![0u32])
            .build_and_seal(&world.seller.secret),
        vec![],
    );
    assert_reject_reason(reason, "Milestone is not pending");
}

#[test]
fn deliver_at_deadline_ok_but_after_is_rejected() {
    let mut world = setup();
    let deal = funded_deal(&mut world);
    // Exactly at the deadline epoch is allowed.
    set_epoch(&mut world, 10);
    call_as(&mut world, deal, "mark_delivered", args![0u32], Who::Seller);
    assert_eq!(get_milestone(&mut world, deal, 0).3, DELIVERED);

    // One epoch past milestone #2's deadline is rejected.
    set_epoch(&mut world, 21);
    let reason = world.test.execute_expect_failure(
        world
            .test
            .transaction()
            .call_method(deal, "mark_delivered", args![1u32])
            .build_and_seal(&world.seller.secret),
        vec![],
    );
    assert_reject_reason(reason, "Milestone deadline has passed");
}

#[test]
fn release_rejects_undelivered_wrong_signer_and_double() {
    let mut world = setup();
    let deal = funded_deal(&mut world);
    let seller_account = world.seller.account;

    // Undelivered.
    let reason = world.test.execute_expect_failure(
        world
            .test
            .transaction()
            .call_method(deal, "approve_release", args![0u32, seller_account])
            .build_and_seal(&world.buyer.secret),
        vec![],
    );
    assert_reject_reason(reason, "Milestone has not been delivered");

    call_as(&mut world, deal, "mark_delivered", args![0u32], Who::Seller);

    // Non-buyer.
    let reason = world.test.execute_expect_failure(
        world
            .test
            .transaction()
            .call_method(deal, "approve_release", args![0u32, seller_account])
            .build_and_seal(&world.seller.secret),
        vec![],
    );
    assert_reject_reason(reason, "Only the buyer can approve a release");

    call_as(
        &mut world,
        deal,
        "approve_release",
        args![0u32, seller_account],
        Who::Buyer,
    );

    // Double release.
    let reason = world.test.execute_expect_failure(
        world
            .test
            .transaction()
            .call_method(deal, "approve_release", args![0u32, seller_account])
            .build_and_seal(&world.buyer.secret),
        vec![],
    );
    assert_reject_reason(reason, "Milestone has not been delivered");
}

#[test]
fn cancel_milestone_refunds_buyer_and_rejects_delivered() {
    let mut world = setup();
    let deal = funded_deal(&mut world);
    let buyer_before = balance(&mut world, Who::Buyer);
    let buyer_account = world.buyer.account;

    call_as(
        &mut world,
        deal,
        "cancel_milestone",
        args![1u32, buyer_account],
        Who::Buyer,
    );
    assert_eq!(get_milestone(&mut world, deal, 1).3, REFUNDED);
    assert_eq!(
        balance(&mut world, Who::Buyer),
        buyer_before + Amount::new(M2)
    );

    // Delivered work cannot be cancelled silently.
    call_as(&mut world, deal, "mark_delivered", args![0u32], Who::Seller);
    let reason = world.test.execute_expect_failure(
        world
            .test
            .transaction()
            .call_method(deal, "cancel_milestone", args![0u32, buyer_account])
            .build_and_seal(&world.buyer.secret),
        vec![],
    );
    assert_reject_reason(reason, "Only a pending milestone can be cancelled");
}

#[test]
fn dispute_and_split_resolution() {
    let mut world = setup();
    let deal = funded_deal(&mut world);
    call_as(&mut world, deal, "mark_delivered", args![0u32], Who::Seller);
    call_as(&mut world, deal, "open_dispute", args![0u32], Who::Buyer);
    assert_eq!(get_milestone(&mut world, deal, 0).3, DISPUTED);

    let seller_before = balance(&mut world, Who::Seller);
    let buyer_before = balance(&mut world, Who::Buyer);
    let seller_account = world.seller.account;
    let buyer_account = world.buyer.account;
    call_as(
        &mut world,
        deal,
        "resolve_dispute",
        args![0u32, Amount::new(60_000_000), seller_account, buyer_account],
        Who::Arbiter,
    );
    assert_eq!(get_milestone(&mut world, deal, 0).3, SPLIT);
    assert_eq!(
        balance(&mut world, Who::Seller),
        seller_before + Amount::new(60_000_000)
    );
    assert_eq!(
        balance(&mut world, Who::Buyer),
        buyer_before + Amount::new(40_000_000)
    );
    let (_, _, vault, _, _, _) = get_info(&mut world, deal);
    assert_eq!(vault, Amount::new(M2));
}

#[test]
fn dispute_rejects_bad_states_and_wrong_parties() {
    let mut world = setup();
    let deal = funded_deal(&mut world);

    // Cannot dispute pending work.
    let reason = world.test.execute_expect_failure(
        world
            .test
            .transaction()
            .call_method(deal, "open_dispute", args![0u32])
            .build_and_seal(&world.buyer.secret),
        vec![],
    );
    assert_reject_reason(reason, "Only a delivered milestone can be disputed");

    call_as(&mut world, deal, "mark_delivered", args![0u32], Who::Seller);

    // A stranger cannot dispute.
    let reason = world.test.execute_expect_failure(
        world
            .test
            .transaction()
            .call_method(deal, "open_dispute", args![0u32])
            .build_and_seal(&world.stranger.secret),
        vec![],
    );
    assert_reject_reason(reason, "Only the buyer or the seller can open a dispute");

    // Cannot resolve what is not disputed.
    let seller_account = world.seller.account;
    let buyer_account = world.buyer.account;
    let reason = world.test.execute_expect_failure(
        world
            .test
            .transaction()
            .call_method(
                deal,
                "resolve_dispute",
                args![0u32, Amount::new(1), seller_account, buyer_account],
            )
            .build_and_seal(&world.arbiter.secret),
        vec![],
    );
    assert_reject_reason(reason, "Milestone is not under dispute");

    call_as(&mut world, deal, "open_dispute", args![0u32], Who::Seller);

    // Only the arbiter resolves.
    let reason = world.test.execute_expect_failure(
        world
            .test
            .transaction()
            .call_method(
                deal,
                "resolve_dispute",
                args![0u32, Amount::new(1), seller_account, buyer_account],
            )
            .build_and_seal(&world.buyer.secret),
        vec![],
    );
    assert_reject_reason(reason, "Only the arbiter can resolve a dispute");

    // Over-amount split.
    let reason = world.test.execute_expect_failure(
        world
            .test
            .transaction()
            .call_method(
                deal,
                "resolve_dispute",
                args![0u32, Amount::new(M1 + 1), seller_account, buyer_account],
            )
            .build_and_seal(&world.arbiter.secret),
        vec![],
    );
    assert_reject_reason(reason, "Seller share exceeds the milestone amount");
}

#[test]
fn dispute_without_arbiter_is_rejected() {
    let mut world = setup();
    let deal = create_deal_with_arbiter(&mut world, None);
    fund(&mut world, deal, TOTAL);
    call_as(&mut world, deal, "mark_delivered", args![0u32], Who::Seller);
    let reason = world.test.execute_expect_failure(
        world
            .test
            .transaction()
            .call_method(deal, "open_dispute", args![0u32])
            .build_and_seal(&world.buyer.secret),
        vec![],
    );
    assert_reject_reason(reason, "This deal has no arbiter");
}

#[test]
fn resolve_full_and_zero_amounts() {
    // Full seller share reads as Released.
    let mut world = setup();
    let deal = funded_deal(&mut world);
    call_as(&mut world, deal, "mark_delivered", args![0u32], Who::Seller);
    call_as(&mut world, deal, "open_dispute", args![0u32], Who::Buyer);
    let seller_account = world.seller.account;
    let buyer_account = world.buyer.account;
    call_as(
        &mut world,
        deal,
        "resolve_dispute",
        args![0u32, Amount::new(M1), seller_account, buyer_account],
        Who::Arbiter,
    );
    assert_eq!(get_milestone(&mut world, deal, 0).3, RELEASED);

    // Zero seller share reads as Refunded.
    let mut world = setup();
    let deal = funded_deal(&mut world);
    call_as(&mut world, deal, "mark_delivered", args![0u32], Who::Seller);
    call_as(&mut world, deal, "open_dispute", args![0u32], Who::Buyer);
    let seller_account = world.seller.account;
    let buyer_account = world.buyer.account;
    call_as(
        &mut world,
        deal,
        "resolve_dispute",
        args![0u32, Amount::new(0), seller_account, buyer_account],
        Who::Arbiter,
    );
    assert_eq!(get_milestone(&mut world, deal, 0).3, REFUNDED);
}

#[test]
fn expiry_refund_after_deadline_only() {
    let mut world = setup();
    let deal = funded_deal(&mut world);
    let buyer_account = world.buyer.account;
    let buyer_before = balance(&mut world, Who::Buyer);

    // Too early — at the deadline epoch the milestone has not expired yet.
    set_epoch(&mut world, 10);
    let reason = world.test.execute_expect_failure(
        world
            .test
            .transaction()
            .call_method(deal, "claim_expired_refund", args![0u32, buyer_account])
            .build_and_seal(&world.stranger.secret),
        vec![],
    );
    assert_reject_reason(reason, "Milestone has not expired yet");

    // Anyone can trigger it once expired; the money goes to the buyer.
    set_epoch(&mut world, 11);
    call_as(
        &mut world,
        deal,
        "claim_expired_refund",
        args![0u32, buyer_account],
        Who::Stranger,
    );
    assert_eq!(get_milestone(&mut world, deal, 0).3, REFUNDED);
    assert_eq!(
        balance(&mut world, Who::Buyer),
        buyer_before + Amount::new(M1)
    );
}

#[test]
fn delivered_work_never_auto_refunds() {
    let mut world = setup();
    let deal = funded_deal(&mut world);
    call_as(&mut world, deal, "mark_delivered", args![0u32], Who::Seller);
    set_epoch(&mut world, 999);
    let buyer_account = world.buyer.account;
    let reason = world.test.execute_expect_failure(
        world
            .test
            .transaction()
            .call_method(deal, "claim_expired_refund", args![0u32, buyer_account])
            .build_and_seal(&world.buyer.secret),
        vec![],
    );
    assert_reject_reason(
        reason,
        "Only a pending milestone can be refunded as expired",
    );
}

#[test]
fn cancel_deal_before_funding_only() {
    let mut world = setup();
    let deal = create_deal(&mut world);

    // A stranger cannot cancel.
    let reason = world.test.execute_expect_failure(
        world
            .test
            .transaction()
            .call_method(deal, "cancel_deal", args![])
            .build_and_seal(&world.stranger.secret),
        vec![],
    );
    assert_reject_reason(reason, "Only the buyer or the seller can cancel the deal");

    call_as(&mut world, deal, "cancel_deal", args![], Who::Seller);
    let (_, _, _, _, cancelled, _) = get_info(&mut world, deal);
    assert!(cancelled);

    // Funding a cancelled deal fails.
    let resource = world.resource;
    let reason = world.test.execute_expect_failure(
        world
            .test
            .transaction()
            .call_method(
                world.buyer.account,
                "withdraw",
                args![resource, Amount::new(TOTAL)],
            )
            .put_last_instruction_output_on_workspace("payment")
            .call_method(deal, "fund", args![Workspace("payment")])
            .build_and_seal(&world.buyer.secret),
        vec![world.buyer.proof.clone()],
    );
    assert_reject_reason(reason, "Deal was cancelled");

    // A funded deal cannot be cancelled.
    let deal = funded_deal(&mut world);
    let reason = world.test.execute_expect_failure(
        world
            .test
            .transaction()
            .call_method(deal, "cancel_deal", args![])
            .build_and_seal(&world.buyer.secret),
        vec![],
    );
    assert_reject_reason(reason, "Cannot cancel a funded deal");
}

#[test]
fn full_lifecycle_settles_vault_to_zero() {
    let mut world = setup();
    // Three milestones: 100 + 100 + 100 TARI, deadlines 10/20/30.
    let (seller, buyer, arb, title, resource, _, _, _) =
        deal_args(&world, Some(world.arbiter.pubkey));
    world.test.execute_expect_success(
        world
            .test
            .transaction()
            .call_function(
                world.template,
                "new",
                args![
                    seller,
                    buyer,
                    arb,
                    title,
                    resource,
                    vec!["A".to_string(), "B".to_string(), "C".to_string()],
                    vec![Amount::new(100_000_000); 3],
                    vec![10u64, 20u64, 30u64],
                ],
            )
            .build_and_seal(&world.seller.secret),
        vec![],
    );
    let deal = find_deal(&world);
    fund(&mut world, deal, 300_000_000);

    let seller_before = balance(&mut world, Who::Seller);
    let buyer_before = balance(&mut world, Who::Buyer);
    let seller_account = world.seller.account;
    let buyer_account = world.buyer.account;

    // Milestone A: deliver + approve (seller +100).
    call_as(&mut world, deal, "mark_delivered", args![0u32], Who::Seller);
    call_as(
        &mut world,
        deal,
        "approve_release",
        args![0u32, seller_account],
        Who::Buyer,
    );
    // Milestone B: cancel (buyer +100).
    call_as(
        &mut world,
        deal,
        "cancel_milestone",
        args![1u32, buyer_account],
        Who::Buyer,
    );
    // Milestone C: deliver + dispute + 30/70 split.
    call_as(&mut world, deal, "mark_delivered", args![2u32], Who::Seller);
    call_as(&mut world, deal, "open_dispute", args![2u32], Who::Buyer);
    call_as(
        &mut world,
        deal,
        "resolve_dispute",
        args![2u32, Amount::new(30_000_000), seller_account, buyer_account],
        Who::Arbiter,
    );

    assert_eq!(get_milestone(&mut world, deal, 0).3, RELEASED);
    assert_eq!(get_milestone(&mut world, deal, 1).3, REFUNDED);
    assert_eq!(get_milestone(&mut world, deal, 2).3, SPLIT);
    let (_, _, vault, _, _, _) = get_info(&mut world, deal);
    assert_eq!(vault, Amount::new(0));
    assert!(is_complete(&mut world, deal));
    assert_eq!(
        balance(&mut world, Who::Seller),
        seller_before + Amount::new(130_000_000)
    );
    assert_eq!(
        balance(&mut world, Who::Buyer),
        buyer_before + Amount::new(170_000_000)
    );
}

#[test]
fn events_are_emitted() {
    let mut world = setup();
    let deal = create_deal(&mut world);
    let result = world.test.execute_expect_success(
        world
            .test
            .transaction()
            .call_method(
                world.buyer.account,
                "withdraw",
                args![world.resource, Amount::new(TOTAL)],
            )
            .put_last_instruction_output_on_workspace("payment")
            .call_method(deal, "fund", args![Workspace("payment")])
            .build_and_seal(&world.buyer.secret),
        vec![world.buyer.proof.clone()],
    );
    let topics: Vec<String> = result
        .finalize
        .events
        .iter()
        .map(|e| e.topic().to_string())
        .collect();
    assert!(
        topics.iter().any(|t| t == "OotleEscrow.Funded"),
        "Funded event missing, got: {topics:?}"
    );
}
