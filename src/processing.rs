use std::fs;
use std::process::{Command, ExitStatus, Stdio};
use log::{debug, info};
use std::fs::File;
use regex::Regex;


//TODO(fm): use a temp folder to store the output file, keep a copy of the original file in somewhere to avoid downloading it again

pub fn process_subtitles() -> String {
    let output_file = "output.en.vtt";
    let cleaned_subtitles = vtt_cleanup_pipeline(output_file);

    // Clean up regular file
    fs::remove_file(output_file).expect("Failed to remove subtitle file");

    return cleaned_subtitles;
}

pub fn download_subtitles(video_url: String) -> std::result::Result<Option<i32>, ExitStatus> {
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

//TODO(fm): Replace this with proper text processing
//TODO(fm): Atempt to fix paragraphs that are broken in the middle of a sentence
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
