use strsim::jaro_winkler;
use teloxide::{prelude::Requester, types::Message, Bot};

use crate::{translation::*, PracticeSessions};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

const SIMILARITY_THRESHOLD: f64 = 0.85;
const STATS_INTERVAL: u32 = 10;
const ARTICLES: [&str; 3] = ["der", "die", "das"];

#[derive(Debug)]
enum AnswerResult {
    Correct,
    AlmostCorrect { expected: String, similarity: f64 },
    WrongArticle { expected: String },
    Wrong { expected: String },
}

struct AnswerCheck {
    result: AnswerResult,
    feedback: String,
}

impl AnswerCheck {
    fn format_message(&self) -> String {
        match &self.result {
            AnswerResult::Correct => "✅ Правильно!".to_string(),
            AnswerResult::AlmostCorrect { expected, similarity } => {
                format!(
                    "⚠️ Почти правильно! Ожидалось: {}\nПохожесть: {:.0}%",
                    expected,
                    similarity * 100.0
                )
            }
            AnswerResult::WrongArticle { expected } => {
                format!("❌ Неправильный артикль! Правильный ответ: {}", expected)
            }
            AnswerResult::Wrong { expected } => {
                format!("❌ Неправильно! Правильный ответ: {}", expected)
            }
        }
    }
}

#[derive(Clone)]
pub struct PracticeSession {
    current_word: Translation,
    expecting_russian: bool,
    words_practiced: u32,
    correct_answers: u32,
    wrong_answers: u32,
}

fn format_practice_question(translation: &Translation, expecting_russian: bool) -> String {
    if expecting_russian {
        if let Some(first_form) = translation.grammar_forms.first() {
            if ARTICLES.contains(&first_form.trim()) {
                format!(
                    "Переведите на русский:\n👅{} {}",
                    first_form.trim(),
                    translation.original
                )
            } else {
                format!("Переведите на русский:\n👅{}", translation.original)
            }
        } else {
            format!("Переведите на русский:\n👅{}", translation.original)
        }
    } else {
        if let Some(first_form) = translation.grammar_forms.first() {
            if ARTICLES.contains(&first_form.trim()) {
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
    }
}

fn normalize(text: &str) -> String {
    text.trim()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphabetic() || c.is_whitespace())
        .collect()
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

    let translation = get_weighted_translation(&translations)
        .ok_or("Failed to get weighted translation")?;
    let expecting_russian = rand::random::<bool>();
    let question = format_practice_question(&translation, expecting_russian);

    let mut sessions = sessions.lock().await;
    sessions.insert(
        msg.chat.id.0,
        PracticeSession {
            current_word: translation,
            expecting_russian,
            words_practiced: 0,
            correct_answers: 0,
            wrong_answers: 0,
        },
    );

    bot.send_message(msg.chat.id, "Practice mode started! Use /stop to end practice.")
        .await?;
    bot.send_message(msg.chat.id, question).await?;

    Ok(())
}

fn check_answer(answer: &str, translation: &Translation, expecting_russian: bool) -> AnswerCheck {
    let answer = normalize(answer);

    if expecting_russian {
        check_russian_answer(answer, translation)
    } else {
        check_german_answer(answer, translation)
    }
}

fn check_russian_answer(answer: String, translation: &Translation) -> AnswerCheck {
    let expected = normalize(&translation.translation);
    let expected_variants: Vec<String> = translation
        .translation
        .split(',')
        .map(normalize)
        .chain(translation.examples.iter().map(|ex| normalize(&ex.russian)))
        .collect();

    if expected_variants.contains(&answer) || answer == expected {
        return AnswerCheck {
            result: AnswerResult::Correct,
            feedback: String::new(),
        };
    }

    let best_match = expected_variants
        .iter()
        .map(|variant| jaro_winkler(&answer, variant))
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap_or(0.0);

    if best_match > SIMILARITY_THRESHOLD {
        AnswerCheck {
            result: AnswerResult::AlmostCorrect {
                expected: translation.translation.clone(),
                similarity: best_match,
            },
            feedback: String::new(),
        }
    } else {
        AnswerCheck {
            result: AnswerResult::Wrong {
                expected: translation.translation.clone(),
            },
            feedback: String::new(),
        }
    }
}

fn check_german_answer(answer: String, translation: &Translation) -> AnswerCheck {
    let is_noun = translation
        .grammar_forms
        .first()
        .map(|form| ARTICLES.contains(&form.trim()))
        .unwrap_or(false);

    if is_noun {
        check_german_noun_answer(answer, translation)
    } else {
        check_german_word_answer(answer, translation)
    }
}

fn check_german_noun_answer(answer: String, translation: &Translation) -> AnswerCheck {
    let expected_article = translation
        .grammar_forms
        .first()
        .map(|a| a.trim().to_lowercase())
        .unwrap_or_default();
    let expected_noun = normalize(&translation.original);
    let expected = format!("{} {}", expected_article, expected_noun);

    let parts: Vec<&str> = answer.split_whitespace().collect();
    match parts.as_slice() {
        [article, noun, ..] => {
            if article.to_lowercase() != expected_article {
                return AnswerCheck {
                    result: AnswerResult::WrongArticle { expected },
                    feedback: String::new(),
                };
            }

            let similarity = jaro_winkler(&normalize(noun), &expected_noun);
            if similarity > SIMILARITY_THRESHOLD {
                AnswerCheck {
                    result: AnswerResult::Correct,
                    feedback: String::new(),
                }
            } else {
                AnswerCheck {
                    result: AnswerResult::AlmostCorrect {
                        expected: expected.clone(),
                        similarity,
                    },
                    feedback: String::new(),
                }
            }
        }
        _ => AnswerCheck {
            result: AnswerResult::Wrong { expected },
            feedback: "Не забудьте указать артикль!".to_string(),
        },
    }
}

fn check_german_word_answer(answer: String, translation: &Translation) -> AnswerCheck {
    let mut correct_variants = vec![normalize(&translation.original)];
    if let Some(conjugations) = &translation.conjugations {
        correct_variants.extend(
            conjugations
                .iter()
                .filter_map(|conj| conj.split_whitespace().last())
                .map(normalize),
        );
    }

    correct_variants.extend(
        translation
            .examples
            .iter()
            .flat_map(|ex| ex.german.split_whitespace())
            .map(normalize),
    );

    if correct_variants.contains(&answer) {
        return AnswerCheck {
            result: AnswerResult::Correct,
            feedback: String::new(),
        };
    }

    let best_match = correct_variants
        .iter()
        .map(|variant| jaro_winkler(&answer, variant))
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap_or(0.0);

    if best_match > SIMILARITY_THRESHOLD {
        AnswerCheck {
            result: AnswerResult::AlmostCorrect {
                expected: translation.original.clone(),
                similarity: best_match,
            },
            feedback: String::new(),
        }
    } else {
        AnswerCheck {
            result: AnswerResult::Wrong {
                expected: translation.original.clone(),
            },
            feedback: String::new(),
        }
    }
}

pub async fn check_practice_answer(
    bot: &Bot,
    msg: &Message,
    sessions: &PracticeSessions,
) -> Result<()> {
    let mut sessions = sessions.lock().await;

    if let Some(mut session) = sessions.get(&msg.chat.id.0).cloned() {
        let answer = msg.text().unwrap_or("").trim();
        let check_result = check_answer(answer, &session.current_word, session.expecting_russian);
        
        // Update statistics
        session.words_practiced += 1;
        let is_correct = matches!(check_result.result, AnswerResult::Correct);
        session.correct_answers += is_correct as u32;
        session.wrong_answers += (!is_correct) as u32;

        // Format response
        let mut response = check_result.format_message();
        if !check_result.feedback.is_empty() {
            response.push_str(&format!("\n{}", check_result.feedback));
        }
        if session.words_practiced % STATS_INTERVAL == 0 {
            response.push_str(&format_practice_stats(&session));
        }

        // Update word statistics in database
        let word = if session.expecting_russian {
            &session.current_word.original
        } else {
            &session.current_word.translation
        };
        update_translation_stats(word, is_correct)?;

        bot.send_message(msg.chat.id, response).await?;

        // If correct, get next word
        if is_correct {
            if let Some(next_translation) = get_weighted_translation(&read_translations()?) {
                let expecting_russian = rand::random::<bool>();
                let question = format_practice_question(&next_translation, expecting_russian);
                
                session.current_word = next_translation;
                session.expecting_russian = expecting_russian;
                
                bot.send_message(msg.chat.id, question).await?;
            }
        }

        sessions.insert(msg.chat.id.0, session);
    }

    Ok(())
}

pub async fn stop_practice_session(
    bot: &Bot,
    msg: &Message,
    sessions: &PracticeSessions,
) -> Result<()> {
    let mut sessions = sessions.lock().await;
    if let Some(session) = sessions.get(&msg.chat.id.0) {
        let stats = format_practice_stats(session);
        let message = format!("Practice mode stopped!\n{}", stats);
        bot.send_message(msg.chat.id, message).await?;
    } else {
        bot.send_message(msg.chat.id, "Practice mode stopped!").await?;
    }
    sessions.remove(&msg.chat.id.0);
    Ok(())
}

fn format_practice_stats(session: &PracticeSession) -> String {
    let accuracy = if session.words_practiced > 0 {
        (session.correct_answers as f64 / session.words_practiced as f64) * 100.0
    } else {
        0.0
    };

    format!(
        "\n📊 Статистика практики:\nСлов пройдено: {}\nПравильно: {}\nНеправильно: {}\nТочность: {:.1}%",
        session.words_practiced,
        session.correct_answers,
        session.wrong_answers,
        accuracy
    )
}