use serde::{Deserialize, Serialize};
use std::{env, fs};
use teloxide::{macros::BotCommands, prelude::*, types::InputFile};
use tokio::sync::broadcast;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

// Constants
const WELCOME_MESSAGE: &str = r#"Доступные комманды:
/start - Запустить бота
/help - Показать это сообщение
/export - Экспортировать базу данных переводов
/clear - Очистить базу данных переводов
/exit - Остановить бота

Специальные префиксы для запросов:
!: [запрос] - Проверить грамматику немецкого текста
-: [запрос] - Упростить немецкое предложение
?: [запрос]  - Объяснить грамматику немецкого текста
??: [запрос] - Задать вопрос о немецком языке в свободной форме

Как пользоваться:
• Отправьте немецкое или русское слово для перевода и грамматической справки
• Отправьте немецкое или русское предложение для перевода
• Ответьте на любой перевод вопросом, чтобы получить ответ, учитывающий контекст

Примеры:
??: Как использоваь Akkusative?
?: Der Mann isst einen Apfel
!: Ich habe gestern nach Berlin gefahren
-: Ich würde gerne wissen, ob Sie morgen Zeit haben

Бот автоматически определяет язык ввода и тип запроса."#;

const SHUTDOWN_MESSAGE: &str = "Shutting down...";

const RUSSIAN_TO_GERMAN_PROMPT: &str = r#"You are a Russian-German translator. 
Simply translate the given Russian word or phrase to German without any additional information."#;

const GERMAN_WORD_PROMPT: &str = r#"You are a German-Russian translator. 
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

const RUSSIAN_WORD_PROMPT: &str = r#"You are a Russian-German translator. 
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

const GERMAN_SENTENCE_PROMPT: &str = r#"You are a German-Russian translator.
Simply translate the given German sentence to Russian without any additional information."#;

const EXPLANATION_PROMPT: &str = r#"You are a German language teacher.
Explain the grammar and meaning of each word in the given German text.
Provide your explanation in Russian. Try to be concise and short. Focus on
- Why is the sentence structured this way?
- Grammar forms
- Usage rules
- Any special considerations or common mistakes"#;

const GRAMMAR_CHECK_PROMPT: &str = r#"You are a German language grammar checker.
Check the given German text for grammar mistakes and explain any issues found.
Be concise and short. Don't list mistakes. Don't give an explanation for correct text. 
Provide your response in Russian in the following format:
- First line: Original text with mistakes marked in bold (using *word* format)
- Second line: Corrected version (if there are mistakes)"#;

const FREEFORM_PROMPT: &str = r#"You are a German language expert. 
Please answer the following question about German language in Russian."#;

const SIMPLIFY_PROMPT: &str = r#"You are a German language teacher.
Simplify the given German sentence while preserving its main meaning.
Make it easier to understand for beginners by:
- Using simpler vocabulary
- Simplifying grammar structures
- Breaking complex sentences into shorter ones if needed

Provide your response in the following format:
- First line: Original sentence
- Second line: Simplified version
- Third line: Russian translation of the simplified version"#;

const CONTEXT_PROMPT: &str = r#"You are a German language expert.
The following query is about this word/phrase: {context}
Please answer the query in Russian, providing relevant information about the context word/phrase."#;

const HELP_MESSAGE: &str = r#"Доступные комманды:
/start - Запустить бота
/help - Показать это сообщение
/export - Экспортировать базу данных переводов
/clear - Очистить базу данных переводов
/exit - Остановить бота

Специальные префиксы для запросов:
!: [запрос] - Проверить грамматику немецкого текста
-: [запрос] - Упростить немецкое предложение
?: [запрос]  - Объяснить грамматику немецкого текста
??: [запрос] - Задать вопрос о немецком языке в свободной форме

Как пользоваться:
• Отправьте немецкое или русское слово для перевода и грамматической справки
• Отправьте немецкое или русское предложение для перевода
• Ответьте на любой перевод вопросом, чтобы получить ответ, учитывающий контекст

Примеры:
??: Как использоваь Akkusative?
?: Der Mann isst einen Apfel
!: Ich habe gestern nach Berlin gefahren
-: Ich würde gerne wissen, ob Sie morgen Zeit haben

Бот автоматически определяет язык ввода и тип запроса."#;

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

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Translation {
    original: String,
    translation: String,
    grammar_forms: Vec<String>,
    conjugations: Option<Vec<String>>,
    examples: Vec<Example>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Example {
    german: String,
    russian: String,
}

#[derive(BotCommands, Clone)]
#[command(
    rename_rule = "lowercase",
    description = "These commands are supported:"
)]
enum Command {
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

async fn translate_text(text: &str) -> Result<String> {
    let api_key =
        env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY environment variable not set");

    let client = reqwest::Client::new();

    let (system_prompt, processed_text) = if text.starts_with("Context: ") {
        // Handle contextual query
        let parts: Vec<&str> = text.splitn(2, "Query: ").collect();
        let context = parts[0].trim_start_matches("Context: ").trim();
        let query = parts.get(1).unwrap_or(&"").trim();

        (CONTEXT_PROMPT.replace("{context}", context), query)
    } else {
        match analyze_input(text) {
            InputType::Explanation => {
                let clean_text = text.trim_start_matches("?:").trim();
                (EXPLANATION_PROMPT.to_string(), clean_text)
            }
            InputType::GrammarCheck => {
                let clean_text = text.trim_start_matches("!:").trim();
                (GRAMMAR_CHECK_PROMPT.to_string(), clean_text)
            }
            InputType::Freeform => {
                let clean_text = text.trim_start_matches("??:").trim();
                (FREEFORM_PROMPT.to_string(), clean_text)
            }
            InputType::Simplify => {
                let clean_text = text.trim_start_matches("-:").trim();
                (SIMPLIFY_PROMPT.to_string(), clean_text)
            }
            _ => {
                let prompt = match analyze_input(text) {
                    InputType::RussianWord => RUSSIAN_WORD_PROMPT,
                    InputType::RussianSentence => RUSSIAN_TO_GERMAN_PROMPT,
                    InputType::GermanWord => GERMAN_WORD_PROMPT,
                    InputType::GermanSentence => GERMAN_SENTENCE_PROMPT,
                    InputType::Explanation
                    | InputType::GrammarCheck
                    | InputType::Freeform
                    | InputType::Simplify => {
                        unreachable!()
                    }
                };
                (prompt.to_string(), text)
            }
        }
    };

    let messages = vec![ClaudeMessage {
        role: "user".to_string(),
        content: format!("{}\n\n{}", system_prompt, processed_text),
    }];

    let request = ClaudeRequest {
        model: "claude-3-5-sonnet-20241022".to_string(),
        max_tokens: 1024,
        messages,
    };

    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&request)
        .send()
        .await?
        .json::<ClaudeResponse>()
        .await?;

    Ok(response.content[0].text.clone())
}

fn get_storage_path() -> String {
    std::env::var("STORAGE_FILE").unwrap_or_else(|_| "translations_storage.json".to_string())
}

fn read_translations() -> Result<Vec<Translation>> {
    let path = get_storage_path();
    if !std::path::Path::new(&path).exists() {
        fs::write(&path, "[]")?;
    }
    if let Ok(data) = fs::read_to_string(&path) {
        let translations: Vec<Translation> = serde_json::from_str(&data)?;
        Ok(translations)
    } else {
        Ok(Vec::new())
    }
}

fn write_translations(translations: &[Translation]) -> Result<()> {
    let path = get_storage_path();
    let data = serde_json::to_string(translations)?;
    fs::write(&path, data)?;
    Ok(())
}

fn find_translation<'a>(word: &str, translations: &'a [Translation]) -> Option<&'a Translation> {
    translations.iter().find(|t| {
        t.original.to_lowercase() == word.to_lowercase()
            || t.translation.to_lowercase() == word.to_lowercase()
    })
}

fn clear_translations() -> Result<()> {
    let path = get_storage_path();
    fs::write(&path, "[]")?;
    Ok(())
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    log::info!("Starting translation bot...");

    let bot = Bot::from_env();
    let (shutdown_tx, _) = broadcast::channel(1);

    let shutdown_tx_clone = shutdown_tx.clone();

    let message_handler = Update::filter_message()
        .branch(dptree::entry().filter_command::<Command>().endpoint(
            move |bot: Bot, msg: Message, cmd: Command| {
                let shutdown = shutdown_tx_clone.clone();
                async move {
                    if let Err(e) = handle_command(&bot, &msg, cmd, &shutdown).await {
                        log::error!("Error: {:?}", e);
                    }
                    ResponseResult::Ok(())
                }
            },
        ))
        .branch(
            dptree::filter(|msg: Message| msg.text().is_some()).endpoint(
                move |bot: Bot, msg: Message| async move {
                    if let Err(e) = handle_message(&bot, &msg).await {
                        log::error!("Error: {:?}", e);
                    }
                    ResponseResult::Ok(())
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

async fn handle_command(
    bot: &Bot,
    msg: &Message,
    cmd: Command,
    shutdown: &broadcast::Sender<()>,
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

            // Send the file
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

async fn handle_message(bot: &Bot, msg: &Message) -> Result<()> {
    if !is_user_authorized(msg).await {
        bot.send_message(
            msg.chat.id,
            "Sorry, you are not authorized to use this bot.",
        )
        .await?;
        return Ok(());
    }

    if let Some(text) = msg.text() {
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
                // Only store translations for single words
                let mut translations = read_translations()?;
                translations.push(translation.clone());
                write_translations(&translations)?;
                format_translation_response(&translation)
            }
            InputType::RussianSentence | InputType::GermanSentence => {
                // Don't store sentence translations
                format!("{} ➜ {}", text, claude_response.trim())
            }
        };

        bot.send_message(msg.chat.id, response).await?;
    }
    Ok(())
}

fn parse_translation_response(original: &str, response: &str) -> Translation {
    let lines: Vec<&str> = response.lines().collect();

    let mut translation = Translation {
        original: lines.first().unwrap_or(&original).trim().to_string(),
        translation: lines.get(1).unwrap_or(&"").trim().to_string(),
        grammar_forms: Vec::new(),
        conjugations: None,
        examples: Vec::new(),
    };

    if lines.len() > 2 {
        let mut current_line = 2;
        let mut conjugations = Vec::new();
        let mut in_conjugation_section = false;

        while current_line < lines.len() && !lines[current_line].trim().starts_with('1') {
            let line = lines[current_line].trim();

            if !line.is_empty() {
                if line.contains("ich ")
                    || line.contains("du ")
                    || line.contains("er/")
                    || line.contains("wir ")
                    || line.contains("ihr ")
                    || line.contains("sie/Sie")
                {
                    in_conjugation_section = true;
                    conjugations.push(line.to_string());
                } else if in_conjugation_section {
                    conjugations.push(line.to_string());
                } else {
                    translation.grammar_forms.push(line.to_string());
                }
            }
            current_line += 1;
        }

        if !conjugations.is_empty() {
            translation.conjugations = Some(conjugations);
        }

        // Process examples
        while current_line < lines.len() {
            let line = lines[current_line].trim();
            if line.starts_with('1') || line.starts_with('2') {
                let parts: Vec<&str> = line.split('-').map(|s| s.trim()).collect();
                if let Some((german_part, russian_parts)) = parts.split_first() {
                    let german = german_part
                        .trim_start_matches('1')
                        .trim_start_matches('2')
                        .trim()
                        .to_string();
                    let russian = russian_parts.join("-").trim().to_string();

                    translation.examples.push(Example { german, russian });
                }
            }
            current_line += 1;
        }
    }

    translation
}

fn format_translation_response(translation: &Translation) -> String {
    let mut response = String::new();

    // Check if the word has grammar forms and the first form is an article
    let is_noun = translation
        .grammar_forms
        .first()
        .map(|form| {
            form.trim().matches(' ').count() == 0 && ["der", "die", "das"].contains(&form.trim())
        })
        .unwrap_or(false);

    // Check if the original word contains Cyrillic characters (Russian input)
    let is_russian = translation
        .original
        .chars()
        .any(|c| matches!(c, '\u{0400}'..='\u{04FF}' | '\u{0500}'..='\u{052F}'));

    // Check if the original word already starts with an article
    let already_has_article = translation
        .original
        .split_whitespace()
        .next()
        .map(|first_word| ["der", "die", "das"].contains(&first_word))
        .unwrap_or(false);

    if is_noun {
        if let Some(article) = translation.grammar_forms.first() {
            if is_russian {
                // For Russian input, show original word first, then article with German translation
                response.push_str(&format!("➡️ {}\n", translation.original));
                response.push_str(&format!("⬅️ {} {}\n", article, translation.translation));
            } else {
                // For German input, show original word as is if it already has an article
                if already_has_article {
                    response.push_str(&format!("➡️ {}\n", translation.original));
                } else {
                    response.push_str(&format!("➡️ {} {}\n", article, translation.original));
                }
                response.push_str(&format!("⬅️ {}\n", translation.translation));
            }
        }
    } else {
        response.push_str(&format!("➡️ {}\n", translation.original));
        response.push_str(&format!("⬅️ {}\n", translation.translation));
    }

    if !translation.grammar_forms.is_empty() {
        response.push_str("\n🔤 Грамматика:\n");
        for form in &translation.grammar_forms {
            response.push_str(&format!("• {}\n", form));
        }
    }

    if let Some(conjugations) = &translation.conjugations {
        response.push_str("\n📖 Спряжение:\n");
        for conj in conjugations {
            response.push_str(&format!("• {}\n", conj));
        }
    }

    if !translation.examples.is_empty() {
        response.push_str("\n📚 Примеры:\n");
        for (i, example) in translation.examples.iter().enumerate() {
            response.push_str(&format!(
                "{} {} — {}\n",
                i + 1,
                example.german,
                example.russian
            ));
        }
    }

    response
}
