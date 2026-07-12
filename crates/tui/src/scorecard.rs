//! Token / cache / cost scorecard (#3388).
//!
//! A release-gate view of an agent run's token economics: per-turn input /
//! output / cache-read tokens and cost, aggregate totals + cache-hit ratio, and
//! regression detection against a committed baseline. This is the measurement
//! layer the "token, cache, and context discipline" EPIC asks for — it makes a
//! cost/token regression visible instead of silently shipping.
//!
//! The core here is pure and offline: it turns already-recorded per-turn
//! [`Usage`] (captured on every turn, persisted in `TurnRecord`) into a
//! scorecard, reusing the existing pricing layer rather than reinventing cost
//! math. The `scorecard` subcommand is a thin I/O wrapper over this module.

use codewhale_config::pricing::{Currency, OfferingPricing, TokenUsage};
use serde::{Deserialize, Serialize};

use crate::config::ApiProvider;
use crate::models::Usage;
use crate::pricing::{calculate_turn_cost_estimate_for_provider, token_usage_for_pricing};
use crate::provider_lake::catalog_offering_for_model;

/// One turn's normalized token economics.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TurnScore {
    pub turn_id: String,
    /// Effective provider recorded for this turn. `None` means legacy or
    /// otherwise unknown provenance, so cost must remain unpriced.
    #[serde(default)]
    pub provider: Option<String>,
    pub model: String,
    /// Non-cached (billable) input tokens.
    pub input_tokens: u64,
    /// Output tokens, including reasoning output.
    pub output_tokens: u64,
    /// Cache-read (cache-hit) input tokens.
    pub cache_read_tokens: u64,
    pub cost_usd: f64,
    pub cost_cny: f64,
    /// True when provider provenance is missing/unknown or no authoritative USD
    /// pricing row exists: numeric cost stays 0 for compatibility, while this
    /// flag prevents it from being represented as a real zero-dollar charge.
    pub cost_unpriced: bool,
    /// Same availability marker for CNY. Most catalog offerings publish only
    /// USD, so their CNY value is unavailable rather than a real zero.
    #[serde(default)]
    pub cost_cny_unpriced: bool,
}

/// Aggregate metrics for a run. Serializes/deserializes as the baseline file.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ScorecardMetrics {
    pub turns: usize,
    /// Turns whose provider/model route could not be priced authoritatively in
    /// USD.
    /// Defaults to zero so existing baseline JSON remains readable.
    #[serde(default)]
    pub unpriced_turns: usize,
    /// Turns without authoritative CNY pricing.
    #[serde(default)]
    pub cny_unpriced_turns: usize,
    /// Whether every turn contributed authoritative USD pricing. Legacy
    /// baselines lack this field and therefore default to `false`, preventing
    /// comparisons against totals that may have been inferred from model ids
    /// alone.
    #[serde(default)]
    pub cost_complete: bool,
    /// Whether every turn contributed authoritative CNY pricing.
    #[serde(default)]
    pub cny_cost_complete: bool,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cache_read_tokens: u64,
    pub total_cost_usd: f64,
    pub total_cost_cny: f64,
    /// `cache_read / (input + cache_read)`; `0.0` when there are no input
    /// tokens. Higher is better (more of the prompt was served from cache).
    pub cache_hit_ratio: f64,
}

/// A metric that grew beyond the allowed threshold versus the baseline.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Regression {
    pub metric: String,
    pub baseline: f64,
    pub current: f64,
    /// Percent increase over baseline. `f64::INFINITY` when baseline was 0.
    pub pct_increase: f64,
}

/// Full scorecard: per-turn breakdown plus aggregates.
#[derive(Debug, Clone, Serialize)]
pub struct Scorecard {
    pub per_turn: Vec<TurnScore>,
    pub metrics: ScorecardMetrics,
}

/// One row of input to the scorecard: a turn id, the model that served it, and
/// the turn's recorded usage.
pub struct TurnInput<'a> {
    pub turn_id: String,
    pub provider: Option<&'a str>,
    pub model: String,
    pub usage: &'a Usage,
}

/// A recorded turn as read from a scorecard input file (a JSON array of these).
/// The base shape matches the per-turn data a `TurnEnd` hook emits. Recorders
/// and persisted runtime exports can add `provider` / `effective_provider`;
/// legacy model-only recordings remain readable but deliberately unpriced.
#[derive(Debug, Clone, Deserialize)]
pub struct RecordedTurn {
    #[serde(default)]
    pub turn_id: String,
    #[serde(default, alias = "effective_provider")]
    pub provider: Option<String>,
    pub model: String,
    pub usage: Usage,
}

#[derive(Debug, Clone, Copy, Default)]
struct AvailableCost {
    usd: Option<f64>,
    cny: Option<f64>,
}

fn provider_scoped_cost(
    provider: ApiProvider,
    model: &str,
    usage: &Usage,
    token_usage: &TokenUsage,
) -> AvailableCost {
    let Some(offering) = catalog_offering_for_model(provider, model) else {
        return AvailableCost::default();
    };

    // Direct DeepSeek routes retain the repository's hand-sourced, time-aware
    // USD+CNY table. Requiring an exact provider offering first prevents a
    // foreign wire id from matching merely because its text contains
    // "deepseek".
    if matches!(
        provider,
        ApiProvider::Deepseek | ApiProvider::DeepseekCN | ApiProvider::DeepseekAnthropic
    ) {
        return calculate_turn_cost_estimate_for_provider(provider, model, usage).map_or_else(
            AvailableCost::default,
            |cost| AvailableCost {
                usd: Some(cost.usd),
                cny: Some(cost.cny),
            },
        );
    }

    let Some(pricing) = OfferingPricing::from_catalog_offering(&offering) else {
        return AvailableCost::default();
    };
    let Some(amount) = pricing.estimate_cost(token_usage) else {
        return AvailableCost::default();
    };
    match &pricing.currency {
        Currency::Usd => AvailableCost {
            usd: Some(amount),
            cny: None,
        },
        Currency::Cny => AvailableCost {
            usd: None,
            cny: Some(amount),
        },
        Currency::Other(_) => AvailableCost::default(),
    }
}

impl Scorecard {
    /// Build a scorecard from recorded per-turn usage. Pure + offline; cost is
    /// computed via the shared pricing layer (`None` pricing → unpriced, 0 cost).
    #[must_use]
    pub fn from_turns(turns: &[TurnInput<'_>]) -> Self {
        let mut per_turn = Vec::with_capacity(turns.len());
        let mut metrics = ScorecardMetrics::default();

        for turn in turns {
            // Normalize provider usage into canonical billable classes once.
            let classes = token_usage_for_pricing(turn.usage);
            let provider = turn
                .provider
                .map(str::trim)
                .filter(|value| !value.is_empty());
            let cost = provider
                .and_then(ApiProvider::parse)
                .map_or_else(AvailableCost::default, |provider| {
                    provider_scoped_cost(provider, &turn.model, turn.usage, &classes)
                });
            let cost_unpriced = cost.usd.is_none();
            let cost_cny_unpriced = cost.cny.is_none();
            let cost_usd = cost.usd.unwrap_or(0.0);
            let cost_cny = cost.cny.unwrap_or(0.0);

            metrics.turns += 1;
            metrics.unpriced_turns += usize::from(cost_unpriced);
            metrics.cny_unpriced_turns += usize::from(cost_cny_unpriced);
            metrics.total_input_tokens += classes.input;
            metrics.total_output_tokens += classes.output;
            metrics.total_cache_read_tokens += classes.cache_read;
            metrics.total_cost_usd += cost_usd;
            metrics.total_cost_cny += cost_cny;

            per_turn.push(TurnScore {
                turn_id: turn.turn_id.clone(),
                provider: provider.map(str::to_string),
                model: turn.model.clone(),
                input_tokens: classes.input,
                output_tokens: classes.output,
                cache_read_tokens: classes.cache_read,
                cost_usd,
                cost_cny,
                cost_unpriced,
                cost_cny_unpriced,
            });
        }

        let cacheable = metrics.total_input_tokens + metrics.total_cache_read_tokens;
        metrics.cache_hit_ratio = if cacheable > 0 {
            metrics.total_cache_read_tokens as f64 / cacheable as f64
        } else {
            0.0
        };
        metrics.cost_complete = metrics.unpriced_turns == 0;
        metrics.cny_cost_complete = metrics.cny_unpriced_turns == 0;

        Self { per_turn, metrics }
    }

    /// Render a compact human-readable summary (used for non-JSON output).
    #[must_use]
    pub fn to_summary(&self) -> String {
        let m = &self.metrics;
        let mut out = String::new();
        out.push_str("Token / cache / cost scorecard\n");
        out.push_str(&format!("turns: {}\n", m.turns));
        out.push_str(&format!(
            "input_tokens: {}  output_tokens: {}  cache_read_tokens: {}\n",
            m.total_input_tokens, m.total_output_tokens, m.total_cache_read_tokens
        ));
        out.push_str(&format!(
            "cache_hit_ratio: {:.1}%\n",
            m.cache_hit_ratio * 100.0
        ));
        append_currency_summary(
            &mut out,
            "cost_usd",
            "priced_cost_subtotal_usd",
            "$",
            m.total_cost_usd,
            m.unpriced_turns,
            m.turns,
        );
        append_currency_summary(
            &mut out,
            "cost_cny",
            "priced_cost_subtotal_cny",
            "¥",
            m.total_cost_cny,
            m.cny_unpriced_turns,
            m.turns,
        );
        if m.unpriced_turns > 0 {
            out.push_str(&format!(
                "note: {} turn(s) had missing/unknown provider provenance or no authoritative USD pricing row; their USD cost is unavailable and excluded.\n",
                m.unpriced_turns
            ));
        }
        if m.cny_unpriced_turns > 0 {
            out.push_str(&format!(
                "note: {} turn(s) had no authoritative CNY pricing row; their CNY cost is unavailable and excluded.\n",
                m.cny_unpriced_turns
            ));
        }
        out
    }
}

fn append_currency_summary(
    out: &mut String,
    complete_label: &str,
    subtotal_label: &str,
    symbol: &str,
    total: f64,
    unpriced_turns: usize,
    turns: usize,
) {
    if unpriced_turns == 0 {
        out.push_str(&format!("{complete_label}: {symbol}{total:.4}\n"));
    } else if unpriced_turns == turns {
        out.push_str(&format!("{complete_label}: unavailable\n"));
    } else {
        out.push_str(&format!("{subtotal_label}: {symbol}{total:.4}\n"));
    }
}

impl ScorecardMetrics {
    /// Flag metrics that grew more than `threshold_pct` over `baseline`. Cost
    /// and token counts are "lower is better", so only *increases* are
    /// regressions. (Cache-hit ratio is the opposite, reported separately.)
    #[must_use]
    pub fn regressions_against(
        &self,
        baseline: &ScorecardMetrics,
        threshold_pct: f64,
    ) -> Vec<Regression> {
        let mut out = Vec::new();
        // A partial/unknown subtotal is not comparable to a complete baseline.
        if self.cost_complete && baseline.cost_complete {
            push_regression(
                &mut out,
                "total_cost_usd",
                baseline.total_cost_usd,
                self.total_cost_usd,
                threshold_pct,
            );
        }
        push_regression(
            &mut out,
            "total_input_tokens",
            baseline.total_input_tokens as f64,
            self.total_input_tokens as f64,
            threshold_pct,
        );
        push_regression(
            &mut out,
            "total_output_tokens",
            baseline.total_output_tokens as f64,
            self.total_output_tokens as f64,
            threshold_pct,
        );
        // Cache-hit ratio regresses when it *drops*; express the drop as a
        // positive percentage so it reads like the others.
        if baseline.cache_hit_ratio > 0.0 {
            let drop_pct = (baseline.cache_hit_ratio - self.cache_hit_ratio)
                / baseline.cache_hit_ratio
                * 100.0;
            if drop_pct > threshold_pct {
                out.push(Regression {
                    metric: "cache_hit_ratio_drop".to_string(),
                    baseline: baseline.cache_hit_ratio,
                    current: self.cache_hit_ratio,
                    pct_increase: drop_pct,
                });
            }
        }
        out
    }
}

fn push_regression(
    out: &mut Vec<Regression>,
    metric: &str,
    base: f64,
    cur: f64,
    threshold_pct: f64,
) {
    if base > 0.0 {
        let pct = (cur - base) / base * 100.0;
        if pct > threshold_pct {
            out.push(Regression {
                metric: metric.to_string(),
                baseline: base,
                current: cur,
                pct_increase: pct,
            });
        }
    } else if cur > 0.0 {
        out.push(Regression {
            metric: metric.to_string(),
            baseline: base,
            current: cur,
            pct_increase: f64::INFINITY,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usage(input: u32, output: u32, cache_hit: u32) -> Usage {
        Usage {
            input_tokens: input,
            output_tokens: output,
            prompt_cache_hit_tokens: Some(cache_hit),
            ..Default::default()
        }
    }

    #[test]
    fn aggregates_tokens_and_cache_hit_ratio_independent_of_pricing() {
        // input_tokens includes cache hits; token_usage_for_pricing splits them:
        // non-cached input = 1000-200 = 800, cache_read = 200.
        let u1 = usage(1000, 500, 200);
        let u2 = usage(2000, 100, 800); // non-cached = 1200, cache_read = 800
        let turns = [
            TurnInput {
                turn_id: "t1".into(),
                provider: None,
                model: "unpriced-x".into(),
                usage: &u1,
            },
            TurnInput {
                turn_id: "t2".into(),
                provider: None,
                model: "unpriced-x".into(),
                usage: &u2,
            },
        ];
        let card = Scorecard::from_turns(&turns);

        assert_eq!(card.metrics.turns, 2);
        assert_eq!(card.metrics.total_input_tokens, 800 + 1200);
        assert_eq!(card.metrics.total_output_tokens, 600); // 500 + 100
        assert_eq!(card.metrics.total_cache_read_tokens, 1000); // 200 + 800
        assert_eq!(card.metrics.unpriced_turns, 2);
        // cache_read / (input + cache_read) = 1000 / (2000 + 1000)
        let expected = 1000.0 / 3000.0;
        assert!((card.metrics.cache_hit_ratio - expected).abs() < 1e-9);
    }

    #[test]
    fn unknown_model_is_marked_unpriced_with_zero_cost() {
        let u = usage(1000, 500, 0);
        let turns = [TurnInput {
            turn_id: "t1".into(),
            provider: Some("openai"),
            model: "definitely-not-a-real-model".into(),
            usage: &u,
        }];
        let card = Scorecard::from_turns(&turns);
        assert!(card.per_turn[0].cost_unpriced);
        assert_eq!(card.per_turn[0].cost_usd, 0.0);
        assert_eq!(card.metrics.total_cost_usd, 0.0);
        assert!(card.to_summary().contains("cost_usd: unavailable"));
    }

    #[test]
    fn same_model_is_priced_only_for_its_authoritative_provider_route() {
        let u = usage(1000, 500, 0);
        let turns = [
            TurnInput {
                turn_id: "api".into(),
                provider: Some("openai"),
                model: "gpt-5.5".into(),
                usage: &u,
            },
            TurnInput {
                turn_id: "oauth".into(),
                provider: Some("openai-codex"),
                model: "gpt-5.5".into(),
                usage: &u,
            },
            TurnInput {
                turn_id: "local".into(),
                provider: Some("ollama"),
                model: "gpt-5.5".into(),
                usage: &u,
            },
        ];

        let card = Scorecard::from_turns(&turns);

        assert!(!card.per_turn[0].cost_unpriced);
        assert!(card.per_turn[0].cost_usd > 0.0);
        assert!(card.per_turn[1].cost_unpriced);
        assert_eq!(card.per_turn[1].cost_usd, 0.0);
        assert!(card.per_turn[2].cost_unpriced);
        assert_eq!(card.per_turn[2].cost_usd, 0.0);
        assert_eq!(card.metrics.unpriced_turns, 2);
        assert_eq!(card.metrics.cny_unpriced_turns, 3);
        assert!(!card.metrics.cost_complete);
        assert!(!card.metrics.cny_cost_complete);
        assert!(card.to_summary().contains("priced_cost_subtotal_usd"));
        assert!(card.to_summary().contains("cost_cny: unavailable"));

        let json = serde_json::to_value(&card).expect("serialize scorecard");
        assert_eq!(json["per_turn"][0]["provider"], "openai");
        assert_eq!(json["per_turn"][1]["provider"], "openai-codex");
        assert_eq!(json["per_turn"][2]["provider"], "ollama");
        assert_eq!(json["metrics"]["unpriced_turns"], 2);
        assert_eq!(json["metrics"]["cost_complete"], false);
        assert_eq!(json["metrics"]["cny_cost_complete"], false);
    }

    #[test]
    fn known_zero_usage_is_zero_cost_not_unavailable() {
        let u = usage(0, 0, 0);
        let turns = [TurnInput {
            turn_id: "zero".into(),
            provider: Some("openai"),
            model: "gpt-5.5".into(),
            usage: &u,
        }];

        let card = Scorecard::from_turns(&turns);

        assert!(!card.per_turn[0].cost_unpriced);
        assert_eq!(card.per_turn[0].cost_usd, 0.0);
        assert!(card.per_turn[0].cost_cny_unpriced);
        assert_eq!(card.metrics.unpriced_turns, 0);
        assert_eq!(card.metrics.cny_unpriced_turns, 1);
        assert!(card.metrics.cost_complete);
        assert!(!card.metrics.cny_cost_complete);
        assert!(card.to_summary().contains("cost_usd: $0.0000"));
        assert!(card.to_summary().contains("cost_cny: unavailable"));
    }

    #[test]
    fn direct_deepseek_route_keeps_authoritative_dual_currency_pricing() {
        let u = usage(1000, 500, 0);
        let turns = [TurnInput {
            turn_id: "deepseek".into(),
            provider: Some("deepseek"),
            model: "deepseek-v4-pro".into(),
            usage: &u,
        }];

        let card = Scorecard::from_turns(&turns);

        assert!(!card.per_turn[0].cost_unpriced);
        assert!(!card.per_turn[0].cost_cny_unpriced);
        assert!(card.per_turn[0].cost_usd > 0.0);
        assert!(card.per_turn[0].cost_cny > 0.0);
        assert!(card.metrics.cost_complete);
        assert!(card.metrics.cny_cost_complete);
    }

    #[test]
    fn legacy_model_only_record_is_readable_but_unpriced() {
        let recorded: RecordedTurn = serde_json::from_value(serde_json::json!({
            "turn_id": "legacy",
            "model": "gpt-5.5",
            "usage": {
                "input_tokens": 0,
                "output_tokens": 0
            }
        }))
        .expect("parse legacy scorecard turn");
        assert_eq!(recorded.provider, None);

        let turns = [TurnInput {
            turn_id: recorded.turn_id.clone(),
            provider: recorded.provider.as_deref(),
            model: recorded.model.clone(),
            usage: &recorded.usage,
        }];
        let card = Scorecard::from_turns(&turns);

        assert!(card.per_turn[0].cost_unpriced);
        assert_eq!(card.per_turn[0].cost_usd, 0.0);
        assert_eq!(card.metrics.unpriced_turns, 1);
        assert!(card.to_summary().contains("cost_usd: unavailable"));
    }

    #[test]
    fn recorded_turn_accepts_effective_provider_alias() {
        let recorded: RecordedTurn = serde_json::from_value(serde_json::json!({
            "turn_id": "runtime",
            "effective_provider": "openai-codex",
            "model": "gpt-5.5",
            "usage": {
                "input_tokens": 1,
                "output_tokens": 1
            }
        }))
        .expect("parse runtime scorecard turn");

        assert_eq!(recorded.provider.as_deref(), Some("openai-codex"));
    }

    #[test]
    fn blank_unknown_and_custom_providers_fail_closed_as_unpriced() {
        let u = usage(1000, 500, 0);
        let turns = [
            TurnInput {
                turn_id: "blank".into(),
                provider: Some("   "),
                model: "gpt-5.5".into(),
                usage: &u,
            },
            TurnInput {
                turn_id: "named-custom".into(),
                provider: Some("my-openai-proxy"),
                model: "gpt-5.5".into(),
                usage: &u,
            },
            TurnInput {
                turn_id: "generic-custom".into(),
                provider: Some("custom"),
                model: "gpt-5.5".into(),
                usage: &u,
            },
        ];

        let card = Scorecard::from_turns(&turns);

        assert_eq!(card.per_turn[0].provider, None);
        assert_eq!(
            card.per_turn[1].provider.as_deref(),
            Some("my-openai-proxy")
        );
        assert_eq!(card.per_turn[2].provider.as_deref(), Some("custom"));
        assert!(card.per_turn.iter().all(|turn| turn.cost_unpriced));
        assert_eq!(card.metrics.unpriced_turns, 3);
        assert!(!card.metrics.cost_complete);
        assert!(card.to_summary().contains("cost_usd: unavailable"));
    }

    #[test]
    fn regression_flags_cost_and_token_increases_over_threshold() {
        let baseline = ScorecardMetrics {
            turns: 1,
            unpriced_turns: 0,
            cny_unpriced_turns: 0,
            cost_complete: true,
            cny_cost_complete: true,
            total_input_tokens: 1000,
            total_output_tokens: 1000,
            total_cache_read_tokens: 0,
            total_cost_usd: 0.10,
            total_cost_cny: 0.7,
            cache_hit_ratio: 0.5,
        };
        let current = ScorecardMetrics {
            total_cost_usd: 0.20,      // +100% → regression
            total_input_tokens: 1010,  // +1% → under 5% threshold, no regression
            total_output_tokens: 2000, // +100% → regression
            cache_hit_ratio: 0.5,      // unchanged
            ..baseline.clone()
        };
        let regs = current.regressions_against(&baseline, 5.0);
        let names: Vec<&str> = regs.iter().map(|r| r.metric.as_str()).collect();
        assert!(names.contains(&"total_cost_usd"));
        assert!(names.contains(&"total_output_tokens"));
        assert!(!names.contains(&"total_input_tokens")); // under threshold
    }

    #[test]
    fn regression_skips_cost_when_either_scorecard_is_incomplete() {
        let baseline = ScorecardMetrics {
            cost_complete: true,
            total_cost_usd: 0.10,
            ..Default::default()
        };
        let current = ScorecardMetrics {
            turns: 1,
            unpriced_turns: 1,
            total_cost_usd: 0.20,
            ..Default::default()
        };

        let regs = current.regressions_against(&baseline, 5.0);
        assert!(!regs.iter().any(|r| r.metric == "total_cost_usd"));
    }

    #[test]
    fn legacy_baseline_is_readable_but_cost_is_not_comparable() {
        let baseline: ScorecardMetrics = serde_json::from_value(serde_json::json!({
            "turns": 1,
            "total_input_tokens": 10,
            "total_output_tokens": 5,
            "total_cache_read_tokens": 0,
            "total_cost_usd": 0.10,
            "total_cost_cny": 0.0,
            "cache_hit_ratio": 0.0
        }))
        .expect("parse legacy scorecard baseline");
        assert!(!baseline.cost_complete);

        let current = ScorecardMetrics {
            cost_complete: true,
            total_cost_usd: 0.20,
            total_input_tokens: 10,
            total_output_tokens: 5,
            ..Default::default()
        };
        let regs = current.regressions_against(&baseline, 5.0);
        assert!(!regs.iter().any(|r| r.metric == "total_cost_usd"));
    }

    #[test]
    fn regression_flags_cache_hit_ratio_drop() {
        let baseline = ScorecardMetrics {
            cache_hit_ratio: 0.80,
            ..Default::default()
        };
        let current = ScorecardMetrics {
            cache_hit_ratio: 0.40,
            ..Default::default()
        };
        let regs = current.regressions_against(&baseline, 10.0);
        assert!(regs.iter().any(|r| r.metric == "cache_hit_ratio_drop"));
    }

    #[test]
    fn no_regressions_when_within_threshold() {
        let baseline = ScorecardMetrics {
            total_cost_usd: 1.0,
            total_input_tokens: 1000,
            total_output_tokens: 1000,
            cache_hit_ratio: 0.5,
            ..Default::default()
        };
        let current = baseline.clone();
        assert!(current.regressions_against(&baseline, 5.0).is_empty());
    }
}
