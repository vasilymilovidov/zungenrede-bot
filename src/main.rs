mod commands_messages;
mod input;
mod practice;
mod promts_consts;
mod translation;

use commands_messages::{handle_command, handle_message, Command};
use practice::PracticeSession;
use std::{collections::HashMap, sync::Arc};
use teloxide::prelude::*;
use tokio::sync::{broadcast, Mutex};
use translation::get_storage_path;

type PracticeSessions = Arc<Mutex<HashMap<i64, PracticeSession>>>;

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

    let shutdown_tx_clone = shutdown_tx.clone();
    let sessions_clone = sessions.clone();

    let message_handler = Update::filter_message()
        .branch(dptree::entry().filter_command::<Command>().endpoint(
            move |bot: Bot, msg: Message, cmd: Command| {
                let shutdown = shutdown_tx_clone.clone();
                let sessions = sessions_clone.clone();
                async move {
                    if let Err(e) = handle_command(&bot, &msg, cmd, &shutdown, &sessions).await {
                        log::error!("Error: {:?}", e);
                    }
                    ResponseResult::Ok(())
                }
            },
        ))
        .branch(
            dptree::filter(|msg: Message| msg.text().is_some()).endpoint(
                move |bot: Bot, msg: Message| {
                    let sessions = sessions.clone();
                    async move {
                        if let Err(e) = handle_message(&bot, &msg, &sessions).await {
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
