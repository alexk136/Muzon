// SPDX-License-Identifier: MIT OR Apache-2.0
//! LLM provider abstraction.
//!
//! v0.6.0 minimum: the typed request/response types and the
//! `LlmProvider` enum, the opt-in enforcement (per decision
//! 0001-N4), and a collection-context payload builder. v0.6.0
//! hardening adds the actual `reqwest` integration for each
//! provider.
//!
//! See TZ §3.5.4 for the contract; the providers are
//! Ollama (local, default), OpenAI, Anthropic, and
//! OpenRouter. Each provider takes the same `LlmRequest`
//! and returns the same `LlmResponse`; the difference is the
//! HTTP endpoint, the auth header, and the request body
//! schema.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// LLM provider kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LlmProvider {
    #[default]
    Ollama,
    OpenAi,
    Anthropic,
    OpenRouter,
}

impl LlmProvider {
    /// The default base URL for this provider.
    pub fn default_base_url(&self) -> &'static str {
        match self {
            LlmProvider::Ollama => "http://localhost:11434",
            LlmProvider::OpenAi => "https://api.openai.com",
            LlmProvider::Anthropic => "https://api.anthropic.com",
            LlmProvider::OpenRouter => "https://openrouter.ai",
        }
    }

    /// The default model name for this provider. v0.6.0
    /// minimum: the user picks the model via Settings (0019).
    pub fn default_model(&self) -> &'static str {
        match self {
            LlmProvider::Ollama => "llama3.1:8b",
            LlmProvider::OpenAi => "gpt-4o",
            LlmProvider::Anthropic => "claude-3-5-sonnet",
            LlmProvider::OpenRouter => "openai/gpt-4o",
        }
    }
}

/// A request to an LLM provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmRequest {
    /// The system prompt (the AI's role / behaviour).
    pub system: String,
    /// The user prompt (the actual question).
    pub user: String,
    /// The model to use. If empty, the provider's default
    /// model is used.
    #[serde(default)]
    pub model: String,
    /// The maximum number of tokens to generate. v0.6.0
    /// minimum: defaults to 1024.
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
}

fn default_max_tokens() -> u32 {
    1024
}

/// A response from an LLM provider.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct LlmResponse {
    /// The generated text.
    pub text: String,
    /// The number of tokens consumed (prompt + completion).
    #[serde(default)]
    pub tokens_used: u32,
    /// The provider that produced the response.
    #[serde(default)]
    pub provider: LlmProvider,
}

impl LlmResponse {
    /// True if the response is empty.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}

/// Errors produced by the LLM provider abstraction.
#[derive(Debug, Error)]
pub enum LlmError {
    #[error("opt-in required: LLM providers are gated by `network.llm_providers` per decision 0001-N4")]
    OptInRequired,
    #[error("missing API key for provider {0:?}")]
    MissingApiKey(LlmProvider),
    #[error("network error: {0}")]
    Network(String),
    #[error("invalid response: {0}")]
    InvalidResponse(String),
    #[error("provider error: {0}")]
    Provider(String),
}

/// A collection-context payload that the LLM receives as
/// the system prompt. v0.6.0 minimum: a typed struct that
/// summarises the library's stats (track count, artist count,
/// album count, total size, total duration); the user can
/// ask the LLM "find similar tracks" or "what's missing" and
/// the LLM has the context to answer.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CollectionContext {
    pub track_count: u32,
    pub artist_count: u32,
    pub album_count: u32,
    pub total_size_bytes: u64,
    pub total_duration_ms: u64,
}

impl CollectionContext {
    /// Render the context as a system prompt.
    pub fn to_system_prompt(&self) -> String {
        format!(
            "You are an AI music expert. The user's library has {} tracks, \
             {} artists, {} albums, {:.1} MB total, {:.1} minutes total. \
             Answer the user's questions about the library concisely.",
            self.track_count,
            self.artist_count,
            self.album_count,
            self.total_size_bytes as f64 / 1_000_000.0,
            self.total_duration_ms as f64 / 60_000.0,
        )
    }
}

/// Opt-in enforcement. v0.6.0 minimum: a typed check; the
/// caller is responsible for reading the `network.llm_providers`
/// flag from the config (per decision 0001-N4).
pub fn check_opt_in(enabled: bool) -> Result<(), LlmError> {
    if enabled {
        Ok(())
    } else {
        Err(LlmError::OptInRequired)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_default_urls_and_models() {
        assert_eq!(LlmProvider::Ollama.default_base_url(), "http://localhost:11434");
        assert_eq!(LlmProvider::OpenAi.default_model(), "gpt-4o");
        assert_eq!(LlmProvider::Anthropic.default_model(), "claude-3-5-sonnet");
        assert_eq!(LlmProvider::OpenRouter.default_model(), "openai/gpt-4o");
    }

    #[test]
    fn request_serializes_round_trip() {
        let req = LlmRequest {
            system: "You are a music expert.".into(),
            user: "Find similar tracks".into(),
            model: "gpt-4o".into(),
            max_tokens: 512,
        };
        let json = serde_json::to_string(&req).expect("serialise");
        let parsed: LlmRequest = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(req, parsed);
    }

    #[test]
    fn response_empty_check() {
        let r = LlmResponse::default();
        assert!(r.is_empty());
        let r = LlmResponse {
            text: "hello".into(),
            ..Default::default()
        };
        assert!(!r.is_empty());
    }

    #[test]
    fn opt_in_enforced() {
        assert!(check_opt_in(false).is_err());
        assert!(check_opt_in(true).is_ok());
    }

    #[test]
    fn collection_context_to_system_prompt() {
        let ctx = CollectionContext {
            track_count: 1000,
            artist_count: 250,
            album_count: 150,
            total_size_bytes: 90_000_000_000,
            total_duration_ms: 4_320_000_000, // 72 hours = 4320 min
        };
        let prompt = ctx.to_system_prompt();
        assert!(prompt.contains("1000 tracks"));
        assert!(prompt.contains("250 artists"));
        assert!(prompt.contains("150 albums"));
    }
}
