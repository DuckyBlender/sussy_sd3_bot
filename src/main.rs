use std::collections::HashMap;

use comfyui_rs::ClientError;
use log::{error, info};
use teloxide::types::{ChatAction, InputFile};
use teloxide::{prelude::*, utils::command::BotCommands};

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    pretty_env_logger::init();
    info!("Starting command bot...");

    let bot = Bot::from_env();
    bot.set_my_commands(Command::bot_commands()).await.unwrap();
    info!(
        "{} has started!",
        bot.get_me().send().await.unwrap().user.username.unwrap()
    );

    Command::repl(bot, answer).await;
}

#[derive(BotCommands, Clone)]
#[command(
    rename_rule = "lowercase",
    description = "These commands are supported:"
)]
enum Command {
    #[command(description = "display this text.")]
    Help,
    #[command(description = "sd3")]
    SD3(String),
}

async fn answer(bot: Bot, msg: Message, cmd: Command) -> ResponseResult<()> {
    match cmd {
        Command::Help => {
            bot.send_message(msg.chat.id, Command::descriptions().to_string())
                .await?
        }
        Command::SD3(prompt) => {
            // Check if prompt is empty
            let prompt = prompt.trim();
            if prompt.is_empty() {
                bot.send_message(msg.chat.id, "Please provide a prompt")
                    .await?;
                return Ok(());
            }

            // Send generating... message
            let generating_message = bot
                .send_message(msg.chat.id, "Generating image...")
                .reply_to_message_id(msg.id)
                .disable_notification(true)
                .await?;

            // Send typing indicator
            bot.send_chat_action(msg.chat.id, ChatAction::Typing)
                .await?;

            // Send the response to dalle 3
            let now = std::time::Instant::now();
            let imgs = process_image_generation(&prompt).await;
            bot.send_chat_action(msg.chat.id, ChatAction::UploadPhoto)
                .await?;
            let elapsed = now.elapsed().as_secs_f32();

            match imgs {
                Ok(imgs) => {
                    // Remove the empty images and get the correct image
                    let img = imgs
                        .iter()
                        .filter(|(_, img)| !img.is_empty())
                        .map(|(_, img)| img)
                        .next()
                        .unwrap();
                    let img = InputFile::memory(img.to_vec());
                    let res = bot
                        .send_photo(msg.chat.id, img)
                        .caption(format!("{prompt} | Generated image in {:.2}s", elapsed))
                        .reply_to_message_id(msg.id)
                        .await?;
                    bot.delete_message(generating_message.chat.id, generating_message.id)
                        .await?;
                    res
                }
                Err(e) => {
                    error!("Error generating image: {}", e);
                    bot.edit_message_text(
                        generating_message.chat.id,
                        generating_message.id,
                        format!("Failed to generate image: {}", e),
                    )
                    .await?
                }
            }
        }
    };
    Ok(())
}

pub async fn process_image_generation(
    prompt: &str,
) -> Result<HashMap<String, Vec<u8>>, ClientError> {
    let client = comfyui_rs::Client::new("127.0.0.1:8188");

    let json_prompt = serde_json::from_str(include_str!("../comfyui-rs/jsons/sd3.json")).unwrap();

    let mut json_prompt: serde_json::Value = json_prompt;
    json_prompt["16"]["inputs"]["text"] = serde_json::Value::String(prompt.to_string());
    json_prompt["3"]["inputs"]["seed"] =
        serde_json::Value::Number(serde_json::Number::from(rand::random::<u64>()));
    let images = client.get_images(json_prompt).await;
    if images.is_err() {
        return Err(images.err().unwrap());
    }
    Ok(images.unwrap())
}
