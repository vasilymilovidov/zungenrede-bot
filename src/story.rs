use crate::{
    ai::STORY_PROMPT,
    translation::{read_translations, translate_text},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub fn select_random_words(words: &[String], count: usize) -> Vec<String> {
    use rand::seq::IteratorRandom;
    let mut rng = rand::thread_rng();
    words
        .iter()
        .choose_multiple(&mut rng, count.min(words.len()))
        .into_iter()
        .cloned()
        .collect()
}

pub fn get_german_words() -> Result<Vec<String>> {
    let translations = read_translations()?;
    let mut words = Vec::new();

    for translation in translations {
        if !translation.original.contains(' ') {
            words.push(translation.original);
        } else {
            if let Some(word) = translation.original.split_whitespace().last() {
                words.push(word.to_string());
            }
        }

        for example in translation.examples {
            words.extend(
                example
                    .german
                    .split_whitespace()
                    .filter(|w| w.chars().next().map_or(false, |c| c.is_uppercase()))
                    .map(|w| w.trim_matches(|c: char| !c.is_alphabetic()).to_string()),
            );
        }
    }

    words.sort();
    words.dedup();

    Ok(words)
}

pub async fn generate_story(use_chatgpt: bool) -> Result<String> {
    let words = get_german_words()?;
    let selected_words = select_random_words(&words, 100);

    let prompt = format!(
        "STORY_GENERATION:{}",
        STORY_PROMPT.replace("{word list}", &selected_words.join(", "))
    );
    translate_text(&prompt, use_chatgpt).await
}