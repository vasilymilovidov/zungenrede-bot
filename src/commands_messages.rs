use std::{collections::HashSet, env, sync::Arc};

use teloxide::{
    macros::BotCommands,
    net::Download,
    payloads::SendDocumentSetters,
    prelude::Requester,
    types::{InputFile, Message},
    Bot,
};
use tokio::sync::{broadcast, Mutex};

use crate::{
    consts::{HELP_MESSAGE, SHUTDOWN_MESSAGE},
    input::{analyze_input, InputType},
    practice::{check_practice_answer, start_practice_session, stop_practice_session},
    picture::{handle_picture_message, start_picture_session, stop_picture_session, PictureSessions},
    story::generate_story,
    talk::{handle_talk_message, start_talk_session, stop_talk_session, TalkSessions},
    translation::{
        add_translation, clear_translations, delete_translation, find_translation,
        format_translation_response, get_storage_path, import_translations,
        parse_translation_response, read_translations, translate_text,
    },
    PracticeSessions,
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
pub type DeleteMode = Arc<tokio::sync::Mutex<HashSet<i64>>>;

#[derive(BotCommands, Clone)]
#[command(
    rename_rule = "lowercase",
    description = "These commands are supported:"
)]
pub enum Command {
    #[command(description = "start the bot")]
    Start,
    #[command(description = "show help information")]
    Help,
    #[command(description = "shutdown the bot")]
    Exit,
    #[command(description = "export translations database")]
    Export,
    #[command(description = "clear translations database")]
    Clear,
    #[command(description = "start practice mode")]
    Practice,
    #[command(description = "import translations database from JSON file")]
    Import,
    #[command(description = "stop practice mode")]
    Stop,
    #[command(description = "enter delete mode")]
    Delete,
    #[command(description = "exit delete mode")]
    StopDelete,
    #[command(description = "show word statistics")]
    Stats(String),
    #[command(description = "generate a short story in German")]
    Story,
    #[command(description = "switch to ChatGPT")]
    UseChatGPT,
    #[command(description = "switch to Claude")]
    UseClaude,
    #[command(description = "start talk mode")]
    Talk,
    #[command(description = "stop talk mode")]
    StopTalk,
    #[command(description = "start picture description mode")]
    Pic,
    #[command(description = "stop picture description mode")]
    Stoppic,
}

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
        .from()
        .map(|u| i64::try_from(u.id.0).unwrap_or(0))
        .unwrap_or(0);
    let is_authorized = allowed_users.contains(&user_id);
    log::info!(
        "Authorization check - User ID: {}, Authorized: {}, Allowed users: {:?}",
        user_id,
        is_authorized,
        allowed_users
    );
    is_authorized
}

pub async fn handle_command(
    bot: &Bot,
    msg: &Message,
    cmd: Command,
    shutdown: &broadcast::Sender<()>,
    sessions: &PracticeSessions,
    talk_sessions: &TalkSessions,
    picture_sessions: &PictureSessions,
    delete_mode: &DeleteMode,
    use_chatgpt: &Arc<Mutex<bool>>,
) -> Result<()> {
    if !is_user_authorized(msg).await {
        bot.send_message(
            msg.chat.id,
            "Sorry, you are not authorized to use this bot.",
        )
        .await?;
        return Ok(());
    }
    match cmd {
        Command::Practice => {
            start_practice_session(bot, msg, sessions).await?;
        }
        Command::Stop => {
            stop_practice_session(bot, msg, sessions).await?;
        }
        Command::Start => {
            bot.send_message(msg.chat.id, HELP_MESSAGE).await?;
        }
        Command::Help => {
            bot.send_message(msg.chat.id, HELP_MESSAGE).await?;
        }
        Command::Exit => {
            bot.send_message(msg.chat.id, SHUTDOWN_MESSAGE).await?;
            shutdown.send(()).ok();
        }
        Command::Export => {
            let translations = read_translations()?;
            let file_path = get_storage_path();

            let input_file = InputFile::file(file_path);
            bot.send_document(msg.chat.id, input_file)
                .caption(format!(
                    "Translation database with {} entries",
                    translations.len()
                ))
                .await?;
        }
        Command::Clear => {
            clear_translations()?;
            bot.send_message(msg.chat.id, "Translations database has been cleared.")
                .await?;
        }
        Command::Import => {
            bot.send_message(msg.chat.id, "Please send me a JSON file with translations.")
                .await?;
        }
        Command::Delete => {
            let mut delete_mode = delete_mode.lock().await;
            delete_mode.insert(msg.chat.id.0);
            bot.send_message(
                       msg.chat.id,
                       "Delete mode activated. Send any word to delete it from the database. Use /stopdelete to exit delete mode.",
                   )
                   .await?;
        }
        Command::StopDelete => {
            let mut delete_mode = delete_mode.lock().await;
            delete_mode.remove(&msg.chat.id.0);
            bot.send_message(msg.chat.id, "Delete mode deactivated.")
                .await?;
        }
        Command::Stats(word) => {
            if let Some(translation) = find_translation(&word, &read_translations()?) {
                let total = translation.correct_answers + translation.wrong_answers;
                let accuracy = if total > 0 {
                    (translation.correct_answers as f64 / total as f64) * 100.0
                } else {
                    0.0
                };

                let stats_message = format!(
                    "📊 Statistics for '{}'\n\nTotal attempts: {}\nCorrect: {}\nWrong: {}\nAccuracy: {:.1}%",
                    word, total, translation.correct_answers, translation.wrong_answers, accuracy
                );

                bot.send_message(msg.chat.id, stats_message).await?;
            } else {
                bot.send_message(msg.chat.id, "Word not found in database.")
                    .await?;
            }
        }
        Command::Story => {
            bot.send_message(msg.chat.id, "Generating a story...")
                .await?;
            let use_chatgpt = *use_chatgpt.lock().await;
            match generate_story(use_chatgpt).await {
                Ok(story) => {
                    bot.send_message(msg.chat.id, story).await?;
                }
                Err(e) => {
                    bot.send_message(msg.chat.id, format!("Failed to generate story: {}", e))
                        .await?;
                }
            }
        }
        Command::UseChatGPT => {
            let mut use_chatgpt = use_chatgpt.lock().await;
            *use_chatgpt = true;
            bot.send_message(msg.chat.id, "Switched to ChatGPT.").await?;
        }
        Command::UseClaude => {
            let mut use_chatgpt = use_chatgpt.lock().await;
            *use_chatgpt = false;
            bot.send_message(msg.chat.id, "Switched to Claude.").await?;
        }
        Command::Talk => {
            start_talk_session(bot, msg, talk_sessions).await?;
        }
        Command::StopTalk => {
            stop_talk_session(bot, msg, talk_sessions).await?;
        }
        Command::Pic => {
            start_picture_session(bot, msg, picture_sessions).await?;
        }
        Command::Stoppic => {
            stop_picture_session(bot, msg, picture_sessions).await?;
        }
    }
    Ok(())
}

pub async fn handle_message(
    bot: &Bot,
    msg: &Message,
    sessions: &PracticeSessions,
    talk_sessions: &TalkSessions,
    picture_sessions: &PictureSessions,
    delete_mode: &DeleteMode,
    use_chatgpt: &Arc<Mutex<bool>>,
) -> Result<()> {
    if !is_user_authorized(msg).await {
        bot.send_message(
            msg.chat.id,
            "Sorry, you are not authorized to use this bot.",
        )
        .await?;
        return Ok(());
    }

    let chat_id = msg.chat.id;
    
    // Check if user is in picture mode
    {
        let picture_lock = picture_sessions.lock().await;
        if picture_lock.contains_key(&chat_id.0) {
            drop(picture_lock);
            handle_picture_message(bot, msg, picture_sessions).await?;
            return Ok(());
        }
    }

    // Check if user is in talk mode
    {
        let talk_lock = talk_sessions.lock().await;
        let is_talking = talk_lock.contains_key(&chat_id.0);
        drop(talk_lock);

        if is_talking {
            handle_talk_message(bot, msg, talk_sessions, use_chatgpt).await?;
            return Ok(());
        }
    }

    if let Some(text) = msg.text() {
        let is_practicing = sessions.lock().await.contains_key(&chat_id.0);
        let is_deleting = delete_mode.lock().await.contains(&chat_id.0);

        if is_practicing {
            check_practice_answer(bot, msg, sessions).await?;
        } else if is_deleting {
            match delete_translation(text) {
                Ok(true) => {
                    bot.send_message(msg.chat.id, "✅ Word deleted successfully.")
                        .await?;
                }
                Ok(false) => {
                    bot.send_message(msg.chat.id, "❌ Word not found.")
                        .await?;
                }
                Err(e) => {
                    bot.send_message(msg.chat.id, format!("❌ Error: {}", e))
                        .await?;
                }
            }
        } else {
            let input_type = analyze_input(text);

            // Check local database first for single words
            if matches!(input_type, InputType::GermanWord | InputType::RussianWord) {
                let translations = read_translations()?;
                if let Some(existing_translation) = find_translation(text, &translations) {
                    let response = format_translation_response(existing_translation);
                    bot.send_message(msg.chat.id, response).await?;
                    return Ok(());
                }
            }

            // Continue with existing logic for API calls
            let context = if let Some(reply) = msg.reply_to_message() {
                reply.text().map(|original_text| {
                    if let Some(first_line) = original_text.lines().next() {
                        if first_line.starts_with("➡️ ") {
                            first_line.trim_start_matches("➡️ ").trim().to_string()
                        } else {
                            first_line.trim().to_string()
                        }
                    } else {
                        String::new()
                    }
                })
            } else {
                None
            };

            let use_chatgpt = *use_chatgpt.lock().await;
            let claude_response = if let Some(context) = context {
                let combined_text = format!("Context: {}\nQuery: {}", context, text);
                translate_text(&combined_text, use_chatgpt).await?
            } else {
                translate_text(text, use_chatgpt).await?
            };

            let response = match input_type {
                InputType::Explanation
                | InputType::GrammarCheck
                | InputType::Freeform
                | InputType::Simplify => claude_response.trim().to_string(),
                InputType::GermanWord | InputType::RussianWord => {
                    let translation = parse_translation_response(text, &claude_response);
                    if let Err(e) = add_translation(translation.clone()) {
                        log::error!("Failed to add translation: {}", e);
                    }
                    format_translation_response(&translation)
                }
                InputType::RussianSentence | InputType::GermanSentence => {
                    format!("{} ➜ {}", text, claude_response.trim())
                }
            };

            bot.send_message(msg.chat.id, response).await?;
        }
    }
    Ok(())
}

pub async fn handle_document(bot: &Bot, msg: &Message) -> Result<()> {
    if !is_user_authorized(msg).await {
        bot.send_message(
            msg.chat.id,
            "Sorry, you are not authorized to use this bot.",
        )
        .await?;
        return Ok(());
    }

    if let Some(document) = msg.document() {
        if document
            .file_name
            .as_ref()
            .map_or(false, |name| name.ends_with(".json"))
        {
            let file = bot.get_file(&document.file.id).await?;
            let mut bytes = Vec::new();
            bot.download_file(&file.path, &mut bytes).await?;

            match String::from_utf8(bytes) {
                Ok(json_str) => match import_translations(&json_str) {
                    Ok(count) => {
                        bot.send_message(
                            msg.chat.id,
                            format!("✅ Successfully imported {} translations", count),
                        )
                        .await?;
                    }
                    Err(e) => {
                        bot.send_message(
                            msg.chat.id,
                            format!("❌ Error importing translations: {}", e),
                        )
                        .await?;
                    }
                },
                Err(e) => {
                    bot.send_message(msg.chat.id, format!("❌ Error reading file: {}", e))
                        .await?;
                }
            }
        } else {
            bot.send_message(msg.chat.id, "❌ Please send a JSON file")
                .await?;
        }
    }
    Ok(())
}
