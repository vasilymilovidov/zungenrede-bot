use rand::seq::SliceRandom;
use std::collections::HashMap;
use std::sync::Arc;
use teloxide::{prelude::Requester, types::Message, Bot};
use tokio::sync::Mutex;

use crate::ai::{
    ChatGPTMessage, ChatGPTRequest, ChatGPTResponse, ClaudeMessage, ClaudeRequest, ClaudeResponse,
    CHATGPT_API_URL, CHATGPT_MODEL, TALK_MODE_PROMPT,
};
use std::env;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

const GREETINGS: [&str; 5] = ["Hallo!", "Hi!", "Guten Tag!", "Servus!", "Grüß dich!"];

const INTRODUCTIONS: [&str; 5] = [
    "Ich freue mich, mit dir auf Deutsch zu sprechen.",
    "Schön, dass wir uns unterhalten können.",
    "Ich bin gespannt, dich kennenzulernen.",
    "Lass uns ein bisschen plaudern.",
    "Ich würde gerne mehr über dich erfahren.",
];

const QUESTIONS: [&str; 8] = [
    "Was machst du beruflich? Gefällt dir deine Arbeit?",
    "Was beschäftigt dich zurzeit? Arbeitest du an interessanten Projekten?",
    "Was hast du am Wochenende gemacht? Hattest du Zeit für deine Hobbys?",
    "Was interessiert dich besonders - Sport, Musik, Reisen oder etwas ganz anderes?",
    "Wie sieht ein typischer Tag bei dir aus?",
    "Was sind deine Lieblingsorte in deiner Stadt?",
    "Hast du besondere Pläne für die nächsten Wochen?",
    "Was machst du gerne in deiner Freizeit?",
];

#[derive(Clone)]
pub struct TalkSession {
    context: Vec<String>,
}

impl TalkSession {
    fn new() -> Self {
        Self {
            context: Vec::new(),
        }
    }

    fn add_message(&mut self, message: &str) {
        self.context.push(message.to_string());
        // Keep only the last 5 messages for context
        if self.context.len() > 5 {
            self.context.remove(0);
        }
    }

    fn get_context(&self) -> String {
        self.context.join("\n")
    }
}

pub type TalkSessions = Arc<Mutex<HashMap<i64, TalkSession>>>;

async fn make_claude_request(request: &ClaudeRequest) -> Result<ClaudeResponse> {
    let api_key =
        env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY environment variable not set");
    let client = reqwest::Client::new();

    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(request)
        .send()
        .await?
        .json::<ClaudeResponse>()
        .await?;

    Ok(response)
}

async fn talk_with_claude(context: &str, message: &str) -> Result<String> {
    let prompt = TALK_MODE_PROMPT
        .replace("{context}", context)
        .replace("{message}", message);

    let messages = vec![ClaudeMessage {
        role: "user".to_string(),
        content: prompt,
    }];

    let request = ClaudeRequest {
        model: "claude-sonnet-4-5".to_string(),
        max_tokens: 4000,
        messages,
    };

    let response = make_claude_request(&request).await?;
    Ok(response.content[0].text.clone())
}

async fn talk_with_chatgpt(context: &str, message: &str) -> Result<String> {
    let api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY environment variable not set");
    let client = reqwest::Client::new();

    let prompt = TALK_MODE_PROMPT
        .replace("{context}", context)
        .replace("{message}", message);

    let messages = vec![ChatGPTMessage {
        role: "user".to_string(),
        content: prompt,
    }];

    let request = ChatGPTRequest {
        model: CHATGPT_MODEL.to_string(),
        messages,
    };

    let response = client
        .post(CHATGPT_API_URL)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await?
        .json::<ChatGPTResponse>()
        .await?;

    Ok(response.choices[0].message.content.clone())
}

fn generate_initial_prompt() -> String {
    let mut rng = rand::thread_rng();
    format!(
        "{} {} {}",
        GREETINGS.choose(&mut rng).unwrap(),
        INTRODUCTIONS.choose(&mut rng).unwrap(),
        QUESTIONS.choose(&mut rng).unwrap()
    )
}

pub async fn start_talk_session(bot: &Bot, msg: &Message, sessions: &TalkSessions) -> Result<()> {
    let mut sessions = sessions.lock().await;

    if sessions.contains_key(&msg.chat.id.0) {
        bot.send_message(msg.chat.id, "Du bist bereits im Gesprächsmodus!")
            .await?;
        return Ok(());
    }

    let initial_prompt = generate_initial_prompt();
    let mut session = TalkSession::new();
    session.add_message(&initial_prompt);
    sessions.insert(msg.chat.id.0, session);
    bot.send_message(msg.chat.id, initial_prompt).await?;

    Ok(())
}

pub async fn stop_talk_session(bot: &Bot, msg: &Message, sessions: &TalkSessions) -> Result<()> {
    let mut sessions = sessions.lock().await;

    if sessions.remove(&msg.chat.id.0).is_some() {
        bot.send_message(
            msg.chat.id,
            "Danke für das Gespräch! Bis zum nächsten Mal! 👋",
        )
        .await?;
    } else {
        bot.send_message(msg.chat.id, "Du bist nicht im Gesprächsmodus!")
            .await?;
    }

    Ok(())
}

pub async fn handle_talk_message(
    bot: &Bot,
    msg: &Message,
    sessions: &TalkSessions,
    use_chatgpt: &Arc<Mutex<bool>>,
) -> Result<()> {
    let mut sessions = sessions.lock().await;

    if let Some(session) = sessions.get_mut(&msg.chat.id.0) {
        if let Some(text) = msg.text() {
            session.add_message(text);

            let use_chatgpt = *use_chatgpt.lock().await;
            let response = if use_chatgpt {
                talk_with_chatgpt(&session.get_context(), text).await?
            } else {
                talk_with_claude(&session.get_context(), text).await?
            };

            session.add_message(&response);
            bot.send_message(msg.chat.id, response).await?;
        }
    }

    Ok(())
}
