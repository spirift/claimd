use chrono::Utc;
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::model::{ProjectMeta, Status, TaskItem, TaskList};
use crate::store::Store;

/// Find item by UUID prefix (minimum 4 chars). Returns mutable reference.
fn find_by_prefix_mut<'a>(items: &'a mut [TaskItem], prefix: &str) -> Result<&'a mut TaskItem> {
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
fn find_by_prefix<'a>(items: &'a [TaskItem], prefix: &str) -> Result<&'a TaskItem> {
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
fn find_index_by_prefix(items: &[TaskItem], prefix: &str) -> Result<usize> {
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
fn resolve_prefixes(items: &[TaskItem], prefixes: &[String]) -> Result<Vec<Uuid>> {
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
) -> Result<TaskItem> {
    store.with_lock(|list| {
        let deps = resolve_prefixes(&list.items, dep_prefixes)?;
        let item = TaskItem::new(
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

pub fn list_items<'a>(items: &'a TaskList, status: Option<&Status>, tag: Option<&str>, all: bool) -> Vec<&'a TaskItem> {
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

pub fn list(store: &Store, status: Option<&Status>, tag: Option<&str>, all: bool) -> Result<Vec<TaskItem>> {
    store.with_shared_lock(|list| {
        let filtered: Vec<TaskItem> = list_items(list, status, tag, all)
            .into_iter()
            .cloned()
            .collect();
        Ok(filtered)
    })
}

pub fn show(store: &Store, id_prefix: &str) -> Result<TaskItem> {
    store.with_shared_lock(|list| {
        let item = find_by_prefix(&list.items, id_prefix)?;
        Ok(item.clone())
    })
}

/// Move the current claimed_by to previously_claimed_by if present.
fn rotate_claimed_by(item: &mut TaskItem, new_agent: Option<&str>) {
    if let Some(prev) = item.claimed_by.take() {
        if !item.previously_claimed_by.contains(&prev) {
            item.previously_claimed_by.push(prev);
        }
    }
    item.claimed_by = new_agent.map(String::from);
}

pub fn claim(store: &Store, id_prefix: &str, agent: Option<&str>) -> Result<TaskItem> {
    let meta = store.read_project_meta()?;
    store.with_try_lock(|list| {
        let item = find_by_prefix_mut(&mut list.items, id_prefix)?;
        match item.status {
            Status::New | Status::Incomplete => {
                if !meta.active {
                    return Err(Error::ProjectInactive);
                }
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
                if !meta.active {
                    return Err(Error::ProjectInactive);
                }
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

pub fn claim_multi(store: &Store, id_prefixes: &[String], agent: Option<&str>) -> Result<Vec<TaskItem>> {
    let meta = store.read_project_meta()?;
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
                    if !meta.active {
                        return Err(Error::ProjectInactive);
                    }
                    if item.has_pending_deps() {
                        return Err(Error::HasPendingDeps {
                            id: item.id,
                            pending: item.depends_on.clone(),
                        });
                    }
                }
                Status::PrChangesRequested => {
                    if !meta.active {
                        return Err(Error::ProjectInactive);
                    }
                }
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

pub fn pr_open(store: &Store, id_prefix: &str, pr_url: &str) -> Result<TaskItem> {
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

pub fn pr_changes_requested(store: &Store, id_prefix: &str) -> Result<TaskItem> {
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

pub fn done(store: &Store, id_prefix: &str) -> Result<TaskItem> {
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

pub fn incomplete(store: &Store, id_prefix: &str, reason: Option<&str>) -> Result<TaskItem> {
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

pub fn unclaim(store: &Store, id_prefix: &str) -> Result<TaskItem> {
    store.with_lock(|list| {
        let item = find_by_prefix_mut(&mut list.items, id_prefix)?;
        match item.status {
            Status::InProgress | Status::Incomplete => {
                item.status = Status::New;
                item.claimed_by = None;
                item.updated_at = Utc::now();
                Ok(item.clone())
            }
            _ => Err(Error::InvalidTransition {
                id: item.id,
                from: item.status.clone(),
                to: Status::New,
            }),
        }
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
) -> Result<TaskItem> {
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

pub fn reorder(store: &Store, id_prefix: &str, position: usize) -> Result<TaskItem> {
    store.with_lock(|list| {
        let idx = find_index_by_prefix(&list.items, id_prefix)?;
        let item = list.items.remove(idx);
        let pos = position.min(list.items.len());
        list.items.insert(pos, item);
        Ok(list.items[pos].clone())
    })
}

pub fn remove(store: &Store, id_prefix: &str) -> Result<TaskItem> {
    store.with_lock(|list| {
        let idx = find_index_by_prefix(&list.items, id_prefix)?;
        let item = list.items.remove(idx);
        Ok(item)
    })
}

pub fn project_get_meta(store: &Store) -> Result<ProjectMeta> {
    store.read_project_meta()
}

pub fn project_set_active(store: &Store, active: bool) -> Result<ProjectMeta> {
    let mut meta = store.read_project_meta()?;
    meta.active = active;
    store.write_project_meta(&meta)?;
    Ok(meta)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, Store) {
        let dir = TempDir::new().unwrap();
        let store = Store::new(dir.path().to_path_buf());
        store.init().unwrap();
        (dir, store)
    }

    fn add_task(store: &Store, title: &str) -> TaskItem {
        add(store, title, None, 5, &[], None, None, None, &[]).unwrap()
    }

    // --- add / list / show ---

    #[test]
    fn add_creates_new_task() {
        let (_dir, store) = setup();
        let item = add(&store, "hello", Some("desc"), 2, &["tag1".into()], None, None, None, &[]).unwrap();
        assert_eq!(item.title, "hello");
        assert_eq!(item.description.as_deref(), Some("desc"));
        assert_eq!(item.priority, 2);
        assert_eq!(item.tags, vec!["tag1"]);
        assert_eq!(item.status, Status::New);
    }

    #[test]
    fn list_excludes_done_by_default() {
        let (_dir, store) = setup();
        let a = add_task(&store, "a");
        let b = add_task(&store, "b");
        done(&store, &a.short_id()).unwrap();
        let items = list(&store, None, None, false).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, b.id);
    }

    #[test]
    fn list_all_includes_done() {
        let (_dir, store) = setup();
        let a = add_task(&store, "a");
        done(&store, &a.short_id()).unwrap();
        let items = list(&store, None, None, true).unwrap();
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn list_filters_by_status() {
        let (_dir, store) = setup();
        let a = add_task(&store, "a");
        add_task(&store, "b");
        claim(&store, &a.short_id(), None).unwrap();
        let items = list(&store, Some(&Status::InProgress), None, false).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, a.id);
    }

    #[test]
    fn list_filters_by_tag() {
        let (_dir, store) = setup();
        add(&store, "a", None, 5, &["backend".into()], None, None, None, &[]).unwrap();
        add(&store, "b", None, 5, &["frontend".into()], None, None, None, &[]).unwrap();
        let items = list(&store, None, Some("backend"), false).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "a");
    }

    #[test]
    fn show_by_prefix() {
        let (_dir, store) = setup();
        let item = add_task(&store, "hello");
        let found = show(&store, &item.short_id()).unwrap();
        assert_eq!(found.id, item.id);
    }

    #[test]
    fn show_not_found() {
        let (_dir, store) = setup();
        let err = show(&store, "0000").unwrap_err();
        assert!(matches!(err, Error::NotFound { .. }));
    }

    // --- claim ---

    #[test]
    fn claim_new_to_in_progress() {
        let (_dir, store) = setup();
        let item = add_task(&store, "task");
        let claimed = claim(&store, &item.short_id(), Some("agent-1")).unwrap();
        assert_eq!(claimed.status, Status::InProgress);
        assert_eq!(claimed.claimed_by.as_deref(), Some("agent-1"));
    }

    #[test]
    fn claim_already_in_progress_fails() {
        let (_dir, store) = setup();
        let item = add_task(&store, "task");
        claim(&store, &item.short_id(), Some("agent-1")).unwrap();
        let err = claim(&store, &item.short_id(), Some("agent-2")).unwrap_err();
        assert!(matches!(err, Error::AlreadyClaimed { .. }));
    }

    #[test]
    fn claim_done_fails_with_invalid_transition() {
        let (_dir, store) = setup();
        let item = add_task(&store, "task");
        done(&store, &item.short_id()).unwrap();
        let err = claim(&store, &item.short_id(), None).unwrap_err();
        assert!(matches!(err, Error::InvalidTransition { .. }));
    }

    #[test]
    fn claim_with_pending_deps_fails() {
        let (_dir, store) = setup();
        let dep = add_task(&store, "dep");
        let task = add(&store, "task", None, 5, &[], None, None, None, &[dep.short_id()]).unwrap();
        let err = claim(&store, &task.short_id(), None).unwrap_err();
        assert!(matches!(err, Error::HasPendingDeps { .. }));
    }

    #[test]
    fn claim_inactive_project_fails() {
        let (_dir, store) = setup();
        let item = add_task(&store, "task");
        project_set_active(&store, false).unwrap();
        let err = claim(&store, &item.short_id(), None).unwrap_err();
        assert!(matches!(err, Error::ProjectInactive));
    }

    #[test]
    fn claim_pr_changes_requested_rotates_agent() {
        let (_dir, store) = setup();
        let item = add_task(&store, "task");
        claim(&store, &item.short_id(), Some("agent-1")).unwrap();
        pr_open(&store, &item.short_id(), "https://example.com/pr/1").unwrap();
        pr_changes_requested(&store, &item.short_id()).unwrap();
        let reclaimed = claim(&store, &item.short_id(), Some("agent-2")).unwrap();
        assert_eq!(reclaimed.status, Status::InProgress);
        assert_eq!(reclaimed.claimed_by.as_deref(), Some("agent-2"));
        assert!(reclaimed.previously_claimed_by.contains(&"agent-1".to_string()));
    }

    // --- done / incomplete / unclaim ---

    #[test]
    fn done_clears_claimed_by() {
        let (_dir, store) = setup();
        let item = add_task(&store, "task");
        claim(&store, &item.short_id(), Some("agent-1")).unwrap();
        let finished = done(&store, &item.short_id()).unwrap();
        assert_eq!(finished.status, Status::Done);
        assert!(finished.claimed_by.is_none());
    }

    #[test]
    fn done_propagates_to_dependents() {
        let (_dir, store) = setup();
        let dep = add_task(&store, "dep");
        let task = add(&store, "task", None, 5, &[], None, None, None, &[dep.short_id()]).unwrap();
        done(&store, &dep.short_id()).unwrap();
        let updated = show(&store, &task.short_id()).unwrap();
        assert!(updated.depends_on.is_empty());
        assert!(updated.depends_on_completed.contains(&dep.id));
    }

    #[test]
    fn done_dep_unblocks_claim() {
        let (_dir, store) = setup();
        let dep = add_task(&store, "dep");
        let task = add(&store, "task", None, 5, &[], None, None, None, &[dep.short_id()]).unwrap();
        done(&store, &dep.short_id()).unwrap();
        let claimed = claim(&store, &task.short_id(), None).unwrap();
        assert_eq!(claimed.status, Status::InProgress);
    }

    #[test]
    fn incomplete_appends_reason() {
        let (_dir, store) = setup();
        let item = add(&store, "task", Some("original"), 5, &[], None, None, None, &[]).unwrap();
        claim(&store, &item.short_id(), None).unwrap();
        let result = incomplete(&store, &item.short_id(), Some("blocked")).unwrap();
        assert_eq!(result.status, Status::Incomplete);
        assert!(result.description.unwrap().contains("[incomplete] blocked"));
    }

    #[test]
    fn unclaim_resets_to_new() {
        let (_dir, store) = setup();
        let item = add_task(&store, "task");
        claim(&store, &item.short_id(), Some("agent-1")).unwrap();
        let reset = unclaim(&store, &item.short_id()).unwrap();
        assert_eq!(reset.status, Status::New);
        assert!(reset.claimed_by.is_none());
    }

    #[test]
    fn unclaim_from_done_fails() {
        let (_dir, store) = setup();
        let item = add_task(&store, "task");
        done(&store, &item.short_id()).unwrap();
        let err = unclaim(&store, &item.short_id()).unwrap_err();
        assert!(matches!(err, Error::InvalidTransition { .. }));
    }

    #[test]
    fn unclaim_from_incomplete() {
        let (_dir, store) = setup();
        let item = add_task(&store, "task");
        claim(&store, &item.short_id(), None).unwrap();
        incomplete(&store, &item.short_id(), None).unwrap();
        let reset = unclaim(&store, &item.short_id()).unwrap();
        assert_eq!(reset.status, Status::New);
    }

    // --- PR lifecycle ---

    #[test]
    fn pr_open_records_url() {
        let (_dir, store) = setup();
        let item = add_task(&store, "task");
        claim(&store, &item.short_id(), None).unwrap();
        let result = pr_open(&store, &item.short_id(), "https://github.com/org/repo/pull/1").unwrap();
        assert_eq!(result.status, Status::PrOpen);
        assert_eq!(result.pr_url.as_deref(), Some("https://github.com/org/repo/pull/1"));
    }

    #[test]
    fn pr_changes_requested_from_pr_open() {
        let (_dir, store) = setup();
        let item = add_task(&store, "task");
        claim(&store, &item.short_id(), None).unwrap();
        pr_open(&store, &item.short_id(), "https://example.com").unwrap();
        let result = pr_changes_requested(&store, &item.short_id()).unwrap();
        assert_eq!(result.status, Status::PrChangesRequested);
    }

    #[test]
    fn pr_changes_requested_not_from_in_progress_fails() {
        let (_dir, store) = setup();
        let item = add_task(&store, "task");
        claim(&store, &item.short_id(), None).unwrap();
        let err = pr_changes_requested(&store, &item.short_id()).unwrap_err();
        assert!(matches!(err, Error::InvalidTransition { .. }));
    }

    // --- claim_multi ---

    #[test]
    fn claim_multi_all_or_nothing_success() {
        let (_dir, store) = setup();
        let a = add_task(&store, "a");
        let b = add_task(&store, "b");
        let ids = vec![a.short_id(), b.short_id()];
        let claimed = claim_multi(&store, &ids, Some("agent-1")).unwrap();
        assert_eq!(claimed.len(), 2);
        assert!(claimed.iter().all(|t| t.status == Status::InProgress));
    }

    #[test]
    fn claim_multi_rolls_back_if_any_invalid() {
        let (_dir, store) = setup();
        let a = add_task(&store, "a");
        let b = add_task(&store, "b");
        claim(&store, &b.short_id(), None).unwrap();
        let ids = vec![a.short_id(), b.short_id()];
        let err = claim_multi(&store, &ids, Some("agent-2")).unwrap_err();
        assert!(matches!(err, Error::AlreadyClaimed { .. }));
        // a should still be New since the whole operation failed
        let a_state = show(&store, &a.short_id()).unwrap();
        assert_eq!(a_state.status, Status::New);
    }

    // --- edit ---

    #[test]
    fn edit_updates_fields() {
        let (_dir, store) = setup();
        let item = add_task(&store, "original");
        let updated = edit(
            &store, &item.short_id(),
            Some("updated"), Some("new desc"), Some(1),
            Some(&["newtag".to_string()]),
            None, None, None, &[], &[],
        ).unwrap();
        assert_eq!(updated.title, "updated");
        assert_eq!(updated.description.as_deref(), Some("new desc"));
        assert_eq!(updated.priority, 1);
        assert_eq!(updated.tags, vec!["newtag"]);
    }

    #[test]
    fn edit_add_and_remove_deps() {
        let (_dir, store) = setup();
        let dep = add_task(&store, "dep");
        let task = add_task(&store, "task");
        edit(&store, &task.short_id(), None, None, None, None, None, None, None,
            &[dep.short_id()], &[]).unwrap();
        let with_dep = show(&store, &task.short_id()).unwrap();
        assert!(with_dep.depends_on.contains(&dep.id));
        edit(&store, &task.short_id(), None, None, None, None, None, None, None,
            &[], &[dep.short_id()]).unwrap();
        let without_dep = show(&store, &task.short_id()).unwrap();
        assert!(without_dep.depends_on.is_empty());
    }

    // --- reorder / remove ---

    #[test]
    fn reorder_moves_task_to_position() {
        let (_dir, store) = setup();
        let a = add_task(&store, "a");
        let b = add_task(&store, "b");
        let c = add_task(&store, "c");
        reorder(&store, &c.short_id(), 0).unwrap();
        let items = list(&store, None, None, false).unwrap();
        assert_eq!(items[0].id, c.id);
        assert_eq!(items[1].id, a.id);
        assert_eq!(items[2].id, b.id);
    }

    #[test]
    fn remove_deletes_task() {
        let (_dir, store) = setup();
        let item = add_task(&store, "task");
        remove(&store, &item.short_id()).unwrap();
        let err = show(&store, &item.short_id()).unwrap_err();
        assert!(matches!(err, Error::NotFound { .. }));
    }
}
