use chatgpt::{
	client::ChatGPT,
	config::ChatGPTEngine,
	prelude::{ModelConfigurationBuilder, ResponseChunk},
};
use futures_util::stream::StreamExt;
use std::{
	io::{stdout, Write},
	sync::{Arc, Mutex},
};

//TODO(fm): Auto load prompts from somewhere
const PROMPTS: [&str; 3] = [
	"Provide an in-depth, summary of the following content in a structured outline. Include any \
	 additional relevant information or insight applying the concepts of smart brevity. Enhance \
	 the summary by incorporating a conclusion block when necessary to clarify or support \
	 explanations. Ignore sponsorship messages and focus on the overall idea \n The output result \
	 should be in markdown markup\n",
	"system: I need you to create a comprehensive, detailed summary of the provided content in a \
	 clearly structured outline. Make sure to add any significant information or insights that \
	 are related to smart brevity principles. To strengthen the summary, don't hesitate to \
	 include a conclusion section if it helps in clarifying or supporting explanations. Please \
	 specifically omit any messages pertaining to sponsorship, and prioritize the overarching \
	 idea. The finalized product should be delivered in markdown format.",
	"system: I need you to create a comprehensive, detailed summary of the provided content in a \
	 clearly structured outline. This is a partial input, therefore don't provide introduction or \
	 conclusions unless the content mentions it. Please specifically omit any messages pertaining \
	 to sponsorship, and prioritize the overarching idea. The finalized product should be \
	 delivered in markdown format with top level topics as headers and subtopics as items in a \
	 list. Don't use enumerations.",
];

// FIXME: this is a temporary solution, I should replace it with proper
// tokenization
pub const MAX_TOKENS: usize = 15000;

async fn process_message_stream(client: ChatGPT, prompt: &str) -> chatgpt::Result<String> {
	log::debug!("Sending message to OpenAI API, prompt length: {}", prompt.len());
	log::debug!(
		"Prompt preview (first 500 chars): {}",
		prompt.chars().take(500).collect::<String>()
	);

	let stream = match client.send_message_streaming(prompt).await {
		Ok(stream) => {
			log::debug!("Successfully created OpenAI stream");
			stream
		},
		Err(e) => {
			let error_msg = e.to_string();
			if error_msg.contains("insufficient_quota")
				|| error_msg.contains("exceeded your current quota")
			{
				log::error!("OpenAI API quota exceeded. Please check your billing at https://platform.openai.com/account/billing");
				return Err(e);
			} else if error_msg.contains("invalid_api_key")
				|| error_msg.contains("Incorrect API key")
			{
				log::error!("Invalid OpenAI API key. Please check your API key at https://platform.openai.com/account/api-keys");
				return Err(e);
			} else {
				log::error!("Failed to create OpenAI stream: {}", e);
				return Err(e);
			}
		},
	};

	// Wrapping the buffer in an Arc and Mutex
	let buffer = Arc::new(Mutex::new(Vec::<String>::new()));
	let chunk_count = Arc::new(Mutex::new(0));

	// Iterating over stream contents
	stream
		.for_each({
			// Cloning the Arc to be moved into the outer move closure
			let buffer = Arc::clone(&buffer);
			let chunk_count = Arc::clone(&chunk_count);
			move |each| {
				// Cloning the Arc again to be moved into the async block
				let buffer_clone = Arc::clone(&buffer);
				let chunk_count_clone = Arc::clone(&chunk_count);
				async move {
					match each {
						ResponseChunk::Content { delta, response_index: _ } => {
							// Printing part of response without the newline
							print!("{delta}");
							// print!(".");
							// Manually flushing the standard output, as `print` macro does not do
							// that
							stdout().lock().flush().unwrap();
							// Appending delta to buffer
							let mut locked_buffer = buffer_clone.lock().unwrap();
							locked_buffer.push(delta);

							// Track chunk count
							let mut count = chunk_count_clone.lock().unwrap();
							*count += 1;
						},
						ResponseChunk::Done => {
							log::debug!("OpenAI stream completed");
						},
						_ => {
							log::debug!("Received other response chunk type");
						},
					}
				}
			}
		})
		.await;

	// Use buffer outside of for_each, by locking and dereferencing
	let final_buffer = buffer.lock().unwrap();
	let final_chunk_count = chunk_count.lock().unwrap();
	let result = final_buffer.join("");

	log::debug!(
		"OpenAI response complete. Chunks received: {}, Total length: {}",
		*final_chunk_count,
		result.len()
	);

	if result.is_empty() {
		log::warn!("OpenAI API returned empty response despite successful connection. This may indicate quota limits or content filtering.");
	}

	Ok(result)
}

pub async fn process_long_input(
	gpt_client: ChatGPT,
	input: String,
	prompt: usize,
) -> chatgpt::Result<String> {
	let mut chunks: Vec<String> = Vec::new();
	let mut buffer = String::new();

	// split subtitles in chunks of 15000 characters
	for chunk in input.as_bytes().chunks(MAX_TOKENS) {
		chunks.push(String::from_utf8(chunk.to_vec()).unwrap());
	}

	//TODO(fm): Check if context is kept between loop iterations
	for chunk in chunks.iter() {
		// create a new conversation and client instance for each chunk
		let new_client = gpt_client.clone();
		let mut prompt_text = PROMPTS[prompt].to_string();
		// append chunk to prompt
		prompt_text.push_str(chunk);
		let result = process_message_stream(new_client, &prompt_text).await?;
		buffer.push_str(&result);
	}

	Ok(buffer)
}

pub async fn process_short_input(
	gpt_client: ChatGPT,
	input: String,
	prompt: usize,
) -> chatgpt::Result<String> {
	let prompt_text = format!("{} {}", PROMPTS[prompt], input);

	let result = process_message_stream(gpt_client, &prompt_text).await?;

	Ok(result)
}

pub fn build_chat_client(api_key: String, engine: ChatGPTEngine) -> chatgpt::Result<ChatGPT> {
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
