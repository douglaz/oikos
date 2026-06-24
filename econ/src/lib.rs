//! praxsim-core — ordinal Crusoe, designated-gold spot, and time-market kernels.
//!
//! Implementation specs: `impl-01.md`, `impl-02.md`, and `impl-03.md`
//! (in the parent directory).
//! Pure std only; no external crates; ordinal ranks and integer quantities only.

pub mod agent;
pub mod agio;
pub mod arena;
pub mod bank;
pub mod barter;
pub mod bundle;
pub mod cantillon;
pub mod capital;
pub mod command;
pub mod emergence;
pub mod expect;
pub mod factor;
pub mod good;
pub mod issuer;
pub mod ledger;
pub mod market;
pub mod marketability;
pub mod menger;
pub mod metrics;
pub mod money;
pub mod project;
pub mod purpose;
pub mod record;
pub mod registry;
pub mod report;
pub mod rng;
pub mod scenario;
pub mod shadow;
pub mod sim;
pub mod society;
pub mod sweep;
pub mod timemarket;
pub mod worldgen;

#[cfg(test)]
mod tests {
    #[test]
    fn m4_source_gate_keeps_metrics_out_of_decisions() {
        let decision_patterns = m4_decision_patterns();

        for (module, source) in m4_decision_sources() {
            assert_no_patterns(
                module,
                source,
                decision_patterns.as_slice(),
                "M4 measurement pattern",
            );
        }

        let society_source = include_str!("society.rs");
        for pattern in m4_output_patterns() {
            assert!(
                !society_source.contains(&pattern),
                "society.rs may capture observations, but must not branch on M4 output pattern {pattern}"
            );
        }
    }

    #[test]
    fn m5_source_gate_keeps_v2_out_of_banking_modules() {
        let v2_import_patterns = [
            concat!("crate", "::", "barter").to_string(),
            concat!("barter", "::").to_string(),
            concat!("crate", "::", "menger").to_string(),
            concat!("menger", "::").to_string(),
        ];
        for (module, source) in m5_banking_sources() {
            assert_no_patterns(
                module,
                source,
                &v2_import_patterns,
                "M5 barter or saleability import",
            );
        }

        let saleability_patterns = [
            concat!("Saleability", "Tracker").to_string(),
            concat!("Saleability", "Snapshot").to_string(),
            concat!("Mengerian", "Emergence").to_string(),
            concat!("Mengerian", "Config").to_string(),
        ];
        for (module, source) in m5_banking_sources() {
            assert_no_patterns(module, source, &saleability_patterns, "M5 saleability type");
        }
    }

    #[test]
    fn m6_source_gate_keeps_reporting_out_of_v2_decisions() {
        let reporting_patterns = [
            concat!("crate", "::", "report").to_string(),
            concat!("report", "::").to_string(),
            concat!("crate", "::", "metrics").to_string(),
            concat!("metrics", "::").to_string(),
            concat!("V2", "Record").to_string(),
            concat!("M4", "Record").to_string(),
            concat!("Sweep", "Record").to_string(),
        ];

        for (module, source) in [
            ("agent.rs", include_str!("agent.rs")),
            ("barter.rs", include_str!("barter.rs")),
            ("menger.rs", include_str!("menger.rs")),
        ] {
            assert_no_patterns(
                module,
                source,
                &reporting_patterns,
                "M6 reporting or metrics feedback",
            );
        }

        let society_source = include_str!("society.rs");
        let promotion_patterns = [
            concat!("crate", "::", "report").to_string(),
            concat!("report", "::").to_string(),
            concat!("crate", "::", "metrics").to_string(),
            concat!("metrics", "::").to_string(),
            concat!("Metric", "Observation").to_string(),
            concat!("M4", "Record").to_string(),
            concat!("Sweep", "Record").to_string(),
        ];
        for signature in [
            "fn generate_direct_barter_offers(",
            "fn generate_indirect_barter_offers(",
            "fn v2_promotion_candidate_after_tick(",
            "fn promote_v2_money_good(",
        ] {
            assert_no_patterns(
                signature,
                function_source(society_source, signature),
                &promotion_patterns,
                "M6 reporting or metrics feedback",
            );
        }
    }

    #[test]
    fn m18_source_gate_keeps_worldgen_and_emergence_out_of_decisions() {
        let harness_import_patterns = [
            concat!("crate", "::", "worldgen").to_string(),
            concat!("worldgen", "::").to_string(),
            concat!("crate", "::", "emergence").to_string(),
            concat!("emergence", "::").to_string(),
        ];
        for (module, source) in m4_decision_sources() {
            assert_no_patterns(
                module,
                source,
                &harness_import_patterns,
                "M18 worldgen or emergence import",
            );
        }
    }

    fn m4_decision_patterns() -> Vec<String> {
        let mut patterns = vec![
            concat!("metrics", "::").to_string(),
            concat!("crate", "::", "metrics").to_string(),
            concat!("sweep", "::").to_string(),
            concat!("crate", "::", "sweep").to_string(),
        ];
        patterns.extend(m4_output_patterns());
        patterns
    }

    fn m4_output_patterns() -> Vec<String> {
        vec![
            concat!("M4", "Record").to_string(),
            concat!("Sweep", "Record").to_string(),
            concat!("real_wealth_", "gini").to_string(),
            concat!("lor", "enz").to_string(),
            concat!("idle_labor_", "bps").to_string(),
            concat!("sector_price_", "dispersion").to_string(),
        ]
    }

    fn m4_decision_sources() -> [(&'static str, &'static str); 11] {
        [
            ("agent.rs", include_str!("agent.rs")),
            ("agio.rs", include_str!("agio.rs")),
            ("bank.rs", include_str!("bank.rs")),
            ("barter.rs", include_str!("barter.rs")),
            ("bundle.rs", include_str!("bundle.rs")),
            ("capital.rs", include_str!("capital.rs")),
            ("factor.rs", include_str!("factor.rs")),
            ("market.rs", include_str!("market.rs")),
            ("menger.rs", include_str!("menger.rs")),
            ("timemarket.rs", include_str!("timemarket.rs")),
            ("issuer.rs", include_str!("issuer.rs")),
        ]
    }

    fn m5_banking_sources() -> [(&'static str, &'static str); 5] {
        [
            ("bank.rs", include_str!("bank.rs")),
            ("issuer.rs", include_str!("issuer.rs")),
            ("ledger.rs", include_str!("ledger.rs")),
            ("timemarket.rs", include_str!("timemarket.rs")),
            ("shadow.rs", include_str!("shadow.rs")),
        ]
    }

    fn assert_no_patterns(module: &str, source: &str, patterns: &[String], label: &str) {
        for pattern in patterns {
            assert!(
                !source.contains(pattern.as_str()),
                "{module} must not contain {label} {pattern}"
            );
        }
    }

    fn function_source<'a>(source: &'a str, signature: &str) -> &'a str {
        let start = source
            .find(signature)
            .unwrap_or_else(|| panic!("missing function signature {signature}"));
        let rest = &source[start + signature.len()..];
        let end = rest
            .find("\n    fn ")
            .or_else(|| rest.find("\n    pub fn "))
            .unwrap_or(rest.len());
        &source[start..start + signature.len() + end]
    }
}
