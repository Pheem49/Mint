use mint_core::MintConfig;
use serde::Serialize;
use serde_json::{Value, json};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInfo {
    pub name: &'static str,
    pub description: &'static str,
    pub migrated: bool,
    pub configured: bool,
}

pub fn list_plugins(config: &MintConfig) -> Vec<PluginInfo> {
    let mut plugins = vec![
        PluginInfo {
            name: "desktop-actions",
            description: "Allowlisted URL and desktop application launcher",
            migrated: true,
            configured: true,
        },
        PluginInfo {
            name: "mcp-stdio",
            description: "Configured MCP server bridge over stdio JSON-RPC",
            migrated: true,
            configured: config.extra.get("mcpServers").is_some_and(|servers| {
                servers
                    .as_object()
                    .is_some_and(|servers| !servers.is_empty())
            }),
        },
        PluginInfo {
            name: "gmail",
            description: "Gmail OAuth search bridge",
            migrated: true,
            configured: has_values(
                config,
                &["gmailClientId", "gmailClientSecret", "gmailRefreshToken"],
            ),
        },
        PluginInfo {
            name: "google_calendar",
            description: "Google Calendar OAuth list and browser-open bridge",
            migrated: true,
            configured: has_values(
                config,
                &[
                    "googleCalendarClientId",
                    "googleCalendarClientSecret",
                    "googleCalendarRefreshToken",
                ],
            ),
        },
        PluginInfo {
            name: "notion",
            description: "Notion page creation bridge",
            migrated: true,
            configured: has_values(config, &["notionApiKey", "notionDatabaseId"]),
        },
        PluginInfo {
            name: "discord",
            description: "Discord Rich Presence over native desktop IPC",
            migrated: true,
            configured: has_values(config, &["discordApplicationId"]),
        },
    ];
    plugins.extend(
        mint_core::native_plugins()
            .into_iter()
            .map(|plugin| PluginInfo {
                name: plugin.name,
                description: plugin.description,
                migrated: true,
                configured: true,
            }),
    );
    plugins
}

pub fn channel_inventory(config: &MintConfig) -> Value {
    json!([
        channel(
            config,
            "telegram",
            &["telegramBotToken"],
            "native-long-poll"
        ),
        channel(config, "discord", &["discordBotToken"], "native-gateway"),
        channel(
            config,
            "slack",
            &["slackBotToken", "slackAppToken"],
            "native-socket-mode"
        ),
        channel(
            config,
            "line",
            &["lineChannelAccessToken", "lineChannelSecret"],
            "native-webhook-127.0.0.1:3000"
        ),
        channel(
            config,
            "whatsapp",
            &[
                "whatsappCloudAccessToken",
                "whatsappPhoneNumberId",
                "whatsappVerifyToken"
            ],
            "native-cloud-api-webhook-127.0.0.1:3001"
        )
    ])
}

fn channel(config: &MintConfig, name: &str, keys: &[&str], runtime: &str) -> Value {
    json!({ "name": name, "configured": has_values(config, keys), "runtime": runtime })
}

fn has_values(config: &MintConfig, keys: &[&str]) -> bool {
    keys.iter().all(|key| {
        config
            .extra
            .get(*key)
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty())
    })
}
