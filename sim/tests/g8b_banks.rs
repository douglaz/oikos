//! G8b acceptance suite — banks & credit on the M3 ledger.
//!
//! G8a put the settlement on M3 ledger **specie** (no banks, no fiat). G8b adds the
//! **bank**: a chartered institution that takes **deposits** and lends **fiduciary
//! credit** (demand claims beyond its reserves), gated by its reserve ratio. The
//! reuse is total — deposits and fiduciary lending route through econ's existing M3
//! ledger / bank balance-sheet paths *unchanged*; G8b only wires the sim's
//! deposit/lend actions into them (the bank is chartered in the settlement, not in a
//! new econ scenario). So the six econ goldens stay byte-identical and the spot
//! market is the same as G8a's — the bank is purely additive.
//!
//! These pin the contract and its tripwires:
//! - the banked run is deterministic, with real deposits AND lending (test 1);
//! - a deposit moves specie → the bank's reserves and gives the depositor demand
//!   claims that circulate as money; specie + reserves are conserved (test 2);
//! - a fractional-reserve bank issues fiduciary credit (claims beyond reserves) to
//!   borrowers who spend it — `fiduciary_issued > 0` (test 3);
//! - the 100%-reserve **control** lends ZERO fiduciary while its deposits still
//!   circulate as claims — paired with test 3 this isolates credit creation to the
//!   fractional reserve (test 4);
//! - whole-system conservation holds with nonzero claims/reserves/fiduciary:
//!   `fiduciary <= demand_claims`, reserves back claims, specie conserved, goods
//!   conserved — every econ tick (test 5);
//! - issuing fiduciary does NOT change the specie total; it expands claims — the TMS
//!   distinction (test 6);
//! - the engine is otherwise unperturbed: the no-bank path is byte-identical and a
//!   banked run keeps the econ invariants (test 7; the six econ goldens staying
//!   byte-identical + the workspace `cargo clippy -D warnings` + `cargo fmt --check`
//!   are the real gate).
//!
//! Scope is the lending **mechanism** + the reserve control. NO fiat / regime ladder
//! / tender / taxation, and NO full ABCT boom/bust demonstration (the cycle needs the
//! regime ladder to enable-then-stop credit) — all G8c.

use econ::good::{Gold, FOOD};
use econ::money::ReserveRatioBps;
use sim::{Settlement, SettlementConfig, Vocation};

/// The id of a living colonist of `vocation` at the lowest generation slot — the
/// depositor (a consumer) or borrower (a gatherer) the per-agent assertions inspect.
fn first_living(s: &Settlement, vocation: Vocation) -> econ::agent::AgentId {
    for i in 0..s.population() {
        if s.is_alive(i) && s.vocation_of(i) == Some(vocation) {
            return s.colonist_id(i).expect("a living colonist has an id");
        }
    }
    panic!("no living colonist of vocation {vocation:?}");
}

/// 1. Same `(seed, config)` → a byte-identical banked run. Integer state, the econ
///    `Rng` consumed only at generation, nothing drawn in the bank phase — deposits
///    and fiduciary lending are deterministic functions of the run.
#[test]
fn bank_run_is_deterministic() {
    let config = SettlementConfig::bank();
    let mut a = Settlement::generate(0xC0FFEE, &config);
    let mut b = Settlement::generate(0xC0FFEE, &config);
    a.run(60);
    b.run(60);
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "a banked settlement run must be byte-identical for the same seed + config"
    );
    assert!(
        a.is_m3() && a.is_banked(),
        "the bank config is an M3 banked settlement"
    );
    // The run actually exercised deposits AND lending — determinism over a live bank,
    // not over an inert one.
    let composition = a
        .money_composition()
        .expect("a banked settlement has a composition");
    assert!(
        composition.bank_reserves > Gold::ZERO,
        "deposits must have built the bank's reserves"
    );
    assert!(
        composition.fiduciary > Gold::ZERO,
        "the fractional bank must have lent fiduciary credit"
    );
}

/// 2. A deposit moves M3 specie into the bank's reserves and gives the depositor an
///    equal demand claim; those claims circulate as money; specie + reserves are
///    conserved. The depositor's spendable total is unchanged by the deposit (specie
///    became a claim), so the colony keeps trading and stays fed on a claim-dominated
///    money supply — the operational proof that claims spend as money.
#[test]
fn deposits_become_claims_backed_by_reserves() {
    let mut s = Settlement::generate(7, &SettlementConfig::bank());
    let total_money = s.total_gold();
    s.run(30);

    let composition = s
        .money_composition()
        .expect("a banked settlement has a composition");
    // Specie moved into the bank's reserves, and the depositors hold demand claims.
    assert!(
        composition.bank_reserves > Gold::ZERO,
        "a deposit must move specie into the bank's reserves"
    );
    assert!(
        composition.demand_claims > Gold::ZERO,
        "a deposit must give the depositor demand claims"
    );
    // The reserve-backed portion of the claims never exceeds the demand claims — the
    // reserves back claims (the M3 invariant). The bank's balance sheet mirrors the
    // ledger reserves exactly.
    assert!(composition.bank_reserves <= composition.demand_claims);
    let bank = s.bank().expect("a banked settlement charters a bank");
    assert_eq!(
        bank.reserves, composition.bank_reserves,
        "the bank's reserves equal the M3 ledger's bank reserves"
    );

    // A depositor (a consumer) holds a demand claim it received for its specie.
    let depositor = first_living(&s, Vocation::Consumer);
    assert!(
        s.demand_claim_of(depositor) > Gold::ZERO,
        "a depositing consumer holds the demand claims its deposit created"
    );

    // Specie + reserves are conserved: the specie base never moves (a deposit only
    // relocates specie from the public to the bank's reserves), and every unit of the
    // specie base sits either in public hands or in the reserves.
    assert_eq!(
        s.total_gold(),
        total_money,
        "a deposit must not create or destroy specie"
    );
    assert_eq!(
        composition.public_specie.0 + composition.bank_reserves.0,
        s.total_gold().0,
        "specie lives only in public hands or in bank reserves"
    );

    // Claims spend as money: the market clears and the colony stays fed even though
    // most of the money is now demand claims, not specie — so trades must be settling
    // in claims (a claim-less market could not feed the colony from this little
    // specie).
    assert!(
        !s.society().trades.is_empty(),
        "the claim-based spot market must clear trades"
    );
    assert!(
        composition.demand_claims > composition.public_specie,
        "claims dominate the circulating money, so the cleared trades move claims"
    );
    assert_eq!(
        s.living_total(),
        s.population(),
        "the colony stays fed on a claim-based money supply (claims circulate as money)"
    );
}

/// 3. A fractional-reserve bank issues fiduciary credit — demand claims **beyond** its
///    reserves — to borrowers, who spend it into the economy. `fiduciary_issued > 0`,
///    and the ledger tracks the same fiduciary (claims − reserves).
#[test]
fn fractional_bank_lends_fiduciary() {
    let mut s = Settlement::generate(7, &SettlementConfig::bank());
    s.run(30);

    let composition = s
        .money_composition()
        .expect("a banked settlement has a composition");
    let bank = s.bank().expect("a banked settlement charters a bank");

    // The bank lent fiduciary: its balance sheet records it, and the ledger derives
    // the same amount as the demand claims that exceed the reserves.
    assert!(
        bank.fiduciary_issued > Gold::ZERO,
        "a fractional-reserve bank must issue fiduciary credit"
    );
    assert!(
        composition.fiduciary > Gold::ZERO,
        "the ledger must carry fiduciary credit"
    );
    assert_eq!(
        bank.fiduciary_issued, composition.fiduciary,
        "the bank's issued fiduciary equals the ledger's fiduciary (claims beyond reserves)"
    );
    let recorded_bank_credit: u64 = s
        .society()
        .m3_records
        .iter()
        .map(|record| record.bank_credit_issued.0)
        .sum();
    assert_eq!(
        recorded_bank_credit, bank.fiduciary_issued.0,
        "the M3 records expose the sim-side bank credit issuance"
    );
    assert_eq!(
        composition.fiduciary.0,
        composition.demand_claims.0 - composition.bank_reserves.0,
        "fiduciary is exactly the demand claims that exceed the reserves"
    );

    // The borrowers (gatherers) hold the fiduciary claims and spend them into the
    // economy: a borrower holds claims, and the FOOD market keeps clearing and
    // provisioning the colony.
    let borrower = first_living(&s, Vocation::Gatherer);
    assert!(
        s.demand_claim_of(borrower) > Gold::ZERO,
        "a borrower holds the fiduciary claims the bank lent it"
    );
    assert_eq!(
        s.living_total(),
        s.population(),
        "borrowers spend the fiduciary credit into the economy (the colony stays fed)"
    );
}

/// 4. The control: a 100%-reserve bank lends ZERO fiduciary (`fiduciary_issued == 0`)
///    while its deposits still circulate as claims. Paired with test 3 — same
///    deposits, same regime, same economy, only the reserve ratio differs — this
///    isolates credit creation to the fractional reserve. The lab's
///    `hundred_pct_reserve_lends_no_fiduciary`, in the sim.
#[test]
fn full_reserve_lends_no_fiduciary() {
    let mut control = Settlement::generate(7, &SettlementConfig::bank_full_reserve());
    control.run(30);

    let bank = control.bank().expect("the control charters a bank");
    assert_eq!(
        bank.reserve_ratio_bps,
        ReserveRatioBps::FULL,
        "the control bank is a 100%-reserve bank"
    );
    let composition = control
        .money_composition()
        .expect("a banked settlement has a composition");

    // No fiduciary credit is created — the falsification.
    assert_eq!(
        bank.fiduciary_issued,
        Gold::ZERO,
        "a 100%-reserve bank must lend no fiduciary credit"
    );
    assert_eq!(
        composition.fiduciary,
        Gold::ZERO,
        "the ledger carries no fiduciary credit"
    );
    assert!(
        control
            .society()
            .m3_records
            .iter()
            .all(|record| record.bank_credit_issued == Gold::ZERO),
        "the full-reserve control must report zero bank credit issued"
    );

    // Yet its deposits still circulate as claims, fully backed by reserves.
    assert!(
        composition.bank_reserves > Gold::ZERO,
        "the control's deposits still build reserves"
    );
    assert!(
        composition.demand_claims > Gold::ZERO,
        "the control's deposits still circulate as claims"
    );
    assert_eq!(
        composition.demand_claims, composition.bank_reserves,
        "every demand claim is fully backed by reserves — no claim beyond reserves"
    );

    // The twin: the fractional bank DID create credit from the same deposits. The
    // reserve ratio is the only difference, so it is the cause.
    let mut fractional = Settlement::generate(7, &SettlementConfig::bank());
    fractional.run(30);
    assert!(
        fractional.bank().expect("bank").fiduciary_issued > Gold::ZERO,
        "the fractional twin creates fiduciary credit from the same deposits — credit \
         creation comes from the fractional reserve, not the deposits alone"
    );
}

/// 5. Whole-system conservation holds with nonzero claims/reserves/fiduciary + goods,
///    every econ tick: `fiduciary <= demand_claims`, reserves back claims, specie
///    conserved, goods conserved, and the M3 ledger reconciles.
#[test]
fn m3_conserves_with_credit() {
    let mut s = Settlement::generate(7, &SettlementConfig::bank());
    let total_money = s.total_gold();
    for t in 0..80 {
        let report = s.econ_tick();
        // Goods conservation (the whole-system identity).
        assert!(
            report.conserves(),
            "goods conservation broke at econ tick {t}"
        );
        // The M3 ledger's own conservation: specie/claims/reserves/fiduciary all
        // reconcile, the bank reserves equal the sum of bank balance sheets, and every
        // balance maps to a live agent.
        assert!(
            s.society().money_ledgers_reconcile(),
            "the M3 ledger (with credit) must reconcile at econ tick {t}"
        );
        // Specie is conserved: the specie base never moves — fiduciary is credit, not
        // minted specie.
        assert_eq!(
            s.total_gold(),
            total_money,
            "specie was created or destroyed at econ tick {t}"
        );
        let composition = s
            .money_composition()
            .expect("a banked settlement has a composition");
        // Fiduciary never exceeds the demand claims (it is the unbacked subset).
        assert!(
            composition.fiduciary <= composition.demand_claims,
            "fiduciary exceeded demand claims at econ tick {t}"
        );
        // Reserves back claims: the reserve-backed portion is a subset of the claims.
        assert!(
            composition.bank_reserves <= composition.demand_claims,
            "bank reserves exceeded demand claims at econ tick {t}"
        );
    }

    // The run actually carried nonzero credit (so the conservation above spanned a
    // live M3 ledger, not an inert one) plus the conserved goods.
    let composition = s
        .money_composition()
        .expect("a banked settlement has a composition");
    assert!(
        composition.demand_claims > Gold::ZERO,
        "the run must carry demand claims"
    );
    assert!(
        composition.bank_reserves > Gold::ZERO,
        "the run must carry bank reserves"
    );
    assert!(
        composition.fiduciary > Gold::ZERO,
        "the run must carry fiduciary credit"
    );
    assert!(
        s.tracked_goods().contains(&FOOD),
        "FOOD is a conserved tracked good"
    );
}

/// 6. Issuing fiduciary does NOT change the specie total — it expands the claims (the
///    TMS distinction). The specie base is constant across the run, every unit of it
///    sits in public hands or bank reserves (never in fiduciary), and the total money
///    supply (TMS = specie + claims) exceeds the specie base by exactly the fiduciary.
#[test]
fn fiduciary_is_not_minted_specie() {
    let mut s = Settlement::generate(7, &SettlementConfig::bank());
    let specie_base = s.total_gold();
    for t in 0..60 {
        s.econ_tick();
        let composition = s
            .money_composition()
            .expect("a banked settlement has a composition");
        // The specie base is unchanged by fiduciary issuance.
        assert_eq!(
            s.total_gold(),
            specie_base,
            "fiduciary issuance changed the specie total at econ tick {t}"
        );
        // Every unit of specie is in public hands or bank reserves — fiduciary is
        // never specie.
        assert_eq!(
            composition.public_specie.0 + composition.bank_reserves.0,
            specie_base.0,
            "fiduciary leaked into the specie locations at econ tick {t}"
        );
    }

    // Fiduciary expanded the claims: the broad money (TMS = specie + fiat + claims)
    // exceeds the specie base by exactly the fiduciary credit — credit expansion
    // without new specie.
    let composition = s
        .money_composition()
        .expect("a banked settlement has a composition");
    assert!(
        composition.fiduciary > Gold::ZERO,
        "the run must issue fiduciary credit"
    );
    assert_eq!(
        composition.tms().0,
        specie_base.0 + composition.fiduciary.0,
        "the money supply expanded above the specie base by exactly the fiduciary credit"
    );
}

/// 7. `econ_unchanged` — the no-bank path is byte-identical (the bank additions are
///    inert without a charter) and a banked run keeps the econ invariants (the M3
///    ledger reconciles and specie is conserved). The six econ goldens staying
///    byte-identical and the full workspace suite + `cargo clippy --workspace
///    --all-targets -- -D warnings` + `cargo fmt --check` are the real gate; this
///    checks the local seam.
#[test]
fn econ_unchanged() {
    // A no-bank settlement (the G8a M3 settlement and the closed-GOLD M1 viable
    // settlement) surfaces no bank — the bank phase is skipped entirely.
    let m3 = Settlement::generate(7, &SettlementConfig::m3_settlement());
    assert!(
        !m3.is_banked(),
        "the bank-free M3 settlement charters no bank"
    );
    assert!(
        m3.bank().is_none(),
        "a bank-free settlement surfaces no bank balance sheet"
    );
    let plain = Settlement::generate(7, &SettlementConfig::viable());
    assert!(
        !plain.is_banked(),
        "the closed-GOLD M1 settlement charters no bank"
    );

    // The no-bank M3 settlement is byte-identical to a twin — the G8b additions are
    // present but unexercised.
    let mut a = Settlement::generate(42, &SettlementConfig::m3_settlement());
    let mut b = Settlement::generate(42, &SettlementConfig::m3_settlement());
    a.run(40);
    b.run(40);
    assert_eq!(a.canonical_bytes(), b.canonical_bytes());

    // A banked run keeps the econ invariants tick over tick: the M3 ledger reconciles
    // and specie is conserved with credit live.
    let mut banked = Settlement::generate(3, &SettlementConfig::bank());
    let total_money = banked.total_gold();
    for _ in 0..120 {
        let report = banked.econ_tick();
        assert!(report.conserves());
        assert!(
            banked.society().money_ledgers_reconcile(),
            "the M3 ledger with credit reconciles every tick"
        );
        assert_eq!(
            banked.total_gold(),
            total_money,
            "specie stays conserved with credit"
        );
    }
}

// ---- unit tests -------------------------------------------------------------

/// A chartered bank requires the M3 ledger: a bank config without `m3` is rejected at
/// generation (there is no bank without ledger money).
#[test]
#[should_panic(expected = "requires the M3 ledger")]
fn bank_requires_m3_ledger() {
    let mut config = SettlementConfig::bank();
    config.m3 = false;
    let _ = Settlement::generate(1, &config);
}

/// G8b has no demand-claim estate routing. Reject a caller-composed banked
/// demography config rather than letting old-age removals stall on funded claims.
#[test]
#[should_panic(expected = "cannot run with demography")]
fn bank_rejects_demography_until_claim_estates_exist() {
    let mut config = SettlementConfig::lineages();
    config.m3 = true;
    config.bank = SettlementConfig::bank().bank;
    let _ = Settlement::generate(1, &config);
}

/// The bank phase is inert without a charter: a bank-free settlement creates no bank,
/// no reserves, no claims, and no fiduciary.
#[test]
fn bank_free_settlement_has_no_credit() {
    let mut s = Settlement::generate(7, &SettlementConfig::m3_settlement());
    s.run(20);
    assert!(s.bank().is_none());
    let composition = s
        .money_composition()
        .expect("an M3 settlement has a composition");
    assert_eq!(composition.bank_reserves, Gold::ZERO);
    assert_eq!(composition.demand_claims, Gold::ZERO);
    assert_eq!(composition.fiduciary, Gold::ZERO);
}

/// The bank's balance sheet mirrors the M3 ledger: its reserves equal the ledger's
/// bank reserves and its issued fiduciary equals the ledger's fiduciary — the bank is
/// a view of the same reused machinery, not a parallel ledger.
#[test]
fn bank_balance_sheet_mirrors_the_ledger() {
    let mut s = Settlement::generate(11, &SettlementConfig::bank());
    s.run(25);
    let composition = s
        .money_composition()
        .expect("a banked settlement has a composition");
    let bank = s.bank().expect("a banked settlement charters a bank");
    assert_eq!(bank.reserves, composition.bank_reserves);
    assert_eq!(bank.fiduciary_issued, composition.fiduciary);
    // The demand claims the bank surfaces equal the ledger's demand claims.
    assert_eq!(s.demand_claims_outstanding(), composition.demand_claims);
}
