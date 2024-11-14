use strsim::jaro_winkler;
use teloxide::{prelude::Requester, types::Message, Bot};

use crate::{translation::*, PracticeSessions};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

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
                format!("⚠️ Почти правильно! Ожидалось: {}\nПохожесть: {:.0}%", 
                    expected, similarity * 100.0)
            },
            AnswerResult::WrongArticle { expected } => {
                format!("❌ Неправильный артикль! Правильный ответ: {}", expected)
            },
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

fn check_answer(answer: &str, translation: &Translation, expecting_russian: bool) -> AnswerCheck {
    let answer = answer.trim().to_lowercase();
    
    // Helper function to check string similarity
    fn is_similar(a: &str, b: &str) -> f64 {
        let similarity = jaro_winkler(a, b);
        similarity
    }

    // Helper function to normalize text for comparison
    fn normalize(text: &str) -> String {
        text.trim()
            .to_lowercase()
            .chars()
            .filter(|c| c.is_alphabetic() || c.is_whitespace())
            .collect()
    }

    if expecting_russian {
        let expected = normalize(&translation.translation);
        let answer = normalize(&answer);

        // Get possible variations of correct answers
        let mut correct_variants = vec![expected.clone()];
        
        // Add normalized variants without punctuation
        correct_variants.extend(translation.examples.iter()
            .map(|ex| normalize(&ex.russian)));

        // Check for exact matches first
        if correct_variants.contains(&answer) {
            return AnswerCheck {
                result: AnswerResult::Correct,
                feedback: "".to_string(),
            };
        }

        // Check for similar answers
        let best_match = correct_variants.iter()
            .map(|variant| is_similar(&answer, variant))
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(0.0);

        if best_match > 0.85 {
            return AnswerCheck {
                result: AnswerResult::AlmostCorrect {
                    expected: translation.translation.clone(),
                    similarity: best_match,
                },
                feedback: "".to_string(),
            };
        }

        return AnswerCheck {
            result: AnswerResult::Wrong {
                expected: translation.translation.clone(),
            },
            feedback: "".to_string(),
        };

    } else {
        // Handling German answers
        let is_noun = translation.grammar_forms.first()
            .map(|form| ["der", "die", "das"].contains(&form.trim()))
            .unwrap_or(false);

        if is_noun {
            let expected_article = translation.grammar_forms.first()
                .map(|a| a.trim().to_lowercase())
                .unwrap_or_default();
            let expected_noun = normalize(&translation.original);
            let expected = format!("{} {}", expected_article, expected_noun);

            let answer_parts: Vec<&str> = answer.split_whitespace().collect();
            
            match answer_parts.as_slice() {
                [article, noun, ..] => {
                    let article = article.to_lowercase();
                    let noun = normalize(noun);

                    if article == expected_article {
                        let similarity = is_similar(&noun, &expected_noun);
                        if similarity > 0.85 {
                            return AnswerCheck {
                                result: AnswerResult::Correct,
                                feedback: "".to_string(),
                            };
                        } else {
                            return AnswerCheck {
                                result: AnswerResult::AlmostCorrect {
                                    expected: expected.clone(),
                                    similarity,
                                },
                                feedback: "".to_string(),
                            };
                        }
                    } else {
                        return AnswerCheck {
                            result: AnswerResult::WrongArticle { expected },
                            feedback: "".to_string(),
                        };
                    }
                }
                _ => return AnswerCheck {
                    result: AnswerResult::Wrong { expected },
                    feedback: "Не забудьте указать артикль!".to_string(),
                },
            }
        } else {
            // Non-noun German words
            let expected = normalize(&translation.original);
            let answer = normalize(&answer);

            // Get possible variations including conjugations
            let mut correct_variants = vec![expected.clone()];
            if let Some(conjugations) = &translation.conjugations {
                correct_variants.extend(conjugations.iter()
                    .filter_map(|conj| conj.split_whitespace().last())
                    .map(normalize));
            }

            // Add examples
            correct_variants.extend(translation.examples.iter()
                .flat_map(|ex| ex.german.split_whitespace())
                .map(normalize));

            // Check for exact matches
            if correct_variants.contains(&answer) {
                return AnswerCheck {
                    result: AnswerResult::Correct,
                    feedback: "".to_string(),
                };
            }

            // Check for similar answers
            let best_match = correct_variants.iter()
                .map(|variant| is_similar(&answer, variant))
                .max_by(|a, b| a.partial_cmp(b).unwrap())
                .unwrap_or(0.0);

            if best_match > 0.85 {
                return AnswerCheck {
                    result: AnswerResult::AlmostCorrect {
                        expected: translation.original.clone(),
                        similarity: best_match,
                    },
                    feedback: "".to_string(),
                };
            }

            return AnswerCheck {
                result: AnswerResult::Wrong {
                    expected: translation.original.clone(),
                },
                feedback: "".to_string(),
            };
        }
    }
}

pub async fn check_practice_answer(
    bot: &Bot,
    msg: &Message,
    sessions: &PracticeSessions,
) -> Result<()> {
    let mut sessions = sessions.lock().await;

    if let Some(session) = sessions.get(&msg.chat.id.0) {
        let answer = msg.text().unwrap_or("").trim();
        let check_result = check_answer(
            answer,
            &session.current_word,
            session.expecting_russian,
        );

        let mut response = check_result.format_message();
        if !check_result.feedback.is_empty() {
            response.push_str(&format!("\n{}", check_result.feedback));
        }

        bot.send_message(msg.chat.id, response).await?;

        // Only proceed to next word if answer was correct
        if matches!(check_result.result, AnswerResult::Correct) {
            // Send next word
            let translations = read_translations()?;
            if let Some(next_translation) = get_random_translation(&translations) {
                let expecting_russian = rand::random::<bool>();
                let question = if expecting_russian {
                    format!("Переведите на русский:\n👅{}", next_translation.original)
                } else {
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
