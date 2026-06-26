// SPDX-License-Identifier: MIT OR Apache-2.0
//! LLM provider abstraction.
//!
//! v0.6.0 minimum: the typed request/response types and the
//! `LlmProvider` enum. v0.6.0 hardening adds the actual
//! `reqwest` integration for each provider.
//!
//! See TZ §3.5.4 for the contract; the providers are
//! Ollama (local, default), OpenAI, Anthropic, and
//! OpenRouter. Each provider takes the same `LlmRequest`
//! and returns the same `LlmResponse`; the difference is the
//! HTTP endpoint, the auth header, and the request body
//! schema.

use serde::{Deserialize, Serialize};

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
}
