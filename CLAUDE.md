# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & run

```bash
cargo build                  # debug build
cargo build --release        # release build (applies LTO + stripping — see Cargo.toml)
cargo install --path .       # install claimd to ~/.cargo/bin
cargo run -- --project foo list   # run without installing
```

## Build & test

```bash
cargo test                   # run all tests (39 unit tests as of v0.2)
```

## Architecture

Seven modules, each with a single responsibility:

- **`cli.rs`** — Clap structs only. `Cli` (global flags), `Command` (subcommands), `ProjectCommand`, `EventsCommand`. No logic here.
- **`model.rs`** — Data types: `TaskItem`, `TaskList`, `Status`, `ProjectMeta`. Pure data, no I/O. `ProjectMeta` carries `active`, `events_enabled` (default true), and `events_ttl_days` (default 7).
- **`event.rs`** — `EventType` enum (10 variants) and `EventRecord` struct. `EventRecord` has `ts`, `project`, `event`, `from: Option<Status>`, `to: Option<Status>`, `task: TaskItem`. Constructed inside command closures; project name filled in by `main.rs` before appending.
- **`store.rs`** — All filesystem I/O. `Store` wraps a directory path and provides three locking primitives: `with_try_lock` (non-blocking, used by claim), `with_lock` (blocking), `with_shared_lock` (read-only). Writes are atomic via `tasks.json.tmp` → rename. Lock file is `tasks.lock`. Also owns `append_event` (best-effort O_APPEND write to `events.jsonl`), `read_events`, and `prune_events`.
- **`commands.rs`** — Business logic. Takes a `&Store`, operates on `TaskList` inside the lock closure. All mutating functions return `Result<(TaskItem, EventRecord)>` — the event is constructed inside the closure where `from` status is known, but emission happens outside the lock in `main.rs`.
- **`output.rs`** — All printing. `OutputContext` carries `project_active` and `project_name` for injecting into every task output. `--json` is handled here, not in commands.
- **`main.rs`** — Wires everything together. Resolves the store path, enforces the project-required rule, dispatches commands, calls `emit()` after each mutation, handles the cross-project `show` scan, and owns the `project` and `events` subcommand logic directly.

## Key invariants

**Project is always required** for task commands. `--project <name>` (or `CLAIMD_PROJECT` env var) maps to `~/.claimd/projects/<name>/`. The only exception is `show <id>` with no project, which scans all project directories to find the task.

**Claim uses try-lock.** Exit code 2 means "try the next task" — it covers: lock contention, already claimed, pending dependencies, and inactive project. All other errors are exit code 1.

**`project` subcommand** bypasses the store set up in `main()` and builds its own `Store` instances directly from `base_dir/projects/<name>`. The `base_dir` is derived from `cli.dir` (or `~/.claimd`) in both `main()` and `run()` — keep them in sync if changing the default.

**Status transitions are enforced in `commands.rs`**, not in the model. `claim` accepts `New | Incomplete | PrChangesRequested → InProgress`. `unclaim` accepts `InProgress | Incomplete → New`. Invalid transitions return `Error::InvalidTransition`.

**Dependency propagation happens on `done`**: when a task is marked done, `commands::done` iterates all other tasks and moves the completed ID from `depends_on` to `depends_on_completed`. A task is claimable only when `depends_on` is empty.

**Event emission is best-effort and outside the write lock.** Commands return `(TaskItem, EventRecord)`. `main.rs` calls `emit(store, &project_name, event)` which sets the project field then calls `store.append_event`. If the append fails (disk full, etc.) it logs to stderr and continues — the state change has already committed. Events are never written inside the `with_lock` closure to avoid borrow-checker entanglement; the only failure window is a crash between the task-file commit and the event append.

**Project removal deletes everything.** `project remove <name>` calls `fs::remove_dir_all` on the project directory, taking `tasks.json`, `project.json`, `tasks.lock`, and `events.jsonl` with it.
