#[derive(Debug)]
pub enum InputType {
    RussianWord,
    RussianSentence,
    GermanWord,
    GermanSentence,
    Explanation,
    GrammarCheck,
    Freeform,
    Simplify,
}

pub fn analyze_input(text: &str) -> InputType {
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
