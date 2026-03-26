use log::{debug, error};
use notify::{RecommendedWatcher, RecursiveMode, Result as NotifyResult, Watcher};
use std::error::Error;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

pub type FileChangeCallback = Arc<dyn Fn(&[PathBuf]) + Send + Sync>;

/// A file watcher that monitors changes in specified files and calls a callback
pub struct FileWatcher {
    paths: Vec<PathBuf>,
    callback: FileChangeCallback,
    watcher: Option<RecommendedWatcher>,
}

impl fmt::Debug for FileWatcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileWatcher")
            .field("paths", &self.paths)
            .finish()
    }
}

impl FileWatcher {
    /// Create a new FileWatcher
    ///
    /// # Arguments
    /// * `paths` - Vector of file paths to watch
    /// * `callback` - Callback function that gets called when files change
    /// * `timeout` - Maximum timeout for file watching operations (e.g., 5 seconds)
    pub fn new(paths: Vec<PathBuf>, callback: FileChangeCallback) -> Result<Self, String> {
        if paths.is_empty() {
            return Err("At least one path must be provided".to_string());
        }

        Ok(FileWatcher {
            paths,
            callback,
            watcher: None,
        })
    }

    /// Start watching the files
    pub async fn start(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let callback = self.callback.clone();
        let mut watcher =
            notify::recommended_watcher(move |res: NotifyResult<notify::Event>| match res {
                Ok(event) => {
                    let changed_paths: Vec<PathBuf> = event.paths;

                    if !changed_paths.is_empty() {
                        debug!("Files changed: {:?}", changed_paths);
                        callback(&changed_paths);
                    }
                }
                Err(e) => {
                    error!("Watcher error: {:?}", e);
                }
            })?;

        // Watch each path
        for path in &self.paths {
            debug!("Watching path: {:?}", path);
            watcher.watch(path, RecursiveMode::NonRecursive)?;
        }

        let _ = self.watcher.insert(watcher);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc as StdArc, Mutex as StdMutex};

    #[tokio::test(flavor = "multi_thread")]
    async fn test_file_watcher_detects_changes() {
        use std::fs;

        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test_file.txt");

        // Create initial file
        fs::write(&file_path, "initial").unwrap();

        let changed_files: StdArc<StdMutex<Vec<PathBuf>>> = StdArc::new(StdMutex::new(Vec::new()));
        let changed_files_clone = changed_files.clone();

        let callback: FileChangeCallback = Arc::new(move |paths: &[PathBuf]| {
            if let Ok(mut files) = changed_files_clone.lock() {
                files.extend_from_slice(paths);
            }
        });

        let mut watcher = FileWatcher::new(vec![file_path.clone()], callback).unwrap();

        watcher.start().await.unwrap();

        // Give the watcher time to initialize
        tokio::time::sleep(Duration::from_millis(800)).await;

        // Modify the file
        fs::write(&file_path, "modified content").unwrap();

        // Wait for the change to be detected (with timeout)
        let timeout = Duration::from_secs(5);
        let start = std::time::Instant::now();
        loop {
            if let Ok(files) = changed_files.lock() {
                if !files.is_empty() {
                    break;
                }
            }
            if start.elapsed() > timeout {
                panic!("File change was not detected within timeout");
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }

        let detected_files = changed_files.lock().unwrap();
        assert!(
            !detected_files.is_empty(),
            "No files were detected as changed"
        );

        // Canonicalize both paths to handle symlinks on macOS
        let expected_path = fs::canonicalize(&file_path).unwrap_or_else(|_| file_path.clone());
        let detected_path =
            fs::canonicalize(&detected_files[0]).unwrap_or_else(|_| detected_files[0].clone());
        assert_eq!(detected_path, expected_path);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_file_watcher_with_multiple_files() {
        use std::fs;

        let temp_dir = tempfile::tempdir().unwrap();
        let file_path1 = temp_dir.path().join("file1.txt");
        let file_path2 = temp_dir.path().join("file2.txt");

        // Create initial files
        fs::write(&file_path1, "content1").unwrap();
        fs::write(&file_path2, "content2").unwrap();

        let changed_count: StdArc<StdMutex<u32>> = StdArc::new(StdMutex::new(0));
        let changed_count_clone = changed_count.clone();

        let callback: FileChangeCallback = Arc::new(move |_paths: &[PathBuf]| {
            if let Ok(mut count) = changed_count_clone.lock() {
                *count += 1;
            }
        });

        let mut watcher =
            FileWatcher::new(vec![file_path1.clone(), file_path2.clone()], callback).unwrap();

        watcher.start().await.unwrap();

        // Give the watcher time to initialize
        tokio::time::sleep(Duration::from_millis(800)).await;

        // Modify first file
        fs::write(&file_path1, "modified1").unwrap();

        // Wait for detection with timeout
        let timeout = Duration::from_secs(5);
        let start = std::time::Instant::now();
        loop {
            if let Ok(count) = changed_count.lock() {
                if *count > 0 {
                    break;
                }
            }
            if start.elapsed() > timeout {
                panic!("First file change was not detected within timeout");
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }

        assert!(*changed_count.lock().unwrap() > 0);
    }

    #[test]
    fn test_file_watcher_creation_fails_with_empty_paths() {
        let callback: FileChangeCallback = Arc::new(|_: &[PathBuf]| {});
        let result = FileWatcher::new(vec![], callback);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "At least one path must be provided");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_file_watcher_timeout_safety() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "test").unwrap();

        let callback: FileChangeCallback = Arc::new(|_: &[PathBuf]| {
            // Do nothing
        });

        let mut watcher = FileWatcher::new(vec![file_path], callback).unwrap();

        // This should not panic or hang
        let result = tokio::time::timeout(Duration::from_secs(3), watcher.start()).await;

        assert!(
            result.is_ok(),
            "Watcher startup should complete within timeout"
        );
    }
}
