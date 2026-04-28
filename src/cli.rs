use clap::{Parser, Subcommand};
use std::path::PathBuf;

use crate::model::Status;

#[derive(Subcommand)]
pub enum ProjectCommand {
    /// List all projects and their active status
    List,
    /// Show active status of a project
    Status {
        /// Project name
        name: String,
    },
    /// Activate a project (allow claiming tasks)
    Activate {
        /// Project name
        name: String,
    },
    /// Deactivate a project (block new claims)
    Deactivate {
        /// Project name
        name: String,
    },
}

#[derive(Parser)]
#[command(name = "claimd", about = "Task manager for multi-agent AI collaboration", long_about = "Concurrent task manager CLI for multi-agent AI workflows.\n\nAgents can add, view, claim, and complete tasks with atomic file locking that prevents two agents picking up the same work. Tasks are scoped per project; inactive projects block new claims while keeping existing work visible and completable.")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Output as JSON instead of human-readable text
    #[arg(long, global = true)]
    pub json: bool,

    /// Path to the task store directory
    #[arg(long, global = true, env = "CLAIMD_DIR")]
    pub dir: Option<PathBuf>,

    /// Project name (isolates tasks per project)
    #[arg(long, global = true, env = "CLAIMD_PROJECT")]
    pub project: Option<String>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Initialize the task store
    Init,

    /// Add a new task
    Add {
        /// Title of the task
        title: String,
        /// Description
        #[arg(long)]
        desc: Option<String>,
        /// Priority (0 = highest)
        #[arg(long, default_value = "5")]
        priority: u8,
        /// Tags
        #[arg(long = "tag")]
        tags: Vec<String>,
        /// Link (URL or reference)
        #[arg(long)]
        link: Option<String>,
        /// Source (where this task came from)
        #[arg(long)]
        source: Option<String>,
        /// Author (who created this task)
        #[arg(long)]
        author: Option<String>,
        /// Depends on these task UUIDs/prefixes (repeatable)
        #[arg(long = "depends-on")]
        depends_on: Vec<String>,
    },

    /// List tasks
    List {
        /// Filter by status
        #[arg(long)]
        status: Option<Status>,
        /// Filter by tag
        #[arg(long)]
        tag: Option<String>,
        /// Show all items including Done
        #[arg(long)]
        all: bool,
    },

    /// Show a single task in detail
    Show {
        /// UUID or prefix
        id: String,
    },

    /// Atomically claim a task (New/Incomplete/PrChangesRequested → InProgress). Blocked if the project is inactive.
    Claim {
        /// UUID or prefix
        id: String,
        /// Agent identifier
        #[arg(long)]
        agent: Option<String>,
    },

    /// Atomically claim multiple tasks (all-or-nothing). Blocked if the project is inactive.
    ClaimMulti {
        /// UUIDs or prefixes
        ids: Vec<String>,
        /// Agent identifier
        #[arg(long)]
        agent: Option<String>,
    },

    /// Mark a task as having a PR open (InProgress → PrOpen)
    PrOpen {
        /// UUID or prefix
        id: String,
        /// GitHub PR URL
        #[arg(long)]
        pr_url: String,
    },

    /// Mark a task's PR as having changes requested (PrOpen → PrChangesRequested)
    PrChangesRequested {
        /// UUID or prefix
        id: String,
    },

    /// Mark a task as done
    Done {
        /// UUID or prefix
        id: String,
    },

    /// Mark a task as incomplete
    Incomplete {
        /// UUID or prefix
        id: String,
        /// Reason for marking incomplete
        #[arg(long)]
        reason: Option<String>,
    },

    /// Release a claim and reset to New (InProgress or Incomplete → New)
    Unclaim {
        /// UUID or prefix
        id: String,
    },

    /// Edit a task's fields
    Edit {
        /// UUID or prefix
        id: String,
        /// New title
        #[arg(long)]
        title: Option<String>,
        /// New description
        #[arg(long)]
        desc: Option<String>,
        /// New priority
        #[arg(long)]
        priority: Option<u8>,
        /// Replace tags
        #[arg(long = "tag")]
        tags: Option<Vec<String>>,
        /// New link
        #[arg(long)]
        link: Option<String>,
        /// New source
        #[arg(long)]
        source: Option<String>,
        /// New author
        #[arg(long)]
        author: Option<String>,
        /// Add dependency on a task UUID/prefix (repeatable)
        #[arg(long = "add-dep")]
        add_deps: Vec<String>,
        /// Remove dependency on a task UUID/prefix (repeatable)
        #[arg(long = "remove-dep")]
        remove_deps: Vec<String>,
    },

    /// Move a task to a specific position in the list
    Reorder {
        /// UUID or prefix
        id: String,
        /// Target position (0-indexed)
        #[arg(long)]
        position: usize,
    },

    /// Remove a task entirely
    Remove {
        /// UUID or prefix
        id: String,
    },

    /// Manage projects
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
}
