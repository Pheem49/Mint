#[tauri::command]
pub(crate) async fn mods_operation(
    operation: mint_core::mods::Operation,
) -> Result<serde_json::Value, String> {
    mint_core::mods::operate_async(operation).await
}
