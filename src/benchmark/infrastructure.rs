pub(crate) fn contains_external_infrastructure_failure_signal(text: &str) -> bool {
    let text = text.to_ascii_lowercase();
    [
        "insufficient balance",
        "insufficient-balance",
        "insufficient_quota",
        "quota exceeded",
        "rate limit exceeded",
        "too many requests",
        "resource exhausted",
        "usage limit",
        "you've hit your usage limit",
    ]
    .iter()
    .any(|signal| text.contains(signal))
        || text.contains("\"statuscode\":429")
        || text.contains("\"statuscode\": 429")
        || (text.contains("\"statuscode\":401") && text.contains("insufficient"))
        || (text.contains("\"statuscode\": 401") && text.contains("insufficient"))
}

pub(crate) fn external_infrastructure_retry_hint(text: &str) -> Option<String> {
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let parsed = serde_json::from_str::<serde_json::Value>(line).ok();
        let message = parsed
            .as_ref()
            .and_then(|value| {
                value
                    .get("message")
                    .and_then(serde_json::Value::as_str)
                    .or_else(|| {
                        value
                            .get("error")
                            .and_then(|error| error.get("message"))
                            .and_then(serde_json::Value::as_str)
                    })
                    .or_else(|| value.get("error").and_then(serde_json::Value::as_str))
                    .map(ToOwned::to_owned)
            })
            .unwrap_or_else(|| line.to_string());
        let normalized = message.split_whitespace().collect::<Vec<_>>().join(" ");
        if let Some(hint) = retry_hint_from_message(&normalized) {
            return Some(hint);
        }
        if let Some(hint) =
            retry_hint_from_reset_fields(line).or_else(|| retry_hint_from_reset_fields(&message))
        {
            return Some(hint);
        }
    }
    None
}

fn retry_hint_from_message(message: &str) -> Option<String> {
    let lower = message.to_ascii_lowercase();
    let marker = "try again ";
    let start = lower.find(marker)? + marker.len();
    let rest = message.get(start..)?.trim();
    let rest_lower = rest.to_ascii_lowercase();
    if !(rest_lower.starts_with("at ") || rest_lower.starts_with("in ")) {
        return None;
    }
    let end = rest
        .find('.')
        .or_else(|| rest.find(';'))
        .unwrap_or(rest.len());
    let hint = rest[..end].trim();
    (!hint.is_empty()).then(|| format!("try again {hint}"))
}

fn retry_hint_from_reset_fields(text: &str) -> Option<String> {
    if let Some(timestamp) = number_after_key(text, "resets_at") {
        return Some(format!(
            "try again after {}",
            unix_seconds_to_utc_label(timestamp)
        ));
    }
    number_after_key(text, "resets_in_seconds").map(|seconds| format!("try again in {seconds}s"))
}

fn number_after_key(text: &str, key: &str) -> Option<u64> {
    let start = text.find(key)? + key.len();
    let rest = &text[start..];
    let digits_start = rest.find(|character: char| character.is_ascii_digit())?;
    let digits = rest[digits_start..]
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .collect::<String>();
    digits.parse().ok()
}

fn unix_seconds_to_utc_label(timestamp: u64) -> String {
    let days = (timestamp / 86_400) as i64;
    let seconds_of_day = timestamp % 86_400;
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn civil_from_days(days_since_unix_epoch: i64) -> (i64, i64, i64) {
    let z = days_since_unix_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    if month <= 2 {
        year += 1;
    }
    (year, month, day)
}

pub(crate) fn failure_points_contain(failure_points: &str, expected: &str) -> bool {
    failure_points
        .split(';')
        .any(|point| point.trim() == expected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_quota_and_usage_limit_failures() {
        assert!(contains_external_infrastructure_failure_signal(
            "You've hit your usage limit for GPT-5.3-Codex-Spark."
        ));
        assert!(contains_external_infrastructure_failure_signal(
            r#"{"statusCode":429,"message":"Too Many Requests"}"#
        ));
        assert!(contains_external_infrastructure_failure_signal(
            r#"{"statusCode":401,"message":"Insufficient balance"}"#
        ));
    }

    #[test]
    fn extracts_retry_hints_from_plain_text_and_json_messages() {
        assert_eq!(
            external_infrastructure_retry_hint(
                "You've hit your usage limit for GPT-5.3-Codex-Spark. Try again at 5:38 PM."
            )
            .as_deref(),
            Some("try again at 5:38 PM")
        );
        assert_eq!(
            external_infrastructure_retry_hint(
                r#"{"error":{"message":"Rate limit exceeded. Please try again in 12 minutes."}}"#
            )
            .as_deref(),
            Some("try again in 12 minutes")
        );
        assert_eq!(
            external_infrastructure_retry_hint(
                r#"{"error":"Spark request failed: {\"error\":{\"type\":\"usage_limit_reached\",\"resets_in_seconds\":21328,\"resets_at\":1781138326}}"}"#
            )
            .as_deref(),
            Some("try again after 2026-06-11T00:38:46Z")
        );
        assert_eq!(
            external_infrastructure_retry_hint(
                r#"{"error":"Spark request failed: {\"error\":{\"type\":\"usage_limit_reached\",\"resets_in_seconds\":21328}}"#
            )
            .as_deref(),
            Some("try again in 21328s")
        );
        assert_eq!(
            external_infrastructure_retry_hint("validation failed"),
            None
        );
    }

    #[test]
    fn ignores_task_validation_failures() {
        assert!(!contains_external_infrastructure_failure_signal(
            "validation_failed: schemaVersion not 2"
        ));
    }

    #[test]
    fn matches_exact_failure_point_segments() {
        assert!(failure_points_contain(
            "request_failure;nonzero_exit",
            "request_failure"
        ));
        assert!(!failure_points_contain(
            "not_request_failure;nonzero_exit",
            "request_failure"
        ));
    }
}
