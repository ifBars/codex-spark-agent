use anyhow::{Context, Result};
use reqwest::header::{ACCEPT, AUTHORIZATION, HeaderValue, USER_AGENT};
use serde::Serialize;
use serde_json::Value;

use crate::auth::AuthTokens;

pub(crate) const CODEX_USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";
const PRICING_SOURCE_URL: &str = "https://learn.chatgpt.com/docs/pricing";
const PRICING_CHECKED_ON: &str = "2026-08-01";

#[derive(Debug, Clone, Serialize, PartialEq)]
pub(crate) struct AccountUsage {
    pub(crate) source: UsageSource,
    pub(crate) fetched_at_unix_seconds: Option<u64>,
    pub(crate) plan_type: Option<String>,
    pub(crate) rate_limits: RateLimits,
    pub(crate) credits: Option<Credits>,
    pub(crate) spend_control_reached: Option<bool>,
    pub(crate) individual_spend_control: Option<IndividualSpendControl>,
    pub(crate) additional_rate_limits: Vec<AdditionalRateLimit>,
    pub(crate) rate_limit_reached_type: Option<String>,
    pub(crate) rate_limit_reset_credits_available: Option<i64>,
    pub(crate) pricing: PricingAvailability,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub(crate) struct UsageSource {
    pub(crate) kind: &'static str,
    pub(crate) url: &'static str,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub(crate) struct RateLimits {
    pub(crate) primary_window: Option<RateLimitWindow>,
    pub(crate) secondary_window: Option<RateLimitWindow>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub(crate) struct RateLimitWindow {
    pub(crate) used_percent: Option<f64>,
    pub(crate) limit_window_seconds: Option<u64>,
    pub(crate) reset_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub(crate) struct Credits {
    pub(crate) has_credits: Option<bool>,
    pub(crate) unlimited: Option<bool>,
    pub(crate) balance: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub(crate) struct IndividualSpendControl {
    pub(crate) source: Option<String>,
    pub(crate) used_percent: Option<f64>,
    pub(crate) remaining_percent: Option<f64>,
    pub(crate) reset_after_seconds: Option<u64>,
    pub(crate) reset_at: Option<i64>,
    pub(crate) limit: Option<String>,
    pub(crate) used: Option<String>,
    pub(crate) remaining: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub(crate) struct AdditionalRateLimit {
    pub(crate) limit_name: Option<String>,
    pub(crate) metered_feature: Option<String>,
    pub(crate) rate_limits: RateLimits,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub(crate) struct PricingAvailability {
    pub(crate) model: &'static str,
    pub(crate) billing_basis: &'static str,
    pub(crate) api_token_price_usd: Option<f64>,
    pub(crate) status: &'static str,
    pub(crate) reason: &'static str,
    pub(crate) source: &'static str,
    pub(crate) checked_on: &'static str,
}

impl AccountUsage {
    fn pricing() -> PricingAvailability {
        PricingAvailability {
            model: "gpt-5.3-codex-spark",
            billing_basis: "chatgpt_plan_quota",
            api_token_price_usd: None,
            status: "unavailable",
            reason: "no_public_api_price",
            source: PRICING_SOURCE_URL,
            checked_on: PRICING_CHECKED_ON,
        }
    }
}

pub(crate) fn build_usage_request(
    client: &reqwest::Client,
    auth: &AuthTokens,
) -> Result<reqwest::Request> {
    let mut authorization = HeaderValue::from_str(&format!("Bearer {}", auth.access_token))
        .context("failed to build the Codex account-usage authorization header")?;
    authorization.set_sensitive(true);
    let mut request = client
        .get(CODEX_USAGE_URL)
        .header(ACCEPT, HeaderValue::from_static("application/json"))
        .header(
            USER_AGENT,
            HeaderValue::from_static(concat!("spark/", env!("CARGO_PKG_VERSION"))),
        )
        .header(AUTHORIZATION, authorization);

    if let Some(account_id) = auth
        .account_id
        .as_deref()
        .filter(|id| !id.trim().is_empty())
    {
        let mut account_id = HeaderValue::from_str(account_id)
            .context("failed to build the Codex account-usage account header")?;
        account_id.set_sensitive(true);
        request = request.header("ChatGPT-Account-Id", account_id);
    }

    request
        .build()
        .context("failed to build the Codex account-usage request")
}

pub(crate) async fn fetch_usage(auth: &AuthTokens) -> Result<AccountUsage> {
    let client = reqwest::Client::new();
    let request = build_usage_request(&client, auth)?;
    let response = client.execute(request).await.context(
        "could not reach the Codex account-usage service; check your network connection and try again",
    )?;
    let status = response.status();
    if !status.is_success() {
        let guidance = if status.as_u16() == 401 || status.as_u16() == 403 {
            "authentication was rejected; run `spark login` and try again"
        } else {
            "the account-usage service did not accept this request; try again later"
        };
        anyhow::bail!("Codex account-usage request failed with HTTP {status}: {guidance}");
    }

    let body = response
        .json::<Value>()
        .await
        .context("Codex account-usage service returned invalid JSON")?;
    let mut usage = parse_usage(&body)?;
    usage.fetched_at_unix_seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs());
    Ok(usage)
}

pub(crate) fn parse_usage(body: &Value) -> Result<AccountUsage> {
    let root = body
        .as_object()
        .context("Codex account-usage service returned a non-object JSON payload")?;
    let rate_limit = root.get("rate_limit").and_then(Value::as_object);
    let primary_window = rate_limit
        .and_then(|value| value.get("primary_window"))
        .and_then(parse_window);
    let secondary_window = rate_limit
        .and_then(|value| value.get("secondary_window"))
        .and_then(parse_window);
    let additional_rate_limits = root
        .get("additional_rate_limits")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(parse_additional_rate_limit)
                .collect()
        })
        .unwrap_or_default();

    Ok(AccountUsage {
        source: UsageSource {
            kind: "chatgpt_codex_usage",
            url: CODEX_USAGE_URL,
        },
        fetched_at_unix_seconds: None,
        plan_type: root
            .get("plan_type")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        rate_limits: RateLimits {
            primary_window,
            secondary_window,
        },
        credits: root.get("credits").and_then(parse_credits),
        spend_control_reached: root
            .get("spend_control")
            .and_then(|value| value.get("reached"))
            .and_then(Value::as_bool),
        individual_spend_control: root
            .get("spend_control")
            .and_then(|value| value.get("individual_limit"))
            .and_then(parse_individual_spend_control),
        additional_rate_limits,
        rate_limit_reached_type: root
            .get("rate_limit_reached_type")
            .and_then(|value| value.get("type").or(Some(value)))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        rate_limit_reset_credits_available: root
            .get("rate_limit_reset_credits")
            .and_then(|value| value.get("available_count"))
            .and_then(Value::as_i64),
        pricing: AccountUsage::pricing(),
    })
}

pub(crate) fn render_human(usage: &AccountUsage) -> String {
    let mut lines = vec![format!("Account usage source: {}", usage.source.url)];
    lines.push(format!(
        "Fetched at (Unix seconds): {}",
        option_unsigned(usage.fetched_at_unix_seconds)
    ));
    lines.push(format!(
        "Plan: {}",
        usage.plan_type.as_deref().unwrap_or("not reported")
    ));
    render_window(
        &mut lines,
        "Primary quota",
        usage.rate_limits.primary_window.as_ref(),
    );
    render_window(
        &mut lines,
        "Secondary quota",
        usage.rate_limits.secondary_window.as_ref(),
    );
    if let Some(credits) = &usage.credits {
        lines.push(format!(
            "Credits: has_credits={} unlimited={} balance={}",
            option_bool(credits.has_credits),
            option_bool(credits.unlimited),
            option_text(credits.balance.as_deref()),
        ));
    }
    if let Some(reached) = usage.spend_control_reached {
        lines.push(format!("Spend control reached: {reached}"));
    }
    if let Some(control) = &usage.individual_spend_control {
        lines.push(format!(
            "Individual spend control: source={} used_percent={} remaining_percent={} limit={} used={} remaining={} reset_after_seconds={} reset_at={}",
            option_text(control.source.as_deref()),
            option_percent(control.used_percent),
            option_percent(control.remaining_percent),
            option_text(control.limit.as_deref()),
            option_text(control.used.as_deref()),
            option_text(control.remaining.as_deref()),
            option_unsigned(control.reset_after_seconds),
            option_integer(control.reset_at),
        ));
    }
    for limit in &usage.additional_rate_limits {
        let name = limit.limit_name.as_deref().unwrap_or("unnamed");
        let feature = limit.metered_feature.as_deref().unwrap_or("not reported");
        lines.push(format!("Additional quota ({name}, {feature}):"));
        render_window(
            &mut lines,
            "  Primary",
            limit.rate_limits.primary_window.as_ref(),
        );
        render_window(
            &mut lines,
            "  Secondary",
            limit.rate_limits.secondary_window.as_ref(),
        );
    }
    if let Some(reached_type) = &usage.rate_limit_reached_type {
        lines.push(format!("Rate-limit reached type: {reached_type}"));
    }
    if let Some(available) = usage.rate_limit_reset_credits_available {
        lines.push(format!("Rate-limit reset credits available: {available}"));
    }
    lines.push("Pricing availability:".to_string());
    lines.push(format!("  Model: {}", usage.pricing.model));
    lines.push(format!("  Billing basis: {}", usage.pricing.billing_basis));
    lines.push("  API token price (USD): unavailable".to_string());
    lines.push(format!("  Reason: {}", usage.pricing.reason));
    lines.push(format!("  Source: {}", usage.pricing.source));
    lines.push(format!("  Checked: {}", usage.pricing.checked_on));
    lines.join("\n") + "\n"
}

fn parse_window(value: &Value) -> Option<RateLimitWindow> {
    let object = value.as_object()?;
    let window = RateLimitWindow {
        used_percent: number(object.get("used_percent")),
        limit_window_seconds: unsigned(object.get("limit_window_seconds")),
        reset_at: integer(object.get("reset_at")),
    };
    (window.used_percent.is_some()
        || window.limit_window_seconds.is_some()
        || window.reset_at.is_some())
    .then_some(window)
}

fn parse_credits(value: &Value) -> Option<Credits> {
    let object = value.as_object()?;
    let credits = Credits {
        has_credits: object.get("has_credits").and_then(Value::as_bool),
        unlimited: object.get("unlimited").and_then(Value::as_bool),
        balance: decimal_text(object.get("balance")),
    };
    (credits.has_credits.is_some() || credits.unlimited.is_some() || credits.balance.is_some())
        .then_some(credits)
}

fn parse_individual_spend_control(value: &Value) -> Option<IndividualSpendControl> {
    let object = value.as_object()?;
    let control = IndividualSpendControl {
        source: object
            .get("source")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        used_percent: number(object.get("used_percent")),
        remaining_percent: number(object.get("remaining_percent")),
        reset_after_seconds: unsigned(object.get("reset_after_seconds")),
        reset_at: integer(object.get("reset_at")),
        limit: decimal_text(object.get("limit")),
        used: decimal_text(object.get("used")),
        remaining: decimal_text(object.get("remaining")),
    };
    (control.source.is_some()
        || control.used_percent.is_some()
        || control.remaining_percent.is_some()
        || control.reset_after_seconds.is_some()
        || control.reset_at.is_some()
        || control.limit.is_some()
        || control.used.is_some()
        || control.remaining.is_some())
    .then_some(control)
}

fn parse_additional_rate_limit(value: &Value) -> Option<AdditionalRateLimit> {
    let object = value.as_object()?;
    let rate_limit = object.get("rate_limit")?.as_object()?;
    let rate_limits = RateLimits {
        primary_window: rate_limit.get("primary_window").and_then(parse_window),
        secondary_window: rate_limit.get("secondary_window").and_then(parse_window),
    };
    if rate_limits.primary_window.is_none() && rate_limits.secondary_window.is_none() {
        return None;
    }
    Some(AdditionalRateLimit {
        limit_name: object
            .get("limit_name")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        metered_feature: object
            .get("metered_feature")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        rate_limits,
    })
}

fn number(value: Option<&Value>) -> Option<f64> {
    value.and_then(Value::as_f64)
}

fn unsigned(value: Option<&Value>) -> Option<u64> {
    value.and_then(Value::as_u64)
}

fn integer(value: Option<&Value>) -> Option<i64> {
    value.and_then(Value::as_i64)
}

fn decimal_text(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn render_window(lines: &mut Vec<String>, label: &str, window: Option<&RateLimitWindow>) {
    let value = window
        .map(render_window_value)
        .unwrap_or_else(|| "not reported".to_string());
    lines.push(format!("{label}: {value}"));
}

fn render_window_value(window: &RateLimitWindow) -> String {
    format!(
        "used_percent={} window_seconds={} reset_at={}",
        option_percent(window.used_percent),
        option_unsigned(window.limit_window_seconds),
        option_integer(window.reset_at),
    )
}

fn option_bool(value: Option<bool>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "not reported".to_string())
}

fn option_unsigned(value: Option<u64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "not reported".to_string())
}

fn option_integer(value: Option<i64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "not reported".to_string())
}

fn option_text(value: Option<&str>) -> String {
    value.unwrap_or("not reported").to_string()
}

fn option_percent(value: Option<f64>) -> String {
    value
        .map(|value| format!("{value}%"))
        .unwrap_or_else(|| "not reported".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn auth(account_id: Option<&str>) -> AuthTokens {
        AuthTokens {
            id_token: "id".to_string(),
            access_token: "secret-token".to_string(),
            refresh_token: "refresh".to_string(),
            expires_at: 0,
            account_id: account_id.map(ToOwned::to_owned),
        }
    }

    #[test]
    fn request_targets_codex_usage_endpoint_with_expected_headers() {
        let request =
            build_usage_request(&reqwest::Client::new(), &auth(Some("acct_123"))).expect("request");

        assert_eq!(request.url().as_str(), CODEX_USAGE_URL);
        assert_eq!(request.headers()[ACCEPT], "application/json");
        assert_eq!(
            request.headers()[USER_AGENT],
            concat!("spark/", env!("CARGO_PKG_VERSION"))
        );
        assert_eq!(request.headers()["ChatGPT-Account-Id"], "acct_123");
        assert_eq!(request.headers()[AUTHORIZATION], "Bearer secret-token");
    }

    #[test]
    fn request_omits_blank_account_header() {
        let request =
            build_usage_request(&reqwest::Client::new(), &auth(Some("  "))).expect("request");
        assert!(request.headers().get("ChatGPT-Account-Id").is_none());
    }

    #[test]
    fn parsing_preserves_primary_windows_when_additional_limits_are_malformed() {
        let usage = parse_usage(&json!({
            "plan_type": "pro",
            "rate_limit": {
                "primary_window": {"used_percent": 25.0, "limit_window_seconds": 10800, "reset_at": 1_800_000_000},
                "secondary_window": {"used_percent": 4, "limit_window_seconds": 604800, "reset_at": 1_800_500_000}
            },
            "credits": {"has_credits": true, "unlimited": false, "balance": "7.50"},
            "spend_control": {
                "reached": false,
                "individual_limit": {
                    "source": "user",
                    "used": "2.00",
                    "limit": "10.00",
                    "remaining": "8.00",
                    "used_percent": 20,
                    "remaining_percent": 80,
                    "reset_after_seconds": 3600,
                    "reset_at": 1_800_000_000
                }
            },
            "additional_rate_limits": [
                "not an object",
                {
                    "limit_name": "good",
                    "metered_feature": "spark",
                    "rate_limit": {
                        "primary_window": {"used_percent": 60.0, "limit_window_seconds": 60, "reset_at": 1_800_000_060},
                        "secondary_window": {"used_percent": 5.0, "limit_window_seconds": 600, "reset_at": 1_800_000_600}
                    }
                },
                {"limit_name": "bad", "rate_limit": {"primary_window": {"used_percent": "wrong type"}}}
            ],
            "rate_limit_reached_type": {"type": "rate_limit_reached"},
            "rate_limit_reset_credits": {"available_count": 3}
        }))
        .expect("parse usage");

        assert_eq!(usage.plan_type.as_deref(), Some("pro"));
        assert_eq!(
            usage
                .rate_limits
                .primary_window
                .as_ref()
                .unwrap()
                .used_percent,
            Some(25.0)
        );
        assert_eq!(
            usage
                .rate_limits
                .secondary_window
                .as_ref()
                .unwrap()
                .limit_window_seconds,
            Some(604800)
        );
        assert_eq!(usage.additional_rate_limits.len(), 1);
        assert_eq!(
            usage.additional_rate_limits[0].limit_name.as_deref(),
            Some("good")
        );
        assert_eq!(
            usage.additional_rate_limits[0]
                .rate_limits
                .secondary_window
                .as_ref()
                .and_then(|window| window.used_percent),
            Some(5.0)
        );
        assert_eq!(usage.credits.unwrap().balance.as_deref(), Some("7.50"));
        assert_eq!(usage.spend_control_reached, Some(false));
        assert_eq!(
            usage.individual_spend_control.unwrap().remaining.as_deref(),
            Some("8.00")
        );
        assert_eq!(
            usage.rate_limit_reached_type.as_deref(),
            Some("rate_limit_reached")
        );
        assert_eq!(usage.rate_limit_reset_credits_available, Some(3));
    }

    #[test]
    fn normalized_json_and_human_output_never_infer_spark_token_prices() {
        let usage = parse_usage(&json!({"plan_type": "plus"})).expect("parse usage");
        let serialized = serde_json::to_value(&usage).expect("serialize usage");
        let output = render_human(&usage);

        assert_eq!(serialized["pricing"]["model"], "gpt-5.3-codex-spark");
        assert_eq!(serialized["pricing"]["billing_basis"], "chatgpt_plan_quota");
        assert!(serialized["pricing"]["api_token_price_usd"].is_null());
        assert_eq!(serialized["pricing"]["status"], "unavailable");
        assert_eq!(serialized["pricing"]["reason"], "no_public_api_price");
        assert_eq!(serialized["pricing"]["checked_on"], "2026-08-01");
        assert!(output.contains("API token price (USD): unavailable"));
        assert!(!output.contains("$0"));
    }
}
