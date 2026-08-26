mod deal_files;
mod errors;
mod quarry_api;
mod save_file;
mod security;

use tauri::menu::{AboutMetadataBuilder, MenuBuilder, SubmenuBuilder};

use crate::{
    deal_files::{read_deal_source_files, select_deal_data_room, LocalDealRoots},
    quarry_api::{
        quarry_api_get, quarry_api_get_pdf, quarry_api_post, quarry_api_post_multipart,
        subscribe_document_job, QuarryApiService,
    },
    save_file::save_text_file,
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

    MenuBuilder::new(app)
        .item(&app_menu)
        .item(&file_menu)
        .item(&edit_menu)
        .item(&view_menu)
        .item(&window_menu)
        .build()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let quarry_api =
        QuarryApiService::from_environment().expect("failed to initialize the Quarry API service");
    tauri::Builder::default()
        .manage(LocalDealRoots::default())
        .manage(quarry_api)
        .setup(|app| {
            app.set_menu(build_app_menu(app.handle())?)?;
            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            read_deal_source_files,
            quarry_api_get,
            quarry_api_get_pdf,
            quarry_api_post,
            quarry_api_post_multipart,
            save_text_file,
            select_deal_data_room,
            subscribe_document_job
        ])
        .run(tauri::generate_context!())
        .expect("error while running Quarry");
}
