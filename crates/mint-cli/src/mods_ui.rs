use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};
use serde_json::Value;
#[derive(Clone)]
pub(crate) struct Control {
    pub id: String,
    pub instance: String,
    pub action: String,
    pub label: String,
    pub input: bool,
    pub value: String,
}
fn clean(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control() || *c == '\n')
        .take(4096)
        .collect()
}
pub(crate) fn node_lines(node: &Value) -> Vec<String> {
    match node["type"].as_str().unwrap_or("") {
        "text" | "badge" => clean(node["text"].as_str().unwrap_or(""))
            .lines()
            .map(str::to_owned)
            .collect(),
        "stack" => node["children"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(node_lines)
            .collect(),
        "button" => vec![format!(
            "[ {} ]",
            clean(node["label"].as_str().unwrap_or(""))
        )],
        "input" => vec![format!(
            "{}: {}",
            clean(node["label"].as_str().unwrap_or("")),
            node["value"].as_str().unwrap_or("")
        )],
        "progress" => vec![format!(
            "{}: {:.0}/{:.0}",
            clean(node["label"].as_str().unwrap_or("")),
            node["value"].as_f64().unwrap_or(0.0),
            node["max"].as_f64().unwrap_or(1.0)
        )],
        "table" => {
            let mut lines = vec![
                node["columns"]
                    .as_array()
                    .map(|c| {
                        c.iter()
                            .map(|s| clean(s.as_str().unwrap_or("")))
                            .collect::<Vec<_>>()
                            .join(" | ")
                    })
                    .unwrap_or_default(),
            ];
            for row in node["rows"].as_array().into_iter().flatten() {
                lines.push(
                    row.as_array()
                        .map(|r| {
                            r.iter()
                                .map(|v| {
                                    clean(
                                        &v.as_str()
                                            .map(str::to_owned)
                                            .unwrap_or_else(|| v.to_string()),
                                    )
                                })
                                .collect::<Vec<_>>()
                                .join(" | ")
                        })
                        .unwrap_or_default(),
                );
            }
            lines
        }
        _ => vec![],
    }
}
fn styled_lines(node: &Value, inherited: Style) -> Vec<Line<'static>> {
    let color = match node["color"].as_str() {
        Some("yellow") => Some(Color::Yellow),
        Some("cyan") => Some(Color::Cyan),
        Some("blue") => Some(Color::Blue),
        Some("magenta") => Some(Color::Magenta),
        Some("red") => Some(Color::Red),
        Some("default") => Some(Color::Reset),
        _ => None,
    };
    let style = color.map(|c| inherited.fg(c)).unwrap_or(inherited);
    if node["type"] == "stack" {
        let children = node["children"].as_array().into_iter().flatten();
        if node["direction"] == "row" {
            let mut spans = vec![];
            for child in children {
                let lines = styled_lines(child, style);
                for line in lines {
                    if !spans.is_empty() {
                        spans.push(Span::raw(" "));
                    }
                    spans.extend(line.spans);
                }
            }
            vec![Line::from(spans)]
        } else {
            children
                .flat_map(|child| styled_lines(child, style))
                .collect()
        }
    } else {
        node_lines(node)
            .into_iter()
            .map(|text| Line::from(Span::styled(text, style)))
            .collect()
    }
}
pub(crate) fn controls(mods: &[Value]) -> Vec<Control> {
    fn walk(node: &Value, m: &Value, result: &mut Vec<Control>) {
        if matches!(node["type"].as_str(), Some("button" | "input")) {
            result.push(Control {
                id: m["id"].as_str().unwrap_or("").into(),
                instance: m["instanceId"].as_str().unwrap_or("").into(),
                action: node["action"].as_str().unwrap_or("").into(),
                label: clean(node["label"].as_str().unwrap_or("")),
                input: node["type"] == "input",
                value: node["value"].as_str().unwrap_or("").into(),
            });
        }
        for child in node["children"].as_array().into_iter().flatten() {
            walk(child, m, result);
        }
    }
    let mut result = vec![];
    for m in mods {
        if m["enabled"] == true {
            walk(&m["snapshot"]["ui"]["above_composer"], m, &mut result);
            walk(&m["snapshot"]["ui"]["side_panel"], m, &mut result);
        }
    }
    result
}
pub(crate) fn slot_lines(mods: &[Value], slot: &str) -> Vec<Line<'static>> {
    let mut lines = vec![];
    for m in mods {
        if m["enabled"] != true {
            continue;
        }
        if slot == "side_panel" {
            lines.push(Line::raw(clean(m["name"].as_str().unwrap_or("Mod"))));
        }
        lines.extend(styled_lines(&m["snapshot"]["ui"][slot], Style::default()));
        if let Some(error) = m["error"].as_str() {
            lines.push(Line::raw(format!("Error: {}", clean(error))));
        }
        if let Some(error) = m["reloadError"].as_str() {
            lines.push(Line::raw(format!("Reload: {}", clean(error))));
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn keyboard_controls_include_actions_from_both_ui_slots() {
        let mods = vec![
            json!({"id":"example","enabled":true,"instanceId":"revision",
            "snapshot":{"ui":{
                "above_composer":{"type":"button","label":"Inspect","action":"inspect"},
                "side_panel":{"type":"input","label":"Draft","action":"save","value":"existing"}
            }}}),
        ];
        let available = controls(&mods);
        assert_eq!(available.len(), 2);
        assert!(
            available
                .iter()
                .any(|c| c.action == "inspect" && c.instance == "revision")
        );
    }
}
