use chrono::Utc;
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::model::{Status, TodoItem, TodoList};
use crate::store::Store;

/// Find item by UUID prefix (minimum 4 chars). Returns mutable reference.
fn find_by_prefix_mut<'a>(items: &'a mut [TodoItem], prefix: &str) -> Result<&'a mut TodoItem> {
    let matches: Vec<usize> = items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.id.to_string().starts_with(prefix))
        .map(|(i, _)| i)
        .collect();

    match matches.len() {
        0 => Err(Error::NotFound { id_prefix: prefix.to_string() }),
        1 => Ok(&mut items[matches[0]]),
        _ => {
            let ids: Vec<Uuid> = matches.iter().map(|&i| items[i].id).collect();
            Err(Error::AmbiguousPrefix { id_prefix: prefix.to_string(), matches: ids })
        }
    }
}

/// Find item by UUID prefix. Returns immutable reference.
fn find_by_prefix<'a>(items: &'a [TodoItem], prefix: &str) -> Result<&'a TodoItem> {
    let matches: Vec<usize> = items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.id.to_string().starts_with(prefix))
        .map(|(i, _)| i)
        .collect();

    match matches.len() {
        0 => Err(Error::NotFound { id_prefix: prefix.to_string() }),
        1 => Ok(&items[matches[0]]),
        _ => {
            let ids: Vec<Uuid> = matches.iter().map(|&i| items[i].id).collect();
            Err(Error::AmbiguousPrefix { id_prefix: prefix.to_string(), matches: ids })
        }
    }
}

/// Find index by UUID prefix.
fn find_index_by_prefix(items: &[TodoItem], prefix: &str) -> Result<usize> {
    let matches: Vec<usize> = items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.id.to_string().starts_with(prefix))
        .map(|(i, _)| i)
        .collect();

    match matches.len() {
        0 => Err(Error::NotFound { id_prefix: prefix.to_string() }),
        1 => Ok(matches[0]),
        _ => {
            let ids: Vec<Uuid> = matches.iter().map(|&i| items[i].id).collect();
            Err(Error::AmbiguousPrefix { id_prefix: prefix.to_string(), matches: ids })
        }
    }
}

pub fn init(store: &Store) -> Result<()> {
    store.init()
}

/// Resolve a list of UUID prefixes to full UUIDs against the current list.
fn resolve_prefixes(items: &[TodoItem], prefixes: &[String]) -> Result<Vec<Uuid>> {
    prefixes
        .iter()
        .map(|p| find_by_prefix(items, p).map(|item| item.id))
        .collect()
}

pub fn add(
    store: &Store,
    title: &str,
    desc: Option<&str>,
    priority: u8,
    tags: &[String],
    link: Option<&str>,
    source: Option<&str>,
    author: Option<&str>,
    dep_prefixes: &[String],
) -> Result<TodoItem> {
    store.with_lock(|list| {
        let deps = resolve_prefixes(&list.items, dep_prefixes)?;
        let item = TodoItem::new(
            title.to_string(),
            desc.map(String::from),
            priority,
            tags.to_vec(),
            link.map(String::from),
            source.map(String::from),
            author.map(String::from),
            deps,
        );
        let result = item.clone();
        list.items.push(item);
        Ok(result)
    })
}

pub fn list_items<'a>(items: &'a TodoList, status: Option<&Status>, tag: Option<&str>, all: bool) -> Vec<&'a TodoItem> {
    items
        .items
        .iter()
        .filter(|item| {
            if let Some(s) = status {
                return &item.status == s;
            }
            if !all && item.status == Status::Done {
                return false;
            }
            true
        })
        .filter(|item| {
            if let Some(t) = tag {
                return item.tags.iter().any(|it| it == t);
            }
            true
        })
        .collect()
}

pub fn list(store: &Store, status: Option<&Status>, tag: Option<&str>, all: bool) -> Result<Vec<TodoItem>> {
    store.with_shared_lock(|list| {
        let filtered: Vec<TodoItem> = list_items(list, status, tag, all)
            .into_iter()
            .cloned()
            .collect();
        Ok(filtered)
    })
}

pub fn show(store: &Store, id_prefix: &str) -> Result<TodoItem> {
    store.with_shared_lock(|list| {
        let item = find_by_prefix(&list.items, id_prefix)?;
        Ok(item.clone())
    })
}

/// Move the current claimed_by to previously_claimed_by if present.
fn rotate_claimed_by(item: &mut TodoItem, new_agent: Option<&str>) {
    if let Some(prev) = item.claimed_by.take() {
        if !item.previously_claimed_by.contains(&prev) {
            item.previously_claimed_by.push(prev);
        }
    }
    item.claimed_by = new_agent.map(String::from);
}

pub fn claim(store: &Store, id_prefix: &str, agent: Option<&str>) -> Result<TodoItem> {
    store.with_try_lock(|list| {
        let item = find_by_prefix_mut(&mut list.items, id_prefix)?;
        match item.status {
            Status::New | Status::Incomplete => {
                if item.has_pending_deps() {
                    return Err(Error::HasPendingDeps {
                        id: item.id,
                        pending: item.depends_on.clone(),
                    });
                }
                item.status = Status::InProgress;
                rotate_claimed_by(item, agent);
                item.updated_at = Utc::now();
                Ok(item.clone())
            }
            Status::PrChangesRequested => {
                item.status = Status::InProgress;
                rotate_claimed_by(item, agent);
                item.updated_at = Utc::now();
                Ok(item.clone())
            }
            Status::InProgress => Err(Error::AlreadyClaimed {
                id: item.id,
                by: item.claimed_by.clone(),
            }),
            _ => Err(Error::InvalidTransition {
                id: item.id,
                from: item.status.clone(),
                to: Status::InProgress,
            }),
        }
    })
}

pub fn claim_multi(store: &Store, id_prefixes: &[String], agent: Option<&str>) -> Result<Vec<TodoItem>> {
    store.with_try_lock(|list| {
        // First pass: validate all items are claimable
        let indices: Vec<usize> = id_prefixes
            .iter()
            .map(|p| find_index_by_prefix(&list.items, p))
            .collect::<Result<Vec<_>>>()?;

        for &idx in &indices {
            let item = &list.items[idx];
            match item.status {
                Status::New | Status::Incomplete => {
                    if item.has_pending_deps() {
                        return Err(Error::HasPendingDeps {
                            id: item.id,
                            pending: item.depends_on.clone(),
                        });
                    }
                }
                Status::PrChangesRequested => {}
                Status::InProgress => {
                    return Err(Error::AlreadyClaimed {
                        id: item.id,
                        by: item.claimed_by.clone(),
                    });
                }
                _ => {
                    return Err(Error::InvalidTransition {
                        id: item.id,
                        from: item.status.clone(),
                        to: Status::InProgress,
                    });
                }
            }
        }

        // Second pass: mutate all
        let now = Utc::now();
        let mut claimed = Vec::with_capacity(indices.len());
        for &idx in &indices {
            let item = &mut list.items[idx];
            item.status = Status::InProgress;
            rotate_claimed_by(item, agent);
            item.updated_at = now;
            claimed.push(item.clone());
        }
        Ok(claimed)
    })
}

pub fn pr_open(store: &Store, id_prefix: &str, pr_url: &str) -> Result<TodoItem> {
    store.with_lock(|list| {
        let item = find_by_prefix_mut(&mut list.items, id_prefix)?;
        if item.status != Status::InProgress && item.status != Status::PrChangesRequested {
            return Err(Error::InvalidTransition {
                id: item.id,
                from: item.status.clone(),
                to: Status::PrOpen,
            });
        }
        item.status = Status::PrOpen;
        item.pr_url = Some(pr_url.to_string());
        item.updated_at = Utc::now();
        Ok(item.clone())
    })
}

pub fn pr_changes_requested(store: &Store, id_prefix: &str) -> Result<TodoItem> {
    store.with_lock(|list| {
        let item = find_by_prefix_mut(&mut list.items, id_prefix)?;
        if item.status != Status::PrOpen {
            return Err(Error::InvalidTransition {
                id: item.id,
                from: item.status.clone(),
                to: Status::PrChangesRequested,
            });
        }
        item.status = Status::PrChangesRequested;
        item.updated_at = Utc::now();
        Ok(item.clone())
    })
}

pub fn done(store: &Store, id_prefix: &str) -> Result<TodoItem> {
    store.with_lock(|list| {
        let idx = find_index_by_prefix(&list.items, id_prefix)?;
        let now = Utc::now();
        list.items[idx].status = Status::Done;
        list.items[idx].claimed_by = None;
        list.items[idx].updated_at = now;
        let completed_id = list.items[idx].id;
        let result = list.items[idx].clone();

        // Propagate: move completed_id from depends_on to depends_on_completed
        for item in &mut list.items {
            if let Some(pos) = item.depends_on.iter().position(|&id| id == completed_id) {
                item.depends_on.remove(pos);
                item.depends_on_completed.push(completed_id);
                item.updated_at = now;
            }
        }

        Ok(result)
    })
}

pub fn incomplete(store: &Store, id_prefix: &str, reason: Option<&str>) -> Result<TodoItem> {
    store.with_lock(|list| {
        let item = find_by_prefix_mut(&mut list.items, id_prefix)?;
        item.status = Status::Incomplete;
        item.claimed_by = None;
        if let Some(r) = reason {
            let desc = item.description.get_or_insert_with(String::new);
            if !desc.is_empty() {
                desc.push_str("\n");
            }
            desc.push_str(&format!("[incomplete] {r}"));
        }
        item.updated_at = Utc::now();
        Ok(item.clone())
    })
}

pub fn unclaim(store: &Store, id_prefix: &str) -> Result<TodoItem> {
    store.with_lock(|list| {
        let item = find_by_prefix_mut(&mut list.items, id_prefix)?;
        if item.status != Status::InProgress {
            return Err(Error::InvalidTransition {
                id: item.id,
                from: item.status.clone(),
                to: Status::New,
            });
        }
        item.status = Status::New;
        item.claimed_by = None;
        item.updated_at = Utc::now();
        Ok(item.clone())
    })
}

pub fn edit(
    store: &Store,
    id_prefix: &str,
    title: Option<&str>,
    desc: Option<&str>,
    priority: Option<u8>,
    tags: Option<&[String]>,
    link: Option<&str>,
    source: Option<&str>,
    author: Option<&str>,
    add_dep_prefixes: &[String],
    remove_dep_prefixes: &[String],
) -> Result<TodoItem> {
    store.with_lock(|list| {
        // Resolve dep prefixes before borrowing mutably
        let add_deps = resolve_prefixes(&list.items, add_dep_prefixes)?;
        let remove_deps = resolve_prefixes(&list.items, remove_dep_prefixes)?;

        let item = find_by_prefix_mut(&mut list.items, id_prefix)?;
        if let Some(t) = title { item.title = t.to_string(); }
        if let Some(d) = desc { item.description = Some(d.to_string()); }
        if let Some(p) = priority { item.priority = p; }
        if let Some(t) = tags { item.tags = t.to_vec(); }
        if let Some(l) = link { item.link = Some(l.to_string()); }
        if let Some(s) = source { item.source = Some(s.to_string()); }
        if let Some(a) = author { item.author = Some(a.to_string()); }
        for dep in &add_deps {
            if !item.depends_on.contains(dep) && !item.depends_on_completed.contains(dep) {
                item.depends_on.push(*dep);
            }
        }
        for dep in &remove_deps {
            item.depends_on.retain(|id| id != dep);
            item.depends_on_completed.retain(|id| id != dep);
        }
        item.updated_at = Utc::now();
        Ok(item.clone())
    })
}

pub fn reorder(store: &Store, id_prefix: &str, position: usize) -> Result<TodoItem> {
    store.with_lock(|list| {
        let idx = find_index_by_prefix(&list.items, id_prefix)?;
        let item = list.items.remove(idx);
        let pos = position.min(list.items.len());
        list.items.insert(pos, item);
        Ok(list.items[pos].clone())
    })
}

pub fn remove(store: &Store, id_prefix: &str) -> Result<TodoItem> {
    store.with_lock(|list| {
        let idx = find_index_by_prefix(&list.items, id_prefix)?;
        let item = list.items.remove(idx);
        Ok(item)
    })
}
