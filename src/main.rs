mod processing;
mod summarizer;

use chatgpt::prelude::*;
use chrono::prelude::*;
use clap::Parser;
use log::debug;
use regex::Regex;
use std::{env, fs::File, io::Write};
use summarizer::MAX_TOKENS;
use tokio;

#[derive(Parser, Default, Debug)]
#[command(name = "tldw")]
#[command(author = "Fernando Meyer <fm@pobox.com>")]
#[command(version = "0.4.0")]
#[command(help_template = "tldw - summarize youtube videos with ChatGPT\n {author-with-newline} \
                           {about-section}Version: {version} \n {usage-heading} {usage} \n \
                           {all-args} {tab}")]
#[command(about, long_about = None)]
struct Args {
	#[arg(short, long)]
	video_url: String,

	#[arg(short, long, default_value_t = 4)]
	engine: u8,

	#[arg(short, long, default_value_t = 1)]
	prompt: usize,
}

fn extract_video_id(url: &str) -> Option<&str> {
	let re = Regex::new(r"(?i)[/|=]([\w-]{11})").unwrap();
	re.captures(url).and_then(|cap| cap.get(1).map(|m| m.as_str()))
}

fn generate_filename(video_id: &str) -> String {
	let now = Utc::now();
	let date_str = now.format("%Y%m%d").to_string();
	let unix_timestamp = now.timestamp();
	format!("{}_{}_{}.md", date_str, video_id, unix_timestamp)
}

fn write_to_file(filename: &str, content: &str) -> std::io::Result<()> {
	let mut file = File::create(filename)?;
	file.write_all(content.as_bytes())?;
	Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
	let args = Args::parse();
	debug!("Debug enabled");
	debug!("Video URL: {}", &args.video_url);
	debug!("Engine: {}", &args.engine);

	let api_key = env::var("OPENAI_API_KEY").expect("Missing OPENAI_API_KEY environment variable");

	let status = processing::download_subtitles(&args.video_url);

	let chat_engine = match args.engine {
		4 => ChatGPTEngine::Custom("gpt-4o-2024-05-13"),
		_ => ChatGPTEngine::Gpt4,
	};

	let gpt_client =
		summarizer::build_chat_client(api_key, chat_engine).expect("Could not build GPT client");

	let result: String = match status {
		Ok(_v) => {
			let input = processing::process_subtitles();

			if input.len() > MAX_TOKENS {
				summarizer::process_long_input(gpt_client, input, 2).await?
			} else {
				summarizer::process_short_input(gpt_client, input, args.prompt).await?
			}
		},
		Err(e) => {
			format!("yt-dlp command failed with status: {}", e)
		},
	};

	let video_id = extract_video_id(&args.video_url);
	let filename = generate_filename(video_id.unwrap());
	write_to_file(&filename, &result).expect("Failed to write to the file");

	Ok(())
}
