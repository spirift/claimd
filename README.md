# claimd

A concurrent task manager CLI built in Rust for multi-agent AI collaboration. Agents can add, view, claim, and complete tasks with atomic locking that prevents two agents from picking up the same work.

## Features

- **Atomic claiming** -- when two agents race to claim the same task, exactly one wins; the other gets an immediate error (exit code 2)
- **Multi-claim** -- `claim-multi` claims multiple tasks in a single atomic operation with all-or-nothing semantics
- **Statuses** -- New, InProgress, PrOpen, PrChangesRequested, Done, Incomplete with transition validation
- **Project active/inactive** -- mark a project inactive to block new claims while keeping all tasks visible and existing in-progress work completable
- **JSON mode** -- `--json` flag on any command for machine-parseable output; every task includes a `project_active` field
- **UUID prefix matching** -- reference any task by the first 4+ characters of its ID
- **Dependencies** -- declare dependencies between tasks; a task with pending deps cannot be claimed until all deps are done
- **Minimal footprint** -- ~764KB release binary, 6 dependencies, no async runtime or database

## Installation

```bash
cargo install --path .
```

Or build manually:

```bash
cargo build --release
# Binary at ./target/release/claimd
```

## Quick start

```bash
claimd --project myapp init
claimd --project myapp add "Set up database schema" --priority 0 --tag backend
claimd --project myapp add "Implement auth" --desc "OAuth2 flow" --priority 1
claimd --project myapp list
claimd --project myapp claim 6d45 --agent "agent-1"
claimd --project myapp done 6d45
```

## Storage

Data is stored in `~/.claimd/projects/<name>/` by default:

- `tasks.json` -- canonical data file
- `tasks.lock` -- flock target for atomic operations
- `tasks.json.tmp` -- transient write target (atomic rename)

Override the base location with `--dir /path` or the `CLAIMD_DIR` environment variable.

## Projects

`--project <name>` (or `CLAIMD_PROJECT` env var) is required for all task commands. Each project gets its own isolated store directory with no cross-pollution between projects.

```bash
# Initialize separate projects
claimd --project backend init
claimd --project frontend init

# Tasks are completely isolated
claimd --project backend add "Design API schema"
claimd --project frontend add "Build login page"

# Each project only sees its own tasks
claimd --project backend list    # only backend tasks
claimd --project frontend list   # only frontend tasks

# Use env var for convenience
export CLAIMD_PROJECT=backend
claimd list   # shows backend tasks
```

### Project active/inactive state

Projects have an `active` flag (default: `true`). When a project is **inactive**, all `→ InProgress` transitions are blocked — `claim` and `claim-multi` fail immediately with exit code 2. Existing in-progress tasks are unaffected and can still be progressed to `pr_open`, `done`, `incomplete`, etc. This is designed for spinning down a project: block new work starting while letting active agents finish up cleanly.

Every task output (JSON and human-readable) includes the project's active state so agents always know the current mode without a separate lookup.

```bash
# Deactivate a project — blocks new claims
claimd project deactivate backend

# Reactivate when ready
claimd project activate backend

# Check state of a single project
claimd project status backend

# List all projects with their active state
claimd project list
```

## Global options

These flags can be used with any command:

| Flag               | Description                                             |
| ------------------ | ------------------------------------------------------- |
| `--json`           | Output as JSON instead of human-readable text           |
| `--dir <DIR>`      | Path to the task store directory (env: `CLAIMD_DIR`)    |
| `--project <NAME>` | Project name for task isolation (env: `CLAIMD_PROJECT`) |
| `-h, --help`       | Print help                                              |

## Exit codes

| Code | Meaning                                                                                   |
| ---- | ----------------------------------------------------------------------------------------- |
| `0`  | Success                                                                                   |
| `1`  | General error (not found, invalid args, IO)                                               |
| `2`  | Claim conflict (already claimed, lock held, unresolved dependencies, or project inactive) |

## Commands

### `init`

Initialize the task store for a project. Creates the store directory and an empty `tasks.json`. Idempotent.

```bash
claimd --project myapp init
```

### `add`

Add a new task with status `New`.

```bash
claimd add <TITLE> [OPTIONS]
```

| Option              | Description                                              |
| ------------------- | -------------------------------------------------------- |
| `--desc <DESC>`     | Description text                                         |
| `--priority <N>`    | Priority, 0 = highest (default: 5)                       |
| `--tag <TAG>`       | Tag (repeatable for multiple tags)                       |
| `--link <URL>`      | Link (URL or reference)                                  |
| `--source <SOURCE>` | Source (where this task came from, e.g. "jira", "slack") |
| `--author <AUTHOR>` | Author (who created this task)                           |
| `--depends-on <ID>` | Depends on this task UUID/prefix (repeatable)            |

Examples:

```bash
claimd add "Fix login bug" --priority 0 --tag urgent --tag auth
claimd add "Write docs" --desc "API reference for v2" --priority 3
claimd add "Investigate crash" --link "https://jira.example.com/BUG-42" --source jira --author agent-alpha
claimd add "Write integration tests" --depends-on 6d45 --depends-on f5b8
```

### `list`

List tasks. By default shows `New`, `InProgress`, and `Incomplete` tasks only.

```bash
claimd list [OPTIONS]
```

| Option              | Description                                                                                     |
| ------------------- | ----------------------------------------------------------------------------------------------- |
| `--status <STATUS>` | Filter by status: `new`, `in_progress`, `pr_open`, `pr_changes_requested`, `done`, `incomplete` |
| `--tag <TAG>`       | Filter by tag                                                                                   |
| `--all`             | Show all tasks including `Done`                                                                 |

Examples:

```bash
claimd list
claimd list --status in_progress
claimd list --tag backend --all
claimd list --json
```

### `show`

Show full detail of a single task. For convenience this command works without `--project` — when no project is given it scans all projects to find the task by ID.

```bash
claimd show <ID>
claimd --project myapp show <ID>
```

| Argument | Description                           |
| -------- | ------------------------------------- |
| `<ID>`   | UUID or prefix (minimum 4 characters) |

Example:

```bash
claimd show 6d45             # searches all projects
claimd --project myapp show 6d45
```

### `claim`

Atomically claim a task, transitioning it from `New`, `Incomplete`, or `PrChangesRequested` to `InProgress`. Uses a non-blocking lock -- if another process holds the lock, fails immediately with exit code 2. Also fails with exit code 2 if the task has unresolved dependencies or if the project is inactive.

When claiming a `PrChangesRequested` task, the previous `claimed_by` agent is moved to the `previously_claimed_by` list, and the new agent takes over.

```bash
claimd claim <ID> [OPTIONS]
```

| Option            | Description             |
| ----------------- | ----------------------- |
| `--agent <AGENT>` | Agent identifier string |

Examples:

```bash
claimd claim 6d45 --agent "agent-1"
claimd claim 6d45 --agent "agent-2"   # fails if already claimed (InProgress)
claimd claim c022 --agent "agent-1"   # fails if has pending dependencies
claimd claim 6d45 --agent "agent-2"   # succeeds if PrChangesRequested, agent-1 → previously_claimed_by
claimd claim 6d45 --agent "agent-1"   # fails with exit code 2 if project is inactive
```

### `claim-multi`

Atomically claim multiple tasks in a single operation. All-or-nothing: if any task is not claimable, none are claimed.

```bash
claimd claim-multi [IDS]... [OPTIONS]
```

| Option            | Description             |
| ----------------- | ----------------------- |
| `--agent <AGENT>` | Agent identifier string |

Example:

```bash
claimd claim-multi 6d45 f5b8 31c9 --agent "manager-1"
```

### `pr-open`

Mark a task as having a PR open and record the PR URL. Valid from `InProgress` or `PrChangesRequested`.

```bash
claimd pr-open <ID> --pr-url <URL>
```

| Option           | Description              |
| ---------------- | ------------------------ |
| `--pr-url <URL>` | GitHub PR URL (required) |

Example:

```bash
claimd pr-open 6d45 --pr-url "https://github.com/org/repo/pull/42"
```

### `pr-changes-requested`

Mark a task's PR as having changes requested. Valid from `PrOpen`.

```bash
claimd pr-changes-requested <ID>
```

Example:

```bash
claimd pr-changes-requested 6d45
```

### `done`

Mark a task as done. Clears the `claimed_by` field. When a task is marked done, it is automatically moved from the `depends_on` list to the `depends_on_completed` list of any task that depended on it. Once all dependencies are resolved, that task becomes claimable.

```bash
claimd done <ID>
```

Example:

```bash
claimd done 6d45
# Any task with 6d45 in its depends_on list now has it in depends_on_completed instead
```

### `incomplete`

Mark a task as incomplete. Clears the `claimed_by` field. Optionally appends a reason to the description.

```bash
claimd incomplete <ID> [OPTIONS]
```

| Option              | Description                                             |
| ------------------- | ------------------------------------------------------- |
| `--reason <REASON>` | Reason for marking incomplete (appended to description) |

Example:

```bash
claimd incomplete 6d45 --reason "blocked on design review"
```

### `unclaim`

Reset a task to `New`, transitioning from `InProgress` or `Incomplete`. Clears the `claimed_by` field.

```bash
claimd unclaim <ID>
```

Example:

```bash
claimd unclaim 6d45
```

### `edit`

Edit fields on an existing task. Only specified fields are changed.

```bash
claimd edit <ID> [OPTIONS]
```

| Option              | Description                                            |
| ------------------- | ------------------------------------------------------ |
| `--title <TITLE>`   | New title                                              |
| `--desc <DESC>`     | New description                                        |
| `--priority <N>`    | New priority                                           |
| `--tag <TAG>`       | Replace tags (repeatable)                              |
| `--link <URL>`      | New link                                               |
| `--source <SOURCE>` | New source                                             |
| `--author <AUTHOR>` | New author                                             |
| `--add-dep <ID>`    | Add a dependency on a task UUID/prefix (repeatable)    |
| `--remove-dep <ID>` | Remove a dependency on a task UUID/prefix (repeatable) |

Example:

```bash
claimd edit 6d45 --title "Updated title" --priority 1 --tag new-tag
claimd edit 6d45 --link "https://github.com/org/repo/issues/99" --source github
claimd edit c022 --add-dep 6d45 --add-dep f5b8
claimd edit c022 --remove-dep f5b8
```

### `reorder`

Move a task to a specific position in the list (0-indexed).

```bash
claimd reorder <ID> --position <N>
```

| Option           | Description                |
| ---------------- | -------------------------- |
| `--position <N>` | Target position, 0-indexed |

Example:

```bash
claimd reorder 6d45 --position 0   # move to top
```

### `remove`

Delete a task entirely.

```bash
claimd remove <ID>
```

Example:

```bash
claimd remove 6d45
```

### `project list`

List all known projects, their active state, and store paths.

```bash
claimd project list
```

Example output:

```
backend               active   /Users/me/.claimd/projects/backend
frontend              INACTIVE /Users/me/.claimd/projects/frontend
```

### `project status`

Show the active state of a specific project.

```bash
claimd project status <NAME>
```

Example:

```bash
claimd project status backend
# backend: active
```

### `project activate`

Activate a project, re-enabling `claim` and `claim-multi`.

```bash
claimd project activate <NAME>
```

Example:

```bash
claimd project activate backend
# backend: active
```

### `project deactivate`

Deactivate a project, blocking all `→ InProgress` transitions. Existing in-progress tasks are unaffected.

```bash
claimd project deactivate <NAME>
```

Example:

```bash
claimd project deactivate frontend
# frontend: INACTIVE
```

## Dependencies

Tasks can declare dependencies on other tasks. A task with unresolved dependencies cannot be claimed.

```bash
# Create tasks with a dependency chain
claimd add "Design schema"
claimd add "Build API" --depends-on 6d45
claimd add "Write tests" --depends-on 6d45 --depends-on f5b8

# Trying to claim "Write tests" fails (exit code 2)
claimd claim c022 --agent "agent-1"
# Error: Task c0228801 has unresolved dependencies: 6d45a75a, f5b883f1

# Complete dependencies one by one
claimd done 6d45   # "Design schema" done — auto-moves from depends_on to depends_on_completed
claimd done f5b8   # "Build API" done — all deps resolved

# Now claim succeeds
claimd claim c022 --agent "agent-1"
```

Dependencies can also be added/removed after creation via `edit`:

```bash
claimd edit c022 --add-dep 6d45
claimd edit c022 --remove-dep 6d45
```

## Concurrency model

All mutations acquire an exclusive file lock (`flock`) on `tasks.lock`:

- **`claim` / `claim-multi`**: Use `try_lock_exclusive` (non-blocking). If the lock is held by another process, the command fails immediately with exit code 2.
- **Other mutations** (`add`, `done`, `edit`, etc.): Use `lock_exclusive` (blocking). The command waits until the lock is available.
- **Read-only** (`list`, `show`): Use `lock_shared`. Multiple concurrent readers are allowed.

Writes use an atomic rename pattern (write to `tasks.json.tmp`, then rename over `tasks.json`) so the data file is never left in a partial state.

## JSON output

Pass `--json` to any command for machine-parseable output. Every task object includes a `project_active` field so agents can inspect the project state without a separate command.

```bash
# List as JSON array — each item includes project_active
claimd list --json
# [{"id":"...","title":"...","status":"new","project_active":true,...}]

# Single item as pretty-printed JSON object
claimd show 6d45 --json

# Project commands as JSON
claimd project list --json
# {"projects":[{"name":"backend","active":true},{"name":"frontend","active":false}]}

claimd project status backend --json
# {"name":"backend","active":true}

# Errors as JSON to stderr
claimd claim 6d45 --json
# {"error":"Task 6d45a75a is already being worked on by 'agent-1'","code":"already_claimed"}

# Project inactive error
claimd --project frontend claim 6d45 --json
# {"error":"Project is inactive — claiming is disabled","code":"project_inactive"}
```

## Data model

Each task has the following fields:

| Field                   | Type     | Description                                                                                            |
| ----------------------- | -------- | ------------------------------------------------------------------------------------------------------ |
| `id`                    | UUID     | Unique identifier                                                                                      |
| `title`                 | string   | Title text                                                                                             |
| `description`           | string?  | Optional description                                                                                   |
| `status`                | enum     | `new`, `in_progress`, `pr_open`, `pr_changes_requested`, `done`, `incomplete`                          |
| `priority`              | u8       | 0 = highest priority                                                                                   |
| `created_at`            | datetime | Creation timestamp (UTC)                                                                               |
| `updated_at`            | datetime | Last modification timestamp (UTC)                                                                      |
| `claimed_by`            | string?  | Agent identifier that claimed this task                                                                |
| `pr_url`                | string?  | GitHub PR URL, set when transitioning to `pr_open`                                                     |
| `previously_claimed_by` | string[] | Agents that previously worked on this task                                                             |
| `link`                  | string?  | URL or reference link                                                                                  |
| `source`                | string?  | Where this task came from (e.g. "jira", "slack", "github")                                             |
| `author`                | string?  | Who created this task                                                                                  |
| `tags`                  | string[] | List of tags                                                                                           |
| `depends_on`            | UUID[]   | IDs of tasks that must be completed before this one can be claimed                                     |
| `depends_on_completed`  | UUID[]   | IDs of dependencies that have been completed                                                           |
| `project_active`        | bool     | Whether the owning project currently allows claiming (injected at output time, not stored on the task) |
