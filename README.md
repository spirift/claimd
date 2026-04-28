# claimd

A concurrent todo list CLI built in Rust for multi-agent AI collaboration. Agents can add, view, claim, and complete tasks with atomic locking that prevents two agents from picking up the same work.

## Features

- **Atomic claiming** -- when two agents race to claim the same todo, exactly one wins; the other gets an immediate error (exit code 2)
- **Manager support** -- `claim-multi` claims multiple items in a single atomic operation with all-or-nothing semantics
- **Statuses** -- New, InProgress, PrOpen, PrChangesRequested, Done, Incomplete with transition validation
- **Project active/inactive** -- mark a project inactive to block new claims while keeping all tasks visible and existing in-progress work completable
- **JSON mode** -- `--json` flag on any command for machine-parseable output; every task includes a `project_active` field
- **UUID prefix matching** -- reference any item by the first 4+ characters of its ID
- **Dependencies** -- declare dependencies between todos; a task with pending deps cannot be claimed until all deps are done
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
claimd init
claimd add "Set up database schema" --priority 0 --tag backend
claimd add "Implement auth" --desc "OAuth2 flow" --priority 1
claimd list
claimd claim 6d45 --agent "agent-1"
claimd done 6d45
```

## Storage

Data is stored in `~/.claimd/` by default:

- `todo.json` -- canonical data file
- `todo.lock` -- flock target for atomic operations
- `todo.json.tmp` -- transient write target (atomic rename)

Override the store location with `--dir /path` or the `CLAIMD_DIR` environment variable.

## Projects

Use `--project <name>` (or `CLAIMD_PROJECT` env var) to isolate tasks per project. Each project gets its own store directory under `~/.claimd/projects/<name>/` with no cross-pollution between projects.

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

Without `--project`, the default store at `~/.claimd/` is used (always active, no `project.json`).

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

| Flag | Description |
|------|-------------|
| `--json` | Output as JSON instead of human-readable text |
| `--dir <DIR>` | Path to the todo store directory (env: `CLAIMD_DIR`) |
| `--project <NAME>` | Project name for task isolation (env: `CLAIMD_PROJECT`) |
| `-h, --help` | Print help |

## Exit codes

| Code | Meaning |
|------|---------|
| `0` | Success |
| `1` | General error (not found, invalid args, IO) |
| `2` | Claim conflict (already claimed, lock held, unresolved dependencies, or project inactive) |

## Commands

### `init`

Initialize the todo store. Creates the store directory and an empty `todo.json`. Idempotent.

```bash
claimd init
```

### `add`

Add a new todo item with status `New`.

```bash
claimd add <TITLE> [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--desc <DESC>` | Description text |
| `--priority <N>` | Priority, 0 = highest (default: 5) |
| `--tag <TAG>` | Tag (repeatable for multiple tags) |
| `--link <URL>` | Link (URL or reference) |
| `--source <SOURCE>` | Source (where this todo came from, e.g. "jira", "slack") |
| `--author <AUTHOR>` | Author (who created this todo) |
| `--depends-on <ID>` | Depends on this todo UUID/prefix (repeatable) |

Examples:

```bash
claimd add "Fix login bug" --priority 0 --tag urgent --tag auth
claimd add "Write docs" --desc "API reference for v2" --priority 3
claimd add "Investigate crash" --link "https://jira.example.com/BUG-42" --source jira --author agent-alpha
claimd add "Write integration tests" --depends-on 6d45 --depends-on f5b8
```

### `list`

List todo items. By default shows `New` and `InProgress` items only.

```bash
claimd list [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--status <STATUS>` | Filter by status: `new`, `in_progress`, `pr_open`, `pr_changes_requested`, `done`, `incomplete` |
| `--tag <TAG>` | Filter by tag |
| `--all` | Show all items including `Done` |

Examples:

```bash
claimd list
claimd list --status in_progress
claimd list --tag backend --all
claimd list --json
```

### `show`

Show full detail of a single todo item.

```bash
claimd show <ID>
```

| Argument | Description |
|----------|-------------|
| `<ID>` | UUID or prefix (minimum 4 characters) |

Example:

```bash
claimd show 6d45
```

### `claim`

Atomically claim a todo, transitioning it from `New`, `Incomplete`, or `PrChangesRequested` to `InProgress`. Uses a non-blocking lock -- if another process holds the lock, fails immediately with exit code 2. Also fails with exit code 2 if the todo has unresolved dependencies or if the project is inactive.

When claiming a `PrChangesRequested` item, the previous `claimed_by` agent is moved to the `previously_claimed_by` list, and the new agent takes over.

```bash
claimd claim <ID> [OPTIONS]
```

| Option | Description |
|--------|-------------|
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

Atomically claim multiple todos in a single operation. All-or-nothing: if any item is not claimable, none are claimed.

```bash
claimd claim-multi [IDS]... [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--agent <AGENT>` | Agent identifier string |

Example:

```bash
claimd claim-multi 6d45 f5b8 31c9 --agent "manager-1"
```

### `pr-open`

Mark a todo as having a PR open and record the PR URL. Valid from `InProgress` or `PrChangesRequested`.

```bash
claimd pr-open <ID> --pr-url <URL>
```

| Option | Description |
|--------|-------------|
| `--pr-url <URL>` | GitHub PR URL (required) |

Example:

```bash
claimd pr-open 6d45 --pr-url "https://github.com/org/repo/pull/42"
```

### `pr-changes-requested`

Mark a todo's PR as having changes requested. Valid from `PrOpen`.

```bash
claimd pr-changes-requested <ID>
```

Example:

```bash
claimd pr-changes-requested 6d45
```

### `done`

Mark a todo as done. Clears the `claimed_by` field. When a todo is marked done, it is automatically moved from the `depends_on` list to the `depends_on_completed` list of any todo that depended on it. Once all dependencies are resolved, that todo becomes claimable.

```bash
claimd done <ID>
```

Example:

```bash
claimd done 6d45
# Any todo with 6d45 in its depends_on list now has it in depends_on_completed instead
```

### `incomplete`

Mark a todo as incomplete. Clears the `claimed_by` field. Optionally appends a reason to the description.

```bash
claimd incomplete <ID> [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--reason <REASON>` | Reason for marking incomplete (appended to description) |

Example:

```bash
claimd incomplete 6d45 --reason "blocked on design review"
```

### `unclaim`

Reset a todo to `New`, transitioning from `InProgress` or `Incomplete`. Clears the `claimed_by` field.

```bash
claimd unclaim <ID>
```

Example:

```bash
claimd unclaim 6d45
```

### `edit`

Edit fields on an existing todo item. Only specified fields are changed.

```bash
claimd edit <ID> [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--title <TITLE>` | New title |
| `--desc <DESC>` | New description |
| `--priority <N>` | New priority |
| `--tag <TAG>` | Replace tags (repeatable) |
| `--link <URL>` | New link |
| `--source <SOURCE>` | New source |
| `--author <AUTHOR>` | New author |
| `--add-dep <ID>` | Add a dependency on a todo UUID/prefix (repeatable) |
| `--remove-dep <ID>` | Remove a dependency on a todo UUID/prefix (repeatable) |

Example:

```bash
claimd edit 6d45 --title "Updated title" --priority 1 --tag new-tag
claimd edit 6d45 --link "https://github.com/org/repo/issues/99" --source github
claimd edit c022 --add-dep 6d45 --add-dep f5b8
claimd edit c022 --remove-dep f5b8
```

### `reorder`

Move a todo to a specific position in the list (0-indexed).

```bash
claimd reorder <ID> --position <N>
```

| Option | Description |
|--------|-------------|
| `--position <N>` | Target position, 0-indexed |

Example:

```bash
claimd reorder 6d45 --position 0   # move to top
```

### `remove`

Delete a todo entirely.

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
(default)             active   /Users/me/.claimd
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

Todos can declare dependencies on other todos. A todo with unresolved dependencies cannot be claimed.

```bash
# Create tasks with a dependency chain
claimd add "Design schema"
claimd add "Build API" --depends-on 6d45
claimd add "Write tests" --depends-on 6d45 --depends-on f5b8

# Trying to claim "Write tests" fails (exit code 2)
claimd claim c022 --agent "agent-1"
# Error: Todo c0228801 has unresolved dependencies: 6d45a75a, f5b883f1

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

All mutations acquire an exclusive file lock (`flock`) on `todo.lock`:

- **`claim` / `claim-multi`**: Use `try_lock_exclusive` (non-blocking). If the lock is held by another process, the command fails immediately with exit code 2.
- **Other mutations** (`add`, `done`, `edit`, etc.): Use `lock_exclusive` (blocking). The command waits until the lock is available.
- **Read-only** (`list`, `show`): Use `lock_shared`. Multiple concurrent readers are allowed.

Writes use an atomic rename pattern (write to `todo.json.tmp`, then rename over `todo.json`) so the data file is never left in a partial state.

## JSON output

Pass `--json` to any command for machine-parseable output. Every task object includes a `project_active` field so agents can inspect the project state without a separate command.

```bash
# List as JSON array — each item includes project_active
claimd list --json
# [{"id":"...","title":"...","status":"new","project_active":true,...}]

# Single item as JSON object
claimd show 6d45 --json

# Project commands as JSON
claimd project list --json
# {"default_store":true,"projects":[{"name":"backend","active":true},{"name":"frontend","active":false}]}

claimd project status backend --json
# {"name":"backend","active":true}

# Errors as JSON to stderr
claimd claim 6d45 --json
# {"error":"Todo 6d45a75a is already being worked on by 'agent-1'","code":"already_claimed"}

# Project inactive error
claimd --project frontend claim 6d45 --json
# {"error":"Project is inactive — claiming is disabled","code":"project_inactive"}
```

## Data model

Each todo item has the following fields:

| Field | Type | Description |
|-------|------|-------------|
| `id` | UUID | Unique identifier |
| `title` | string | Title text |
| `description` | string? | Optional description |
| `status` | enum | `new`, `in_progress`, `pr_open`, `pr_changes_requested`, `done`, `incomplete` |
| `priority` | u8 | 0 = highest priority |
| `created_at` | datetime | Creation timestamp (UTC) |
| `updated_at` | datetime | Last modification timestamp (UTC) |
| `claimed_by` | string? | Agent identifier that claimed this item |
| `pr_url` | string? | GitHub PR URL, set when transitioning to `pr_open` |
| `previously_claimed_by` | string[] | Agents that previously worked on this item |
| `link` | string? | URL or reference link |
| `source` | string? | Where this todo came from (e.g. "jira", "slack", "github") |
| `author` | string? | Who created this todo |
| `tags` | string[] | List of tags |
| `depends_on` | UUID[] | IDs of todos that must be completed before this one can be claimed |
| `depends_on_completed` | UUID[] | IDs of dependencies that have been completed |
| `project_active` | bool | Whether the owning project currently allows claiming (injected at output time, not stored on the item) |
