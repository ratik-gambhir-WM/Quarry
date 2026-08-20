use std::{env, process};

use quarry_backend::core::{
    clients::{helix::HelixClient, openai::OpenAiClient},
    helix_queries::user::persistence::{
        create_user_indexes, get_user_by_email as build_get_user_by_email_query,
        save_user as build_save_user_query,
    },
    models::user::UserNode,
};
use serde_json::Value;

const APP_NAME: &str = "DataRoomCLI";

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();
    let _ = tracing_subscriber::fmt()
        .with_env_filter("quarry_backend=info")
        .try_init();
    let args: Vec<String> = env::args().collect();
    let helix_db = match HelixClient::new() {
        Ok(client) => client,
        Err(message) => {
            eprintln!("error: {message}");
            process::exit(1);
        }
    };
    let api_key = match env::var("OPENAI_API_KEY") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => {
            eprintln!("error: OPENAI_API_KEY environment variable is not set");
            process::exit(1);
        }
    };
    let client = OpenAiClient::new(api_key.as_str());

    let result = match args.get(1).map(String::as_str) {
        None | Some("-h") | Some("--help") | Some("help") => {
            print_help();
            Ok(())
        }
        Some("-V") | Some("--version") | Some("version") => {
            println!("{APP_NAME} {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some("add-user") => add_user(&helix_db, &args).await,
        Some("get-user") => get_user(&helix_db, args.get(2)).await,
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
            .gen_model_response(
                Option::from("What is capital of Ohio and how was it founded?"),
                None,
                None,
            )
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

async fn add_user(client: &HelixClient, args: &[String]) -> Result<(), String> {
    let required = |index: usize, name: &str| {
        args.get(index)
            .cloned()
            .ok_or_else(|| format!("missing required argument: <{name}>"))
    };
    let user = UserNode {
        id: required(2, "id")?
            .parse()
            .map_err(|err| format!("invalid id: {err}"))?,
        first_name: required(3, "first_name")?,
        last_name: required(4, "last_name")?,
        email: required(5, "email")?,
        api_key: required(6, "api_key")?,
        role: required(7, "role")?,
        created_at: required(8, "created_at")?,
        updated_at: required(9, "updated_at")?,
    };
    let query = build_save_user_query(user)?;
    let _: Value = client.execute_dynamic_query(create_user_indexes).await?;
    let result: Value = client.execute_dynamic_query(move || query).await?;
    println!(
        "{}",
        serde_json::to_string_pretty(&result).map_err(|err| err.to_string())?
    );
    Ok(())
}

async fn get_user(client: &HelixClient, email: Option<&String>) -> Result<(), String> {
    let email = email
        .cloned()
        .ok_or_else(|| "missing required argument: <email>".to_string())?;
    let query = build_get_user_by_email_query(email)?;
    let result: Value = client.execute_dynamic_query(move || query).await?;
    println!(
        "{}",
        serde_json::to_string_pretty(&result).map_err(|err| err.to_string())?
    );
    Ok(())
}

fn print_help() {
    println!(
        "{APP_NAME}

Usage:
  dataroomcli <command> [options]

Commands:
  add-user <id> <first_name> <last_name> <email> <api_key> <role> <created_at> <updated_at>
                                      Upsert a complete Quarry user in Helix
  get-user <email>                    Fetch a user from Helix by email
  embed <content>                      Generate an embedding and print it as JSON
  help            Show this help message
  version         Show the current version
"
    );
}
