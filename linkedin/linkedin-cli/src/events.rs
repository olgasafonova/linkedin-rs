use crate::session::load_session_client;

pub async fn cmd_event_view(event_id: &str, raw_json: bool) -> Result<(), String> {
    let (client, _path) = load_session_client()?;

    let event = client
        .get_event(event_id)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    if raw_json {
        let pretty =
            serde_json::to_string_pretty(&event).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
        return Ok(());
    }

    let elements = event
        .get("elements")
        .and_then(|e| e.as_array())
        .map(|a| a.as_slice())
        .unwrap_or(&[]);

    let ev = elements.first().unwrap_or(&event);

    let name = ev
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("(unknown)");
    let time = ev
        .get("displayEventTime")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let state = ev
        .get("lifecycleState")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let external_url = ev.get("externalUrl").and_then(|v| v.as_str()).unwrap_or("");
    let vanity = ev.get("vanityName").and_then(|v| v.as_str()).unwrap_or("");
    let host = ev
        .get("viewerHost")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    println!("{}", name);
    println!("{}", "=".repeat(name.len()));
    println!("When: {}", time);
    println!("Status: {}", state);
    if host {
        println!("Role: Host");
    }
    if !external_url.is_empty() {
        println!("Link: {}", external_url);
    }
    if !vanity.is_empty() {
        println!("URL: https://www.linkedin.com/events/{}/", vanity);
    }

    // Speakers.
    if let Some(speakers) = ev.get("speakers").and_then(|s| s.as_array()) {
        if !speakers.is_empty() {
            println!("\nSpeakers:");
            for speaker in speakers {
                let first = speaker
                    .pointer("/assigneeProfile/firstName")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let last = speaker
                    .pointer("/assigneeProfile/lastName")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let state = speaker.get("state").and_then(|v| v.as_str()).unwrap_or("");
                println!("  - {} {} ({})", first, last, state.to_lowercase());
            }
        }
    }

    Ok(())
}

pub async fn cmd_event_attendees(
    event_id: &str,
    start: u32,
    count: u32,
    raw_json: bool,
) -> Result<(), String> {
    let (client, _path) = load_session_client()?;

    let resp = client
        .get_event_attendees(event_id, start, count)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    if raw_json {
        let pretty =
            serde_json::to_string_pretty(&resp).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
        return Ok(());
    }

    let elements = resp
        .get("elements")
        .and_then(|e| e.as_array())
        .map(|a| a.as_slice())
        .unwrap_or(&[]);

    let total = resp
        .pointer("/paging/total")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let mut idx = start;
    let mut printed = 0u32;

    for cluster in elements {
        let items = cluster
            .get("items")
            .and_then(|i| i.as_array())
            .map(|a| a.as_slice())
            .unwrap_or(&[]);
        for item_wrapper in items {
            // REST.li search uses `itemUnion.entityResult`.
            let entity = item_wrapper
                .get("itemUnion")
                .and_then(|iu| iu.get("entityResult"))
                .or_else(|| item_wrapper.get("item").and_then(|i| i.get("entityResult")));
            if let Some(entity) = entity {
                idx += 1;
                printed += 1;
                let name = entity
                    .pointer("/title/text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                let headline = entity
                    .pointer("/primarySubtitle/text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                println!("{:3}. {} — {}", idx, name, headline);
            }
        }
    }

    if printed == 0 {
        match client.get_event(event_id).await {
            Ok(_) => {
                eprintln!("Warning: the event exists but no attendees were returned.");
                eprintln!(
                    "This usually means your LinkedIn session has expired. \
                     Browse LinkedIn in Chrome to refresh cookies, then run \
                     `li auth login` to update the session."
                );
            }
            Err(_) => {
                println!("(no attendees found)");
            }
        }
    } else {
        println!("---");
        println!(
            "Showing {}-{} of {} attendees",
            start + 1,
            start + printed,
            total
        );
    }

    Ok(())
}
