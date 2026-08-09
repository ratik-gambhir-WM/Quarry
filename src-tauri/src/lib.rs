pub mod commands;
pub mod core;
pub mod document_jobs;
pub mod errors;
pub mod prompts;
pub mod repository;
pub mod services;
pub mod state;
pub mod utils;

mod events;

use tauri::{
    menu::{AboutMetadataBuilder, MenuBuilder, SubmenuBuilder},
    Manager,
};

use crate::{
    commands::{
        data_room::{list_deal_data_room, preview_deal_document},
        deal::{
            archive_deal, database_status, extract_deal_questions_and_thesis, get_deal, list_deals,
            save_deal_and_extract, select_deal_data_room_folder,
        },
        documents::{
            describe_document_files, get_document_job, search_document_chunks_keyword,
            search_document_chunks_vector, select_document_files, start_document_jobs,
        },
        research::{
            export_activity_log, list_summary_files, login_demo_command, save_markdown_summary,
            select_summary_source, summarize, summarize_selected,
        },
        users::{
            create_user, create_wm_user, get_user_by_email, get_wm_user_by_email, greet,
            user_exists_by_email,
        },
    },
    events::register_login_demo_events,
    state::AppState,
};

const APP_NAME: &str = "Quarry";

fn build_app_menu<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> tauri::Result<tauri::menu::Menu<R>> {
    let about_metadata = AboutMetadataBuilder::new()
        .name(Some(APP_NAME))
        .version(Some(app.package_info().version.to_string()))
        .build();

    let app_menu = SubmenuBuilder::new(app, APP_NAME)
        .about_with_text(format!("About {APP_NAME}"), Some(about_metadata))
        .separator()
        .services()
        .separator()
        .hide_with_text(format!("Hide {APP_NAME}"))
        .hide_others()
        .separator()
        .quit_with_text(format!("Quit {APP_NAME}"))
        .build()?;

    let file_menu = SubmenuBuilder::new(app, "File").close_window().build()?;

    let edit_menu = SubmenuBuilder::new(app, "Edit")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;

    let view_menu = SubmenuBuilder::new(app, "View").fullscreen().build()?;

    let window_menu = SubmenuBuilder::new(app, "Window")
        .minimize()
        .maximize()
        .separator()
        .close_window()
        .build()?;

    let help_menu = SubmenuBuilder::new(app, "Help").build()?;

    MenuBuilder::new(app)
        .item(&app_menu)
        .item(&file_menu)
        .item(&edit_menu)
        .item(&view_menu)
        .item(&window_menu)
        .item(&help_menu)
        .build()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = dotenvy::dotenv();

    tauri::Builder::default()
        .setup(|app| {
            let menu = build_app_menu(app.handle())?;
            app.set_menu(menu)?;
            app.manage(AppState::new(app.handle())?);
            register_login_demo_events(app.handle());
            Ok(())
        })
        .on_webview_event(|webview, event| {
            if webview.label() != "main" {
                return;
            }
            if let tauri::WebviewEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) = event {
                if let Err(error) = webview.state::<AppState>().grant_paths(paths.clone()) {
                    eprintln!("failed to record native file-drop grants: {error}");
                }
            }
        })
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            create_user,
            create_wm_user,
            get_wm_user_by_email,
            get_user_by_email,
            user_exists_by_email,
            database_status,
            list_deals,
            get_deal,
            archive_deal,
            select_deal_data_room_folder,
            select_document_files,
            describe_document_files,
            start_document_jobs,
            get_document_job,
            search_document_chunks_keyword,
            search_document_chunks_vector,
            list_deal_data_room,
            preview_deal_document,
            save_deal_and_extract,
            extract_deal_questions_and_thesis,
            login_demo_command,
            list_summary_files,
            summarize,
            summarize_selected,
            save_markdown_summary,
            select_summary_source,
            export_activity_log
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
