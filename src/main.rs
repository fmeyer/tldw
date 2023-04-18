use std::env;
use std::fs;
use std::io::{self, Read};
use std::process::Command;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <video_url>", args[0]);
        return;
    }

    let video_url = &args[1];
    let output_file = "output";

    let status = Command::new("yt-dlp")
        .args(&[
            "--write-sub",
            "--write-auto-sub",
            "--skip-download",
            "--sub-lang",
            "en",
            "--output",
            output_file,
            video_url,
        ])
        .status()
        .expect("Failed to execute yt-dlp command");

    if status.success() {
        let mut buffer = String::new();
        let mut file = fs::File::open("output.vtt.en.vtt").expect("Failed to open subtitle file");
        file.read_to_string(&mut buffer).expect("Failed to read subtitle file");

        println!("Subtitles:\n{}", buffer);

        // Clean up regular file
        fs::remove_file("output.vtt.en.vtt").expect("Failed to remove subtitle file");
    } else {
        eprintln!("yt-dlp command failed with status: {}", status);
    }
}
