use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use chrono::Utc;
use fs2::FileExt;

use crate::error::{Error, Result};
use crate::event::EventRecord;
use crate::model::{ProjectMeta, TaskList};

pub struct Store {
    dir: PathBuf,
}

impl Store {
    pub fn new(dir: PathBuf) -> Self {
        Store { dir }
    }

    pub fn data_path(&self) -> PathBuf {
        self.dir.join("tasks.json")
    }

    fn lock_path(&self) -> PathBuf {
        self.dir.join("tasks.lock")
    }

    fn tmp_path(&self) -> PathBuf {
        self.dir.join("tasks.json.tmp")
    }

    fn project_meta_path(&self) -> PathBuf {
        self.dir.join("project.json")
    }

    pub fn events_path(&self) -> PathBuf {
        self.dir.join("events.jsonl")
    }

    pub fn init(&self) -> Result<()> {
        fs::create_dir_all(&self.dir)?;
        // Create lock file
        OpenOptions::new()
            .create(true)
            .write(true)
            .open(self.lock_path())?;
        // Create data file if it doesn't exist
        let data = self.data_path();
        if !data.exists() {
            let list = TaskList::default();
            let bytes = serde_json::to_vec_pretty(&list)?;
            fs::write(&data, &bytes)?;
        }
        Ok(())
    }

    fn ensure_initialized(&self) -> Result<()> {
        if !self.data_path().exists() {
            return Err(Error::StoreNotInitialized);
        }
        Ok(())
    }

    fn open_lock_file(&self) -> Result<File> {
        self.ensure_initialized()?;
        let f = OpenOptions::new()
            .read(true)
            .write(true)
            .open(self.lock_path())?;
        Ok(f)
    }

    fn read_data(&self) -> Result<TaskList> {
        let bytes = fs::read(self.data_path())?;
        let list: TaskList = serde_json::from_slice(&bytes)?;
        Ok(list)
    }

    fn write_data(&self, list: &TaskList) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(list)?;
        fs::write(self.tmp_path(), &bytes)?;
        fs::rename(self.tmp_path(), self.data_path())?;
        Ok(())
    }

    /// Non-blocking lock for claim operations.
    /// Returns AlreadyLocked immediately if another process holds the lock.
    pub fn with_try_lock<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&mut TaskList) -> Result<T>,
    {
        let lock_file = self.open_lock_file()?;
        lock_file
            .try_lock_exclusive()
            .map_err(|_| Error::AlreadyLocked)?;

        let mut list = self.read_data()?;
        let result = f(&mut list)?;
        self.write_data(&list)?;
        // lock released on drop
        Ok(result)
    }

    /// Blocking lock for general mutations.
    pub fn with_lock<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&mut TaskList) -> Result<T>,
    {
        let lock_file = self.open_lock_file()?;
        lock_file.lock_exclusive()?;

        let mut list = self.read_data()?;
        let result = f(&mut list)?;
        self.write_data(&list)?;
        Ok(result)
    }

    /// Shared lock for read-only operations.
    pub fn with_shared_lock<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&TaskList) -> Result<T>,
    {
        let lock_file = self.open_lock_file()?;
        lock_file.lock_shared()?;

        let list = self.read_data()?;
        let result = f(&list)?;
        Ok(result)
    }

    /// Read project metadata. Returns default (active=true) if project.json doesn't exist.
    pub fn read_project_meta(&self) -> Result<ProjectMeta> {
        let path = self.project_meta_path();
        if !path.exists() {
            return Ok(ProjectMeta::default());
        }
        let bytes = fs::read(&path)?;
        let meta: ProjectMeta = serde_json::from_slice(&bytes)?;
        Ok(meta)
    }

    /// Write project metadata. Requires the store to be initialized first.
    pub fn write_project_meta(&self, meta: &ProjectMeta) -> Result<()> {
        self.ensure_initialized()?;
        let bytes = serde_json::to_vec_pretty(meta)?;
        fs::write(self.project_meta_path(), &bytes)?;
        Ok(())
    }

    /// Append an event to events.jsonl. Best-effort: logs to stderr on failure.
    pub fn append_event(&self, event: &EventRecord) {
        let meta = self.read_project_meta().unwrap_or_default();
        if !meta.events_enabled {
            return;
        }
        let line = match serde_json::to_string(event) {
            Ok(s) => s,
            Err(e) => { eprintln!("claimd: event serialize error: {e}"); return; }
        };
        let result = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.events_path())
            .and_then(|mut f| writeln!(f, "{line}"));
        if let Err(e) = result {
            eprintln!("claimd: event write error: {e}");
        }
    }

    /// Read all events from events.jsonl. Skips malformed lines.
    pub fn read_events(&self) -> Result<Vec<EventRecord>> {
        let path = self.events_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let content = fs::read_to_string(&path)?;
        let events = content
            .lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| match serde_json::from_str::<EventRecord>(l) {
                Ok(e) => Some(e),
                Err(e) => { eprintln!("claimd: skipping malformed event: {e}"); None }
            })
            .collect();
        Ok(events)
    }

    /// Remove events older than `ttl_days`. Returns count pruned. Atomic via tmp+rename.
    pub fn prune_events(&self, ttl_days: u32) -> Result<usize> {
        let path = self.events_path();
        if !path.exists() {
            return Ok(0);
        }
        let cutoff = Utc::now() - chrono::Duration::days(i64::from(ttl_days));
        let content = fs::read_to_string(&path)?;
        let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
        let total = lines.len();
        let kept: Vec<&str> = lines
            .iter()
            .filter(|l| {
                serde_json::from_str::<EventRecord>(l)
                    .map(|e| e.ts >= cutoff)
                    .unwrap_or(true)
            })
            .copied()
            .collect();
        let pruned = total - kept.len();
        if pruned > 0 {
            let tmp = self.dir.join("events.jsonl.tmp");
            let mut out = String::with_capacity(content.len());
            for line in &kept {
                out.push_str(line);
                out.push('\n');
            }
            fs::write(&tmp, out.as_bytes())?;
            fs::rename(&tmp, &path)?;
        }
        Ok(pruned)
    }
}
