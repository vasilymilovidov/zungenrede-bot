use teloxide::{prelude::Requester, types::Message, Bot};

use crate::{translation::*, PracticeSessions};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone)]
pub struct PracticeSession {
    current_word: Translation,
    expecting_russian: bool,
}

fn get_random_translation(translations: &[Translation]) -> Option<Translation> {
    use rand::seq::SliceRandom;
    translations.choose(&mut rand::thread_rng()).cloned()
}

pub async fn start_practice_session(
    bot: &Bot,
    msg: &Message,
    sessions: &PracticeSessions,
) -> Result<()> {
    let translations = read_translations()?;
    if translations.is_empty() {
        bot.send_message(msg.chat.id, "No words in the database to practice with!")
            .await?;
        return Ok(());
    }

    let translation =
        get_random_translation(&translations).ok_or("Failed to get random translation")?;

    let expecting_russian = rand::random::<bool>();
    let question = if expecting_russian {
        // When expecting Russian, show German word (original)
        format!("Переведите на русский:\n👅{}", translation.original)
    } else {
        // When expecting German, show Russian word (translation)
        if let Some(first_form) = translation.grammar_forms.first() {
            if ["der", "die", "das"].contains(&first_form.trim()) {
                format!(
                    "Переведите на немецкий (не забудьте артикль!):\n👅{}",
                    translation.translation
                )
            } else {
                format!("Переведите на немецкий:\n👅{}", translation.translation)
            }
        } else {
            format!("Переведите на немецкий:\n👅{}", translation.translation)
        }
    };

    let mut sessions = sessions.lock().await;
    sessions.insert(
        msg.chat.id.0,
        PracticeSession {
            current_word: translation,
            expecting_russian,
        },
    );

    bot.send_message(
        msg.chat.id,
        "Practice mode started! Use /stop to end practice.",
    )
    .await?;
    bot.send_message(msg.chat.id, question).await?;

    Ok(())
}

pub async fn check_practice_answer(
    bot: &Bot,
    msg: &Message,
    sessions: &PracticeSessions,
) -> Result<()> {
    let mut sessions = sessions.lock().await;

    if let Some(session) = sessions.get(&msg.chat.id.0) {
        let answer = msg.text().unwrap_or("").trim().to_lowercase();
        let correct = if session.expecting_russian {
            // When expecting Russian, compare with translation
            session
                .current_word
                .translation
                .to_lowercase()
                .split_whitespace()
                .any(|word| word == answer)
        } else {
            // When expecting German, check if it's a noun
            if let Some(first_form) = session.current_word.grammar_forms.first() {
                if ["der", "die", "das"].contains(&first_form.trim()) {
                    let expected =
                        format!("{} {}", first_form, session.current_word.original).to_lowercase();
                    answer == expected
                } else {
                    session
                        .current_word
                        .original
                        .to_lowercase()
                        .split_whitespace()
                        .any(|word| word == answer)
                }
            } else {
                session
                    .current_word
                    .original
                    .to_lowercase()
                    .split_whitespace()
                    .any(|word| word == answer)
            }
        };

        let response = if correct {
            "✅ Правильно!".to_string()
        } else {
            let correct_answer = if session.expecting_russian {
                session.current_word.translation.clone()
            } else if let Some(first_form) = session.current_word.grammar_forms.first() {
                if ["der", "die", "das"].contains(&first_form.trim()) {
                    format!("{} {}", first_form, session.current_word.original)
                } else {
                    session.current_word.original.clone()
                }
            } else {
                session.current_word.original.clone()
            };
            format!("❌ Неправильно! Правильный ответ: {}", correct_answer)
        };

        bot.send_message(msg.chat.id, response).await?;

        // Send next word
        let translations = read_translations()?;
        if let Some(next_translation) = get_random_translation(&translations) {
            let expecting_russian = rand::random::<bool>();
            let question = if expecting_russian {
                // When expecting Russian, show German word (original)
                format!("Переведите на русский:\n👅{}", next_translation.original)
            } else {
                // When expecting German, show Russian word (translation)
                if let Some(first_form) = next_translation.grammar_forms.first() {
                    if ["der", "die", "das"].contains(&first_form.trim()) {
                        format!(
                            "Переведите на немецкий (не забудьте артикль!):\n👅{}",
                            next_translation.translation
                        )
                    } else {
                        format!(
                            "Переведите на немецкий:\n👅{}",
                            next_translation.translation
                        )
                    }
                } else {
                    format!(
                        "Переведите на немецкий:\n👅{}",
                        next_translation.translation
                    )
                }
            };

            sessions.insert(
                msg.chat.id.0,
                PracticeSession {
                    current_word: next_translation,
                    expecting_russian,
                },
            );

            bot.send_message(msg.chat.id, question).await?;
        }
    }

    Ok(())
}

pub async fn stop_practice_session(
    bot: &Bot,
    msg: &Message,
    sessions: &PracticeSessions,
) -> Result<()> {
    let mut sessions = sessions.lock().await;
    sessions.remove(&msg.chat.id.0);
    bot.send_message(msg.chat.id, "Practice mode stopped!")
        .await?;
    Ok(())
}
