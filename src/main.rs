mod cli;
mod commands;
mod error;
mod model;
mod output;
mod store;

use std::path::PathBuf;

use clap::Parser;

use cli::{Cli, Command};
use store::Store;

fn default_dir() -> PathBuf {
    dirs_next().unwrap_or_else(|| PathBuf::from(".ai-todo"))
}

fn dirs_next() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".ai-todo"))
}

fn main() {
    let cli = Cli::parse();
    let base_dir = cli.dir.clone().unwrap_or_else(default_dir);
    let dir = match &cli.project {
        Some(name) => base_dir.join("projects").join(name),
        None => base_dir,
    };
    let store = Store::new(dir);
    let json = cli.json;

    let result = run(cli, &store);

    if let Err(e) = result {
        output::print_error(&e, json);
        std::process::exit(e.exit_code());
    }
}

fn run(cli: Cli, store: &Store) -> error::Result<()> {
    let json = cli.json;

    match cli.command {
        Command::Init => {
            commands::init(store)?;
            output::print_message("Store initialized.", json);
        }
        Command::Add { title, desc, priority, tags, link, source, author, depends_on } => {
            let item = commands::add(store, &title, desc.as_deref(), priority, &tags, link.as_deref(), source.as_deref(), author.as_deref(), &depends_on)?;
            output::print_item(&item, json);
        }
        Command::List { status, tag, all } => {
            let items = commands::list(store, status.as_ref(), tag.as_deref(), all)?;
            let refs: Vec<&model::TodoItem> = items.iter().collect();
            output::print_items(&refs, json);
        }
        Command::Show { id } => {
            let item = commands::show(store, &id)?;
            output::print_item_detail(&item, json);
        }
        Command::Claim { id, agent } => {
            let item = commands::claim(store, &id, agent.as_deref())?;
            output::print_item(&item, json);
        }
        Command::ClaimMulti { ids, agent } => {
            let items = commands::claim_multi(store, &ids, agent.as_deref())?;
            let refs: Vec<&model::TodoItem> = items.iter().collect();
            output::print_items(&refs, json);
        }
        Command::PrOpen { id, pr_url } => {
            let item = commands::pr_open(store, &id, &pr_url)?;
            output::print_item(&item, json);
        }
        Command::PrChangesRequested { id } => {
            let item = commands::pr_changes_requested(store, &id)?;
            output::print_item(&item, json);
        }
        Command::Done { id } => {
            let item = commands::done(store, &id)?;
            output::print_item(&item, json);
        }
        Command::Incomplete { id, reason } => {
            let item = commands::incomplete(store, &id, reason.as_deref())?;
            output::print_item(&item, json);
        }
        Command::Unclaim { id } => {
            let item = commands::unclaim(store, &id)?;
            output::print_item(&item, json);
        }
        Command::Edit { id, title, desc, priority, tags, link, source, author, add_deps, remove_deps } => {
            let item = commands::edit(store, &id, title.as_deref(), desc.as_deref(), priority, tags.as_deref(), link.as_deref(), source.as_deref(), author.as_deref(), &add_deps, &remove_deps)?;
            output::print_item(&item, json);
        }
        Command::Reorder { id, position } => {
            let item = commands::reorder(store, &id, position)?;
            output::print_item(&item, json);
        }
        Command::Remove { id } => {
            let item = commands::remove(store, &id)?;
            output::print_message(&format!("Removed: {} ({})", item.title, item.short_id()), json);
        }
        Command::Projects => {
            let base_dir = cli.dir.unwrap_or_else(default_dir);
            let projects_dir = base_dir.join("projects");
            let mut names = Vec::new();
            if projects_dir.is_dir() {
                if let Ok(entries) = std::fs::read_dir(&projects_dir) {
                    for entry in entries.flatten() {
                        if entry.path().is_dir() {
                            if let Some(name) = entry.file_name().to_str() {
                                names.push(name.to_string());
                            }
                        }
                    }
                }
            }
            // Also check if there's a default (non-project) store
            let has_default = base_dir.join("todo.json").exists();
            names.sort();
            if json {
                let val = serde_json::json!({
                    "default_store": has_default,
                    "projects": names,
                });
                println!("{}", serde_json::to_string_pretty(&val).unwrap());
            } else {
                if has_default {
                    println!("(default)  {}", base_dir.display());
                }
                if names.is_empty() && !has_default {
                    println!("No projects found.");
                } else {
                    for name in &names {
                        println!("{}  {}", name, projects_dir.join(name).display());
                    }
                }
            }
        }
    }
    Ok(())
}
