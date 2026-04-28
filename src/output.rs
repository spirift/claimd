use crate::error::Error;
use crate::model::{ProjectMeta, Status, TodoItem};

pub struct OutputContext {
    pub project_active: bool,
    pub project_name: Option<String>,
}

impl OutputContext {
    pub fn from_meta(meta: &ProjectMeta, project_name: Option<String>) -> Self {
        OutputContext { project_active: meta.active, project_name }
    }
}

fn inject_project_active(item: &TodoItem, ctx: &OutputContext) -> serde_json::Value {
    let mut val = serde_json::to_value(item).unwrap();
    if let serde_json::Value::Object(ref mut map) = val {
        map.insert("project_active".to_string(), serde_json::Value::Bool(ctx.project_active));
        if let Some(ref name) = ctx.project_name {
            map.insert("project".to_string(), serde_json::Value::String(name.clone()));
        }
    }
    val
}

pub fn print_item(item: &TodoItem, ctx: &OutputContext, json: bool) {
    if json {
        println!("{}", inject_project_active(item, ctx));
    } else {
        let inactive_marker = if ctx.project_active { "" } else { " [INACTIVE PROJECT]" };
        println!("{:<10} {:<12} P{:<3} {}{}", item.short_id(), item.status, item.priority, item.title, inactive_marker);
        if let Some(desc) = &item.description {
            println!("           {desc}");
        }
        if let Some(agent) = &item.claimed_by {
            println!("           claimed by: {agent}");
        }
        if let Some(pr_url) = &item.pr_url {
            println!("           pr: {pr_url}");
        }
        if !item.previously_claimed_by.is_empty() {
            println!("           prev agents: {}", item.previously_claimed_by.join(", "));
        }
        if let Some(link) = &item.link {
            println!("           link: {link}");
        }
        if let Some(source) = &item.source {
            println!("           source: {source}");
        }
        if let Some(author) = &item.author {
            println!("           author: {author}");
        }
        if !item.tags.is_empty() {
            println!("           tags: {}", item.tags.join(", "));
        }
        if !item.depends_on.is_empty() {
            let deps: Vec<String> = item.depends_on.iter().map(|u| u.to_string()[..8].to_string()).collect();
            println!("           blocked by: {}", deps.join(", "));
        }
    }
}

pub fn print_item_detail(item: &TodoItem, ctx: &OutputContext, json: bool) {
    if json {
        let val = inject_project_active(item, ctx);
        println!("{}", serde_json::to_string_pretty(&val).unwrap());
    } else {
        println!("ID:          {}", item.id);
        println!("Title:       {}", item.title);
        println!("Status:      {}", item.status);
        println!("Priority:    {}", item.priority);
        let proj_status = if ctx.project_active { "active" } else { "INACTIVE" };
        match &ctx.project_name {
            Some(name) => println!("Project:     {} ({})", name, proj_status),
            None => println!("Project:     {}", proj_status),
        }
        if let Some(desc) = &item.description {
            println!("Description: {desc}");
        }
        if let Some(agent) = &item.claimed_by {
            println!("Claimed by:  {agent}");
        }
        if let Some(pr_url) = &item.pr_url {
            println!("PR URL:      {pr_url}");
        }
        if !item.previously_claimed_by.is_empty() {
            println!("Prev agents: {}", item.previously_claimed_by.join(", "));
        }
        if let Some(link) = &item.link {
            println!("Link:        {link}");
        }
        if let Some(source) = &item.source {
            println!("Source:      {source}");
        }
        if let Some(author) = &item.author {
            println!("Author:      {author}");
        }
        if !item.tags.is_empty() {
            println!("Tags:        {}", item.tags.join(", "));
        }
        if !item.depends_on.is_empty() {
            let deps: Vec<String> = item.depends_on.iter().map(|u| u.to_string()[..8].to_string()).collect();
            println!("Depends on:  {} (pending)", deps.join(", "));
        }
        if !item.depends_on_completed.is_empty() {
            let deps: Vec<String> = item.depends_on_completed.iter().map(|u| u.to_string()[..8].to_string()).collect();
            println!("Deps done:   {}", deps.join(", "));
        }
        println!("Created:     {}", item.created_at);
        println!("Updated:     {}", item.updated_at);
    }
}

pub fn print_items(items: &[&TodoItem], ctx: &OutputContext, json: bool) {
    if json {
        let vals: Vec<serde_json::Value> = items.iter().map(|item| inject_project_active(item, ctx)).collect();
        println!("{}", serde_json::to_string(&vals).unwrap());
    } else {
        if items.is_empty() {
            println!("No todos found.");
            return;
        }
        if !ctx.project_active {
            println!("! Project is inactive — new claims are disabled");
        }
        for (i, item) in items.iter().enumerate() {
            let claimed = match &item.claimed_by {
                Some(agent) => format!("  (agent: {agent})"),
                None => String::new(),
            };
            let status_str = match item.status {
                Status::New => "New",
                Status::InProgress => "InProgress",
                Status::PrOpen => "PrOpen",
                Status::PrChangesRequested => "PrChangesReq",
                Status::Done => "Done",
                Status::Incomplete => "Incomplete",
            };
            println!(
                "[{:<3}] {:<10} {:<12} P{:<3} {}{}",
                i, item.short_id(), status_str, item.priority, item.title, claimed
            );
        }
    }
}

pub fn print_error(err: &Error, json: bool) {
    if json {
        let msg = serde_json::json!({
            "error": err.to_string(),
            "code": err.error_code(),
        });
        eprintln!("{}", serde_json::to_string(&msg).unwrap());
    } else {
        eprintln!("Error: {err}");
    }
}

pub fn print_message(msg: &str, json: bool) {
    if json {
        println!("{}", serde_json::json!({"message": msg}));
    } else {
        println!("{msg}");
    }
}
