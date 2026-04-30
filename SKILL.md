# claimd

Concurrent task list CLI for multi-agent AI workflows. Agents can add, view, claim, and complete tasks with atomic file locking so two agents can never pick up the same work.

## Binary

```bash
claimd          # after: cargo install --path /Users/moi/code/claimd
```

Storage defaults to the current directory (or set `CLAIMD_DIR` env var). Scope tasks to a project with `--project <name>` or `CLAIMD_PROJECT` env var.

## Setup

```bash
claimd init                        # create the store in current dir (or --dir path)
claimd --dir /path/to/store init   # explicit store location
```

## Global flags (all commands)

| Flag               | Env              | Default | Purpose                                    |
| ------------------ | ---------------- | ------- | ------------------------------------------ |
| `--json`           |                  | false   | Output JSON instead of human-readable text |
| `--dir <path>`     | `CLAIMD_DIR`     | `.`     | Path to the task store directory           |
| `--project <name>` | `CLAIMD_PROJECT` | none    | Isolate tasks per project                  |

## Commands

### List

```bash
claimd list                        # new + in_progress + incomplete (hides Done)
claimd list --all                  # everything including Done
claimd list --status new
claimd list --status in_progress
claimd list --status pr_open
claimd list --status pr_changes_requested
claimd list --status done
claimd list --status incomplete
claimd list --tag backend
```

### Add

```bash
claimd add "task title"
claimd add "title" --priority 1            # 0=highest, 9=lowest, default 5
claimd add "title" --desc "details"
claimd add "title" --tag backend --tag auth
claimd add "title" --source spec --author task-splitter
claimd add "title" --link https://github.com/...
claimd add "title" --depends-on <uuid-or-prefix>   # repeatable
```

### Read

```bash
claimd show <id>     # UUID or 4+ char prefix
```

### Claim / work

```bash
claimd claim <id> --agent my-agent-id     # exit 2 = already claimed, try next
claimd claim-multi <id1> <id2> --agent x  # all-or-nothing atomic claim
claimd done <id>
claimd incomplete <id>
claimd incomplete <id> --reason "why it failed"
claimd unclaim <id>                        # release back to New without marking incomplete
```

### PR lifecycle

```bash
claimd pr-open <id> --pr-url https://github.com/...
claimd pr-changes-requested <id>
```

### Edit

```bash
claimd edit <id> --title "new title"
claimd edit <id> --priority 2
claimd edit <id> --desc "updated description"
claimd edit <id> --tag new-tag             # replaces all tags
claimd edit <id> --add-dep <uuid-or-prefix>
claimd edit <id> --remove-dep <uuid-or-prefix>
claimd edit <id> --link https://...
claimd edit <id> --source spec --author splitter
```

### Other

```bash
claimd reorder <id> --position 0           # move to top (0-indexed)
claimd remove <id>
```

### Projects

```bash
claimd project list
claimd project status <name>
claimd project activate <name>             # allow new claims
claimd project deactivate <name>           # block new claims (existing work still completable)
```

## Status lifecycle

```
New
 └─► InProgress        (claim / claim-multi)
       ├─► PrOpen                       (pr-open)
       │     ├─► Done                   (done)
       │     └─► PrChangesRequested     (pr-changes-requested)
       │               └─► InProgress   (claim again)
       └─► Incomplete                   (incomplete)
             └─► New                    (unclaim)
```

`claim` accepts: `New`, `Incomplete`, `PrChangesRequested` → `InProgress`  
`unclaim` accepts: `InProgress`, `Incomplete` → `New`

## Agent workflow

1. `claimd list --status new --json` — find available work
2. `claimd claim <id> --agent <your-id>` — atomically grab it (exit code 2 = conflict, try next item)
3. Do the work
4. `claimd pr-open <id> --pr-url <url>` — when PR is up
5. `claimd done <id>` — when merged, OR `claimd incomplete <id> --reason "..."` if it failed

## Notes

- IDs can be shortened to any unambiguous prefix (minimum 4 chars)
- `claim` uses `flock` — exit code 2 means another agent grabbed it; move to the next task
- `claim-multi` is all-or-nothing: if any item fails validation, nothing is claimed
- `depends_on` UUIDs block claiming until all dependencies are `Done`
- Inactive projects block new claims but existing `InProgress` work can still be completed
- Append `--json` to any command for machine-readable output
