//! S21h acceptance suite — the **demand-side survival bridge** (impl-31): does keeping the
//! non-cultivating market roles alive bring endogenous money back under the positive check?
//!
//! S21g found that turning the S17 positive check ON over the S21f open-market money colony
//! culls the non-cultivating DEMAND side (the SALT-rich buyers + the specialist woodcutters,
//! who hold no food and never cultivate) in a one-off cold-start cull at ~tick 7, BEFORE the
//! money market can form. The self-feeding cultivation lineage survives into a quiescent
//! subsistence commune; SALT never promotes. The pre-money bootstrap needs the demand side to
//! survive a long hungry, foodless wait until SALT promotes — and mortality kills that
//! patience. S21h asks whether a demand-side survival BRIDGE lets endogenous money coexist
//! with mortality, sliced like the supply arc (S21e seeded → S21f produced):
//!
//!   * **S21h.0 — the consumed-only cushion (the bounded diagnostic).** A finite STARTING
//!     bread cushion for the two culled roles (the buyers' `consumer_staple_buffer`; the
//!     woodcutters' new dedicated `gatherer_food_cushion`), eaten but never sold.
//!     THE OBSERVED RESULT IS THE KNIFE-EDGE FINDING (`cushion_sweep_is_the_knife_edge`):
//!     no cushion size yields a CLEAN demand-bridge success. Too small and most buyers still
//!     starve (only 4–5 of 18 survive — too thin a demand hub to monetize SALT); too large and
//!     the cushion sates the buyers out of the bread market while it lasts, then runs out and
//!     the full cull lands anyway. On the equal-buffer diagonal SALT NEVER promotes at all
//!     (across sizes and seeds — `cushion_sweep_is_the_knife_edge` /
//!     `cushion_knife_edge_holds_across_seeds`); off the diagonal there is likewise no clean
//!     success, and the cells that DO promote do so only by selling seeded `SeededMinted`
//!     cushion bread for SALT — the seeded-supply-DISQUALIFIED path, not an authentic demand
//!     bridge (`cushion_knife_edge_holds_off_diagonal`). The bridge is a one-time stock — it
//!     cannot keep the demand side both ALIVE and HUNGRY through the pre-money wait. The hard
//!     provenance invariant is what classifies those off-diagonal promotions as disqualified:
//!     a clean success requires cushion (`SeededMinted`) bread NEVER sold for SALT
//!     (`seeded_minted_bread_sold_for_salt == 0`), so no cell is an authentic (non-seeded)
//!     demand-bridge promotion.
//!
//!   * **S21h.1 — produced emergency self-provisioning (a configured own-labor survival
//!     institution).** A produced, NO-grain-INPUT, low-yield, self-consumed own-labor BREAD
//!     floor that fires only near starvation (`emergency_hunger_threshold`) and pulls hunger to
//!     one notch below the trigger — a RECURRING near-critical floor. It is the engine's
//!     established own-labor subsistence tier (not ordinary grain→flour→bread production, and
//!     not the removal of all survival scaffolding). THIS THREADS THE KNIFE-EDGE
//!     (`emergency_floor_threads_the_knife_edge`): it keeps 12 of the 18 non-lineage roles
//!     alive AND hungry (so they still demand and BUY bread), SALT promotes on the lineage's
//!     `SelfProduced` bread (`seeded_minted == 0` entirely), so the open colony finally has
//!     money + mortality together — AFTER a one-off cold-start cull (6 of 18 non-lineage roles
//!     still die; starvation then stops: a partial bridged band, not full demand-side survival
//!     nor an ongoing positive-check band). Robust across seeds and across the threshold sweep.
//!
//! Determinism note: every new field/flag defaults off/0 and the two new scenarios are
//! additive, so every existing golden is byte-identical (the diagnostics — the acquisition
//! ledger, the bread-Now-wants probe, the sold-for-SALT and emergency tallies — are all
//! runtime-only, never in `canonical_bytes`).

use econ::good::{GoodId, SALT, WOOD};
use sim::{Settlement, SettlementConfig};

/// A horizon long enough to clear the cold-start, promote (or fail to), and settle (the S21f/g
/// money suites use 1600).
const PROBE_TICKS: u64 = 1_600;

/// The robustness seed set (the S21f/g suites use the same).
const SEEDS: [u64; 5] = [3, 7, 11, 19, 23];

// ---- shared helpers -----------------------------------------------------

fn living(s: &Settlement) -> usize {
    (0..s.population()).filter(|&i| s.is_alive(i)).count()
}

/// Living **lineage** members (the self-feeding cultivators — `household_of` is `Some`).
fn living_lineage(s: &Settlement) -> usize {
    (0..s.population())
        .filter(|&i| s.is_alive(i) && s.household_of(i).is_some())
        .count()
}

/// Living **non-lineage** roles (the SALT-rich buyers + the woodcutters — the demand side the
/// positive check culls in S21g; `household_of` is `None`).
fn living_non_lineage(s: &Settlement) -> usize {
    (0..s.population())
        .filter(|&i| s.is_alive(i) && s.household_of(i).is_none())
        .count()
}

fn bread_good(s: &Settlement) -> GoodId {
    s.bread_good()
        .expect("the demand-bridge chain carries a bread good")
}

/// Sweep the consumed-only cushion (both axes together: the buyers' `consumer_staple_buffer`
/// and the woodcutters' `gatherer_food_cushion` to the same size `c`).
fn with_cushion(c: u32) -> SettlementConfig {
    with_cushion_split(c, c)
}

/// Sweep the two cushion axes INDEPENDENTLY: the buyers' `consumer_staple_buffer` to `consumer`
/// and the woodcutters' `gatherer_food_cushion` to `gatherer`. The diagonal `with_cushion(c)`
/// is the `consumer == gatherer` case; this lets the knife-edge claim also bracket off-diagonal
/// (asymmetric) cushion combinations.
fn with_cushion_split(consumer: u32, gatherer: u32) -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_demand_cushion();
    if let Some(chain) = cfg.chain.as_mut() {
        chain.consumer_staple_buffer = consumer;
        chain.gatherer_food_cushion = gatherer;
    }
    cfg
}

/// The emergency seam at a swept `threshold`.
fn with_emergency(threshold: u16) -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_emergency_provision();
    if let Some(chain) = cfg.chain.as_mut() {
        chain.emergency_hunger_threshold = threshold;
    }
    cfg
}

// ---- the per-cell classification vector (Codex P2) ----------------------

/// The full 5-tuple the knife-edge sweep classifies every cell by, plus the broken-invariant
/// guards a run must always pass. Read-only public accessors only, so collecting it perturbs
/// nothing.
#[derive(Clone, Copy, Debug)]
struct Cell {
    /// (1) The non-lineage demand side is alive at the end of the run (the S21g cull did NOT
    /// wipe it out).
    survived: bool,
    /// (2) The SURVIVING non-lineage roles still emit a present `Horizon::Now` bread want —
    /// the bridge did not satiate them out of the bread market.
    demanded: bool,
    /// (3) SALT promoted to money.
    promoted: bool,
    /// (4) Food was materially BOUGHT on the market over the run (a real demand side).
    bought_materially: bool,
    /// (5) NO `SeededMinted`/cushion bread was ever sold for SALT, and the pre-promotion
    /// bread that monetized SALT was `SelfProduced` (minted volume 0) — the hard provenance
    /// invariant. A cell where this is false is DISQUALIFIED (a seeded-supply result).
    provenance_clean: bool,
    // Broken-invariant guards (must hold regardless of the outcome).
    conserved: bool,
    bread_minted_max: u64,
    extinct: bool,
    credited_seeded_minted: u64,
    // Figures for reporting.
    non_lineage: usize,
    lineage: usize,
    starvation: u64,
    bought: u64,
    self_produced: u64,
}

impl Cell {
    /// The S21h SUCCESS: the demand side survives AND still demands AND SALT promotes AND food
    /// is materially bought AND the provenance is clean.
    fn is_success(&self) -> bool {
        self.survived
            && self.demanded
            && self.promoted
            && self.bought_materially
            && self.provenance_clean
    }
}

/// Bread bought over the run below which we do not call the market demand "material".
const MATERIAL_BOUGHT_FLOOR: u64 = 1_000;

/// Run `(seed, cfg)` for `ticks` and collect the classification vector. The demand probe is
/// measured on the LIVING non-lineage roles each tick and the max retained, so a cell where
/// the survivors demand bread at any point (pre- or post-promotion) reads `demanded = true`.
fn classify(seed: u64, cfg: &SettlementConfig, ticks: u64) -> Cell {
    let mut s = Settlement::generate(seed, cfg);
    let bread = bread_good(&s);
    let mut conserved = true;
    let mut bread_minted_max = 0u64;
    let mut demanded_while_alive = false;
    for _ in 0..ticks {
        let report = s.econ_tick();
        conserved &= report.conserves();
        bread_minted_max = bread_minted_max.max(report.endowment_of(bread));
        // Demand among the LIVING non-lineage roles — read each tick so a cell whose buyers
        // survive and demand (even briefly, before any cull) is distinguished from one whose
        // buyers are sated to zero Now-wants throughout.
        if living_non_lineage(&s) > 0 && s.living_non_lineage_with_bread_now_wants(bread) > 0 {
            demanded_while_alive = true;
        }
    }
    let consumed = s.acquisition_consumed_by_channel();
    let credited = s.acquisition_credited_by_channel();
    let (pp_produced, pp_minted) = s.pre_promotion_bread_for_salt_by_provenance();
    let _ = pp_produced;
    // The hard provenance invariant: NO cushion (SeededMinted) bread sold for SALT, AND the
    // pre-promotion bread that monetized SALT was SelfProduced (minted volume 0).
    let provenance_clean = s.seeded_minted_bread_sold_for_salt() == 0 && pp_minted == 0;
    // Demand among the survivors at the end (the persisting demand side).
    let demanded_at_end = s.living_non_lineage_with_bread_now_wants(bread) > 0;
    Cell {
        survived: living_non_lineage(&s) > 0,
        demanded: demanded_while_alive && demanded_at_end,
        promoted: s.current_money_good() == Some(SALT),
        bought_materially: consumed.bought >= MATERIAL_BOUGHT_FLOOR,
        provenance_clean,
        conserved,
        bread_minted_max,
        extinct: living(&s) == 0,
        credited_seeded_minted: credited.seeded_minted,
        non_lineage: living_non_lineage(&s),
        lineage: living_lineage(&s),
        starvation: s.starvation_deaths_total(),
        bought: consumed.bought,
        self_produced: consumed.self_produced,
    }
}

// =========================================================================
// 1. The determinism / golden contract
// =========================================================================

#[test]
fn canonical_bytes_split_for_demand_cushion() {
    // The cushion changes the non-lineage roles' starting bread, so `frontier_demand_cushion`
    // must SPLIT the canonical digest vs the S21g base...
    let base = Settlement::generate(7, &SettlementConfig::frontier_open_colony_mortality());
    let cushion = Settlement::generate(7, &SettlementConfig::frontier_demand_cushion());
    assert_ne!(
        base.canonical_bytes(),
        cushion.canonical_bytes(),
        "the consumed-only cushion must split the canonical digest vs frontier_open_colony_mortality"
    );

    // ...and zeroing BOTH cushion axes must make it BYTE-IDENTICAL to
    // `frontier_open_colony_mortality` — the cushion is the ONLY behavioural change (additive,
    // canonicalized ON-only).
    let reverted = with_cushion(0);
    let mut a = Settlement::generate(7, &reverted);
    let mut b = Settlement::generate(7, &SettlementConfig::frontier_open_colony_mortality());
    a.run(1_000);
    b.run(1_000);
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "zeroing both cushion axes must equal frontier_open_colony_mortality byte-for-byte"
    );
    assert_eq!(a.digest(), b.digest());
}

#[test]
fn canonical_bytes_split_for_emergency_provision() {
    // The emergency seam is a future-behaviour change (the non-lineage roles produce + eat a
    // survival floor), so `frontier_emergency_provision` must SPLIT the canonical digest...
    let base = Settlement::generate(7, &SettlementConfig::frontier_open_colony_mortality());
    let emergency = Settlement::generate(7, &SettlementConfig::frontier_emergency_provision());
    assert_ne!(
        base.canonical_bytes(),
        emergency.canonical_bytes(),
        "the emergency self-provisioning seam must split the canonical digest"
    );

    // ...and reverting the threshold to 0 must make it BYTE-IDENTICAL to
    // `frontier_open_colony_mortality` (the seam is the ONLY change, canonicalized ON-only).
    let reverted = with_emergency(0);
    let mut a = Settlement::generate(7, &reverted);
    let mut b = Settlement::generate(7, &SettlementConfig::frontier_open_colony_mortality());
    a.run(1_000);
    b.run(1_000);
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "reverting the emergency threshold must equal frontier_open_colony_mortality byte-for-byte"
    );
    assert_eq!(a.digest(), b.digest());
}

#[test]
fn goldens_unchanged() {
    // The S21h additions are two new scenarios + two ON-only flags + runtime-only diagnostics,
    // so the cross-history demographic + emergence goldens are BYTE-IDENTICAL (the same values
    // pinned in tests/mortality.rs, tests/household_barter.rs and tests/open_colony_mortality.rs).
    let digest = |cfg: &SettlementConfig, ticks: u64| {
        let mut s = Settlement::generate(1, cfg);
        s.run(ticks);
        s.digest()
    };
    assert_eq!(
        digest(&SettlementConfig::lineages(), 300),
        0x2335e13c809749fc,
        "the `lineages` demographic golden (the key tripwire) must be byte-identical"
    );
    assert_eq!(
        digest(&SettlementConfig::lineages(), 800),
        0x3ffd78e50842d934,
        "the long `lineages` run must be byte-identical"
    );
    assert_eq!(
        digest(&SettlementConfig::frontier(), 300),
        0xcc83bf2669f0980d,
        "the S5-S13 frontier golden must be byte-identical"
    );
    assert_eq!(
        digest(&SettlementConfig::frontier_spatial_households(), 300),
        0xf30e3ce9345a73b3,
        "the S13 spatial-households golden must be byte-identical"
    );
    let mut viable = Settlement::generate(0xC0FFEE, &SettlementConfig::viable());
    viable.run(60);
    assert_eq!(
        viable.digest(),
        0xa174_8567_db1c_4341,
        "the g4a viable no-death digest must be byte-identical"
    );
}

#[test]
fn demand_bridge_runs_are_deterministic() {
    // Byte-identical `(seed, config)` at a long horizon for both new scenarios (the deaths +
    // emergency floor live in the colonist liveness/estate state `canonical_bytes` pins; the
    // diagnostics are runtime-only).
    for cfg in [
        SettlementConfig::frontier_demand_cushion(),
        SettlementConfig::frontier_emergency_provision(),
    ] {
        let mut a = Settlement::generate(7, &cfg);
        let mut b = Settlement::generate(7, &cfg);
        a.run(2_400);
        b.run(2_400);
        assert_eq!(
            a.canonical_bytes(),
            b.canonical_bytes(),
            "the demand-bridge run must be byte-identical for the same (seed, config)"
        );
        assert_eq!(a.digest(), b.digest());
        let mut c = Settlement::generate(2, &cfg);
        c.run(2_400);
        assert_ne!(a.digest(), c.digest(), "the seed must change the run");
    }
}

// =========================================================================
// 2. S21h.0 — THE KNIFE-EDGE FINDING: no cushion size threads it
// =========================================================================

#[test]
fn cushion_sweep_is_the_knife_edge() {
    // THE S21h.0 HEADLINE FINDING (seed 7, deterministic figures pinned — the report cites
    // them). Sweep the consumed-only cushion across all three regimes and classify every cell
    // by the full 5-tuple. The knife-edge: NO cushion size satisfies all five (in particular
    // SALT promotes in NO cell), because a one-time stock cannot keep the demand side both
    // ALIVE and HUNGRY through the pre-money wait — and the hard provenance invariant
    // (`SeededMinted` bread never sold for SALT) holds on EVERY cell. This sweeps the two axes
    // TOGETHER (the equal-buffer diagonal); `cushion_knife_edge_holds_off_diagonal` brackets
    // the asymmetric (buyer ≠ woodcutter) splits, so the negative claim is not diagonal-only.
    let sweep = [0u32, 4, 8, 12, 16, 24, 32, 48, 64, 96];
    let mut any_success = false;
    let mut saw_cull = false;
    let mut saw_partial_survival = false;
    let mut saw_too_strong = false;
    for c in sweep {
        let cell = classify(7, &with_cushion(c), PROBE_TICKS);

        // Broken-invariant guards hold on every cell.
        assert!(
            cell.conserved,
            "cushion C={c}: conservation must hold every tick"
        );
        assert_eq!(
            cell.bread_minted_max, 0,
            "cushion C={c}: no bread may be minted (the food mints are retired)"
        );
        assert!(
            !cell.extinct,
            "cushion C={c}: the colony must not go extinct"
        );

        // THE HARD PER-CELL INVARIANT: cushion (SeededMinted) bread is NEVER sold for SALT —
        // so no cell is a disguised seeded-supply promotion (Codex P1). The cushion IS eaten
        // (relaxed: `seeded_minted` consumed > 0 is allowed when c > 0), but never SOLD.
        assert!(
            cell.provenance_clean,
            "cushion C={c}: the hard invariant FAILED — cushion bread was sold for SALT \
             (seeded_minted_sold_for_salt or pre-promotion minted volume > 0); the cell is a \
             seeded-supply result, disqualified"
        );
        if c > 0 {
            assert!(
                cell.credited_seeded_minted > 0,
                "cushion C={c}: the cushion enters as SeededMinted (it is a seed buffer)"
            );
        }

        // Classify the regime.
        any_success |= cell.is_success();
        if !cell.survived {
            saw_cull = true;
        } else if cell.demanded {
            saw_partial_survival = true;
        } else {
            saw_too_strong = true;
        }
        eprintln!(
            "cushion C=G={c}: survived={} demanded={} promoted={} bought_mat={} clean={} | \
             non-lineage={} starv={} bought={} seeded_credited={}",
            cell.survived,
            cell.demanded,
            cell.promoted,
            cell.bought_materially,
            cell.provenance_clean,
            cell.non_lineage,
            cell.starvation,
            cell.bought,
            cell.credited_seeded_minted,
        );
    }

    // THE FINDING: no cushion cell is a full success (SALT never promotes), and BOTH failure
    // regimes the spec pre-named appear — too-weak (the cull) and the partial-survival middle
    // that still does not monetize. (The too-strong satiation regime collapses into the cull
    // here: a large cushion delays, sates, then runs out and the full cull lands anyway.)
    assert!(
        !any_success,
        "the S21h.0 knife-edge: NO cushion size may thread it (a one-time stock cannot keep \
         the demand side both alive and hungry through the pre-money wait)"
    );
    assert!(saw_cull, "the sweep must exhibit the too-weak cull regime");
    assert!(
        saw_partial_survival,
        "the sweep must exhibit the partial-survival regime (some buyers survive + demand, but \
         too thin a hub to monetize SALT)"
    );
    // `saw_too_strong` is reported but not required: in this colony the over-large cushion
    // delays then runs out, so the satiation regime is observed as the (delayed) cull rather
    // than a persistent alive-but-sated state.
    let _ = saw_too_strong;
}

#[test]
fn cushion_headline_cell_does_not_promote() {
    // The disclosed headline cell (`frontier_demand_cushion`, C=G=16, seed 7, pinned figures).
    // The cushion keeps a FEW non-lineage roles alive (5) but most still starve (13) — too thin
    // a demand hub — so SALT never promotes. The cushion is eaten but NEVER sold for SALT.
    let s = {
        let mut s = Settlement::generate(7, &SettlementConfig::frontier_demand_cushion());
        s.run(PROBE_TICKS);
        s
    };
    assert_eq!(
        s.current_money_good(),
        None,
        "the headline cushion cell does not promote SALT (the demand hub is too thin)"
    );
    assert!(
        living_non_lineage(&s) > 0,
        "the cushion keeps SOME non-lineage roles alive (vs the S21g full cull)"
    );
    assert_eq!(
        living_non_lineage(&s),
        5,
        "the deterministic seed-7 figure: 5 of the 18 non-lineage roles survive the cushion"
    );
    assert!(
        s.starvation_deaths_total() >= 13,
        "most of the demand side still starves under the cushion (the thin-hub finding)"
    );
    // The hard invariant: the cushion is eaten (SeededMinted consumed) but NEVER sold for SALT.
    assert!(
        s.acquisition_credited_by_channel().seeded_minted > 0,
        "the cushion enters as SeededMinted"
    );
    assert!(
        s.acquisition_consumed_by_channel().seeded_minted > 0,
        "the cushion IS eaten (S21h.0 relaxes the consumed==0 bar)"
    );
    assert_eq!(
        s.seeded_minted_bread_sold_for_salt(),
        0,
        "but cushion bread is NEVER sold for SALT (the hard invariant)"
    );
    assert_eq!(
        s.pre_promotion_bread_for_salt_by_provenance().1,
        0,
        "and no pre-promotion bread sold for SALT was minted-origin"
    );
}

#[test]
fn cushion_knife_edge_holds_across_seeds() {
    // Robustness: the knife-edge (no promotion, hard invariant holds) is not a seed-7 artifact.
    for seed in SEEDS {
        for c in [8u32, 16, 32, 64] {
            let cell = classify(seed, &with_cushion(c), PROBE_TICKS);
            assert!(cell.conserved, "seed {seed} C={c}: conservation must hold");
            assert!(
                !cell.promoted,
                "seed {seed} C={c}: SALT must NOT promote under the cushion (the knife-edge)"
            );
            assert!(
                cell.provenance_clean,
                "seed {seed} C={c}: cushion bread must NEVER be sold for SALT (hard invariant)"
            );
        }
    }
}

#[test]
fn cushion_knife_edge_holds_off_diagonal() {
    // The `cushion_sweep_is_the_knife_edge` headline sweeps the two cushion axes TOGETHER
    // (`consumer_staple_buffer == gatherer_food_cushion`), so on its own the "no cushion size
    // threads it" negative claim is demonstrated only on the equal-buffer diagonal. This
    // brackets the OFF-diagonal: asymmetric buyer/woodcutter cushion combinations — including
    // the extreme corners (all-buyer / all-woodcutter). The result is the same headline finding,
    // and the off-diagonal sharpens WHY: no asymmetric split is a clean SUCCESS either. The
    // cells fall into three documented outcomes — (a) too-weak cull, (b) partial survival that
    // does not promote, and (c) the SEEDED-SUPPLY TRAP the spec pre-named: when the woodcutters
    // are over-cushioned but the buyers are not, the woodcutters' offerable cushion EXCESS is
    // bought for SALT by the hungry SALT-rich buyers, so `seeded_minted_bread_sold_for_salt > 0`
    // and the cell is DISQUALIFIED (a seeded-supply result, never counted as a demand-survival
    // success). On the diagonal this trap is hidden because a symmetric cushion also sates the
    // buyers, so they post no bread bid. Either way, `is_success()` is false on every cell.
    let off_diagonal = [
        (64u32, 0u32), // all-buyer cushion, woodcutters get the S21g cull
        (0, 64),       // all-woodcutter cushion, buyers get the S21g cull
        (32, 8),
        (8, 32), // over-cushioned woodcutters + hungry buyers: the seeded-supply trap
        (24, 12),
        (12, 24),
        (48, 16),
        (16, 48),
    ];
    let mut any_success = false;
    let mut any_disqualified = false;
    for (consumer, gatherer) in off_diagonal {
        let cell = classify(7, &with_cushion_split(consumer, gatherer), PROBE_TICKS);
        // Broken-invariant guards hold on every cell regardless of the outcome.
        assert!(
            cell.conserved,
            "cushion (C={consumer}, G={gatherer}): conservation must hold every tick"
        );
        assert_eq!(
            cell.bread_minted_max, 0,
            "cushion (C={consumer}, G={gatherer}): no bread may be minted"
        );
        assert!(
            !cell.extinct,
            "cushion (C={consumer}, G={gatherer}): the colony must not go extinct"
        );
        // A cell that promotes MUST do so via the cushion's seeded supply (disqualified), never
        // cleanly: a clean promotion off the diagonal would contradict the headline finding.
        // So `promoted` and `provenance_clean` are never BOTH true (== `is_success` is false).
        assert!(
            !(cell.promoted && cell.provenance_clean),
            "cushion (C={consumer}, G={gatherer}): a clean SALT promotion off the diagonal would \
             contradict the knife-edge — any off-diagonal promotion must be the disqualified \
             seeded-supply trap (cushion bread sold for SALT), not a demand-survival success"
        );
        any_success |= cell.is_success();
        any_disqualified |= !cell.provenance_clean;
        eprintln!(
            "off-diagonal cushion C={consumer} G={gatherer}: survived={} demanded={} \
             promoted={} clean={} non-lineage={} starv={} sold_for_salt={}",
            cell.survived,
            cell.demanded,
            cell.promoted,
            cell.provenance_clean,
            cell.non_lineage,
            cell.starvation,
            !cell.provenance_clean,
        );
    }
    assert!(
        !any_success,
        "the S21h.0 knife-edge holds OFF the diagonal too: no asymmetric cushion split threads \
         it cleanly (a one-time stock cannot keep the buyer demand hub both alive and hungry \
         without either culling it or leaking offerable cushion supply that sells for SALT)"
    );
    // The off-diagonal additionally EXHIBITS the seeded-supply trap the diagonal hides: an
    // over-cushioned-woodcutter / hungry-buyer split sells cushion bread for SALT. This is the
    // spec's disqualification path made visible, not a regression.
    assert!(
        any_disqualified,
        "the off-diagonal must exhibit at least one disqualified seeded-supply cell (an \
         over-cushioned woodcutter split leaks offerable cushion bread sold for SALT)"
    );
}

// =========================================================================
// 3. S21h.1 — THE SUCCESS: the produced emergency floor threads the knife-edge
// =========================================================================

#[test]
fn emergency_floor_threads_the_knife_edge() {
    // THE S21h.1 HEADLINE SUCCESS (seed 7, `frontier_emergency_provision`, threshold 11). The
    // recurring near-critical own-labor bread floor keeps 12 of the 18 non-lineage roles alive
    // AND hungry (so they still demand and BUY bread), and SALT promotes on the lineage's
    // `SelfProduced` bread with `seeded_minted == 0` entirely — money + mortality together.
    let cell = classify(
        7,
        &SettlementConfig::frontier_emergency_provision(),
        PROBE_TICKS,
    );
    assert!(cell.conserved, "conservation must hold every tick");
    assert_eq!(cell.bread_minted_max, 0, "no bread may be minted");

    assert!(
        cell.is_success(),
        "the emergency floor must thread the knife-edge (survived + demanded + promoted + \
         bought + clean): {cell:?}"
    );

    // The provenance is FULLY restored vs the cushion: NO seeded/minted bread enters at all
    // (the emergency floor is `SelfProduced`, immediately eaten, never offerable).
    let s = {
        let mut s = Settlement::generate(7, &SettlementConfig::frontier_emergency_provision());
        s.run(PROBE_TICKS);
        s
    };
    assert_eq!(s.current_money_good(), Some(SALT), "SALT promotes");
    assert_eq!(
        s.acquisition_credited_by_channel().seeded_minted,
        0,
        "NO SeededMinted bread ever enters (no cushion — provenance fully restored)"
    );
    assert_eq!(
        s.acquisition_consumed_by_channel().seeded_minted,
        0,
        "no SeededMinted food is ever eaten either"
    );
    assert_eq!(
        s.seeded_minted_bread_sold_for_salt(),
        0,
        "no SeededMinted bread sold for SALT"
    );
    let (pp_produced, pp_minted) = s.pre_promotion_bread_for_salt_by_provenance();
    assert!(
        pp_produced > 0 && pp_minted == 0,
        "the pre-promotion bread that monetized SALT is SelfProduced (produced {pp_produced}, \
         minted {pp_minted})"
    );
    assert!(
        s.indirect_target_goods(SALT).contains(&WOOD),
        "the indirect breadth includes the non-food WOOD target (the {{bread, WOOD}} bar)"
    );
    // The demand side persists (12 of 18) and the lineage churns through births + old age.
    assert_eq!(
        living_non_lineage(&s),
        12,
        "the deterministic seed-7 figure: 12 of 18 non-lineage roles survive the cold-start"
    );
    assert!(
        living_lineage(&s) > 0 && s.births_total() > 0 && s.old_age_deaths_total() > 0,
        "the lineage is a live commune (births + old-age churn)"
    );
}

#[test]
fn emergency_money_colony_persists_to_10k() {
    // The 10k-persistence smoke + the band characterization: the emergency-bridged money colony
    // is STABLE over a long horizon — no extinction, SALT stays the money good, the bridged
    // non-lineage demand side persists (it does not slowly erode), the lineage keeps
    // reproducing, and the positive check is a ONE-OFF cold-start cull (no ongoing starvation
    // band) — conservation holds throughout. So the open colony's money + mortality coexistence
    // is durable, not a transient that decays. (The Malthusian structure here is the lineage's
    // births + old-age churn; the bridged demand side is a stable population fed by buying +
    // the emergency floor, the spec's "clearly characterized partial band".)
    let cfg = SettlementConfig::frontier_emergency_provision();
    let mut s = Settlement::generate(7, &cfg);
    s.run(500); // past the cold-start cull + promotion
    assert_eq!(
        s.current_money_good(),
        Some(SALT),
        "SALT has promoted by tick 500"
    );
    let non_lineage_after_cull = living_non_lineage(&s);
    let starv_after_cull = s.starvation_deaths_total();
    let births0 = s.births_total();
    assert!(
        non_lineage_after_cull > 0,
        "the bridged demand side survives the cold-start"
    );

    let measure = 9_500u64;
    let mut min_living = usize::MAX;
    let mut min_non_lineage = usize::MAX;
    for i in 0..measure {
        let report = s.econ_tick();
        assert!(report.conserves(), "conservation broke at measure tick {i}");
        min_living = min_living.min(living(&s));
        min_non_lineage = min_non_lineage.min(living_non_lineage(&s));
    }
    assert!(min_living > 0, "no extinction over the 10k horizon");
    assert!(
        min_non_lineage > 0,
        "the bridged demand side persists over the 10k horizon (min non-lineage {min_non_lineage})"
    );
    assert_eq!(
        s.current_money_good(),
        Some(SALT),
        "SALT stays the money good over the 10k horizon (a durable money colony)"
    );
    assert!(
        s.births_total() > births0,
        "the lineage keeps reproducing over the 10k horizon (the band churns)"
    );
    assert_eq!(
        s.starvation_deaths_total(),
        starv_after_cull,
        "the positive check is a ONE-OFF cold-start cull: no further starvation over the 10k \
         horizon (the bridged demand side does not erode)"
    );
}

#[test]
fn emergency_preserves_market_demand() {
    // THE DEMAND-PRESERVATION TEST (Codex P2): the emergency floor must NOT secretly satiate
    // the buyers out of the market. Post-promotion the non-lineage roles' food is materially
    // BOUGHT — they buy roughly half their bread on the market and self-provision only the
    // survival shortfall the (supply-limited) market cannot cover. The emergency floor is
    // therefore the survival MINIMUM, never the dominant source that would crowd out demand.
    let mut s = Settlement::generate(7, &SettlementConfig::frontier_emergency_provision());
    s.run(1_200); // well past promotion (~tick 53) into the steady state
    let bought0 = s.acquisition_consumed_by_channel().bought;
    let emergency0 = s.emergency_bread_provisioned();
    s.run(400);
    let bought_tail = s.acquisition_consumed_by_channel().bought - bought0;
    let emergency_tail = s.emergency_bread_provisioned() - emergency0;

    assert!(
        bought_tail > 0,
        "the non-lineage roles BUY bread on the market post-promotion (a live demand side)"
    );
    // Bought is material AND co-dominant with the emergency floor (~half each across seeds),
    // which decisively rules out the crowd-out failure (where the bridge sates the buyers and
    // `bought_tail` collapses to ~0). The floor never overwhelms market demand.
    assert!(
        bought_tail.saturating_mul(2) >= emergency_tail,
        "the emergency floor must NOT dominate tail consumption (bought {bought_tail} vs \
         emergency {emergency_tail}); the demand side really buys, it is not satiated out"
    );
    // The total bought over the run is material (the headline buy-side metric).
    assert!(
        s.acquisition_consumed_by_channel().bought >= MATERIAL_BOUGHT_FLOOR,
        "food is materially bought over the run"
    );
}

#[test]
fn emergency_never_leaves_self_produced_bread_with_non_lineage() {
    // Regression for the post-market ordering trap: once buyers can buy bread in the same tick
    // that they also hit the emergency threshold, the emergency phase must consume that held
    // bought bread before producing any new floor. Since non-lineage roles never cultivate, any
    // `SelfProduced` bread still held by them at tick end would be emergency bread that was not
    // immediately eaten.
    let mut s = Settlement::generate(7, &SettlementConfig::frontier_emergency_provision());
    for tick in 0..PROBE_TICKS {
        let report = s.econ_tick();
        assert!(report.conserves(), "tick {tick}: conservation must hold");
        assert_eq!(
            s.non_lineage_acquisition_held_by_channel().self_produced,
            0,
            "tick {tick}: emergency SelfProduced bread must not remain in non-lineage stock"
        );
    }
    assert!(
        s.emergency_bread_provisioned() > 0,
        "the regression must exercise the emergency floor"
    );
    assert!(
        s.acquisition_consumed_by_channel().bought > 0,
        "the regression must exercise bought bread before/alongside emergency provisioning"
    );
}

#[test]
fn emergency_success_holds_across_seeds() {
    // Robustness: the SUCCESS (promotion on SelfProduced bread, `seeded_minted == 0`, the
    // demand side alive + demanding + buying) is not a seed-7 artifact.
    for seed in SEEDS {
        let cell = classify(
            seed,
            &SettlementConfig::frontier_emergency_provision(),
            PROBE_TICKS,
        );
        assert!(cell.conserved, "seed {seed}: conservation must hold");
        assert!(
            cell.is_success(),
            "seed {seed}: the emergency floor must thread the knife-edge: {cell:?}"
        );
        assert_eq!(
            cell.credited_seeded_minted, 0,
            "seed {seed}: no SeededMinted bread ever enters (provenance fully restored)"
        );
    }
}

#[test]
fn emergency_yield_sweep_promotes_clean() {
    // The emergency-yield sweep (Codex P2): across the validated threshold band
    // (`cultivate_hunger_in = 6 < threshold < hunger_critical = 12`), the recurring
    // near-critical floor promotes SALT on `SelfProduced` bread with the provenance clean —
    // the success is robust to the threshold, not a single tuned value.
    for threshold in [7u16, 8, 9, 10, 11] {
        let cell = classify(7, &with_emergency(threshold), PROBE_TICKS);
        assert!(
            cell.conserved,
            "threshold {threshold}: conservation must hold"
        );
        assert!(
            cell.is_success(),
            "threshold {threshold}: the emergency floor must thread the knife-edge: {cell:?}"
        );
        assert!(
            cell.provenance_clean && cell.credited_seeded_minted == 0,
            "threshold {threshold}: provenance must be fully clean (no seeded/minted bread)"
        );
    }
}

// =========================================================================
// 4. The controls (classify, never tune)
// =========================================================================

#[test]
fn no_bridge_control_is_the_s21g_cull() {
    // The NO-BRIDGE control (`frontier_open_colony_mortality`): the S21g cull — every
    // non-lineage role starves, SALT never promotes, the self-feeding lineage survives. The
    // cause of the S21h need is the positive check culling the demand side.
    let cell = classify(
        7,
        &SettlementConfig::frontier_open_colony_mortality(),
        PROBE_TICKS,
    );
    assert!(cell.conserved, "conservation must hold every tick");
    assert!(
        !cell.survived,
        "the S21g cull: NO non-lineage role survives"
    );
    assert_eq!(cell.non_lineage, 0, "the demand side is wiped out");
    assert!(!cell.promoted, "SALT never promotes (no demand side)");
    assert_eq!(
        cell.bought, 0,
        "no food is ever bought (no surviving buyers)"
    );
    assert!(
        cell.lineage > 0,
        "the self-feeding lineage survives the cull"
    );
    assert!(
        cell.self_produced > 0,
        "the lineage eats its own cultivated bread"
    );
}

#[test]
fn mortality_off_control_is_the_s21f_success() {
    // The MORTALITY-OFF positive control (`frontier_household_barter`): the S21f success — the
    // 18 non-lineage roles survive (no positive check), SALT promotes on SelfProduced bread,
    // food is bought. Proves the bridge scenarios localize the positive check, not the colony.
    let cell = classify(
        7,
        &SettlementConfig::frontier_household_barter(),
        PROBE_TICKS,
    );
    assert!(cell.conserved, "conservation must hold every tick");
    assert_eq!(cell.starvation, 0, "mortality is OFF (no starvation)");
    assert!(
        cell.survived && cell.non_lineage > 0,
        "the demand side survives"
    );
    assert!(cell.promoted, "SALT promotes (the S21f success)");
    assert!(cell.bought_materially, "food is materially bought");
    assert!(
        cell.provenance_clean && cell.credited_seeded_minted == 0,
        "the S21f provenance: no seeded/minted bread"
    );
}

#[test]
fn overpowered_cushion_shows_demand_crowd_out() {
    // The OVERPOWERED-BRIDGE control: a large cushion (C=G=64) does NOT rescue money — it sates
    // the buyers out of the bread market while it lasts, then runs out and the full cull lands
    // anyway. No promotion, no surviving demand side — the demand-crowd-out / delayed-cull
    // failure (the opposite cause from the too-weak cull, the same no-money outcome).
    let cell = classify(7, &with_cushion(64), PROBE_TICKS);
    assert!(cell.conserved, "conservation must hold every tick");
    assert!(
        !cell.promoted,
        "the overpowered cushion does NOT promote SALT"
    );
    assert!(
        !cell.survived,
        "the overpowered cushion sates then runs dry — the demand side is culled anyway"
    );
    assert!(
        cell.provenance_clean,
        "even the overpowered cushion is never sold for SALT (the hard invariant)"
    );
}

// =========================================================================
// 5. Conservation + the live classification run
// =========================================================================

#[test]
fn demand_bridge_conserves_every_tick() {
    // Whole-system conservation EVERY tick for both new scenarios, across the cold-start cull /
    // survival, the emergency floor, and the lineage's births + old-age churn — with no minted
    // bread (the only bread is cultivated or emergency-produced, both conserved sources).
    for cfg in [
        SettlementConfig::frontier_demand_cushion(),
        SettlementConfig::frontier_emergency_provision(),
    ] {
        let bread = bread_good(&Settlement::generate(2, &cfg));
        let mut s = Settlement::generate(2, &cfg);
        for tick in 0..1_600u64 {
            let r = s.econ_tick();
            assert!(r.conserves(), "conservation broke at tick {tick}");
            assert_eq!(
                r.endowment_of(bread),
                0,
                "no minted food: bread is cultivated/emergency-produced, never minted (tick {tick})"
            );
        }
    }
}

#[test]
fn live_classification() {
    // The §7 live run: print the observed classification of BOTH slices (mirrors
    // `open_colony_mortality::live_classification`), so the arc is observable under
    // `--nocapture` and the report can cite the figures. The asserts only guard broken
    // invariants (conservation / minted food / extinction / the hard provenance bar); the
    // CLASSIFICATION line reports whichever outcome actually occurs.
    eprintln!(
        "=== S21h DEMAND-SIDE SURVIVAL BRIDGE — classification @ {PROBE_TICKS} ticks (seed 7) ==="
    );

    let cushion = classify(7, &SettlementConfig::frontier_demand_cushion(), PROBE_TICKS);
    eprintln!("--- S21h.0 consumed-only cushion (headline C=G=16) ---");
    eprintln!("  {cushion:?}");
    assert!(cushion.conserved, "cushion: conservation must hold");
    assert_eq!(cushion.bread_minted_max, 0, "cushion: no minted bread");
    assert!(!cushion.extinct, "cushion: no extinction");
    assert!(
        cushion.provenance_clean,
        "cushion: the hard invariant (no cushion bread sold for SALT) must hold"
    );
    if cushion.is_success() {
        eprintln!("  CLASSIFICATION: SUCCESS — the cushion threaded the knife-edge.");
    } else {
        eprintln!(
            "  CLASSIFICATION: KNIFE-EDGE FINDING — a one-time cushion cannot keep the demand \
             side both alive and hungry through the pre-money wait, so SALT never promotes \
             (survived={}, demanded={}, promoted={}). The cushion is eaten but never sold for \
             SALT.",
            cushion.survived, cushion.demanded, cushion.promoted,
        );
    }

    let emergency = classify(
        7,
        &SettlementConfig::frontier_emergency_provision(),
        PROBE_TICKS,
    );
    eprintln!("--- S21h.1 produced emergency self-provisioning (threshold 11) ---");
    eprintln!("  {emergency:?}");
    assert!(emergency.conserved, "emergency: conservation must hold");
    assert_eq!(emergency.bread_minted_max, 0, "emergency: no minted bread");
    assert!(!emergency.extinct, "emergency: no extinction");
    assert_eq!(
        emergency.credited_seeded_minted, 0,
        "emergency: no SeededMinted bread (provenance fully restored)"
    );
    if emergency.is_success() {
        eprintln!(
            "  CLASSIFICATION: SUCCESS — the produced near-critical emergency floor threads the \
             knife-edge: the non-lineage demand side survives AND still demands AND buys, SALT \
             promotes on SelfProduced bread, seeded_minted == 0. The open colony finally has \
             money + mortality together."
        );
    } else {
        eprintln!(
            "  CLASSIFICATION: FINDING — even the produced emergency floor did not thread it \
             (survived={}, demanded={}, promoted={}).",
            emergency.survived, emergency.demanded, emergency.promoted,
        );
    }
}
