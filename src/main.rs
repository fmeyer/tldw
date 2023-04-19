use std::env;
use std::fs;
use std::io::{self, Read, stdout, Write};
use std::process::Command;

use chatgpt::prelude::*;
use futures_util::stream::StreamExt;
use regex::{Error, Regex};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio;
use tokio::macros::support;
use std::process::Stdio;
use std::fs::File;

fn cleanup_buffer(text: String) -> String {
    let timestamp_regex = Regex::new(r"(\d{2}:\d{2}:\d{2}\.\d{3} --> \d{2}:\d{2}:\d{2}\.\d{3}\n)").unwrap();
    let cleaned_text = timestamp_regex.replace_all(text.as_str(), "");
    let nbsp_regex = Regex::new(r"&nbsp;").unwrap();
    let cleaned_text = nbsp_regex.replace_all(&cleaned_text, " ");
    let empty_line_regex = Regex::new(r"(?m)^\s*\n").unwrap();
    let cleaned_text = empty_line_regex.replace_all(&cleaned_text, "");
    cleaned_text.into_owned()
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <video_url>", args[0]);
        return Ok(());
    }

    let video_url = &args[1];
    let output_file = "output.en.vtt";

    let file = File::create("out.txt").unwrap();;
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






    if status.success() {

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
        // println!("Cleaned text:\n{}", cleaned_text);

        // Clean up regular file
        fs::remove_file(output_file).expect("Failed to remove subtitle file");

        let api_key =
            env::var("OPENAI_API_KEY").expect("Missing OPENAI_API_KEY environment variable");

        // let support = get_summary(&api_key, &cleaned_buffer).await;

        let client = ChatGPT::new_with_config(
            api_key,
            ModelConfigurationBuilder::default()
                .temperature(0.7)
                .engine(ChatGPTEngine::Gpt35Turbo_0301)
                .build()
                .unwrap(),
        )?;


// Acquiring a streamed response
// Note, that the `futures_util` crate is required for most
// stream related utility methods

        println!("processing ....");
        let prompt = format!(
            "sumarize the following content, in a structured outline way\n\n{}",
            cleaned_text
        );
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
    } else {
        eprintln!("yt-dlp command failed with status: {}", status)
    }

    Ok(())
}
