use std::env;
use std::fs;
use std::fs::File;
use std::io::{stdout, Write};
use std::process::{Command, ExitStatus};
use std::process::Stdio;

use chatgpt::prelude::*;
use futures_util::stream::StreamExt;
use regex::{Regex};
use tokio;

const PROMPTS: [&str; 1] = ["Provide an in-depth, graduate-level summary of the following content in a \
    structured outline format. Include any additional relevant information or insights, marking them \
    with <a></a> to indicate that they come from an external source. Enhance the summary by \
    incorporating pertinent quotes from the input text when necessary to clarify or support \
    explanations.\n\n{}"];

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <video_url>", args[0]);
        return Ok(());
    }

    let api_key =
        env::var("OPENAI_API_KEY").expect("Missing OPENAI_API_KEY environment variable");


    let video_url = &args[1];
    let status = download_subtitles(&video_url);

    match status {
        Ok(_v) => {
            process_subtitles(api_key).await?;
        }
        Err(e) => {
            eprintln!("yt-dlp command failed with status: {}", e)
        }
    };

    Ok(())
}

async fn process_subtitles(api_key: String) -> Result<()> {
    let output_file = "output.en.vtt";
    let cleaned_subtitles = vtt_cleanup_pipeline(output_file);

    // Clean up regular file
    fs::remove_file(output_file).expect("Failed to remove subtitle file");

    let client = ChatGPT::new_with_config(
        api_key,
        ModelConfigurationBuilder::default()
            .temperature(0.7)
            .engine(ChatGPTEngine::Gpt35Turbo_0301)
            .build()
            .unwrap(),
    )?;

    let prompt = format!( "{} {}", PROMPTS[0], cleaned_subtitles);
    let stream = client
        .send_message_streaming(prompt)
        .await?;

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
    return Ok(());
}

fn download_subtitles(video_url: &&String) -> std::result::Result<Option<i32>, ExitStatus> {
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
            video_url,
        ])
        .stdout(stdio)
        .status()
        .expect("Failed to execute yt-dlp command");
    fs::remove_file("out.txt").expect("Failed to remove output file");
    if status.success() {
        Ok(status.code())
    } else { Err(status) }
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

    let output = awk.wait_with_output().expect("Failed to wait on awk command");

    let cleaned_text = cleanup_buffer(String::from_utf8_lossy(&output.stdout).to_string());
    cleaned_text
}

fn cleanup_buffer(text: String) -> String {
    let timestamp_regex = Regex::new(r"(\d{2}:\d{2}:\d{2}\.\d{3} --> \d{2}:\d{2}:\d{2}\.\d{3}\n)").unwrap();
    let cleaned_text = timestamp_regex.replace_all(text.as_str(), "");
    let nbsp_regex = Regex::new(r"&nbsp;").unwrap();
    let cleaned_text = nbsp_regex.replace_all(&cleaned_text, " ");
    let empty_line_regex = Regex::new(r"(?m)^\s*\n").unwrap();
    let cleaned_text = empty_line_regex.replace_all(&cleaned_text, "");
    cleaned_text.into_owned()
}
