use std::{env, process};

use quarry_backend::{
    config::AppConfig,
    core::{clients::openai::OpenAiClient, prompts::HELIX_QUERY_EXAMPLE_PROMPT},
};

const APP_NAME: &str = "DataRoomCLI";

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();
    let _ = tracing_subscriber::fmt()
        .with_env_filter("quarry_backend=info")
        .try_init();
    let args: Vec<String> = env::args().collect();
    let config = match AppConfig::from_env() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("error: {error}");
            process::exit(1);
        }
    };
    let Some(openai) = config.openai.as_ref() else {
        eprintln!("error: OpenAI capability is not configured");
        process::exit(1);
    };
    let client = OpenAiClient::from_config(reqwest::Client::new(), openai);

    let result = match args.get(1).map(String::as_str) {
        None | Some("-h") | Some("--help") | Some("help") => {
            print_help();
            Ok(())
        }
        Some("-V") | Some("--version") | Some("version") => {
            println!("{APP_NAME} {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some("embed") => match args.get(2) {
            Some(content) => match client.gen_embedding(content, None).await {
                Ok(embedding) => match serde_json::to_string(&embedding) {
                    Ok(json) => {
                        println!("{json}");
                        Ok(())
                    }
                    Err(err) => Err(format!("failed to serialize embedding: {err}")),
                },
                Err(err) => Err(err),
            },
            None => Err("missing content to embed".to_string()),
        },
        Some("response") => client
            .gen_model_response(Some(HELIX_QUERY_EXAMPLE_PROMPT), None, None)
            .await
            .map(|_t| {}),

        Some(command) => Err(format!("unknown command: {command}")),
    };

    if let Err(message) = result {
        eprintln!("error: {message}");
        eprintln!("run `dataroomcli help` for usage");
        process::exit(1);
    }
}

fn print_help() {
    println!(
        "{APP_NAME}

Usage:
  dataroomcli <command> [options]

Commands:
  embed <content>                      Generate an embedding and print it as JSON
  help            Show this help message
  version         Show the current version
"
    );
}
