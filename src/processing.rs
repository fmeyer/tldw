use log::debug;
use regex::Regex;
use std::{
	collections::HashSet,
	fs,
	fs::File,
	io::{BufRead, BufReader},
	process::Command,
};

//TODO(fm): use a temp folder to store the output file, keep a copy of the
// original file in somewhere to avoid downloading it again
pub fn process_subtitles(subtitle_file_path: &str) -> String {
	let cleaned_subtitles = process_file(subtitle_file_path);

	debug!("Processed subtitle content length: {}", cleaned_subtitles.len());
	debug!(
		"First 200 chars of processed content: {}",
		cleaned_subtitles.chars().take(200).collect::<String>()
	);

	// Clean up the subtitle file
	fs::remove_file(subtitle_file_path).expect("Failed to remove subtitle file");

	return cleaned_subtitles;
}

pub fn download_subtitles(
	video_url: &String,
) -> std::result::Result<String, Box<dyn std::error::Error>> {
	debug!("downloading subtitle");

	let output = Command::new("yt-dlp")
		.args(&[
			"--write-sub",
			"--write-auto-sub",
			"--skip-download",
			"--sub-lang",
			"en",
			"--output",
			"/tmp/output", // Base output filename
			&video_url,
		])
		.output()
		.expect("Failed to execute yt-dlp command");

	if !output.status.success() {
		return Err(format!("yt-dlp command failed with status: {}", output.status).into());
	}

	// Parse the output to find the subtitle file path
	let stdout = String::from_utf8(output.stdout)?;
	let stderr = String::from_utf8(output.stderr)?;

	// Look for "Writing video subtitles to:" in both stdout and stderr to find the subtitle file path
	let subtitle_file_path = if let Some(line) = stdout
		.lines()
		.chain(stderr.lines())
		.find(|line| line.contains("Writing video subtitles to:"))
	{
		if let Some(start) = line.find("Writing video subtitles to: ") {
			let path = &line[start + "Writing video subtitles to: ".len()..];
			path.trim().to_string()
		} else {
			return Err("Could not parse subtitle file path from yt-dlp output".into());
		}
	} else {
		// Fallback: construct the path based on the output pattern
		// Look for available .vtt files in /tmp that match the pattern
		let tmp_dir = std::path::Path::new("/tmp");
		if let Ok(entries) = std::fs::read_dir(tmp_dir) {
			for entry in entries {
				if let Ok(entry) = entry {
					let path = entry.path();
					if let Some(filename) = path.file_name() {
						if let Some(filename_str) = filename.to_str() {
							if filename_str.starts_with("output.") && filename_str.ends_with(".vtt")
							{
								return Ok(path.to_string_lossy().to_string());
							}
						}
					}
				}
			}
		}
		return Err("Could not find subtitle file path in yt-dlp output".into());
	};

	// Verify the file exists
	if !std::path::Path::new(&subtitle_file_path).exists() {
		return Err(format!("Subtitle file not found: {}", subtitle_file_path).into());
	}

	Ok(subtitle_file_path)
}

fn process_file(filename: &str) -> String {
	let file = File::open(filename).expect("Failed to open file");
	let reader = BufReader::new(file);

	let mut seen = HashSet::new();
	let mut buffer = Vec::new();

	// Regex to match timestamp lines with optional attributes
	let timestamp_regex =
		Regex::new(r"^\d{2}:\d{2}:\d{2}\.\d{3} --> \d{2}:\d{2}:\d{2}\.\d{3}").unwrap();

	for line in reader.lines() {
		let line = line.unwrap();
		let cleaned_line = cleanup_buffer(line);

		// Skip empty lines, WEBVTT header lines, and timestamp lines
		if cleaned_line.trim().is_empty()
			|| cleaned_line.starts_with("WEBVTT")
			|| cleaned_line.starts_with("Kind:")
			|| cleaned_line.starts_with("Language:")
			|| timestamp_regex.is_match(cleaned_line.trim())
		{
			continue;
		}

		// Only add non-empty cleaned lines
		if !cleaned_line.is_empty() && !seen.contains(&cleaned_line) {
			buffer.push(cleaned_line.clone());
			seen.insert(cleaned_line);
		}
	}
	let combined_string = buffer.join(" ");
	return combined_string;
}

fn cleanup_buffer(text: String) -> String {
	// Remove embedded timing tags like <00:00:00.399><c> and </c>
	let timing_tag_regex = Regex::new(r"<\d{2}:\d{2}:\d{2}\.\d{3}><c>|</c>").unwrap();
	let cleaned_text = timing_tag_regex.replace_all(text.as_str(), "");

	// Remove HTML entities
	let nbsp_regex = Regex::new(r"&nbsp;").unwrap();
	let cleaned_text = nbsp_regex.replace_all(&cleaned_text, " ");

	// Remove any remaining HTML-like tags
	let html_tag_regex = Regex::new(r"<[^>]*>").unwrap();
	let cleaned_text = html_tag_regex.replace_all(&cleaned_text, "");

	// Clean up extra whitespace
	let whitespace_regex = Regex::new(r"\s+").unwrap();
	let cleaned_text = whitespace_regex.replace_all(&cleaned_text, " ");

	cleaned_text.trim().to_string()
}
