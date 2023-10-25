use log::debug;
use regex::Regex;
use std::fs;
use std::fs::File;
use std::process::{Command, ExitStatus, Stdio};
use std::collections::HashSet;
use std::io::{BufRead, BufReader};

//TODO(fm): use a temp folder to store the output file, keep a copy of the original file in somewhere to avoid downloading it again
pub fn process_subtitles() -> String {
    let output_file = "/tmp/output.en.vtt";
    let cleaned_subtitles = process_file(output_file);

    // Clean up regular file
    fs::remove_file(output_file).expect("Failed to remove subtitle file");

    return cleaned_subtitles;
}

pub fn download_subtitles(video_url: &String) -> std::result::Result<Option<i32>, ExitStatus> {
    debug!("downloading subtitle");

    let file = File::create("/tmp/out.txt").unwrap();

    let stdio = Stdio::from(file);

    let status = Command::new("yt-dlp")
        .args(&[
            "--write-sub",
            "--write-auto-sub",
            "--skip-download",
            "--sub-lang",
            "en",
            "--output",
            "/tmp/output", //TODO(fm): replace with videoID
            &video_url,
        ])
        .stdout(stdio)
        .status()
        .expect("Failed to execute yt-dlp command");
    fs::remove_file("/tmp/out.txt").expect("Failed to remove output file");
    if status.success() {
        debug!("done");
        Ok(status.code())
    } else {
        Err(status)
    }
}

fn process_file(filename: &str) -> String  {
    let file = File::open(filename).expect("Failed to open file");
    let reader = BufReader::new(file);

    let mut seen = HashSet::new();
    let mut buffer = Vec::new();

    for line in reader.lines() {
        let line = line.unwrap();
        let cleaned_line = cleanup_buffer(line);
        if cleaned_line.contains(":") {
            continue;
        }

        if !seen.contains(&cleaned_line) {
            buffer.push(cleaned_line.clone());
            seen.insert(cleaned_line);
        }
    }
    let combined_string = buffer.join("\n");
    return combined_string;
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
