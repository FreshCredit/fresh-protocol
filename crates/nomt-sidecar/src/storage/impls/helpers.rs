// TAG: surface=blockchain owner=blockchain-team rule=BC-001
use crate::storage::*;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::sync::atomic::Ordering;
use tracing::debug;

impl NomtStorage {
    /// Persist current state to disk with file locking for NFS safety
    #[allow(dead_code)]
    fn persist(&self) -> anyhow::Result<()> {
        let state = PersistentState {
            hash_index: self.hash_index.clone(),
            sha256_aliases: self.sha256_aliases.clone(),
            current_root: hex::encode(self.current_root),
            total_hashes: self.total_hashes.load(Ordering::Relaxed),
            total_bytes: self.total_bytes.load(Ordering::Relaxed),
        };

        let lock_file = self.data_dir.join("nomt_state.lock");
        let state_file = self.data_dir.join("nomt_state.json");
        let tmp_file = self.data_dir.join("nomt_state.json.tmp");
        let content = serde_json::to_string_pretty(&state)?;

        // Acquire exclusive lock on dedicated lockfile, write temp + sync_all + atomic rename
        // (never open nomt_state.json with O_TRUNC before locking!)
        let lock = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_file)?;
        fs2::FileExt::lock_exclusive(&lock)?;

        let write_res = (|| -> anyhow::Result<()> {
            let file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&tmp_file)?;
            let mut writer = std::io::BufWriter::new(&file);
            writer.write_all(content.as_bytes())?;
            writer.flush()?;
            drop(writer);
            file.sync_all()?;
            std::fs::rename(&tmp_file, &state_file)?;
            Ok(())
        })();

        let _ = fs2::FileExt::unlock(&lock);
        write_res?;

        debug!(path = %state_file.display(), "Atomically persisted NOMT state to disk with lock");
        Ok(())
    }

    /// Reload state from disk (for shared storage freshness)
    pub fn reload_from_disk(&mut self) -> anyhow::Result<()> {
        let state_file = self.data_dir.join("nomt_state.json");
        if !state_file.exists() {
            return Ok(());
        }

        let lock_file = self.data_dir.join("nomt_state.lock");
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_file)?;
        fs2::FileExt::lock_shared(&lock)?;

        let read_res = (|| -> anyhow::Result<String> {
            let file = OpenOptions::new().read(true).open(&state_file)?;
            let mut content = String::new();
            std::io::BufReader::new(&file).read_to_string(&mut content)?;
            Ok(content)
        })();
        let _ = fs2::FileExt::unlock(&lock);
        let content = read_res?;

        let state: PersistentState = serde_json::from_str(&content)?;

        self.hash_index = state.hash_index;
        self.sha256_aliases = state.sha256_aliases;
        let root_hex = if self.hash_index.is_empty() {
            state.current_root
        } else {
            let ordered: Vec<String> = self.hash_index.keys().cloned().collect();
            freshcredit_nomt_core::compute_merkle_root(&ordered)
        };
        self.current_root = hex::decode(&root_hex)
            .ok()
            .and_then(|b| <[u8; 32]>::try_from(b).ok())
            .unwrap_or([0u8; 32]);
        self.total_hashes
            .store(state.total_hashes, Ordering::Relaxed);
        self.total_bytes.store(state.total_bytes, Ordering::Relaxed);

        debug!(hashes = state.total_hashes, "Reloaded NOMT state from disk");
        Ok(())
    }

    /// Async wrapper for `persist()` to avoid blocking the async runtime
    ///
    /// PERFORMANCE: Uses `spawn_blocking` for file I/O operations
    pub async fn persist_async(&self) -> anyhow::Result<()> {
        let data_dir = self.data_dir.clone();
        let state = PersistentState {
            hash_index: self.hash_index.clone(),
            sha256_aliases: self.sha256_aliases.clone(),
            current_root: hex::encode(self.current_root),
            total_hashes: self.total_hashes.load(Ordering::Relaxed),
            total_bytes: self.total_bytes.load(Ordering::Relaxed),
        };

        tokio::task::spawn_blocking(move || {
            let lock_file = data_dir.join("nomt_state.lock");
            let state_file = data_dir.join("nomt_state.json");
            let tmp_file = data_dir.join("nomt_state.json.tmp");
            let content = serde_json::to_string_pretty(&state)
                .map_err(|e| anyhow::anyhow!("Failed to serialize state: {e}"))?;

            // Acquire exclusive lock on dedicated lockfile, write temp + sync_all + atomic rename
            // TAG: surface=blockchain owner=blockchain-team rule=BC-001
            let lock = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(false)
                .open(&lock_file)
                .map_err(|e| anyhow::anyhow!("Failed to open lock file: {e}"))?;
            fs2::FileExt::lock_exclusive(&lock)
                .map_err(|e| anyhow::anyhow!("Failed to lock state: {e}"))?;

            let write_res = (|| -> anyhow::Result<()> {
                let file = OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(&tmp_file)
                    .map_err(|e| anyhow::anyhow!("Failed to open temp state file: {e}"))?;
                let mut writer = std::io::BufWriter::new(&file);
                writer
                    .write_all(content.as_bytes())
                    .map_err(|e| anyhow::anyhow!("Failed to write temp state: {e}"))?;
                writer
                    .flush()
                    .map_err(|e| anyhow::anyhow!("Failed to flush temp state: {e}"))?;
                drop(writer);
                file.sync_all()
                    .map_err(|e| anyhow::anyhow!("Failed to fsync temp state: {e}"))?;
                std::fs::rename(&tmp_file, &state_file)
                    .map_err(|e| anyhow::anyhow!("Failed to atomically replace state file: {e}"))?;
                Ok(())
            })();

            let _ = fs2::FileExt::unlock(&lock);
            write_res?;

            debug!(path = %state_file.display(), "Atomically persisted NOMT state to disk with lock");
            Ok(())
        })
        .await
        .map_err(|e| anyhow::anyhow!("Persist task failed: {e}"))?
    }

    /// Async wrapper for `reload_from_disk()` to avoid blocking the async runtime
    ///
    /// PERFORMANCE: Uses `spawn_blocking` for file I/O operations
    pub async fn reload_from_disk_async(&mut self) -> anyhow::Result<()> {
        let data_dir = self.data_dir.clone();

        let result: Result<Option<PersistentState>, anyhow::Error> =
                    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
            tokio::task::spawn_blocking(move || {
                let state_file = data_dir.join("nomt_state.json");
                if !state_file.exists() {
                    return Ok(None);
                }

                let lock_file = data_dir.join("nomt_state.lock");
                let lock = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .truncate(false)
                    .open(&lock_file)
                    .map_err(|e| anyhow::anyhow!("Failed to open lock file: {e}"))?;
                fs2::FileExt::lock_shared(&lock)
                    .map_err(|e| anyhow::anyhow!("Failed to shared-lock state: {e}"))?;

                let read_res = (|| -> anyhow::Result<String> {
                    let file = OpenOptions::new()
                        .read(true)
                        .open(&state_file)
                        .map_err(|e| anyhow::anyhow!("Failed to open state file: {e}"))?;
                    let mut content = String::new();
                    std::io::BufReader::new(&file)
                        .read_to_string(&mut content)
                        .map_err(|e| anyhow::anyhow!("Failed to read state: {e}"))?;
                    Ok(content)
                })();
                let _ = fs2::FileExt::unlock(&lock);
                let content = read_res?;

                let state: PersistentState = serde_json::from_str(&content)
                    .map_err(|e| anyhow::anyhow!("Failed to parse state: {e}"))?;

                Ok(Some(state))
            })
            .await
            .map_err(|e| anyhow::anyhow!("Reload task failed: {e}"))?;

        if let Some(state) = result? {
            self.hash_index = state.hash_index;
            self.sha256_aliases = state.sha256_aliases;
            // TAG: surface=blockchain owner=blockchain-team rule=BC-001
            let root_hex = if self.hash_index.is_empty() {
                state.current_root
            } else {
                let ordered: Vec<String> = self.hash_index.keys().cloned().collect();
                freshcredit_nomt_core::compute_merkle_root(&ordered)
            };
            self.current_root = hex::decode(&root_hex)
                .ok()
                .and_then(|b| <[u8; 32]>::try_from(b).ok())
                .unwrap_or([0u8; 32]);
            self.total_hashes
                .store(state.total_hashes, Ordering::Relaxed);
            self.total_bytes.store(state.total_bytes, Ordering::Relaxed);

            debug!(hashes = state.total_hashes, "Reloaded NOMT state from disk");
        }

        Ok(())
    }

    /// Check if storage is ready
    pub const fn is_ready(&self) -> bool {
        self.ready
    }

    /// Get current root hash as hex string
    pub fn current_root(&self) -> String {
        hex::encode(self.current_root)
    }
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
}
