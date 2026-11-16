use serde_json::{Value, json};

pub fn dynamic_tools() -> Value {
    let tools: Vec<Value> = serde_json::from_str(include_str!("../../../sidecar/main-tools.json")).expect("checked tool schemas");
    json!(
        tools
            .into_iter()
            .map(|mut tool| {
                tool["namespace"] = json!("arx");
                tool
            })
            .collect::<Vec<_>>()
    )
}
