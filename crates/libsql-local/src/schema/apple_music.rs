//! Apple Music schema definitions
//!
//! Contains Apple Music tables:
//! - `apple_music_profiles`: User's Apple Music profile
//! - `apple_music_library_songs`: Library songs
//! - `apple_music_library_albums`: Library albums
//! - `apple_music_playlists`: Playlists
//! - `apple_music_recently_played`: Listening history
//! - `apple_music_genre_stats`: Aggregated genre statistics
//!
//! COMPLIANCE: §10 Unified Database Schema Architecture

use anyhow::Result;
use libsql::Connection;
use tracing::info;

/// Initialize Apple Music tables
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn initialize_apple_music_tables(conn: &Connection) -> Result<()> {
    info!("[ARCH-007] Initializing apple_music tables");
    conn.execute(
        "CREATE TABLE IF NOT EXISTS apple_music_profiles (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL REFERENCES user_profile(id),
            apple_music_id TEXT,
            display_name TEXT,
            subscription_type TEXT,
            storefront TEXT,
            country_code TEXT,
            connected_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            last_sync_at TEXT,
            sync_status TEXT DEFAULT 'pending',
            raw_profile_data TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )",
        (),
    )
    .await?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS apple_music_library_songs (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL REFERENCES user_profile(id),
            catalog_id TEXT,
            title TEXT NOT NULL,
            artist_name TEXT,
            album_name TEXT,
            duration_ms INTEGER,
            genre_names TEXT,
            release_date TEXT,
            artwork_url TEXT,
            play_count INTEGER DEFAULT 0,
            date_added TEXT,
            last_played_at TEXT,
            is_favorite BOOLEAN DEFAULT FALSE,
            raw_song_data TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )",
        (),
    )
    .await?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS apple_music_library_albums (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL REFERENCES user_profile(id),
            catalog_id TEXT,
            title TEXT NOT NULL,
            artist_name TEXT,
            track_count INTEGER,
            genre_names TEXT,
            release_date TEXT,
            artwork_url TEXT,
            date_added TEXT,
            is_complete BOOLEAN DEFAULT FALSE,
            raw_album_data TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )",
        (),
    )
    .await?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS apple_music_playlists (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL REFERENCES user_profile(id),
            catalog_id TEXT,
            name TEXT NOT NULL,
            description TEXT,
            track_count INTEGER DEFAULT 0,
            is_public BOOLEAN DEFAULT FALSE,
            curator_name TEXT,
            artwork_url TEXT,
            date_added TEXT,
            last_modified_at TEXT,
            raw_playlist_data TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )",
        (),
    )
    .await?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS apple_music_recently_played (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL REFERENCES user_profile(id),
            content_type TEXT NOT NULL,
            content_id TEXT NOT NULL,
            title TEXT NOT NULL,
            artist_name TEXT,
            album_name TEXT,
            artwork_url TEXT,
            played_at TEXT NOT NULL,
            duration_ms INTEGER,
            raw_play_data TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )",
        (),
    )
    .await?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS apple_music_genre_stats (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL REFERENCES user_profile(id),
            genre_name TEXT NOT NULL,
            song_count INTEGER DEFAULT 0,
            total_play_count INTEGER DEFAULT 0,
            total_duration_ms INTEGER DEFAULT 0,
            percentage REAL DEFAULT 0.0,
            period_start TEXT,
            period_end TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )",
        (),
    )
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_apple_music_module_compiles() {
        let _ = 1 + 1; // Compile-time verification
    }
}
