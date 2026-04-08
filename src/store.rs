use std::fs::{self, File, OpenOptions};
use std::path::PathBuf;

use fs2::FileExt;

use crate::error::{Error, Result};
use crate::model::TodoList;

pub struct Store {
    dir: PathBuf,
}

impl Store {
    pub fn new(dir: PathBuf) -> Self {
        Store { dir }
    }

    pub fn data_path(&self) -> PathBuf {
        self.dir.join("todo.json")
    }

    fn lock_path(&self) -> PathBuf {
        self.dir.join("todo.lock")
    }

    fn tmp_path(&self) -> PathBuf {
        self.dir.join("todo.json.tmp")
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
            let list = TodoList::default();
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

    fn read_data(&self) -> Result<TodoList> {
        let bytes = fs::read(self.data_path())?;
        let list: TodoList = serde_json::from_slice(&bytes)?;
        Ok(list)
    }

    fn write_data(&self, list: &TodoList) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(list)?;
        fs::write(self.tmp_path(), &bytes)?;
        fs::rename(self.tmp_path(), self.data_path())?;
        Ok(())
    }

    /// Non-blocking lock for claim operations.
    /// Returns AlreadyLocked immediately if another process holds the lock.
    pub fn with_try_lock<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&mut TodoList) -> Result<T>,
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
        F: FnOnce(&mut TodoList) -> Result<T>,
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
        F: FnOnce(&TodoList) -> Result<T>,
    {
        let lock_file = self.open_lock_file()?;
        lock_file.lock_shared()?;

        let list = self.read_data()?;
        let result = f(&list)?;
        Ok(result)
    }
}
