use std::collections::HashMap;
use std::sync::Arc;
use teloxide::{payloads::SendPhotoSetters, prelude::Requester, types::{InputFile, Message}, Bot};
use tokio::sync::Mutex;
use serde::Deserialize;
use rand::seq::SliceRandom;
use rand::Rng;
use url::Url;

use crate::ai::{make_claude_request, ClaudeMessage, ClaudeRequest};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

const GRAMMAR_CHECK_PROMPT: &str = "Ты преподаватель немецкого. Проверь следующее описание фотографии на предмет грамматических ошибок и поправь их. Вот описание:\n\n";

#[derive(Clone)]
pub struct PictureSession {
    last_image_url: Option<String>,
}

impl PictureSession {
    fn new() -> Self {
        Self {
            last_image_url: None,
        }
    }
}

pub type PictureSessions = Arc<Mutex<HashMap<i64, PictureSession>>>;

#[derive(Deserialize)]
struct PixabayImage {
    #[serde(rename = "webformatURL")]
    webformat_url: String,
}

fn get_random_search_params() -> (String, u32) {
    let mut rng = rand::thread_rng();
    let search_term = SEARCH_TERMS.choose(&mut rng).unwrap().to_string();
    let page = rng.gen_range(1..=5);
    (search_term, page)
}

async fn fetch_random_image() -> Result<String> {
    let api_key = std::env::var("PIXABAY_API_KEY")?;
    let (search_term, page) = get_random_search_params();
    
    let url = format!(
        "https://pixabay.com/api/?key={}&q={}&image_type=photo&safesearch=true&order=random&page={}&per_page=50",
        api_key, search_term, page
    );
    
    let response = reqwest::get(&url).await?.json::<PixabayResponse>().await?;
    
    response.hits
        .choose(&mut rand::thread_rng())
        .map(|image| image.webformat_url.clone())
        .ok_or_else(|| "No images found".into())
}

async fn check_grammar(description: &str) -> Result<String> {
    let prompt = format!("{}{}", GRAMMAR_CHECK_PROMPT, description);
    
    let request = ClaudeRequest {
        model: "claude-3-opus-20240229".to_string(),
        max_tokens: 1000,
        messages: vec![ClaudeMessage {
            role: "user".to_string(),
            content: prompt,
        }],
    };

    let response = make_claude_request(&request).await?;
    Ok(response.content[0].text.clone())
}

pub async fn start_picture_session(
    bot: &Bot,
    msg: &Message,
    sessions: &PictureSessions,
) -> Result<()> {
    let mut sessions = sessions.lock().await;
    let chat_id = msg.chat.id;

    if sessions.contains_key(&chat_id.0) {
        bot.send_message(
            msg.chat.id,
            "Du bist bereits im Bildbeschreibungsmodus. Benutze /stoppic um den Modus zu beenden.",
        )
        .await?;
        return Ok(());
    }

    let image_url = fetch_random_image().await?;
    let url = Url::parse(&image_url)?;
    
    bot.send_photo(msg.chat.id, InputFile::url(url))
        .caption("Bitte beschreibe dieses Bild auf Deutsch. Was siehst du? Was passiert im Bild?")
        .await?;

    let mut session = PictureSession::new();
    session.last_image_url = Some(image_url);
    sessions.insert(chat_id.0, session);

    Ok(())
}

pub async fn stop_picture_session(
    bot: &Bot,
    msg: &Message,
    sessions: &PictureSessions,
) -> Result<()> {
    let mut sessions = sessions.lock().await;
    let chat_id = msg.chat.id;

    if sessions.remove(&chat_id.0).is_some() {
        bot.send_message(msg.chat.id, "Bildbeschreibungsmodus beendet.")
            .await?;
    } else {
        bot.send_message(
            msg.chat.id,
            "Du bist nicht im Bildbeschreibungsmodus. Benutze /pic um zu starten.",
        )
        .await?;
    }

    Ok(())
}

pub async fn handle_picture_message(
    bot: &Bot,
    msg: &Message,
    sessions: &PictureSessions,
) -> Result<()> {
    if let Some(text) = msg.text() {
        let feedback = check_grammar(text).await?;
        bot.send_message(msg.chat.id, feedback).await?;

        // Send a new image for the next round
        let image_url = fetch_random_image().await?;
        let url = Url::parse(&image_url)?;
        bot.send_photo(msg.chat.id, InputFile::url(url))
            .caption("Gut gemacht! Hier ist das nächste Bild. Was siehst du?")
            .await?;

        let mut sessions = sessions.lock().await;
        if let Some(session) = sessions.get_mut(&msg.chat.id.0) {
            session.last_image_url = Some(image_url);
        }
    }

    Ok(())
}

#[derive(Deserialize)]
struct PixabayResponse {
    hits: Vec<PixabayImage>,
}

const SEARCH_TERMS: &[&str] = &[
    "people", "city", "nature", "food", "animal", "travel",
    "building", "work", "sport", "family", "garden", "market"
];
