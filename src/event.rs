use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::model::{Status, TaskItem};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    Created,
    Claimed,
    Unclaimed,
    PrOpened,
    PrChangesRequested,
    Done,
    Incomplete,
    Edited,
    Removed,
    Reordered,
}

impl fmt::Display for EventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            EventType::Created => "created",
            EventType::Claimed => "claimed",
            EventType::Unclaimed => "unclaimed",
            EventType::PrOpened => "pr_opened",
            EventType::PrChangesRequested => "pr_changes_requested",
            EventType::Done => "done",
            EventType::Incomplete => "incomplete",
            EventType::Edited => "edited",
            EventType::Removed => "removed",
            EventType::Reordered => "reordered",
        };
        write!(f, "{s}")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRecord {
    pub ts: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    pub event: EventType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<Status>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<Status>,
    pub task: TaskItem,
}

impl EventRecord {
    pub fn new(
        event: EventType,
        from: Option<Status>,
        to: Option<Status>,
        task: TaskItem,
    ) -> Self {
        EventRecord {
            ts: Utc::now(),
            project: None,
            event,
            from,
            to,
            task,
        }
    }
}
