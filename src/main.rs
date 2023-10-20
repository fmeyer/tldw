mod processing;
mod summarizer;

use chatgpt::prelude::*;
use clap::Parser;
use log::debug;
use std::env;
use summarizer::MAX_TOKENS;
use tokio;

#[derive(Parser, Default, Debug)]
#[command(name = "tldw")]
#[command(author = "Fernando Meyer <fm@pobox.com>")]
#[command(version = "0.3.0")]
#[command(help_template = "tldw - summarize youtube videos with ChatGPT\
     \n {author-with-newline} {about-section}Version: {version} \n {usage-heading} {usage} \n {all-args} {tab}")]
#[command(about, long_about = None)]
struct Args {
    #[arg(short, long)]
    video_url: String,

    #[arg(short, long, default_value_t = 4)]
    engine: u8,

    #[arg(short, long, default_value_t = 1)]
    prompt: usize,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    debug!("Debug enabled");
    debug!("Video URL: {}", &args.video_url);
    debug!("Engine: {}", &args.engine);



    let api_key = env::var("OPENAI_API_KEY").expect("Missing OPENAI_API_KEY environment variable");

    let status = processing::download_subtitles(args.video_url);

    let chat_engine = match args.engine {
        4 => ChatGPTEngine::Gpt4,
        _ => ChatGPTEngine::Gpt35Turbo,
    };

    let gpt_client =
        summarizer::build_chat_client(api_key, chat_engine).expect("Could not build GPT client");

    match status {
        Ok(_v) => {
            let input = processing::process_subtitles();

            if input.len() > MAX_TOKENS {
                summarizer::process_long_input(gpt_client, input, 2).await?
            } else {
                summarizer::process_short_input(gpt_client, input, args.prompt).await?
            }
        }
        Err(e) => {
            eprintln!("yt-dlp command failed with status: {}", e)
        }
    };

    Ok(())
}
