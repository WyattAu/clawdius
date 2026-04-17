//! OpenRouter provider (OpenAI-compatible API)

use async_trait::async_trait;
use futures::StreamExt;
use genai::adapter::AdapterKind;
use genai::chat::{ChatMessage, ChatRequest};
use genai::resolver::{AuthData, Endpoint, ServiceTargetResolver};
use genai::{ModelIden, ServiceTarget};
use tokio::sync::mpsc;

use crate::llm::providers::LlmClient;
use crate::llm::{ChatMessage as ClawdiusMessage, ChatRole};
use crate::{Error, Result};

pub struct OpenRouterProvider {
    client: genai::Client,
    model: String,
}

impl OpenRouterProvider {
    pub fn new(api_key: &str, model: Option<&str>) -> Result<Self> {
        let model_name = model.unwrap_or("google/gemma-3-4b-it:free").to_string();
        let api_key = api_key.to_string();

        let target_resolver = ServiceTargetResolver::from_resolver_fn(
            move |service_target: ServiceTarget| -> genai::resolver::Result<ServiceTarget> {
                let ServiceTarget { model, .. } = service_target;
                let endpoint = Endpoint::from_owned("https://openrouter.ai/api/v1/".to_string());
                let auth = AuthData::from_single(api_key.clone());
                let model = ModelIden::new(AdapterKind::OpenAI, model.model_name);
                Ok(ServiceTarget {
                    endpoint,
                    auth,
                    model,
                })
            },
        );

        let client = genai::Client::builder()
            .with_service_target_resolver(target_resolver)
            .build();

        Ok(Self {
            client,
            model: model_name,
        })
    }
}

#[async_trait]
impl LlmClient for OpenRouterProvider {
    async fn chat(&self, messages: Vec<ClawdiusMessage>) -> Result<String> {
        let genai_messages: Vec<ChatMessage> = messages
            .into_iter()
            .map(|m| match m.role {
                ChatRole::System => ChatMessage::system(m.content),
                ChatRole::User => ChatMessage::user(m.content),
                ChatRole::Assistant => ChatMessage::assistant(m.content),
            })
            .collect();

        let chat_req = ChatRequest::new(genai_messages);

        let response = self
            .client
            .exec_chat(&self.model, chat_req, None)
            .await
            .map_err(|e| Error::Llm(e.to_string()))?;

        response
            .first_text()
            .map(std::string::ToString::to_string)
            .ok_or_else(|| Error::Llm("No response text".into()))
    }

    async fn chat_stream(&self, messages: Vec<ClawdiusMessage>) -> Result<mpsc::Receiver<String>> {
        let (tx, rx) = mpsc::channel(100);

        let genai_messages: Vec<ChatMessage> = messages
            .into_iter()
            .map(|m| match m.role {
                ChatRole::System => ChatMessage::system(m.content),
                ChatRole::User => ChatMessage::user(m.content),
                ChatRole::Assistant => ChatMessage::assistant(m.content),
            })
            .collect();

        let chat_req = ChatRequest::new(genai_messages);
        let client = self.client.clone();
        let model = self.model.clone();

        tokio::spawn(async move {
            match client.exec_chat_stream(&model, chat_req, None).await {
                Ok(stream_response) => {
                    let mut stream = stream_response.stream;
                    while let Some(result) = stream.next().await {
                        match result {
                            Ok(event) => {
                                if let genai::chat::ChatStreamEvent::Chunk(chunk) = event {
                                    if tx.send(chunk.content).await.is_err() {
                                        break;
                                    }
                                }
                            },
                            Err(e) => {
                                let _ = tx.send(format!("[Error: {e}]")).await;
                                break;
                            },
                        }
                    }
                },
                Err(e) => {
                    let _ = tx.send(format!("[Error: {e}]")).await;
                },
            }
        });

        Ok(rx)
    }

    fn count_tokens(&self, text: &str) -> usize {
        text.split_whitespace().count()
    }
}
