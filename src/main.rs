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

const PROMPTS: [&str; 2] = [ "Provide an in-depth, graduate-level summary for the content \
    between [[ ]] . Include any additional relevant information or insights, marking them \
    with <a></a> to indicate that they come from an external source. Enhance the summary by \
    incorporating pertinent quotes from the input text when necessary to clarify or support \
    explanations. The format I expect is a structured outline format\n\n{}",
    "Provide an in-depth, graduate-level summary in a  structured outline format for the content \
    between [[ ]] . Include any additional relevant information or insights, marking them \
    with <a></a> to indicate that they come from an external source. Enhance the summary by \
    incorporating pertinent quotes from the input text when necessary to clarify or support \
    explanations. \n I will send a few messages with the input wait until I send a message containing <ready> to process my input\n\n{}"];

const MAX_TOKENS: usize = 4096;

macro_rules! dprint {
    ($($arg:tt)*) => (#[cfg(debug_assertions)] print!("DEBUG: "); println!($($arg)*));
}

#[tokio::main]
async fn main() -> Result<()> {
    // dprint!("enabled");

    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <video_url>", args[0]);
        return Ok(());
    }

    let api_key =
        env::var("OPENAI_API_KEY").expect("Missing OPENAI_API_KEY environment variable");


    let video_url = &args[1];
    let status = download_subtitles(&video_url);

    let client = build_chat_client(api_key).expect("Could not build GPT client");

    match status {
        Ok(_v) => {
            process_subtitles(client).await?;
        }
        Err(e) => {
            eprintln!("yt-dlp command failed with status: {}", e)
        }
    };

    Ok(())
}

async fn process_subtitles(client: ChatGPT) -> Result<()> {
    let output_file = "output.en.vtt";
    let cleaned_subtitles = vtt_cleanup_pipeline(output_file);
    let prompt = format!("{} {}", PROMPTS[0], cleaned_subtitles);

    dprint!("prompt.len: {}", prompt.len());


    // Clean up regular file
    fs::remove_file(output_file).expect("Failed to remove subtitle file");

    if prompt.len() > MAX_TOKENS {
        process_long_input(client,   cleaned_subtitles).await
    } else {
        process_short_input(client, prompt).await
    }
}

async fn process_short_input(client: ChatGPT, prompt: String) -> Result<()>  {

    let stream = client
        .send_message_streaming(format!("[[ {} ]]", prompt))
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
    Ok(())
}

async fn process_long_input(client: ChatGPT, prompt: String) -> Result<()>  {
    let mut conversation = client.new_conversation();
    let mut output: Vec<ResponseChunk> = Vec::new();

    // let mut stream = client
    //     .send_message_streaming("--: I'm defining the prefix `--:` to inform you that these messages should not be considered as content when you process my input, I'll send you several messages, and you have to combine then and only process the result once I send you a `--: complete` command; inform me in the `--: result` message how many answers I should expect from you as my result the last message should  print `--: done`",)
    //     .await?;
    //
    // // Iterating over a stream and collecting the results into a vector
    // let mut output: Vec<ResponseChunk> = Vec::new();
    // while let Some(chunk) = stream.next().await {
    //     match chunk {
    //         ResponseChunk::Content {
    //             delta,
    //             response_index,
    //         } => {
    //             // Printing part of response without the newline
    //             print!("{delta}");
    //             // Manually flushing the standard output, as `print` macro does not do that
    //             stdout().lock().flush().unwrap();
    //             output.push(ResponseChunk::Content {
    //                 delta,
    //                 response_index,
    //             });
    //         }
    //         // We don't really care about other types, other than parsing them into a ChatMessage later
    //         other => output.push(other),
    //     }
    // }

    // Parsing ChatMessage from the response chunks and saving it to the conversation history
    // let messages = ChatMessage::from_response_chunks(output);
    // conversation.history.push(messages[0].to_owned());

    // dprint!("prompt.len: {}", prompt.len());
    //
    // let n = (prompt.len() % MAX_TOKENS).max(1);
    // dprint!("number of chunks: {}", n);
    //
    // let chunk_size = (prompt.len() as f64 / n as f64).ceil() as usize;
    // dprint!("chunk size: {}", chunk_size);
    //
    //
    // let mut chunks = Vec::new();
    //
    // chunks.push(PROMPTS[1]);

    let mut chunks: Vec<String> = Vec::new();

    chunks.push(PROMPTS[1].to_string());

    let chars: Vec<char> = prompt.chars().collect();
    let mut split = &chars.chunks(MAX_TOKENS)
        .map(|chunk| chunk.iter().collect::<String>())
        .collect::<Vec<_>>();

    chunks.append(&mut split.clone());

    chunks.push("<ready>".to_string());


    for chunk in chunks.iter() {
        let chunk_text = &chunk.to_string();
        dprint!(">>>>>>>>>{}", chunk_text);
        let stream = client.send_message_streaming(chunk_text).await?;
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
    }

    Ok(())
}


fn build_chat_client(api_key: String) -> Result<ChatGPT> {
    let client = ChatGPT::new_with_config(
        api_key,
        ModelConfigurationBuilder::default()
            .temperature(0.7)
            .engine(ChatGPTEngine::Gpt35Turbo_0301)
            .build()
            .unwrap(),
    )?;
    Ok(client)
}

fn download_subtitles(video_url: &&String) -> std::result::Result<Option<i32>, ExitStatus> {
    // dprint!("downloading subtitle");

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
        // dprint!("done");
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


    // dprint!("subtitles content: {}", cleaned_text);
    // dprint!("subtitles length: {}", cleaned_text.len());
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
