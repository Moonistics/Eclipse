use super::json;
use eclipse_launcher::backend;

pub(super) fn no_ai(state: Option<String>, as_json: bool) -> Result<(), String> {
    if let Some(state) = state {
        backend::set_no_ai(state == "on")?;
    }
    let on = backend::no_ai();
    if as_json {
        json(serde_json::json!({ "no_ai": on }));
    } else if on {
        println!("No AI is on: every AI feature is hidden and off.");
    } else {
        println!("No AI is off: AI features are available.");
    }
    Ok(())
}

pub(super) fn agent_config(
    client: Option<String>,
    test: bool,
    as_json: bool,
) -> Result<(), String> {
    if test {
        let tools = eclipse_launcher::agents::self_test(&backend::update_target())?;
        if as_json {
            json(serde_json::json!({ "answered": true, "tools": tools }));
        } else {
            println!("The MCP server answered with {tools} tools.");
        }
        return Ok(());
    }
    use eclipse_launcher::agents;
    let exe = &backend::update_target();
    if as_json {
        let mut all = agents::json(exe);
        if let Some(id) = &client {
            all["clients"] = all["clients"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|c| c["id"] == id.as_str())
                .cloned()
                .collect();
        }
        json(all);
        return Ok(());
    }
    for c in agents::clients(exe)
        .iter()
        .filter(|c| client.as_ref().is_none_or(|id| id == c.id))
    {
        println!("{}", c.name);
        if let Some(line) = c.add_line() {
            println!("  Add it with:  {line}");
        }
        println!("  Or put this in {}:", c.file);
        for line in c.config.lines() {
            println!("    {line}");
        }
        println!();
    }
    println!("Then ask your agent to say hello: it can call the links tool, or play.");
    Ok(())
}
