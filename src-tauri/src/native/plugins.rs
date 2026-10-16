use super::discord_rpc;
use mint_core::MintConfig;
pub async fn execute_plugin(
    config: &MintConfig,
    name: &str,
    instruction: &str,
) -> Result<String, String> {
    match name {
        "discord" => discord_rpc::set_activity(config, instruction),
        other => mint_core::execute_native_plugin(config, other, instruction)
            .await
            .map_err(|error| error.to_string()),
    }
}
