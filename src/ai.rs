use serde::{Deserialize, Serialize};
use reqwest;
use tokio;
use log;
use rand;

pub const CHATGPT_MODEL: &str = "gpt-4o";
pub const CHATGPT_API_URL: &str = "https://api.openai.com/v1/chat/completions";

pub const RUSSIAN_TO_GERMAN_PROMPT: &str = r#"You are a Russian-German translator. 
Simply translate the given Russian word or phrase to German without any additional information."#;

pub const GERMAN_WORD_PROMPT: &str = r#"You are a German-Russian translator. 
For verbs:
- First line: Original word in German
- Second line: Russian translation without brackets or decorations
- Third line: Partizip II form
- Fourth line: Präteritum form
Then conjugation in Präsens:
- ich form
- du form
- er/sie/es form
- wir form
- ihr form
- sie/Sie form
Then provide 2 simple example sentences in format:
1. German sentence - Russian translation
2. German sentence - Russian translation

For nouns:
- First line: Original word in German
- Second line: Russian translation without brackets or decorations
- Third line: German article in nominative case
- Then provide 2 simple example sentences in format:
1. German sentence - Russian translation
2. German sentence - Russian translation

For other word types:
- First line: Original word in German
- Second line: Russian translation without brackets or decorations
- Then provide 2 simple example sentences in format:
1. German sentence - Russian translation
2. German sentence - Russian translation

If there are spelling mistakes in the input, please correct them without any comments and write the corrected version instead of the original word."#;

pub const RUSSIAN_WORD_PROMPT: &str = r#"You are a Russian-German translator. 
For verbs:
- First line: Original word in Russian
- Second line: German translation without brackets or decorations
- Third line: Partizip II form
- Fourth line: Präteritum form
Then conjugation in Präsens:
- ich form
- du form
- er/sie/es form
- wir form
- ihr form
- sie/Sie form
Then provide 2 simple example sentences in format:
1. Russian sentence - German translation
2. Russian sentence - German translation

For nouns:
- First line: Original word in Russian
- Second line: German translation without brackets or decorations
- Third line: German article in nominative case
- Then provide 2 simple example sentences in format:
1. Russian sentence - German translation
2. Russian sentence - German translation

For other word types:
- First line: Original word in Russian
- Second line: German translation without brackets or decorations
- Then provide 2 simple example sentences in format:
1. Russian sentence - German translation
2. Russian sentence - German translation"#;

pub const GERMAN_SENTENCE_PROMPT: &str = r#"You are a German-Russian translator.
Simply translate the given German sentence to Russian without any additional information."#;

pub const EXPLANATION_PROMPT: &str = r#"You are a German language teacher.
Explain the grammar and meaning of each word in the given German text.
Provide your explanation in Russian. Try to be concise and short. Focus on
- Why is the sentence structured this way?
- Grammar forms
- Usage rules
- Any special considerations or common mistakes"#;

pub const GRAMMAR_CHECK_PROMPT: &str = r#"You are a German language grammar checker.
Check the given German text for grammar mistakes and explain any issues found.
Be concise and short. Don't list mistakes. Don't give an explanation for correct text. 
Provide your response in Russian in the following format:
- First line: Original text with mistakes marked in bold (using *word* format)
- Second line: Corrected version (if there are mistakes)"#;

pub const FREEFORM_PROMPT: &str = r#"You are a German language expert. 
Please answer the following question about German language in Russian."#;

pub const SIMPLIFY_PROMPT: &str = r#"You are a German language teacher.
Simplify the given German sentence while preserving its main meaning.
Make it easier to understand for beginners by:
- Using simpler vocabulary
- Simplifying grammar structures
- Breaking complex sentences into shorter ones if needed

Provide your response in the following format:
- First line: Original sentence
- Second line: Simplified version
- Third line: Russian translation of the simplified version"#;

pub const CONTEXT_PROMPT: &str = r#"You are a German language expert.
The following query is about this word/phrase: {context}
Please answer the query in Russian, providing relevant information about the context word/phrase."#;

pub const STORY_PROMPT: &str = r#"You are a creative storyteller writing modern German short stories in the style of Éric Rohmer.

Write a short story (maximum 3900 characters) with the following characteristics:

THEMATIC ELEMENTS:
Focus on everyday encounters and subtle interpersonal dynamics
Modern themes such as:
Burnout
Dating culture
Big city life
Feminism
Mental health
Gig economy
Gentle social commentary, particularly on:
Consumer behavior
Modern work culture
Relationships in the 21st century

STYLE:
Feminist perspective
Queer friendly
Tragicomic tone
Natural dialogues in German (A2-B1 level)
Subtle, light humor
Understated irony
Contemporary pop culture references
Rohmer-esque "moral" undertones

TECHNICAL REQUIREMENTS:
Organically incorporate these learning vocabulary words: {word list}
Use simple language (A2-B1) but sophisticated narrative structure
Formatting:
Title
Empty line
Story
Maximum length: 3900 characters."#;

pub const TALK_MODE_PROMPT: &str = r#"You are a friendly German conversation partner at B1 level. 
Your task is to engage in natural conversation in German, keeping the language at A2-B1 level.
Focus on daily life topics like hobbies, work, family, interests, and opinions.
Keep your responses concise (1-2 sentences).

If the user makes any grammar mistakes:
1. Start your response with "Kleine Korrektur:" and show the corrected version
2. Then continue the conversation naturally, responding to their message

DO NOT translate the user's message to Russian. Instead, maintain a natural conversation in German.
Always respond in German, except for the grammar corrections which should be brief and clear.

Previous conversation:
{context}

User message: {message}"#;

const MAX_RETRIES: u32 = 5;
const INITIAL_BACKOFF_MS: u64 = 1000;
const MAX_BACKOFF_MS: u64 = 32000;

#[derive(Debug, Serialize, Deserialize)]
pub struct ClaudeRequest {
    pub model: String,
    pub max_tokens: u32,
    pub messages: Vec<ClaudeMessage>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClaudeMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClaudeResponse {
    pub content: Vec<ClaudeContent>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClaudeContent {
    pub text: String,
    pub r#type: String,
}

pub async fn make_claude_request(request: &ClaudeRequest) -> Result<ClaudeResponse, Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::new();
    let anthropic_api_key = std::env::var("ANTHROPIC_API_KEY")?;
    
    let mut current_retry = 0;
    let mut backoff_ms = INITIAL_BACKOFF_MS;

    loop {
        let response = client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &anthropic_api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(request)
            .send()
            .await?;

        let status = response.status();
        
        if status.is_success() {
            return Ok(response.json::<ClaudeResponse>().await?);
        }

        // If we get a 529 (or other 5xx) error
        if status.is_server_error() {
            if current_retry >= MAX_RETRIES {
                return Err(format!("Max retries ({}) exceeded", MAX_RETRIES).into());
            }

            // Calculate exponential backoff with jitter
            let jitter = rand::random::<u64>() % 1000;
            let sleep_duration = std::cmp::min(
                backoff_ms + jitter,
                MAX_BACKOFF_MS
            );

            log::info!(
                "Claude API request failed with status {}. Retrying in {} ms (attempt {}/{})",
                status,
                sleep_duration,
                current_retry + 1,
                MAX_RETRIES
            );

            tokio::time::sleep(std::time::Duration::from_millis(sleep_duration)).await;
            
            current_retry += 1;
            backoff_ms *= 2;
            continue;
        }

        // For other errors, return immediately
        return Err(format!("Claude API request failed with status: {}", status).into());
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatGPTRequest {
    pub model: String,
    pub messages: Vec<ChatGPTMessage>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatGPTMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatGPTResponse {
    pub choices: Vec<ChatGPTChoice>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatGPTChoice {
    pub message: ChatGPTMessage,
}
