use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ResponseUsage {
    pub(crate) input_tokens: Option<u64>,
    pub(crate) cached_input_tokens: Option<u64>,
    #[serde(default)]
    pub(crate) cache_write_input_tokens: Option<u64>,
    pub(crate) output_tokens: Option<u64>,
    pub(crate) reasoning_output_tokens: Option<u64>,
    pub(crate) total_tokens: Option<u64>,
}

impl ResponseUsage {
    pub(crate) fn from_response_raw(raw: &Value) -> Option<Self> {
        let usage = raw
            .get("response")
            .unwrap_or(raw)
            .get("usage")?
            .as_object()?;
        Some(Self {
            input_tokens: usage.get("input_tokens").and_then(Value::as_u64),
            cached_input_tokens: usage
                .get("cached_input_tokens")
                .and_then(Value::as_u64)
                .or_else(|| {
                    usage
                        .get("input_tokens_details")
                        .and_then(|details| details.get("cached_tokens"))
                        .and_then(Value::as_u64)
                }),
            cache_write_input_tokens: usage
                .get("cache_write_input_tokens")
                .and_then(Value::as_u64)
                .or_else(|| {
                    usage
                        .get("input_tokens_details")
                        .and_then(|details| details.get("cache_write_tokens"))
                        .and_then(Value::as_u64)
                }),
            output_tokens: usage.get("output_tokens").and_then(Value::as_u64),
            reasoning_output_tokens: usage
                .get("reasoning_output_tokens")
                .and_then(Value::as_u64)
                .or_else(|| {
                    usage
                        .get("output_tokens_details")
                        .and_then(|details| details.get("reasoning_tokens"))
                        .and_then(Value::as_u64)
                }),
            total_tokens: usage.get("total_tokens").and_then(Value::as_u64),
        })
    }

    fn uncached_input_tokens(self) -> Option<u64> {
        let input_tokens = self.input_tokens?;
        let cached_input_tokens = self.cached_input_tokens?.min(input_tokens);
        let remaining_input_tokens = input_tokens.saturating_sub(cached_input_tokens);
        let cache_write_input_tokens = self.cache_write_input_tokens?.min(remaining_input_tokens);
        Some(remaining_input_tokens.saturating_sub(cache_write_input_tokens))
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct ResponseUsageTotals {
    responses: u64,
    responses_with_usage: u64,
    input_tokens: UsageMetric,
    cached_input_tokens: UsageMetric,
    #[serde(default)]
    cache_write_input_tokens: UsageMetric,
    #[serde(default)]
    uncached_input_tokens: UsageMetric,
    output_tokens: UsageMetric,
    reasoning_output_tokens: UsageMetric,
    total_tokens: UsageMetric,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
struct UsageMetric {
    total: u64,
    reported_responses: u64,
}

impl ResponseUsageTotals {
    pub(crate) fn record(&mut self, usage: Option<ResponseUsage>) {
        self.responses += 1;
        let Some(usage) = usage else {
            return;
        };
        self.responses_with_usage += 1;
        self.input_tokens.record(usage.input_tokens);
        self.cached_input_tokens.record(usage.cached_input_tokens);
        self.cache_write_input_tokens
            .record(usage.cache_write_input_tokens);
        self.uncached_input_tokens
            .record(usage.uncached_input_tokens());
        self.output_tokens.record(usage.output_tokens);
        self.reasoning_output_tokens
            .record(usage.reasoning_output_tokens);
        self.total_tokens.record(usage.total_tokens);
    }

    pub(crate) fn to_json(&self) -> Value {
        json!({
            "source": "provider_responses",
            "completed_responses": self.responses,
            "responses_with_usage": self.responses_with_usage,
            "complete": self.responses > 0 && self.responses_with_usage == self.responses,
            "completeness_scope": "completed_response_payloads",
            "input_tokens": self.metric_json(self.input_tokens),
            "cached_input_tokens": self.metric_json(self.cached_input_tokens),
            "cache_write_input_tokens": self.metric_json(self.cache_write_input_tokens),
            "uncached_input_tokens": self.uncached_input_metric_json(),
            "output_tokens": self.metric_json(self.output_tokens),
            "reasoning_output_tokens": self.metric_json(self.reasoning_output_tokens),
            "total_tokens": self.metric_json(self.total_tokens),
        })
    }

    fn metric_json(&self, metric: UsageMetric) -> Value {
        json!({
            "total": (metric.reported_responses > 0).then_some(metric.total),
            "reported_responses": metric.reported_responses,
            "complete": self.responses > 0 && metric.reported_responses == self.responses,
        })
    }

    fn uncached_input_metric_json(&self) -> Value {
        let complete = self.responses > 0
            && self.input_tokens.reported_responses == self.responses
            && self.cached_input_tokens.reported_responses == self.responses
            && self.cache_write_input_tokens.reported_responses == self.responses
            && self.uncached_input_tokens.reported_responses == self.responses;
        json!({
            "total": complete.then_some(self.uncached_input_tokens.total),
            "reported_responses": self.uncached_input_tokens.reported_responses,
            "complete": complete,
            "derived_from": ["input_tokens", "cached_input_tokens", "cache_write_input_tokens"],
        })
    }
}

impl UsageMetric {
    fn record(&mut self, value: Option<u64>) {
        if let Some(value) = value {
            self.total = self.total.saturating_add(value);
            self.reported_responses += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_responses_usage_details_and_preserves_missing_fields() {
        let usage = ResponseUsage::from_response_raw(&json!({
            "response": {"usage": {
                "input_tokens": 120,
                "input_tokens_details": {"cached_tokens": 20, "cache_write_tokens": 10},
                "output_tokens": 42,
                "output_tokens_details": {"reasoning_tokens": 12},
                "total_tokens": 162
            }}
        }))
        .expect("usage");

        assert_eq!(usage.input_tokens, Some(120));
        assert_eq!(usage.cached_input_tokens, Some(20));
        assert_eq!(usage.cache_write_input_tokens, Some(10));
        assert_eq!(usage.uncached_input_tokens(), Some(90));
        assert_eq!(usage.output_tokens, Some(42));
        assert_eq!(usage.reasoning_output_tokens, Some(12));
        assert_eq!(usage.total_tokens, Some(162));

        let mut totals = ResponseUsageTotals::default();
        totals.record(Some(usage));
        totals.record(None);
        let json = totals.to_json();
        assert_eq!(json["completed_responses"], 2);
        assert_eq!(json["input_tokens"]["total"], 120);
        assert_eq!(json["input_tokens"]["complete"], false);
        assert_eq!(json["uncached_input_tokens"]["total"], Value::Null);
        assert_eq!(json["uncached_input_tokens"]["complete"], false);
        assert_eq!(json["completeness_scope"], "completed_response_payloads");
    }

    #[test]
    fn top_level_cache_write_usage_takes_precedence_over_detail_usage() {
        let usage = ResponseUsage::from_response_raw(&json!({
            "usage": {
                "input_tokens": 120,
                "cache_write_input_tokens": 7,
                "input_tokens_details": {"cache_write_tokens": 10}
            }
        }))
        .expect("usage");

        assert_eq!(usage.cache_write_input_tokens, Some(7));
        assert_eq!(usage.uncached_input_tokens(), None);
    }

    #[test]
    fn derives_uncached_input_from_complete_response_metrics_without_double_counting() {
        let mut totals = ResponseUsageTotals::default();
        totals.record(Some(ResponseUsage {
            input_tokens: Some(100),
            cached_input_tokens: Some(80),
            cache_write_input_tokens: Some(50),
            output_tokens: Some(20),
            reasoning_output_tokens: Some(10),
            total_tokens: Some(120),
        }));
        totals.record(Some(ResponseUsage {
            input_tokens: Some(50),
            cached_input_tokens: Some(10),
            cache_write_input_tokens: Some(5),
            output_tokens: Some(10),
            reasoning_output_tokens: Some(3),
            total_tokens: Some(60),
        }));

        let json = totals.to_json();
        assert_eq!(json["cache_write_input_tokens"]["total"], 55);
        assert_eq!(json["cache_write_input_tokens"]["complete"], true);
        assert_eq!(json["uncached_input_tokens"]["total"], 35);
        assert_eq!(json["uncached_input_tokens"]["complete"], true);
        assert_eq!(json["reasoning_output_tokens"]["total"], 13);
        assert_eq!(json["output_tokens"]["total"], 30);
    }
}
