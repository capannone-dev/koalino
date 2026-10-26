//! Defines provider-neutral configuration for local and cloud AI task selection.

/// Identifies an AI provider without exposing any provider SDK types to application callers.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum AiProvider {
    Local,
    OpenAi,
    Anthropic,
}

/// Selects a provider for one task and specifies safe ordered fallbacks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AiTaskConfig {
    provider: AiProvider,
    fallback_chain: Vec<AiProvider>,
}
impl AiTaskConfig {
    /// Creates a task provider preference and ordered fallbacks.
    pub fn new(provider: AiProvider, fallback_chain: Vec<AiProvider>) -> Self {
        Self {
            provider,
            fallback_chain,
        }
    }
    /// Returns the explicitly selected primary provider.
    pub fn provider(&self) -> AiProvider {
        self.provider
    }
    /// Borrows the ordered fallback providers.
    pub fn fallback_chain(&self) -> &[AiProvider] {
        &self.fallback_chain
    }
}

/// Holds device defaults and optional transcription-task overrides.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AiConfig {
    device_default: AiTaskConfig,
    transcription_override: Option<AiTaskConfig>,
}
impl AiConfig {
    /// Creates provider configuration for one device.
    pub fn new(device_default: AiTaskConfig, transcription_override: Option<AiTaskConfig>) -> Self {
        Self {
            device_default,
            transcription_override,
        }
    }
    /// Returns task-specific transcription selection or the per-device default.
    pub fn transcription_config(&self) -> &AiTaskConfig {
        self.transcription_override
            .as_ref()
            .unwrap_or(&self.device_default)
    }
}
