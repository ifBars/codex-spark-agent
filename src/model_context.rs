use anyhow::Result;

use crate::APPROX_CHARS_PER_TOKEN;
use crate::auth::AuthTokens;
use crate::client::SparkClient;

pub(crate) const DEFAULT_CONTEXT_WINDOW_TOKENS: usize = 128_000;
pub(crate) const DEFAULT_AUTO_COMPACT_PERCENT: usize = 90;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct InputLimits {
    pub(crate) compact_after_chars: usize,
    pub(crate) max_input_chars: usize,
}

pub(crate) async fn resolve_input_limits(
    auth: &AuthTokens,
    model: &str,
    compact_after_chars: Option<usize>,
    compact_after_tokens: Option<usize>,
    max_input_chars: Option<usize>,
    max_input_tokens: Option<usize>,
) -> Result<InputLimits> {
    reject_duplicate_units("compact-after", compact_after_chars, compact_after_tokens)?;
    reject_duplicate_units("max-input", max_input_chars, max_input_tokens)?;

    let needs_context_window = (compact_after_chars.is_none() && compact_after_tokens.is_none())
        || (max_input_chars.is_none() && max_input_tokens.is_none());
    let context_window_tokens = if needs_context_window {
        let client = SparkClient::new(auth.clone(), model.to_string());
        match client.model_context_window_tokens().await {
            Ok(tokens) => tokens,
            Err(error) => {
                eprintln!(
                    "warning: could not resolve context window for {model}; using {DEFAULT_CONTEXT_WINDOW_TOKENS} tokens: {error:#}"
                );
                DEFAULT_CONTEXT_WINDOW_TOKENS
            }
        }
    } else {
        DEFAULT_CONTEXT_WINDOW_TOKENS
    };

    let compact_after_chars = compact_after_chars
        .or_else(|| compact_after_tokens.map(tokens_to_chars))
        .unwrap_or_else(|| default_compact_after_chars(context_window_tokens));
    let max_input_chars = max_input_chars
        .or_else(|| max_input_tokens.map(tokens_to_chars))
        .unwrap_or_else(|| default_max_input_chars(context_window_tokens));
    if compact_after_chars > max_input_chars {
        anyhow::bail!(
            "compact-after threshold ({compact_after_chars} chars) cannot exceed max-input threshold ({max_input_chars} chars)"
        );
    }

    Ok(InputLimits {
        compact_after_chars,
        max_input_chars,
    })
}

pub(crate) const fn default_compact_after_chars(context_window_tokens: usize) -> usize {
    tokens_to_chars(context_window_tokens).saturating_mul(DEFAULT_AUTO_COMPACT_PERCENT) / 100
}

pub(crate) const fn default_max_input_chars(context_window_tokens: usize) -> usize {
    tokens_to_chars(context_window_tokens)
}

const fn tokens_to_chars(tokens: usize) -> usize {
    tokens.saturating_mul(APPROX_CHARS_PER_TOKEN)
}

fn reject_duplicate_units(name: &str, chars: Option<usize>, tokens: Option<usize>) -> Result<()> {
    if chars.is_some() && tokens.is_some() {
        anyhow::bail!("pass either --{name}-chars or --{name}-tokens, not both");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_compaction_uses_ninety_percent_of_context_window() {
        assert_eq!(default_compact_after_chars(128_000), 460_800);
        assert_eq!(default_max_input_chars(128_000), 512_000);
    }

    #[test]
    fn duplicate_threshold_units_are_rejected() {
        let error = reject_duplicate_units("max-input", Some(1), Some(1))
            .expect_err("duplicate units should fail");
        assert!(
            error
                .to_string()
                .contains("pass either --max-input-chars or --max-input-tokens")
        );
    }
}
