mod commands_messages;
mod practice;
mod promts_consts;
mod translation;

use commands_messages::{handle_command, handle_message, Command};
use practice::PracticeSession;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, env, sync::Arc};
use teloxide::prelude::*;
use tokio::sync::{broadcast, Mutex};
use translation::get_storage_path;

type PracticeSessions = Arc<Mutex<HashMap<i64, PracticeSession>>>;

fn get_allowed_users() -> Vec<i64> {
    let users = env::var("ALLOWED_USERS")
        .unwrap_or_default()
        .split(',')
        .filter_map(|id| {
            let parsed = id.trim().parse::<i64>();
            if let Err(e) = &parsed {
                log::warn!("Failed to parse user ID '{}': {}", id, e);
            }
            parsed.ok()
        })
        .collect::<Vec<i64>>();

    log::info!("Allowed users: {:?}", users);
    users
}

async fn is_user_authorized(msg: &Message) -> bool {
    let allowed_users = get_allowed_users();
    let user_id = msg
        .clone()
        .from
        .map(|u| i64::try_from(u.id.0).unwrap_or(0))
        .unwrap_or(0);
    // allowed_users.contains(&user_id);
    let is_authorized = allowed_users.contains(&user_id);
    log::info!(
        "Authorization check - User ID: {}, Authorized: {}, Allowed users: {:?}",
        user_id,
        is_authorized,
        allowed_users
    );
    is_authorized
}

#[derive(Debug, Serialize, Deserialize)]
struct ClaudeRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<ClaudeMessage>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ClaudeMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ClaudeResponse {
    content: Vec<ClaudeContent>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ClaudeContent {
    #[serde(rename = "type")]
    content_type: String,
    text: String,
}

#[derive(Debug)]
enum InputType {
    RussianWord,
    RussianSentence,
    GermanWord,
    GermanSentence,
    Explanation,
    GrammarCheck,
    Freeform,
    Simplify,
}

fn analyze_input(text: &str) -> InputType {
    if text.starts_with("??:") {
        InputType::Freeform
    } else if text.starts_with("?:") {
        InputType::Explanation
    } else if text.starts_with("!:") {
        InputType::GrammarCheck
    } else if text.starts_with("-:") {
        InputType::Simplify
    } else {
        let has_cyrillic = text
            .chars()
            .any(|c| matches!(c, '\u{0400}'..='\u{04FF}' | '\u{0500}'..='\u{052F}'));

        if has_cyrillic {
            if !text.contains(' ') {
                InputType::RussianWord
            } else {
                InputType::RussianSentence
            }
        } else {
            let words: Vec<_> = text.split_whitespace().collect();
            let is_german_noun = words.len() == 2 && ["der", "die", "das"].contains(&words[0]);

            if !text.contains(' ') || is_german_noun {
                InputType::GermanWord
            } else {
                InputType::GermanSentence
            }
        }
    }
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    log::info!("Starting translation bot...");

    if let Some(parent) = std::path::Path::new(&get_storage_path()).parent() {
        std::fs::create_dir_all(parent).expect("Failed to create storage directory");
    }

    let bot = Bot::from_env();
    let (shutdown_tx, _) = broadcast::channel(1);
    let sessions: PracticeSessions = Arc::new(Mutex::new(HashMap::new()));

    let shutdown_tx_clone = shutdown_tx.clone();
    let sessions_clone = sessions.clone();

    let message_handler = Update::filter_message()
        .branch(dptree::entry().filter_command::<Command>().endpoint(
            move |bot: Bot, msg: Message, cmd: Command| {
                let shutdown = shutdown_tx_clone.clone();
                let sessions = sessions_clone.clone();
                async move {
                    if let Err(e) = handle_command(&bot, &msg, cmd, &shutdown, &sessions).await {
                        log::error!("Error: {:?}", e);
                    }
                    ResponseResult::Ok(())
                }
            },
        ))
        .branch(
            dptree::filter(|msg: Message| msg.text().is_some()).endpoint(
                move |bot: Bot, msg: Message| {
                    let sessions = sessions.clone();
                    async move {
                        if let Err(e) = handle_message(&bot, &msg, &sessions).await {
                            log::error!("Error: {:?}", e);
                        }
                        ResponseResult::Ok(())
                    }
                },
            ),
        );

    let mut dispatcher = Dispatcher::builder(bot, message_handler)
        .enable_ctrlc_handler()
        .build();

    let mut rx = shutdown_tx.subscribe();

    tokio::select! {
        _ = dispatcher.dispatch() => log::info!("Bot stopped normally"),
        _ = rx.recv() => log::info!("Shutdown signal received"),
    }

    log::info!("Bot shutdown complete");
}
