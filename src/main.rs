mod cli;
mod commands;
mod error;
mod event;
mod model;
mod output;
mod store;

use std::path::PathBuf;

use clap::Parser;
use uuid::Uuid;

use cli::{Cli, Command, EventsCommand, ProjectCommand};
use event::EventRecord;
use store::Store;

fn emit(store: &Store, project: &Option<String>, mut ev: EventRecord) {
    ev.project = project.clone();
    store.append_event(&ev);
}

fn default_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(|h| PathBuf::from(h).join(".claimd"))
        .unwrap_or_else(|| PathBuf::from(".claimd"))
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
    let project_name = cli.project.clone();
    let base_dir = cli.dir.clone().unwrap_or_else(default_dir);

    // All task commands except `show`, `project` require --project.
    let needs_project = !matches!(cli.command, Command::Project { .. } | Command::Show { .. });
    if needs_project && project_name.is_none() {
        return Err(error::Error::ProjectRequired);
    }

    let project_meta = store.read_project_meta().unwrap_or_default();
    let ctx = output::OutputContext::from_meta(&project_meta, project_name.clone());

    match cli.command {
        Command::Init => {
            commands::init(store)?;
            output::print_message("Store initialized.", json);
        }
        Command::Add { title, desc, priority, tags, link, source, author, depends_on } => {
            let (item, ev) = commands::add(store, &title, desc.as_deref(), priority, &tags, link.as_deref(), source.as_deref(), author.as_deref(), &depends_on)?;
            emit(store, &project_name, ev);
            output::print_item(&item, &ctx, json);
        }
        Command::List { status, tag, all } => {
            let items = commands::list(store, status.as_ref(), tag.as_deref(), all)?;
            let refs: Vec<&model::TaskItem> = items.iter().collect();
            output::print_items(&refs, &ctx, json);
        }
        Command::Show { id } if project_name.is_none() => {
            // No --project: scan all projects for the ID.
            let projects_dir = base_dir.join("projects");
            let mut hits: Vec<(String, model::TaskItem, model::ProjectMeta)> = Vec::new();
            if projects_dir.is_dir() {
                if let Ok(entries) = std::fs::read_dir(&projects_dir) {
                    for entry in entries.flatten() {
                        if entry.path().is_dir() {
                            let name = entry.file_name().to_string_lossy().to_string();
                            let ps = Store::new(entry.path());
                            if let Ok(item) = commands::show(&ps, &id) {
                                let meta = ps.read_project_meta().unwrap_or_default();
                                hits.push((name, item, meta));
                            }
                        }
                    }
                }
            }
            match hits.len() {
                0 => return Err(error::Error::NotFound { id_prefix: id }),
                1 => {
                    let (proj_name, item, meta) = hits.remove(0);
                    let ctx = output::OutputContext::from_meta(&meta, Some(proj_name));
                    output::print_item_detail(&item, &ctx, json);
                }
                _ => {
                    let matches: Vec<Uuid> = hits.iter().map(|(_, item, _)| item.id).collect();
                    return Err(error::Error::AmbiguousPrefix { id_prefix: id, matches });
                }
            }
        }
        Command::Show { id } => {
            let item = commands::show(store, &id)?;
            output::print_item_detail(&item, &ctx, json);
        }
        Command::Claim { id, agent } => {
            let (item, ev) = commands::claim(store, &id, agent.as_deref())?;
            emit(store, &project_name, ev);
            output::print_item(&item, &ctx, json);
        }
        Command::ClaimMulti { ids, agent } => {
            let (items, evs) = commands::claim_multi(store, &ids, agent.as_deref())?;
            for ev in evs { emit(store, &project_name, ev); }
            let refs: Vec<&model::TaskItem> = items.iter().collect();
            output::print_items(&refs, &ctx, json);
        }
        Command::PrOpen { id, pr_url } => {
            let (item, ev) = commands::pr_open(store, &id, &pr_url)?;
            emit(store, &project_name, ev);
            output::print_item(&item, &ctx, json);
        }
        Command::PrChangesRequested { id } => {
            let (item, ev) = commands::pr_changes_requested(store, &id)?;
            emit(store, &project_name, ev);
            output::print_item(&item, &ctx, json);
        }
        Command::Done { id } => {
            let (item, ev) = commands::done(store, &id)?;
            emit(store, &project_name, ev);
            output::print_item(&item, &ctx, json);
        }
        Command::Incomplete { id, reason } => {
            let (item, ev) = commands::incomplete(store, &id, reason.as_deref())?;
            emit(store, &project_name, ev);
            output::print_item(&item, &ctx, json);
        }
        Command::Unclaim { id } => {
            let (item, ev) = commands::unclaim(store, &id)?;
            emit(store, &project_name, ev);
            output::print_item(&item, &ctx, json);
        }
        Command::Edit { id, title, desc, priority, tags, link, source, author, add_deps, remove_deps } => {
            let (item, ev) = commands::edit(store, &id, title.as_deref(), desc.as_deref(), priority, tags.as_deref(), link.as_deref(), source.as_deref(), author.as_deref(), &add_deps, &remove_deps)?;
            emit(store, &project_name, ev);
            output::print_item(&item, &ctx, json);
        }
        Command::Reorder { id, position } => {
            let (item, ev) = commands::reorder(store, &id, position)?;
            emit(store, &project_name, ev);
            output::print_item(&item, &ctx, json);
        }
        Command::Remove { id } => {
            let (item, ev) = commands::remove(store, &id)?;
            emit(store, &project_name, ev);
            output::print_message(&format!("Removed: {} ({})", item.title, item.short_id()), json);
        }
        Command::Events { command } => {
            match command {
                EventsCommand::List { limit } => {
                    let events = store.read_events()?;
                    let slice = if limit == 0 || events.len() <= limit {
                        &events[..]
                    } else {
                        &events[events.len() - limit..]
                    };
                    if json {
                        let vals: Vec<_> = slice.iter()
                            .map(|e| serde_json::to_value(e).unwrap())
                            .collect();
                        println!("{}", serde_json::to_string_pretty(&serde_json::json!({ "events": vals })).unwrap());
                    } else if slice.is_empty() {
                        println!("No events.");
                    } else {
                        for e in slice {
                            let transition = match (&e.from, &e.to) {
                                (Some(f), Some(t)) => format!("{f} → {t}"),
                                (Some(f), None)    => format!("{f} → ?"),
                                (None, Some(t))    => format!("? → {t}"),
                                (None, None)       => String::new(),
                            };
                            let title = if e.task.title.len() > 38 {
                                format!("{}…", &e.task.title[..37])
                            } else {
                                e.task.title.clone()
                            };
                            println!(
                                "{}  {:<22}  {}  {:<39}  {}",
                                e.ts.format("%Y-%m-%dT%H:%M:%SZ"),
                                e.event.to_string(),
                                &e.task.id.to_string()[..8],
                                title,
                                transition,
                            );
                        }
                    }
                }
                EventsCommand::Prune { ttl_days } => {
                    let ttl = ttl_days.unwrap_or_else(|| {
                        store.read_project_meta().unwrap_or_default().events_ttl_days
                    });
                    let pruned = store.prune_events(ttl)?;
                    if json {
                        println!("{}", serde_json::json!({ "pruned": pruned, "ttl_days": ttl }));
                    } else {
                        println!("Pruned {pruned} event(s) older than {ttl} days.");
                    }
                }
                EventsCommand::Config { enabled, ttl_days } => {
                    let meta = commands::project_set_events(store, enabled, ttl_days)?;
                    if json {
                        println!("{}", serde_json::json!({
                            "events_enabled": meta.events_enabled,
                            "events_ttl_days": meta.events_ttl_days,
                        }));
                    } else {
                        let state = if meta.events_enabled { "enabled" } else { "disabled" };
                        println!("Events {state}, TTL: {} days.", meta.events_ttl_days);
                    }
                }
            }
        }
        Command::Project { command } => {
            let projects_dir = base_dir.join("projects");

            match command {
                ProjectCommand::List => {
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
                    names.sort();

                    if json {
                        let projects: Vec<serde_json::Value> = names.iter().map(|name| {
                            let project_store = Store::new(projects_dir.join(name));
                            let active = project_store.read_project_meta().map(|m| m.active).unwrap_or(true);
                            serde_json::json!({ "name": name, "active": active })
                        }).collect();
                        println!("{}", serde_json::to_string_pretty(&serde_json::json!({ "projects": projects })).unwrap());
                    } else if names.is_empty() {
                        println!("No projects found.");
                    } else {
                        for name in &names {
                            let project_store = Store::new(projects_dir.join(name));
                            let active = project_store.read_project_meta().map(|m| m.active).unwrap_or(true);
                            let state = if active { "active" } else { "INACTIVE" };
                            println!("{:<20} {:<8} {}", name, state, projects_dir.join(name).display());
                        }
                    }
                }
                ProjectCommand::Status { name } => {
                    let project_store = Store::new(projects_dir.join(&name));
                    let meta = commands::project_get_meta(&project_store)?;
                    if json {
                        println!("{}", serde_json::json!({ "name": name, "active": meta.active }));
                    } else {
                        let state = if meta.active { "active" } else { "INACTIVE" };
                        println!("{}: {}", name, state);
                    }
                }
                ProjectCommand::Activate { name } => {
                    let project_store = Store::new(projects_dir.join(&name));
                    commands::project_set_active(&project_store, true)?;
                    if json {
                        println!("{}", serde_json::json!({ "name": name, "active": true }));
                    } else {
                        println!("{}: active", name);
                    }
                }
                ProjectCommand::Deactivate { name } => {
                    let project_store = Store::new(projects_dir.join(&name));
                    commands::project_set_active(&project_store, false)?;
                    if json {
                        println!("{}", serde_json::json!({ "name": name, "active": false }));
                    } else {
                        println!("{}: INACTIVE", name);
                    }
                }
                ProjectCommand::Remove { name } => {
                    let path = projects_dir.join(&name);
                    if !path.is_dir() {
                        return Err(error::Error::NotFound { id_prefix: name });
                    }
                    std::fs::remove_dir_all(&path)?;
                    if json {
                        println!("{}", serde_json::json!({ "name": name, "removed": true }));
                    } else {
                        println!("Removed project '{name}' (tasks, events, and metadata deleted).");
                    }
                }
            }
        }
    }
    Ok(())
}
