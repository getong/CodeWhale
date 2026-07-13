//! Route-aware billing presentation.
//!
//! Model pricing and the way a user pays for a route are different facts.
//! The same model can be metered through an API key or covered by an OAuth /
//! token-plan subscription.  Keep that decision in one small module so TUI
//! surfaces do not infer dollars from a model id alone.

use crate::config::{ApiProvider, Config, ProviderConfig};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BillingPresentation {
    /// Per-token API usage may be rendered as a currency estimate.
    Metered,
    /// Account/subscription quota is the truthful owner; dollar estimates are
    /// intentionally hidden unless the provider later exposes real spend.
    Subscription(&'static str),
    /// The route is local or otherwise has no provider bill.
    Local,
}

impl BillingPresentation {
    #[must_use]
    pub const fn shows_money(self) -> bool {
        matches!(self, Self::Metered)
    }

    #[must_use]
    pub const fn label(self) -> Option<&'static str> {
        match self {
            Self::Metered => None,
            Self::Subscription(label) => Some(label),
            Self::Local => Some("local"),
        }
    }
}

/// Resolve how the active provider route should present usage.
#[must_use]
pub fn for_route(config: &Config, provider: ApiProvider) -> BillingPresentation {
    if matches!(
        provider,
        ApiProvider::Ollama | ApiProvider::Sglang | ApiProvider::Vllm
    ) {
        return BillingPresentation::Local;
    }
    if provider == ApiProvider::OpenaiCodex {
        return BillingPresentation::Subscription("Codex OAuth quota");
    }

    let provider_config = config.provider_config_for(provider);
    match provider {
        ApiProvider::XiaomiMimo if !xiaomi_is_explicit_pay_as_you_go(provider_config) => {
            BillingPresentation::Subscription("MiMo token plan")
        }
        ApiProvider::Xai if provider_config.is_some_and(uses_xai_oauth) => {
            BillingPresentation::Subscription("Grok OAuth quota")
        }
        ApiProvider::Moonshot if provider_config.is_some_and(uses_kimi_oauth) => {
            BillingPresentation::Subscription("Kimi OAuth quota")
        }
        ApiProvider::Anthropic if provider_config.is_some_and(uses_anthropic_oauth) => {
            BillingPresentation::Subscription("Claude OAuth quota")
        }
        _ => BillingPresentation::Metered,
    }
}

/// Billing for a child route when its full dispatch config is not present in
/// the completion envelope. Never invent metered dollars for providers that
/// support subscription/OAuth routes; the parent route remains authoritative
/// only when the provider is the same.
#[must_use]
pub fn for_child_route(
    parent_provider: ApiProvider,
    parent_billing: BillingPresentation,
    child_provider: ApiProvider,
) -> BillingPresentation {
    if child_provider == parent_provider {
        return parent_billing;
    }
    match child_provider {
        ApiProvider::Ollama | ApiProvider::Sglang | ApiProvider::Vllm => BillingPresentation::Local,
        ApiProvider::OpenaiCodex
        | ApiProvider::Xai
        | ApiProvider::Moonshot
        | ApiProvider::Anthropic
        | ApiProvider::XiaomiMimo => BillingPresentation::Subscription("provider quota"),
        _ => BillingPresentation::Metered,
    }
}

fn normalized(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace(['-', ' '], "_")
}

fn auth_mode(config: &ProviderConfig) -> Option<String> {
    config
        .auth_mode
        .as_deref()
        .or(config.mode.as_deref())
        .map(normalized)
}

fn uses_xai_oauth(config: &ProviderConfig) -> bool {
    auth_mode(config).is_some_and(|mode| crate::xai_oauth::auth_mode_uses_xai_oauth(&mode))
}

fn uses_kimi_oauth(config: &ProviderConfig) -> bool {
    auth_mode(config).is_some_and(|mode| {
        matches!(
            mode.as_str(),
            "oauth" | "kimi" | "kimi_oauth" | "kimi_cli" | "kimi_code"
        )
    })
}

fn uses_anthropic_oauth(config: &ProviderConfig) -> bool {
    auth_mode(config).is_some_and(|mode| {
        matches!(
            mode.as_str(),
            "oauth"
                | "anthropic_oauth"
                | "claude_oauth"
                | "claude_cli"
                | "claude_code"
                | "max"
                | "subscription"
        )
    })
}

fn xiaomi_is_explicit_pay_as_you_go(config: Option<&ProviderConfig>) -> bool {
    if let Some(mode) = std::env::var("XIAOMI_MIMO_MODE")
        .ok()
        .filter(|mode| !mode.trim().is_empty())
        .map(|mode| normalized(&mode))
    {
        return matches!(
            mode.as_str(),
            "standard" | "default" | "payg" | "paygo" | "pay_as_you_go" | "pay_as_go"
        );
    }
    if let Some(base_url) = std::env::var("XIAOMI_MIMO_BASE_URL")
        .ok()
        .filter(|base_url| !base_url.trim().is_empty())
    {
        return !base_url.to_ascii_lowercase().contains("token-plan-");
    }
    let token_plan_env = ["XIAOMI_MIMO_TOKEN_PLAN_API_KEY", "MIMO_TOKEN_PLAN_API_KEY"]
        .iter()
        .any(|name| std::env::var(name).is_ok_and(|value| !value.trim().is_empty()));
    let standard_env = ["XIAOMI_MIMO_API_KEY", "XIAOMI_API_KEY", "MIMO_API_KEY"]
        .iter()
        .any(|name| std::env::var(name).is_ok_and(|value| !value.trim().is_empty()));
    if standard_env && !token_plan_env {
        return true;
    }
    let Some(config) = config else {
        // The shipped MiMo default is a token-plan endpoint.
        return false;
    };
    if let Some(mode) = config
        .mode
        .as_deref()
        .filter(|mode| !mode.trim().is_empty())
        .map(normalized)
    {
        return matches!(
            mode.as_str(),
            "pay_as_you_go" | "payg" | "paygo" | "api" | "standard" | "default"
        );
    }
    if let Some(api_key) = config
        .api_key
        .as_deref()
        .filter(|key| !key.trim().is_empty())
    {
        return !api_key.trim_start().starts_with("tp-");
    }
    config.base_url.as_deref().is_some_and(|base_url| {
        let lower = base_url.to_ascii_lowercase();
        !lower.contains("token-plan-") && !lower.contains("token_plan_")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_with(provider: ApiProvider, provider_config: ProviderConfig) -> Config {
        let mut config = Config::default();
        *config.provider_config_for_mut(provider) = provider_config;
        config
    }

    #[test]
    fn codex_oauth_never_claims_api_dollars() {
        assert_eq!(
            for_route(&Config::default(), ApiProvider::OpenaiCodex),
            BillingPresentation::Subscription("Codex OAuth quota")
        );
    }

    #[test]
    fn xai_oauth_and_api_key_routes_stay_distinct() {
        let oauth = config_with(
            ApiProvider::Xai,
            ProviderConfig {
                auth_mode: Some("grok-oauth".to_string()),
                ..ProviderConfig::default()
            },
        );
        let api = config_with(
            ApiProvider::Xai,
            ProviderConfig {
                auth_mode: Some("api-key".to_string()),
                ..ProviderConfig::default()
            },
        );
        assert!(!for_route(&oauth, ApiProvider::Xai).shows_money());
        assert!(for_route(&api, ApiProvider::Xai).shows_money());
    }

    #[test]
    fn future_claude_oauth_does_not_inherit_anthropic_api_prices() {
        let oauth = config_with(
            ApiProvider::Anthropic,
            ProviderConfig {
                auth_mode: Some("claude-code".to_string()),
                ..ProviderConfig::default()
            },
        );
        assert_eq!(
            for_route(&oauth, ApiProvider::Anthropic).label(),
            Some("Claude OAuth quota")
        );
    }

    #[test]
    fn xiaomi_defaults_to_token_plan_but_explicit_payg_is_metered() {
        let _lock = crate::test_support::lock_test_env();
        let _mode = crate::test_support::EnvVarGuard::remove("XIAOMI_MIMO_MODE");
        let _base = crate::test_support::EnvVarGuard::remove("XIAOMI_MIMO_BASE_URL");
        let _token = crate::test_support::EnvVarGuard::remove("XIAOMI_MIMO_TOKEN_PLAN_API_KEY");
        let _token_alias = crate::test_support::EnvVarGuard::remove("MIMO_TOKEN_PLAN_API_KEY");
        let _standard_a = crate::test_support::EnvVarGuard::remove("XIAOMI_MIMO_API_KEY");
        let _standard_b = crate::test_support::EnvVarGuard::remove("XIAOMI_API_KEY");
        let _standard_c = crate::test_support::EnvVarGuard::remove("MIMO_API_KEY");
        assert!(!for_route(&Config::default(), ApiProvider::XiaomiMimo).shows_money());
        let payg = config_with(
            ApiProvider::XiaomiMimo,
            ProviderConfig {
                mode: Some("pay-as-you-go".to_string()),
                ..ProviderConfig::default()
            },
        );
        assert!(for_route(&payg, ApiProvider::XiaomiMimo).shows_money());
        let standard_key = config_with(
            ApiProvider::XiaomiMimo,
            ProviderConfig {
                api_key: Some("sk-standard".to_string()),
                ..ProviderConfig::default()
            },
        );
        assert!(for_route(&standard_key, ApiProvider::XiaomiMimo).shows_money());
    }

    #[test]
    fn unknown_cross_provider_oauth_capable_child_never_invents_dollars() {
        assert!(
            !for_child_route(
                ApiProvider::Deepseek,
                BillingPresentation::Metered,
                ApiProvider::Xai,
            )
            .shows_money()
        );
        assert!(
            for_child_route(
                ApiProvider::Deepseek,
                BillingPresentation::Metered,
                ApiProvider::Openrouter,
            )
            .shows_money()
        );
    }

    #[test]
    fn standard_mimo_env_key_uses_metered_presentation() {
        let _lock = crate::test_support::lock_test_env();
        let _mode = crate::test_support::EnvVarGuard::remove("XIAOMI_MIMO_MODE");
        let _base = crate::test_support::EnvVarGuard::remove("XIAOMI_MIMO_BASE_URL");
        let _token = crate::test_support::EnvVarGuard::remove("XIAOMI_MIMO_TOKEN_PLAN_API_KEY");
        let _token_alias = crate::test_support::EnvVarGuard::remove("MIMO_TOKEN_PLAN_API_KEY");
        let _standard_a = crate::test_support::EnvVarGuard::remove("XIAOMI_MIMO_API_KEY");
        let _standard_b = crate::test_support::EnvVarGuard::remove("XIAOMI_API_KEY");
        let _standard = crate::test_support::EnvVarGuard::set("MIMO_API_KEY", "sk-metered");

        assert!(for_route(&Config::default(), ApiProvider::XiaomiMimo).shows_money());
    }
}
