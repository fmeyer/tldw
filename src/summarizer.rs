use chatgpt::client::ChatGPT;
use chatgpt::config::ChatGPTEngine;
use chatgpt::prelude::ModelConfigurationBuilder;
use chatgpt::prelude::ResponseChunk;
use futures_util::stream::StreamExt;
use std::io::{stdout, Write};

//TODO(fm): better prompt structure
const PROMPTS: [&str; 3] = ["Provide an in-depth, summary of the following content in a structured outline. Include any additional relevant information or insight applying the concepts of smart brevity. Enhance the summary by incorporating a conclusion block when necessary to clarify or support explanations. Ignore sponsorship messages and focus on the overall idea \n The output result should be in markdown markup\n",
    "system: I need you to create a comprehensive, detailed summary of the provided content in a clearly structured outline. Make sure to add any significant information or insights that are related to smart brevity principles. To strengthen the summary, don't hesitate to include a conclusion section if it helps in clarifying or supporting explanations. Please specifically omit any messages pertaining to sponsorship, and prioritize the overarching idea. The finalized product should be delivered in markdown format.",
    "system: I need you to create a comprehensive, detailed summary of the provided content in a clearly structured outline. This is a partial input, therefore don't provide introduction or conclusions unless the content mentions it. Please specifically omit any messages pertaining to sponsorship, and prioritize the overarching idea. The finalized product should be delivered in markdown format.",
    ];

pub const MAX_TOKENS: usize = 15000;

async fn process_message_stream(client: ChatGPT, prompt: &str) -> chatgpt::Result<()> {
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

pub async fn process_long_input(
    gpt_client: ChatGPT,
    input: String,
    prompt: usize,
) -> chatgpt::Result<()> {
    let mut chunks: Vec<String> = Vec::new();

    // split subtitles in chunks of 15000 characters
    for chunk in input.as_bytes().chunks(MAX_TOKENS) {
        chunks.push(String::from_utf8(chunk.to_vec()).unwrap());
    }

    //TODO(fm): Check if context is kept between loop iterations
    for chunk in chunks.iter() {
        // create a new conversation and client instance for each chunk
        let new_client = gpt_client.clone();
        let mut prompt = PROMPTS[prompt].to_string();

        // append chunk to prompt
        prompt.push_str(chunk);
        process_message_stream(new_client, &prompt).await?;
    }

    Ok(())
}

pub async fn process_short_input(
    gpt_client: ChatGPT,
    input: String,
    prompt: usize,
) -> chatgpt::Result<()> {
    let prompt = format!("{} {}", PROMPTS[prompt], input);

    process_message_stream(gpt_client, &prompt).await?;

    Ok(())
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
