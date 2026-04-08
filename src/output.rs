use crate::error::Error;
use crate::model::{Status, TodoItem};

pub fn print_item(item: &TodoItem, json: bool) {
    if json {
        println!("{}", serde_json::to_string(item).unwrap());
    } else {
        println!("{:<10} {:<12} P{:<3} {}", item.short_id(), item.status, item.priority, item.title);
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

pub fn print_item_detail(item: &TodoItem, json: bool) {
    if json {
        println!("{}", serde_json::to_string_pretty(item).unwrap());
    } else {
        println!("ID:          {}", item.id);
        println!("Title:       {}", item.title);
        println!("Status:      {}", item.status);
        println!("Priority:    {}", item.priority);
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

pub fn print_items(items: &[&TodoItem], json: bool) {
    if json {
        let serialized = serde_json::to_string(items).unwrap();
        println!("{serialized}");
    } else {
        if items.is_empty() {
            println!("No todos found.");
            return;
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
