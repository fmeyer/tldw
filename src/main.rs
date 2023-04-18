use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::env;
use std::fs;
use std::io::{self, Read};
use std::process::Command;
use tokio;
use regex::Regex;


#[derive(Debug, Deserialize, Serialize)]
struct OpenAIRequest {
    messages: Vec<Message>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OpenAIResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Message,
}

async fn get_summary(api_key: &str, text: &str) -> Result<String, Box<dyn std::error::Error>> {
    let api_url = "https://api.openai.com/v1/chat/completions";

    let client = Client::new();

    let prompt = format!(
        "sumarize the following content, in a structured outline way\n\n{}",
        text
    );

    let req_body = OpenAIRequest {
        messages: vec![
            Message {
                role: "system".to_string(),
                content: "You are ChatGPT, a large language model trained by OpenAI.".to_string(),
            },
            Message {
                role: "user".to_string(),
                content: prompt.to_string(),
            },
        ],
    };

    let response = client
        .post(api_url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&json!({
            "messages": req_body.messages,
            "model": "gpt-3.5-turbo",
            "max_tokens": 2046,
            "temperature": 0.7,
            "stop": ["\n"]
        }))
        .send()
        .await?;

    // println!("{}", response.text().await?);

    let openai_response: OpenAIResponse = response.json().await?;

    for m in openai_response.choices {
        println!("Summary and key takeaways:\n{}", m.message.content);
    }

    let buffer = String::new();


    Ok(buffer.clone())
}

fn cleanup_buffer(text: &str) -> String {
    let timestamp_regex = Regex::new(r"(\d{2}:\d{2}:\d{2}\.\d{3} --> \d{2}:\d{2}:\d{2}\.\d{3}\n)").unwrap();
    let cleaned_text = timestamp_regex.replace_all(text, "");
    let nbsp_regex = Regex::new(r"&nbsp;").unwrap();
    let cleaned_text = nbsp_regex.replace_all(&cleaned_text, " ");
    let empty_line_regex = Regex::new(r"(?m)^\s*\n").unwrap();
    let cleaned_text = empty_line_regex.replace_all(&cleaned_text, "");
    cleaned_text.into_owned()
}


fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <video_url>", args[0]);
        return;
    }

    let video_url = &args[1];
    let output_file = "output.en.vtt";

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
        .status()
        .expect("Failed to execute yt-dlp command");

    if status.success() {
        let mut buffer = String::new();
        // Remove annotated timestamps
        let timestamp_regex = Regex::new(r"(\d{2}:\d{2}:\d{2}\.\d{3} --> \d{2}:\d{2}:\d{2}\.\d{3}\n)").unwrap();


        let mut file = fs::File::open(&output_file).expect("Failed to open subtitle file");
        file.read_to_string(&mut buffer)
            .expect("Failed to read subtitle file");


        let cleaned_buffer = cleanup_buffer(&buffer);

        // Clean up regular file
        fs::remove_file(output_file).expect("Failed to remove subtitle file");

        let api_key =
            env::var("OPENAI_API_KEY").expect("Missing OPENAI_API_KEY environment variable");
        let summary = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(get_summary(&api_key, &cleaned_buffer))
            .expect("Failed to get summary from ChatGPT");
    } else {
        eprintln!("yt-dlp command failed with status: {}", status)
    }
}
