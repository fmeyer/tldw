use std::env;
use std::fs;
use std::fs::File;
use std::io::{stdout, Write};
use std::process::Stdio;
use std::process::{Command, ExitStatus};

use chatgpt::prelude::*;
use clap::Parser;
use futures_util::stream::StreamExt;
use log::{debug, info};
use regex::Regex;
use tokio;

const PROMPTS: [&str; 3] = ["Provide an in-depth, summary of the following content in a \
    structured outline. Include any additional relevant information or insight applying the concepts of smart brevety. Enhance the summary by \
    incorporating a conclusion block when necessary to clarify or support \
    explanations. Ignore sponsorship messages and focus on the overall idea \n The output result should be in markdown markup\n\n{}",
    "system: I need you to create a comprehensive, detailed summary of the provided content in a clearly structured outline. Make sure to add any significant information or insights that are related to smart brevity principles. To strengthen the summary, don't hesitate to include a conclusion section if it helps in clarifying or supporting explanations. Please specifically omit any messages pertaining to sponsorship, and prioritize the overarching idea. The finalized product should be delivered in markdown format.",
    "outline. Make sure to add any significant information or insights that are related to smart brevity principles. This is a partial input, therefore don't provide introduction or conclusions unless the content mentions it. Please specifically omit any messages pertaining to sponsorship, and prioritize the overarching idea. The finalized product should be delivered in markdown format.",
    ];

#[derive(Parser, Default, Debug)]
#[command(name = "tldw")]
#[command(author = "Fernando Meyer <fm@pobox.com>")]
#[command(version = "0.3.0")]
#[command(help_template = "tldw - sumarize youtube videos with ChatGPT\
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

    let status = download_subtitles(args.video_url);

    let chat_engine = match args.engine {
        4 => ChatGPTEngine::Gpt4,
        _ => ChatGPTEngine::Gpt35Turbo,
    };

    let client = build_chat_client(api_key, chat_engine).expect("Could not build GPT client");

    match status {
        Ok(_v) => {
            let cleaned_subtitles = process_subtitles();
            process_short_input(client, cleaned_subtitles, args.prompt).await?
        }
        Err(e) => {
            eprintln!("yt-dlp command failed with status: {}", e)
        }
    };

    Ok(())
}

fn process_subtitles() -> String {
    let output_file = "output.en.vtt";
    let cleaned_subtitles = vtt_cleanup_pipeline(output_file);

    // Clean up regular file
    fs::remove_file(output_file).expect("Failed to remove subtitle file");

    return cleaned_subtitles;
}

async fn process_short_input(
    client: ChatGPT,
    cleaned_subtitles: String,
    prompt: usize,
) -> Result<()> {
    let prompt = format!("{} {}", PROMPTS[prompt], cleaned_subtitles);
    let stream = client.send_message_streaming(prompt).await?;

    // Iterating over stream contents
    stream
        .for_each(|each| async move {
            match each {
                ResponseChunk::Content {
                    delta,
                    response_index: _,
                } => {
                    // Printing part of response without the newline
                    print!("{delta}");
                    // Manually flushing the standard output, as `print` macro does not do that
                    stdout().lock().flush().unwrap();
                }
                _ => {}
            }
        })
        .await;
    Ok(())
}

fn build_chat_client(api_key: String, engine: ChatGPTEngine) -> Result<ChatGPT> {
    let client = ChatGPT::new_with_config(
        api_key,
        ModelConfigurationBuilder::default()
            .temperature(0.7)
            .engine(engine)
            .build()
            .unwrap(),
    )?;
    Ok(client)
}

fn download_subtitles(video_url: String) -> std::result::Result<Option<i32>, ExitStatus> {
    debug!("downloading subtitle");

    let file = File::create("out.txt").unwrap();

    let stdio = Stdio::from(file);

    let status = Command::new("yt-dlp")
        .args(&[
            "--write-sub",
            "--write-auto-sub",
            "--skip-download",
            "--sub-lang",
            "en",
            "--output",
            "output",
            &video_url,
        ])
        .stdout(stdio)
        .status()
        .expect("Failed to execute yt-dlp command");
    fs::remove_file("out.txt").expect("Failed to remove output file");
    if status.success() {
        debug!("done");
        Ok(status.code())
    } else {
        Err(status)
    }
}

fn vtt_cleanup_pipeline(output_file: &str) -> String {
    // cat command
    let cat = Command::new("cat")
        .arg(output_file)
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to execute cat command");

    // grep command
    let grep = Command::new("grep")
        .arg("-v")
        .arg(":")
        .stdin(cat.stdout.unwrap())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to execute grep command");

    // awk command
    let awk = Command::new("awk")
        .arg("!seen[$0]++")
        .stdin(grep.stdout.unwrap())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to execute awk command");

    let output = awk
        .wait_with_output()
        .expect("Failed to wait on awk command");

    let cleaned_text = cleanup_buffer(String::from_utf8_lossy(&output.stdout).to_string());

    debug!("subtitles content: {}", &cleaned_text);

    if cleaned_text.len() >= 2 {
        info!(
            "Input too large: {} - it might generate wrong results",
            &cleaned_text.len()
        );
    }

    cleaned_text
}

fn cleanup_buffer(text: String) -> String {
    let timestamp_regex =
        Regex::new(r"(\d{2}:\d{2}:\d{2}\.\d{3} --> \d{2}:\d{2}:\d{2}\.\d{3}\n)").unwrap();
    let cleaned_text = timestamp_regex.replace_all(text.as_str(), "");
    let nbsp_regex = Regex::new(r"&nbsp;").unwrap();
    let cleaned_text = nbsp_regex.replace_all(&cleaned_text, " ");
    let empty_line_regex = Regex::new(r"(?m)^\s*\n").unwrap();
    let cleaned_text = empty_line_regex.replace_all(&cleaned_text, "");
    cleaned_text.into_owned()
}
