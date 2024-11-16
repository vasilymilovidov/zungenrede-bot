use std::{env, fs};

use serde::{Deserialize, Serialize};

use crate::{
    ai::{
        ClaudeMessage, ClaudeRequest, ClaudeResponse, CONTEXT_PROMPT, EXPLANATION_PROMPT,
        FREEFORM_PROMPT, GERMAN_SENTENCE_PROMPT, GERMAN_WORD_PROMPT, GRAMMAR_CHECK_PROMPT,
        RUSSIAN_TO_GERMAN_PROMPT, RUSSIAN_WORD_PROMPT, SIMPLIFY_PROMPT,
    },
    input::{analyze_input, InputType},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Translation {
    pub original: String,
    pub translation: String,
    pub grammar_forms: Vec<String>,
    pub conjugations: Option<Vec<String>>,
    pub examples: Vec<Example>,
    #[serde(default)]
    pub correct_answers: u32,
    #[serde(default)]
    pub wrong_answers: u32,
}

impl Default for Translation {
    fn default() -> Self {
        Self {
            original: String::new(),
            translation: String::new(),
            grammar_forms: Vec::new(),
            conjugations: None,
            examples: Vec::new(),
            correct_answers: 0,
            wrong_answers: 0,
        }
    }
}

impl Translation {
    fn is_valid(&self) -> bool {
        !self.original.trim().is_empty()
            && !self.translation.trim().is_empty()
            && self
                .examples
                .iter()
                .all(|e| !e.german.trim().is_empty() && !e.russian.trim().is_empty())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Example {
    pub german: String,
    pub russian: String,
}

pub fn update_translation_stats(word: &str, correct: bool) -> Result<()> {
    let mut translations = read_translations()?;

    if let Some(translation) = translations.iter_mut().find(|t| {
        t.original.to_lowercase() == word.to_lowercase()
            || t.translation.to_lowercase() == word.to_lowercase()
    }) {
        if correct {
            translation.correct_answers += 1;
        } else {
            translation.wrong_answers += 1;
        }

        write_translations(&translations)?;
    }

    Ok(())
}

pub fn get_weighted_translation(translations: &[Translation]) -> Option<Translation> {
    use rand::Rng;

    if translations.is_empty() {
        return None;
    }

    // Calculate weights based on error rate
    let weights: Vec<f64> = translations
        .iter()
        .map(|t| {
            let total = t.correct_answers + t.wrong_answers;
            if total == 0 {
                // New words get higher weight to ensure they're practiced
                2.0
            } else {
                // Calculate error rate and add 1 to ensure all words have a chance
                1.0 + (t.wrong_answers as f64 / total as f64)
            }
        })
        .collect();

    let total_weight: f64 = weights.iter().sum();
    let mut rng = rand::thread_rng();
    let mut random_value = rng.gen::<f64>() * total_weight;

    for (i, weight) in weights.iter().enumerate() {
        random_value -= weight;
        if random_value <= 0.0 {
            return Some(translations[i].clone());
        }
    }

    // Fallback to first translation if something goes wrong
    Some(translations[0].clone())
}

pub async fn translate_text(text: &str) -> Result<String> {
    let api_key =
        env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY environment variable not set");

    let client = reqwest::Client::new();

    let (system_prompt, processed_text) = if text.starts_with("STORY_GENERATION:") {
        // Special case for story generation
        (text.trim_start_matches("STORY_GENERATION:").to_string(), "")
    } else if text.starts_with("Context: ") {
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
        content: if processed_text.is_empty() {
            system_prompt
        } else {
            format!("{}\n\n{}", system_prompt, processed_text)
        },
    }];

    let request = ClaudeRequest {
        model: "claude-3-5-sonnet-20241022".to_string(),
        max_tokens: 4000,
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

pub fn add_translation(translation: Translation) -> Result<()> {
    if !translation.is_valid() {
        return Err("Invalid translation data".into());
    }

    let mut translations = read_translations()?;

    // Remove existing translations with the same original or translation text
    translations.retain(|t| {
        t.original.to_lowercase() != translation.original.to_lowercase()
            && t.translation.to_lowercase() != translation.translation.to_lowercase()
    });

    translations.push(translation);

    write_translations(&translations)?;

    Ok(())
}

pub fn get_storage_path() -> String {
    std::env::var("STORAGE_FILE").unwrap_or_else(|_| "translations_storage.json".to_string())
}

pub fn read_translations() -> Result<Vec<Translation>> {
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

pub fn find_translation<'a>(
    word: &str,
    translations: &'a [Translation],
) -> Option<&'a Translation> {
    translations.iter().find(|t| {
        t.original.to_lowercase() == word.to_lowercase()
            || t.translation.to_lowercase() == word.to_lowercase()
    })
}

pub fn clear_translations() -> Result<()> {
    let path = get_storage_path();
    fs::write(&path, "[]")?;
    Ok(())
}

pub fn import_translations(json_data: &str) -> Result<usize> {
    let translations: Vec<Translation> = serde_json::from_str(json_data)?;

    if !translations.iter().all(|t| t.is_valid()) {
        return Err("Invalid translation data in import file".into());
    }

    write_translations(&translations)?;
    Ok(translations.len())
}

pub fn delete_translation(word: &str) -> Result<bool> {
    let mut translations = read_translations()?;
    let initial_len = translations.len();

    translations.retain(|t| {
        t.original.to_lowercase() != word.to_lowercase()
            && t.translation.to_lowercase() != word.to_lowercase()
    });

    write_translations(&translations)?;
    Ok(initial_len != translations.len())
}

pub fn parse_translation_response(original: &str, response: &str) -> Translation {
    let lines: Vec<&str> = response.lines().collect();
    let is_russian_input = original
        .chars()
        .any(|c| matches!(c, '\u{0400}'..='\u{04FF}' | '\u{0500}'..='\u{052F}'));

    let mut translation = if is_russian_input {
        Translation {
            original: lines.get(1).unwrap_or(&"").trim().to_string(),
            translation: lines.first().unwrap_or(&original).trim().to_string(),
            grammar_forms: Vec::new(),
            conjugations: None,
            examples: Vec::new(),
            correct_answers: 0,
            wrong_answers: 0,
        }
    } else {
        Translation {
            original: lines.first().unwrap_or(&original).trim().to_string(),
            translation: lines.get(1).unwrap_or(&"").trim().to_string(),
            grammar_forms: Vec::new(),
            conjugations: None,
            examples: Vec::new(),
            correct_answers: 0,
            wrong_answers: 0,
        }
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

                    if is_russian_input {
                        translation.examples.push(Example {
                            german: russian,
                            russian: german,
                        });
                    } else {
                        translation.examples.push(Example { german, russian });
                    }
                }
            }
            current_line += 1;
        }
    }

    if !is_russian_input {
        let words: Vec<&str> = translation.original.split_whitespace().collect();
        if words.len() == 2 && ["der", "die", "das"].contains(&words[0]) {
            translation.grammar_forms.push(words[0].to_string());
            translation.original = words[1].to_string();
        }
    }

    translation
}

pub fn format_translation_response(translation: &Translation) -> String {
    let mut response = String::new();

    let is_noun = translation
        .grammar_forms
        .first()
        .map(|form| {
            form.trim().matches(' ').count() == 0 && ["der", "die", "das"].contains(&form.trim())
        })
        .unwrap_or(false);

    let is_russian = translation
        .original
        .chars()
        .any(|c| matches!(c, '\u{0400}'..='\u{04FF}' | '\u{0500}'..='\u{052F}'));

    let already_has_article = translation
        .original
        .split_whitespace()
        .next()
        .map(|first_word| ["der", "die", "das"].contains(&first_word))
        .unwrap_or(false);

    if is_noun {
        if let Some(article) = translation.grammar_forms.first() {
            if is_russian {
                response.push_str(&format!("➡️ {}\n", translation.original));
                response.push_str(&format!("⬅️ {} {}\n", article, translation.translation));
            } else {
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