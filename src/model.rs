use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    New,
    InProgress,
    PrOpen,
    PrChangesRequested,
    Done,
    Incomplete,
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Status::New => write!(f, "New"),
            Status::InProgress => write!(f, "InProgress"),
            Status::PrOpen => write!(f, "PrOpen"),
            Status::PrChangesRequested => write!(f, "PrChangesRequested"),
            Status::Done => write!(f, "Done"),
            Status::Incomplete => write!(f, "Incomplete"),
        }
    }
}

impl std::str::FromStr for Status {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "new" => Ok(Status::New),
            "in_progress" | "inprogress" | "in-progress" => Ok(Status::InProgress),
            "pr_open" | "propen" | "pr-open" => Ok(Status::PrOpen),
            "pr_changes_requested" | "prchangesrequested" | "pr-changes-requested" => Ok(Status::PrChangesRequested),
            "done" => Ok(Status::Done),
            "incomplete" => Ok(Status::Incomplete),
            _ => Err(format!("Unknown status: '{s}'. Valid: new, in_progress, pr_open, pr_changes_requested, done, incomplete")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskItem {
    pub id: Uuid,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub status: Status,
    pub priority: u8,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claimed_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr_url: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub previously_claimed_by: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<Uuid>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on_completed: Vec<Uuid>,
}

impl TaskItem {
    pub fn new(
        title: String,
        description: Option<String>,
        priority: u8,
        tags: Vec<String>,
        link: Option<String>,
        source: Option<String>,
        author: Option<String>,
        depends_on: Vec<Uuid>,
    ) -> Self {
        let now = Utc::now();
        TaskItem {
            id: Uuid::new_v4(),
            title,
            description,
            status: Status::New,
            priority,
            created_at: now,
            updated_at: now,
            claimed_by: None,
            pr_url: None,
            previously_claimed_by: Vec::new(),
            link,
            source,
            author,
            tags,
            depends_on,
            depends_on_completed: Vec::new(),
        }
    }

    pub fn short_id(&self) -> String {
        self.id.to_string()[..8].to_string()
    }

    pub fn has_pending_deps(&self) -> bool {
        !self.depends_on.is_empty()
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct TaskList {
    pub items: Vec<TaskItem>,
}

fn default_active() -> bool { true }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMeta {
    #[serde(default = "default_active")]
    pub active: bool,
}

impl Default for ProjectMeta {
    fn default() -> Self {
        ProjectMeta { active: true }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn status_display() {
        assert_eq!(Status::New.to_string(), "New");
        assert_eq!(Status::InProgress.to_string(), "InProgress");
        assert_eq!(Status::PrOpen.to_string(), "PrOpen");
        assert_eq!(Status::PrChangesRequested.to_string(), "PrChangesRequested");
        assert_eq!(Status::Done.to_string(), "Done");
        assert_eq!(Status::Incomplete.to_string(), "Incomplete");
    }

    #[test]
    fn status_from_str() {
        assert_eq!("new".parse::<Status>().unwrap(), Status::New);
        assert_eq!("in_progress".parse::<Status>().unwrap(), Status::InProgress);
        assert_eq!("inprogress".parse::<Status>().unwrap(), Status::InProgress);
        assert_eq!("pr_open".parse::<Status>().unwrap(), Status::PrOpen);
        assert_eq!("done".parse::<Status>().unwrap(), Status::Done);
        assert_eq!("incomplete".parse::<Status>().unwrap(), Status::Incomplete);
        assert!("unknown".parse::<Status>().is_err());
    }

    #[test]
    fn short_id_is_8_chars() {
        let item = TaskItem::new("t".into(), None, 5, vec![], None, None, None, vec![]);
        assert_eq!(item.short_id().len(), 8);
        assert!(item.id.to_string().starts_with(&item.short_id()));
    }

    #[test]
    fn has_pending_deps_empty() {
        let item = TaskItem::new("t".into(), None, 5, vec![], None, None, None, vec![]);
        assert!(!item.has_pending_deps());
    }

    #[test]
    fn has_pending_deps_with_dep() {
        let dep_id = Uuid::new_v4();
        let item = TaskItem::new("t".into(), None, 5, vec![], None, None, None, vec![dep_id]);
        assert!(item.has_pending_deps());
    }

    #[test]
    fn new_task_defaults() {
        let item = TaskItem::new("hello".into(), None, 3, vec![], None, None, None, vec![]);
        assert_eq!(item.title, "hello");
        assert_eq!(item.status, Status::New);
        assert_eq!(item.priority, 3);
        assert!(item.claimed_by.is_none());
        assert!(item.depends_on.is_empty());
        assert!(item.depends_on_completed.is_empty());
    }
}
