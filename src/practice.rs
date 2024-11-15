use std::fs;

use serde::Deserialize;
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
        let mut message = match &self.result {
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
        };

        if !self.feedback.is_empty() {
            message.push_str("\n");
            message.push_str(&self.feedback);
        }

        message
    }
}

#[derive(Clone)]
pub struct PracticeSession {
    current_word: Translation,
    current_sentence: Option<PracticeSentence>,
    practice_type: PracticeType,
    expecting_russian: bool,
    words_practiced: u32,
    correct_answers: u32,
    wrong_answers: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PracticeSentence {
    pub german_sentence: String,
    pub russian_translation: String,
    pub missing_word: String,
}

#[derive(Clone)]
pub enum PracticeType {
    WordTranslation,
    SentenceCompletion,
}

fn load_practice_sentences() -> Result<Vec<PracticeSentence>> {
    let file_path = std::env::current_dir()?.join("practice_sentences.json");
    let file_content = fs::read_to_string(file_path)?;
    let sentences: Vec<PracticeSentence> = serde_json::from_str(&file_content)?;
    Ok(sentences)
}

fn get_random_sentence(sentences: &[PracticeSentence]) -> Option<PracticeSentence> {
    use rand::seq::SliceRandom;
    let mut rng = rand::thread_rng();
    sentences.choose(&mut rng).cloned()
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
    let practice_sentences = load_practice_sentences()?;

    if translations.is_empty() || practice_sentences.is_empty() {
        bot.send_message(msg.chat.id, "No words or practice sentences available!")
            .await?;
        return Ok(());
    }

    let practice_type = if rand::random() {
        PracticeType::WordTranslation
    } else {
        PracticeType::SentenceCompletion
    };

    let (question, session) = match practice_type {
        PracticeType::WordTranslation => {
            let translation = get_weighted_translation(&translations)
                .ok_or("Failed to get weighted translation")?;
            let expecting_russian = rand::random::<bool>();
            let question = format_practice_question(&translation, expecting_russian);
            
            (question, PracticeSession {
                current_word: translation,
                current_sentence: None,
                practice_type,
                expecting_russian,
                words_practiced: 0,
                correct_answers: 0,
                wrong_answers: 0,
            })
        },
        PracticeType::SentenceCompletion => {
            let sentence = get_random_sentence(&practice_sentences)
                .ok_or("Failed to get practice sentence")?;
            let question = format!(
                "Заполните пропуск правильным словом:\n\n{}\n\nПеревод: {}",
                sentence.german_sentence,
                sentence.russian_translation
            );
            
            (question, PracticeSession {
                current_word: Translation::default(), // You'll need to implement Default for Translation
                current_sentence: Some(sentence),
                practice_type,
                expecting_russian: false,
                words_practiced: 0,
                correct_answers: 0,
                wrong_answers: 0,
            })
        }
    };

    let mut sessions = sessions.lock().await;
    sessions.insert(msg.chat.id.0, session);

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
        let (is_correct, feedback) = match &session.practice_type {
            PracticeType::WordTranslation => {
                let check_result = check_answer(answer, &session.current_word, session.expecting_russian);
                let is_correct = matches!(check_result.result, AnswerResult::Correct);
                (is_correct, check_result.format_message())
            },
            PracticeType::SentenceCompletion => {
                if let Some(sentence) = &session.current_sentence {
                    let is_correct = answer.trim().to_lowercase() == sentence.missing_word.to_lowercase();
                    let feedback = if is_correct {
                        "✅ Правильно!".to_string()
                    } else {
                        format!("❌ Неправильно! Правильный ответ: {}", sentence.missing_word)
                    };
                    (is_correct, feedback)
                } else {
                    (false, "Error: No practice sentence available".to_string())
                }
            }
        };

        // Update statistics
        session.words_practiced += 1;
        if is_correct {
            session.correct_answers += 1;
        } else {
            session.wrong_answers += 1;
        }

        // Format response
        let mut response = feedback;
        if session.words_practiced % STATS_INTERVAL == 0 {
            response.push_str(&format_practice_stats(&session));
        }

        // Update word statistics in database if it's a word translation
        if let PracticeType::WordTranslation = session.practice_type {
            let word = if session.expecting_russian {
                &session.current_word.original
            } else {
                &session.current_word.translation
            };
            update_translation_stats(word, is_correct)?;
        }

        bot.send_message(msg.chat.id, response).await?;

        // If correct, get next practice item
        if is_correct {
            let translations = read_translations()?;
            let practice_sentences = load_practice_sentences()?;
            
            let practice_type = if rand::random() {
                PracticeType::WordTranslation
            } else {
                PracticeType::SentenceCompletion
            };

            let question = match practice_type {
                PracticeType::WordTranslation => {
                    if let Some(next_translation) = get_weighted_translation(&translations) {
                        let expecting_russian = rand::random::<bool>();
                        session.current_word = next_translation.clone();
                        session.current_sentence = None;
                        session.practice_type = practice_type;
                        session.expecting_russian = expecting_russian;
                        format_practice_question(&next_translation, expecting_russian)
                    } else {
                        return Ok(());
                    }
                },
                PracticeType::SentenceCompletion => {
                    if let Some(sentence) = get_random_sentence(&practice_sentences) {
                        session.current_sentence = Some(sentence.clone());
                        session.current_word = Translation::default();
                        session.practice_type = practice_type;
                        format!(
                            "Заполните пропуск правильным словом:\n\n{}\n\nПеревод: {}",
                            sentence.german_sentence,
                            sentence.russian_translation
                        )
                    } else {
                        return Ok(());
                    }
                }
            };

            bot.send_message(msg.chat.id, question).await?;
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