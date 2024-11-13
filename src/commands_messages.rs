use teloxide::{
    macros::BotCommands,
    payloads::SendDocumentSetters,
    prelude::Requester,
    types::{InputFile, Message},
    Bot,
};
use tokio::sync::broadcast;

use crate::{
    analyze_input, is_user_authorized,
    practice::{check_practice_answer, start_practice_session, stop_practice_session},
    promts_consts::{HELP_MESSAGE, SHUTDOWN_MESSAGE, WELCOME_MESSAGE},
    translation::{
        add_translation, clear_translations, find_translation, format_translation_response, get_storage_path, parse_translation_response, read_translations, translate_text
    },
    InputType, PracticeSessions,
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

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
    #[command(description = "stop practice mode")]
    Stop,
}

pub async fn handle_command(
    bot: &Bot,
    msg: &Message,
    cmd: Command,
    shutdown: &broadcast::Sender<()>,
    sessions: &PracticeSessions,
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
            bot.send_message(msg.chat.id, WELCOME_MESSAGE).await?;
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
    }
    Ok(())
}

pub async fn handle_message(bot: &Bot, msg: &Message, sessions: &PracticeSessions) -> Result<()> {
    if !is_user_authorized(msg).await {
        bot.send_message(
            msg.chat.id,
            "Sorry, you are not authorized to use this bot.",
        )
        .await?;
        return Ok(());
    }

    if let Some(text) = msg.text() {
        // Check if user is in practice mode
        let is_practicing = sessions.lock().await.contains_key(&msg.chat.id.0);

        if is_practicing {
            check_practice_answer(bot, msg, sessions).await?;
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

            let claude_response = if let Some(context) = context {
                let combined_text = format!("Context: {}\nQuery: {}", context, text);
                translate_text(&combined_text).await?
            } else {
                translate_text(text).await?
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
                    // Don't store sentence translations
                    format!("{} ➜ {}", text, claude_response.trim())
                }
            };

            bot.send_message(msg.chat.id, response).await?;
        }
    }
    Ok(())
}
