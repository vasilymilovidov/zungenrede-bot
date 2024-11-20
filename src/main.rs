mod ai;
mod commands_messages;
mod consts;
mod input;
mod practice;
mod story;
mod talk;
mod translation;
mod picture;

use commands_messages::{handle_command, handle_document, handle_message, Command, DeleteMode};
use practice::PracticeSession;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use talk::TalkSession;
use picture::PictureSession;
use teloxide::prelude::*;
use tokio::sync::{broadcast, Mutex};
use translation::get_storage_path;

type PracticeSessions = Arc<Mutex<HashMap<i64, PracticeSession>>>;
type TalkSessions = Arc<Mutex<HashMap<i64, TalkSession>>>;
type PictureSessions = Arc<Mutex<HashMap<i64, PictureSession>>>;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    log::info!("Starting translation bot...");

    if let Some(parent) = std::path::Path::new(&get_storage_path()).parent() {
        std::fs::create_dir_all(parent).expect("Failed to create storage directory");
    }

    let bot = Bot::from_env();
    let (shutdown_tx, _) = broadcast::channel(1);
    let sessions: PracticeSessions = Arc::new(Mutex::new(HashMap::new()));
    let talk_sessions: TalkSessions = Arc::new(Mutex::new(HashMap::new()));
    let picture_sessions: PictureSessions = Arc::new(Mutex::new(HashMap::new()));
    let delete_mode: DeleteMode = Arc::new(Mutex::new(HashSet::new()));
    let use_chatgpt = Arc::new(Mutex::new(false));

    let shutdown_tx_clone = shutdown_tx.clone();
    let sessions_clone = sessions.clone();
    let talk_sessions_clone = talk_sessions.clone();
    let picture_sessions_clone = picture_sessions.clone();
    let delete_mode_clone = delete_mode.clone();
    let use_chatgpt_clone = use_chatgpt.clone();

    let message_handler = Update::filter_message()
        .branch(dptree::entry().filter_command::<Command>().endpoint(
            move |bot: Bot, msg: Message, cmd: Command| {
                let shutdown = shutdown_tx_clone.clone();
                let sessions = sessions_clone.clone();
                let talk_sessions = talk_sessions_clone.clone();
                let picture_sessions = picture_sessions_clone.clone();
                let delete_mode = delete_mode_clone.clone();
                let use_chatgpt = use_chatgpt_clone.clone();
                async move {
                    if let Err(e) = handle_command(
                        &bot,
                        &msg,
                        cmd,
                        &shutdown,
                        &sessions,
                        &talk_sessions,
                        &picture_sessions,
                        &delete_mode,
                        &use_chatgpt,
                    )
                    .await
                    {
                        log::error!("Error: {:?}", e);
                    }
                    ResponseResult::Ok(())
                }
            },
        ))
        .branch(
            dptree::filter(|msg: Message| msg.document().is_some()).endpoint(
                move |bot: Bot, msg: Message| async move {
                    if let Err(e) = handle_document(&bot, &msg).await {
                        log::error!("Error: {:?}", e);
                    }
                    ResponseResult::Ok(())
                },
            ),
        )
        .branch(
            dptree::filter(|msg: Message| msg.text().is_some()).endpoint(
                move |bot: Bot, msg: Message| {
                    let sessions = sessions.clone();
                    let talk_sessions = talk_sessions.clone();
                    let picture_sessions = picture_sessions.clone();
                    let delete_mode = delete_mode.clone();
                    let use_chatgpt = use_chatgpt.clone();
                    async move {
                        if let Err(e) =
                            handle_message(&bot, &msg, &sessions, &talk_sessions, &picture_sessions, &delete_mode, &use_chatgpt).await
                        {
                            log::error!("Error: {:?}", e);
                        }
                        ResponseResult::Ok(())
                    }
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
