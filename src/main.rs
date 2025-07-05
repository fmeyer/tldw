mod processing;
mod summarizer;

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
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	env_logger::init();
	let args = Args::parse();
	debug!("Debug enabled");
	debug!("Video URL: {}", &args.video_url);
	debug!("Engine: {}", &args.engine);

	let api_key = env::var("OPENAI_API_KEY").expect("Missing OPENAI_API_KEY environment variable");

	let subtitle_result = processing::download_subtitles(&args.video_url);

	let model = match args.engine {
		4 => "gpt-4o-mini-2024-07-18",
		3 => "gpt-4-turbo-2024-04-09",
		2 => "gpt-3.5-turbo-0125",
		1 => "gpt-4o-2024-05-13",
		_ => "gpt-4",
	};

	let gpt_client =
		summarizer::build_chat_client(api_key.clone()).expect("Could not build GPT client");

	let result: String = match subtitle_result {
		Ok(subtitle_file_path) => {
			let input = processing::process_subtitles(&subtitle_file_path);
			debug!("Subtitle processing successful. Input length: {}", input.len());

			if input.len() == 0 {
				debug!("WARNING: Subtitle processing returned empty content!");
				"Error: No subtitle content was extracted from the video.".to_string()
			} else if input.len() > MAX_TOKENS {
				debug!(
					"Processing long input with {} chunks",
					(input.len() + MAX_TOKENS - 1) / MAX_TOKENS
				);
				match summarizer::process_long_input(gpt_client, input, 2, model).await {
					Ok(result) => {
						debug!("Long input processing complete. Result length: {}", result.len());
						if result.is_empty() {
							debug!("WARNING: OpenAI API returned empty result for long input!");
							"Error: OpenAI API returned empty response. This usually indicates:\n1. API quota exceeded - check your billing at https://platform.openai.com/account/billing\n2. Invalid API key - check your API key at https://platform.openai.com/account/api-keys\n3. Content filtering - the content may have been filtered out".to_string()
						} else {
							result
						}
					},
					Err(e) => {
						debug!("Long input processing failed: {}", e);
						let error_msg = e.to_string();
						if error_msg.contains("insufficient_quota")
							|| error_msg.contains("exceeded your current quota")
						{
							format!("Error: OpenAI API quota exceeded. Please check your billing and add credits at https://platform.openai.com/account/billing")
						} else if error_msg.contains("invalid_api_key")
							|| error_msg.contains("Incorrect API key")
						{
							format!("Error: Invalid OpenAI API key. Please check your API key at https://platform.openai.com/account/api-keys")
						} else {
							format!("Error processing long input: {}", e)
						}
					},
				}
			} else {
				debug!("Processing short input");
				match summarizer::process_short_input(gpt_client, input, args.prompt, model).await {
					Ok(result) => {
						debug!("Short input processing complete. Result length: {}", result.len());
						if result.is_empty() {
							debug!("WARNING: OpenAI API returned empty result for short input!");
							"Error: OpenAI API returned empty response. This usually indicates:\n1. API quota exceeded - check your billing at https://platform.openai.com/account/billing\n2. Invalid API key - check your API key at https://platform.openai.com/account/api-keys\n3. Content filtering - the content may have been filtered out".to_string()
						} else {
							result
						}
					},
					Err(e) => {
						debug!("Short input processing failed: {}", e);
						let error_msg = e.to_string();
						if error_msg.contains("insufficient_quota")
							|| error_msg.contains("exceeded your current quota")
						{
							format!("Error: OpenAI API quota exceeded. Please check your billing and add credits at https://platform.openai.com/account/billing")
						} else if error_msg.contains("invalid_api_key")
							|| error_msg.contains("Incorrect API key")
						{
							format!("Error: Invalid OpenAI API key. Please check your API key at https://platform.openai.com/account/api-keys")
						} else {
							format!("Error processing short input: {}", e)
						}
					},
				}
			}
		},
		Err(e) => {
			debug!("Subtitle download failed: {}", e);
			format!("yt-dlp command failed: {}", e)
		},
	};

	let video_id = extract_video_id(&args.video_url);
	let filename = generate_filename(video_id.unwrap_or("unknown"));
	debug!("Writing result to file: {}", filename);
	debug!("Final result length: {}", result.len());
	debug!("Result preview (first 200 chars): {}", result.chars().take(200).collect::<String>());
	write_to_file(&filename, &result).expect("Failed to write to the file");
	debug!("File written successfully");

	Ok(())
}
