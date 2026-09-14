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
            proof_index: self.proof_index.clone(),
            current_root: hex::encode(self.current_root),
            total_hashes: self.total_hashes.load(Ordering::Relaxed),
            total_bytes: self.total_bytes.load(Ordering::Relaxed),
        };

        let state_file = self.data_dir.join("nomt_state.json");
        let content = serde_json::to_string_pretty(&state)?;

        // Use exclusive file lock for NFS-safe writes
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&state_file)?;
        fs2::FileExt::lock_exclusive(&file)?;
        let mut writer = std::io::BufWriter::new(&file);
        writer.write_all(content.as_bytes())?;
        writer.flush()?;
        // TAG: surface=blockchain owner=blockchain-team rule=BC-001
        fs2::FileExt::unlock(&file)?;

        debug!(path = %state_file.display(), "Persisted NOMT state to disk with lock");
        Ok(())
    }

    /// Reload state from disk (for shared storage freshness)
    pub fn reload_from_disk(&mut self) -> anyhow::Result<()> {
        let state_file = self.data_dir.join("nomt_state.json");
        if !state_file.exists() {
            return Ok(());
        }

        // Use shared lock for reading
        let file = OpenOptions::new().read(true).open(&state_file)?;
        fs2::FileExt::lock_shared(&file)?;
        let mut content = String::new();
        std::io::BufReader::new(&file).read_to_string(&mut content)?;
        fs2::FileExt::unlock(&file)?;

        let state: PersistentState = serde_json::from_str(&content)?;

        self.hash_index = state.hash_index;
        self.proof_index = state.proof_index;
        self.current_root = hex::decode(&state.current_root)
            .ok()
            .and_then(|b| {
                if b.len() == 32 {
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(&b);
                    Some(arr)
                } else {
                    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
                    None
                }
            })
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
            proof_index: self.proof_index.clone(),
            current_root: hex::encode(self.current_root),
            total_hashes: self.total_hashes.load(Ordering::Relaxed),
            total_bytes: self.total_bytes.load(Ordering::Relaxed),
        };

        tokio::task::spawn_blocking(move || {
            let state_file = data_dir.join("nomt_state.json");
            let content = serde_json::to_string_pretty(&state)
                .map_err(|e| anyhow::anyhow!("Failed to serialize state: {e}"))?;

            // Use exclusive file lock for NFS-safe writes
            // TAG: surface=blockchain owner=blockchain-team rule=BC-001
            let file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&state_file)
                .map_err(|e| anyhow::anyhow!("Failed to open state file: {e}"))?;
            fs2::FileExt::lock_exclusive(&file)
                .map_err(|e| anyhow::anyhow!("Failed to lock file: {e}"))?;
            let mut writer = std::io::BufWriter::new(&file);
            writer
                .write_all(content.as_bytes())
                .map_err(|e| anyhow::anyhow!("Failed to write state: {e}"))?;
            writer
                .flush()
                .map_err(|e| anyhow::anyhow!("Failed to flush state: {e}"))?;
            fs2::FileExt::unlock(&file)
                .map_err(|e| anyhow::anyhow!("Failed to unlock file: {e}"))?;

            debug!(path = %state_file.display(), "Persisted NOMT state to disk with lock");
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

                // Use shared lock for reading
                let file = OpenOptions::new()
                    .read(true)
                    .open(&state_file)
                    .map_err(|e| anyhow::anyhow!("Failed to open state file: {e}"))?;
                fs2::FileExt::lock_shared(&file)
                    .map_err(|e| anyhow::anyhow!("Failed to lock file: {e}"))?;
                let mut content = String::new();
                std::io::BufReader::new(&file)
                    .read_to_string(&mut content)
                    .map_err(|e| anyhow::anyhow!("Failed to read state: {e}"))?;
                fs2::FileExt::unlock(&file)
                    .map_err(|e| anyhow::anyhow!("Failed to unlock file: {e}"))?;

                let state: PersistentState = serde_json::from_str(&content)
                    .map_err(|e| anyhow::anyhow!("Failed to parse state: {e}"))?;

                Ok(Some(state))
            })
            .await
            .map_err(|e| anyhow::anyhow!("Reload task failed: {e}"))?;

        if let Some(state) = result? {
            self.hash_index = state.hash_index;
            self.proof_index = state.proof_index;
            // TAG: surface=blockchain owner=blockchain-team rule=BC-001
            self.current_root = hex::decode(&state.current_root)
                .ok()
                .and_then(|b| {
                    if b.len() == 32 {
                        let mut arr = [0u8; 32];
                        arr.copy_from_slice(&b);
                        Some(arr)
                    } else {
                        None
                    }
                })
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
