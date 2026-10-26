//! Selects injected transcription engines from portable provider configuration.

use crate::{
    application::ai_config::{AiProvider, AiTaskConfig},
    domain::errors::TranscriptionError,
    ports::ai::TranscriptionEngine,
};
use std::collections::HashMap;

/// Owns registered adapter-neutral engines and applies local-first/fallback selection policy.
pub struct AiRegistry {
    transcription_engines: HashMap<AiProvider, Box<dyn TranscriptionEngine>>,
}
impl AiRegistry {
    /// Creates an empty registry; composition roots register only configured, credentialed adapters.
    pub fn new() -> Self {
        Self {
            transcription_engines: HashMap::new(),
        }
    }
    /// Registers or replaces one transcription engine without exposing its concrete adapter type.
    pub fn register_transcription_engine<Engine>(&mut self, provider: AiProvider, engine: Engine)
    where
        Engine: TranscriptionEngine + 'static,
    {
        self.transcription_engines
            .insert(provider, Box::new(engine));
    }
    /// Selects an explicitly requested provider then configured fallbacks; Anthropic is unsupported today.
    pub fn select_transcription_engine(
        &self,
        configuration: &AiTaskConfig,
    ) -> Result<&dyn TranscriptionEngine, TranscriptionError> {
        let candidates = std::iter::once(configuration.provider())
            .chain(configuration.fallback_chain().iter().copied());
        for provider in candidates {
            if provider == AiProvider::Anthropic {
                continue;
            }
            if let Some(engine) = self.transcription_engines.get(&provider) {
                return Ok(engine.as_ref());
            }
        }
        if configuration.provider() == AiProvider::Anthropic {
            Err(TranscriptionError::ProviderUnsupported)
        } else {
            Err(TranscriptionError::Unavailable)
        }
    }
}
impl Default for AiRegistry {
    fn default() -> Self {
        Self::new()
    }
}
