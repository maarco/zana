//! Model downloader for Whisper AI models from Hugging Face
//!
//! Provides streaming download with progress callbacks, cache checking,
//! and atomic file writing for reliability.

use anyhow::{Context, Result};
use futures_util::StreamExt;
use std::path::PathBuf;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

/// Downloads Whisper models from Hugging Face with progress reporting
pub struct ModelDownloader {
    /// HTTP client for downloads
    client: reqwest::Client,
    /// Directory where models are cached
    model_dir: PathBuf,
}

impl ModelDownloader {
    /// Create a new model downloader
    ///
    /// Models are stored in the system's data directory:
    /// - macOS: ~/Library/Application Support/kvoice/whisper-models
    /// - Linux: ~/.local/share/kvoice/whisper-models
    /// - Windows: C:\Users\<user>\AppData\Local\kvoice\whisper-models
    pub fn new() -> Result<Self> {
        let model_dir = dirs::data_local_dir()
            .context("Failed to get local data directory")?
            .join("kvoice")
            .join("whisper-models");

        Ok(Self {
            client: reqwest::Client::new(),
            model_dir,
        })
    }

    /// Create a downloader with a custom model directory
    pub fn with_model_dir(model_dir: PathBuf) -> Self {
        Self {
            client: reqwest::Client::new(),
            model_dir,
        }
    }

    /// Check if a model is already downloaded and cached
    ///
    /// # Arguments
    /// * `model_name` - The filename of the model (e.g., "ggml-small.bin")
    ///
    /// # Returns
    /// `true` if the model file exists in the cache directory
    pub fn is_model_cached(&self, model_name: &str) -> bool {
        self.model_dir.join(model_name).exists()
    }

    /// Get the full path to a cached model
    pub fn get_model_path(&self, model_name: &str) -> PathBuf {
        self.model_dir.join(model_name)
    }

    /// Download a model with progress reporting
    ///
    /// Streams the download in chunks, reporting progress via callback.
    /// Uses atomic write (temp file + rename) to prevent corruption.
    ///
    /// # Arguments
    /// * `model_name` - The filename to save (e.g., "ggml-small.bin")
    /// * `url` - The Hugging Face URL to download from
    /// * `progress_callback` - Called with (downloaded_bytes, total_bytes)
    ///
    /// # Returns
    /// The path to the downloaded model file
    ///
    /// # Errors
    /// Returns an error if:
    /// - The download fails or is interrupted
    /// - File system writes fail
    /// - The URL is invalid or unreachable
    pub async fn download_model<F>(
        &self,
        model_name: &str,
        url: &str,
        progress_callback: F,
    ) -> Result<PathBuf>
    where
        F: Fn(u64, u64) + Send + 'static,
    {
        // Ensure model directory exists
        tokio::fs::create_dir_all(&self.model_dir)
            .await
            .context("Failed to create model directory")?;

        let file_path = self.model_dir.join(model_name);

        // Check if already cached
        if file_path.exists() {
            log::info!("Model {} already cached at {:?}", model_name, file_path);
            return Ok(file_path);
        }

        log::info!("Downloading model {} from {}", model_name, url);

        // Start HTTP request
        let response = self
            .client
            .get(url)
            .send()
            .await
            .context("Failed to start download from Hugging Face")?;

        // Ensure we got a success status
        if !response.status().is_success() {
            anyhow::bail!(
                "Download failed with HTTP status: {}",
                response.status()
            );
        }

        let total_size = response.content_length().unwrap_or(0);
        let mut downloaded: u64 = 0;

        // Use temp file for atomic write
        let temp_path = file_path.with_extension("tmp");
        let mut file = File::create(&temp_path)
            .await
            .context("Failed to create temp file")?;

        // Stream download in chunks
        let mut stream = response.bytes_stream();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.context("Error downloading model chunk")?;

            file.write_all(&chunk)
                .await
                .context("Failed to write model data to disk")?;

            downloaded += chunk.len() as u64;

            // Report progress
            progress_callback(downloaded, total_size);
        }

        // Ensure all data is flushed to disk
        file.flush()
            .await
            .context("Failed to flush model data to disk")?;
        drop(file);

        // Atomic rename from temp to final path
        tokio::fs::rename(&temp_path, &file_path)
            .await
            .context("Failed to finalize model file")?;

        log::info!("Model downloaded successfully: {:?}", file_path);

        Ok(file_path)
    }
}

impl Default for ModelDownloader {
    fn default() -> Self {
        Self::new().expect("Failed to create ModelDownloader")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_path_construction() {
        let downloader = ModelDownloader::with_model_dir(PathBuf::from("/tmp/models"));
        assert_eq!(
            downloader.get_model_path("ggml-small.bin"),
            PathBuf::from("/tmp/models/ggml-small.bin")
        );
    }

    #[test]
    fn test_is_model_cached() {
        let downloader = ModelDownloader::with_model_dir(PathBuf::from("/tmp/nonexistent"));
        assert!(!downloader.is_model_cached("ggml-small.bin"));
    }
}
