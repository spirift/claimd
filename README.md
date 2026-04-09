# ai-todo

A concurrent todo list CLI built in Rust for multi-agent AI collaboration. Agents can add, view, claim, and complete tasks with atomic locking that prevents two agents from picking up the same work.

## Features

- **Atomic claiming** -- when two agents race to claim the same todo, exactly one wins; the other gets an immediate error (exit code 2)
- **Manager support** -- `claim-multi` claims multiple items in a single atomic operation with all-or-nothing semantics
- **Statuses** -- New, InProgress, PrOpen, PrChangesRequested, Done, Incomplete with transition validation
- **JSON mode** -- `--json` flag on any command for machine-parseable output
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
# Binary at ./target/release/ai-todo
```

## Quick start

```bash
ai-todo init
ai-todo add "Set up database schema" --priority 0 --tag backend
ai-todo add "Implement auth" --desc "OAuth2 flow" --priority 1
ai-todo list
ai-todo claim 6d45 --agent "agent-1"
ai-todo done 6d45
```

## Storage

Data is stored in `~/.ai-todo/` by default:

- `todo.json` -- canonical data file
- `todo.lock` -- flock target for atomic operations
- `todo.json.tmp` -- transient write target (atomic rename)

Override the store location with `--dir /path` or the `AI_TODO_DIR` environment variable.

## Projects

Use `--project <name>` (or `AI_TODO_PROJECT` env var) to isolate tasks per project. Each project gets its own store directory under `~/.ai-todo/projects/<name>/` with no cross-pollution between projects.

```bash
# Initialize separate projects
ai-todo --project backend init
ai-todo --project frontend init

# Tasks are completely isolated
ai-todo --project backend add "Design API schema"
ai-todo --project frontend add "Build login page"

# Each project only sees its own tasks
ai-todo --project backend list    # only backend tasks
ai-todo --project frontend list   # only frontend tasks

# List all projects
ai-todo projects

# Use env var for convenience
export AI_TODO_PROJECT=backend
ai-todo list   # shows backend tasks
```

Without `--project`, the default store at `~/.ai-todo/` is used (backward compatible).

## Global options

These flags can be used with any command:

| Flag | Description |
|------|-------------|
| `--json` | Output as JSON instead of human-readable text |
| `--dir <DIR>` | Path to the todo store directory (env: `AI_TODO_DIR`) |
| `--project <NAME>` | Project name for task isolation (env: `AI_TODO_PROJECT`) |
| `-h, --help` | Print help |

## Exit codes

| Code | Meaning |
|------|---------|
| `0` | Success |
| `1` | General error (not found, invalid args, IO) |
| `2` | Claim conflict (already claimed, lock held, or unresolved dependencies) |

## Commands

### `init`

Initialize the todo store. Creates the store directory and an empty `todo.json`. Idempotent.

```bash
ai-todo init
```

### `add`

Add a new todo item with status `New`.

```bash
ai-todo add <TITLE> [OPTIONS]
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
ai-todo add "Fix login bug" --priority 0 --tag urgent --tag auth
ai-todo add "Write docs" --desc "API reference for v2" --priority 3
ai-todo add "Investigate crash" --link "https://jira.example.com/BUG-42" --source jira --author agent-alpha
ai-todo add "Write integration tests" --depends-on 6d45 --depends-on f5b8
```

### `list`

List todo items. By default shows `New` and `InProgress` items only.

```bash
ai-todo list [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--status <STATUS>` | Filter by status: `new`, `in_progress`, `pr_open`, `pr_changes_requested`, `done`, `incomplete` |
| `--tag <TAG>` | Filter by tag |
| `--all` | Show all items including `Done` |

Examples:

```bash
ai-todo list
ai-todo list --status in_progress
ai-todo list --tag backend --all
ai-todo list --json
```

### `show`

Show full detail of a single todo item.

```bash
ai-todo show <ID>
```

| Argument | Description |
|----------|-------------|
| `<ID>` | UUID or prefix (minimum 4 characters) |

Example:

```bash
ai-todo show 6d45
```

### `claim`

Atomically claim a todo, transitioning it from `New`, `Incomplete`, or `PrChangesRequested` to `InProgress`. Uses a non-blocking lock -- if another process holds the lock, fails immediately with exit code 2. Also fails with exit code 2 if the todo has unresolved dependencies.

When claiming a `PrChangesRequested` item, the previous `claimed_by` agent is moved to the `previously_claimed_by` list, and the new agent takes over.

```bash
ai-todo claim <ID> [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--agent <AGENT>` | Agent identifier string |

Examples:

```bash
ai-todo claim 6d45 --agent "agent-1"
ai-todo claim 6d45 --agent "agent-2"   # fails if already claimed (InProgress)
ai-todo claim c022 --agent "agent-1"   # fails if has pending dependencies
ai-todo claim 6d45 --agent "agent-2"   # succeeds if PrChangesRequested, agent-1 → previously_claimed_by
```

### `claim-multi`

Atomically claim multiple todos in a single operation. All-or-nothing: if any item is not claimable, none are claimed.

```bash
ai-todo claim-multi [IDS]... [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--agent <AGENT>` | Agent identifier string |

Example:

```bash
ai-todo claim-multi 6d45 f5b8 31c9 --agent "manager-1"
```

### `pr-open`

Mark a todo as having a PR open and record the PR URL. Valid from `InProgress` or `PrChangesRequested`.

```bash
ai-todo pr-open <ID> --pr-url <URL>
```

| Option | Description |
|--------|-------------|
| `--pr-url <URL>` | GitHub PR URL (required) |

Example:

```bash
ai-todo pr-open 6d45 --pr-url "https://github.com/org/repo/pull/42"
```

### `pr-changes-requested`

Mark a todo's PR as having changes requested. Valid from `PrOpen`.

```bash
ai-todo pr-changes-requested <ID>
```

Example:

```bash
ai-todo pr-changes-requested 6d45
```

### `done`

Mark a todo as done. Clears the `claimed_by` field. When a todo is marked done, it is automatically moved from the `depends_on` list to the `depends_on_completed` list of any todo that depended on it. Once all dependencies are resolved, that todo becomes claimable.

```bash
ai-todo done <ID>
```

Example:

```bash
ai-todo done 6d45
# Any todo with 6d45 in its depends_on list now has it in depends_on_completed instead
```

### `incomplete`

Mark a todo as incomplete. Clears the `claimed_by` field. Optionally appends a reason to the description.

```bash
ai-todo incomplete <ID> [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--reason <REASON>` | Reason for marking incomplete (appended to description) |

Example:

```bash
ai-todo incomplete 6d45 --reason "blocked on design review"
```

### `unclaim`

Release a claim, transitioning from `InProgress` back to `New`. Clears the `claimed_by` field.

```bash
ai-todo unclaim <ID>
```

Example:

```bash
ai-todo unclaim 6d45
```

### `edit`

Edit fields on an existing todo item. Only specified fields are changed.

```bash
ai-todo edit <ID> [OPTIONS]
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
ai-todo edit 6d45 --title "Updated title" --priority 1 --tag new-tag
ai-todo edit 6d45 --link "https://github.com/org/repo/issues/99" --source github
ai-todo edit c022 --add-dep 6d45 --add-dep f5b8
ai-todo edit c022 --remove-dep f5b8
```

### `reorder`

Move a todo to a specific position in the list (0-indexed).

```bash
ai-todo reorder <ID> --position <N>
```

| Option | Description |
|--------|-------------|
| `--position <N>` | Target position, 0-indexed |

Example:

```bash
ai-todo reorder 6d45 --position 0   # move to top
```

### `remove`

Delete a todo entirely.

```bash
ai-todo remove <ID>
```

Example:

```bash
ai-todo remove 6d45
```

### `projects`

List all known projects and their store paths.

```bash
ai-todo projects
```

Example output:

```
(default)  /Users/me/.ai-todo
backend    /Users/me/.ai-todo/projects/backend
frontend   /Users/me/.ai-todo/projects/frontend
```

## Dependencies

Todos can declare dependencies on other todos. A todo with unresolved dependencies cannot be claimed.

```bash
# Create tasks with a dependency chain
ai-todo add "Design schema"
ai-todo add "Build API" --depends-on 6d45
ai-todo add "Write tests" --depends-on 6d45 --depends-on f5b8

# Trying to claim "Write tests" fails (exit code 2)
ai-todo claim c022 --agent "agent-1"
# Error: Todo c0228801 has unresolved dependencies: 6d45a75a, f5b883f1

# Complete dependencies one by one
ai-todo done 6d45   # "Design schema" done — auto-moves from depends_on to depends_on_completed
ai-todo done f5b8   # "Build API" done — all deps resolved

# Now claim succeeds
ai-todo claim c022 --agent "agent-1"
```

Dependencies can also be added/removed after creation via `edit`:

```bash
ai-todo edit c022 --add-dep 6d45
ai-todo edit c022 --remove-dep 6d45
```

## Concurrency model

All mutations acquire an exclusive file lock (`flock`) on `todo.lock`:

- **`claim` / `claim-multi`**: Use `try_lock_exclusive` (non-blocking). If the lock is held by another process, the command fails immediately with exit code 2.
- **Other mutations** (`add`, `done`, `edit`, etc.): Use `lock_exclusive` (blocking). The command waits until the lock is available.
- **Read-only** (`list`, `show`): Use `lock_shared`. Multiple concurrent readers are allowed.

Writes use an atomic rename pattern (write to `todo.json.tmp`, then rename over `todo.json`) so the data file is never left in a partial state.

## JSON output

Pass `--json` to any command for machine-parseable output:

```bash
# List as JSON array
ai-todo list --json

# Single item as JSON object
ai-todo show 6d45 --json

# Errors as JSON to stderr
ai-todo claim 6d45 --json
# {"error":"Todo 6d45a75a is already being worked on by 'agent-1'","code":"already_claimed"}
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
