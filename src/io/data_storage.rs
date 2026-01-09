use crate::io::data_storage::FileSystemError::{LoadError, NoDirError, Other, StoreError};
use async_trait::async_trait;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::fs::DirEntry;

#[derive(Debug)]
pub enum FileSystemError {
    LoadError(String),
    StoreError(String),
    DeleteError(String),
    NoDirError(String),
    Other(String),
}
#[async_trait]
pub trait DataStorage: Send {
    async fn load_data(&self, path: &Path) -> Result<Vec<u8>, FileSystemError>;
    async fn store_data(&self, data: Vec<u8>, path: &Path) -> Result<(), FileSystemError>;
    async fn delete_data(&self, path: &Path) -> Result<bool, FileSystemError>;
    async fn list_files(
        &self,
        path: &Path,
        recursive: bool,
    ) -> Result<Vec<Box<Path>>, FileSystemError>;
}

pub struct FSDataStorage {
    base_path: Box<Path>,
}

impl FSDataStorage {
    pub fn new(base_path: Box<Path>) -> Self {
        Self { base_path }
    }

    async fn read_dir(
        full_path: PathBuf,
    ) -> Result<(Vec<DirEntry>, Vec<DirEntry>), FileSystemError> {
        let mut read_dir = fs::read_dir(full_path)
            .await
            .map_err(move |e| Other(e.to_string()))?;

        let mut directories: Vec<DirEntry> = vec![];
        let mut entries: Vec<DirEntry> = vec![];

        while let Ok(Some(entry)) = read_dir.next_entry().await {
            let file_type = entry.file_type().await.unwrap();
            if file_type.is_dir() {
                directories.push(entry);
            } else if file_type.is_file() {
                entries.push(entry);
            }
        }
        Ok((directories, entries))
    }
}
#[async_trait]
impl DataStorage for FSDataStorage {
    async fn load_data(&self, path: &Path) -> Result<Vec<u8>, FileSystemError> {
        let full_path = self.base_path.join(path);
        if !full_path.exists() {
            return Err(LoadError("Path does not exist".to_string()));
        }

        fs::read(full_path.as_path())
            .await
            .map_err(move |e| LoadError(e.to_string()))
    }

    async fn store_data(&self, data: Vec<u8>, path: &Path) -> Result<(), FileSystemError> {
        let full_path = self.base_path.join(path);
        if !full_path.exists() {
            let option = full_path.as_path().parent();
            if let None = option {
                return Err(StoreError("Path has no parent directory".to_string()));
            }
            let create_dir = fs::create_dir_all(option.unwrap()).await;
            if let Err(error) = create_dir {
                return Err(StoreError(error.to_string()));
            }
        }

        fs::write(full_path, data)
            .await
            .map_err(move |e| StoreError(e.to_string()))
    }

    async fn delete_data(&self, path: &Path) -> Result<bool, FileSystemError> {
        let full_path = self.base_path.join(path);
        if !full_path.exists() {
            return Ok(false);
        }
        if full_path.is_dir() {
            fs::remove_dir_all(&full_path)
                .await
                .map_err(|e| FileSystemError::DeleteError(e.to_string()))?;
        } else {
            fs::remove_file(&full_path)
                .await
                .map_err(|e| FileSystemError::DeleteError(e.to_string()))?;
        }

        Ok(true)
    }

    async fn list_files(
        &self,
        path: &Path,
        recursive: bool,
    ) -> Result<Vec<Box<Path>>, FileSystemError> {
        let full_path = self.base_path.join(path);
        if !full_path.is_dir() {
            return Err(NoDirError("path is not a directory".to_string()));
        }

        let (mut directories, mut file_entries) = Self::read_dir(full_path).await?;

        if recursive {
            while !directories.is_empty() {
                let dir_path = directories.swap_remove(0).path();
                let (mut dirs, mut entries) = Self::read_dir(dir_path).await?;
                directories.append(&mut dirs);
                file_entries.append(&mut entries);
            }
        }

        Ok(file_entries
            .iter()
            .map(|entry| {
                let buf = entry.path();
                let path = buf.strip_prefix(&self.base_path).unwrap();
                // TODO: unwrap may panic
                return path.to_path_buf().into_boxed_path();
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_storage() -> (FSDataStorage, TempDir) {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let base_path = temp_dir.path().to_path_buf().into_boxed_path();
        let storage = FSDataStorage::new(base_path);
        (storage, temp_dir)
    }

    #[tokio::test]
    async fn test_store_and_load_data() {
        let (storage, _temp_dir) = create_test_storage();
        let test_data = b"Hello, World!".to_vec();
        let test_path = Path::new("test_file.txt");

        // Store data
        storage
            .store_data(test_data.clone(), test_path)
            .await
            .expect("Failed to store data");

        // Load data
        let loaded_data = storage
            .load_data(test_path)
            .await
            .expect("Failed to load data");

        assert_eq!(test_data, loaded_data);
    }

    #[tokio::test]
    async fn test_store_data_creates_parent_directories() {
        let (storage, _temp_dir) = create_test_storage();
        let test_data = b"Nested file content".to_vec();
        let nested_path = Path::new("subdir1/subdir2/nested_file.txt");

        // Store data in nested path
        storage
            .store_data(test_data.clone(), nested_path)
            .await
            .expect("Failed to store data in nested path");

        // Verify file was created and can be loaded
        let loaded_data = storage
            .load_data(nested_path)
            .await
            .expect("Failed to load nested file");

        assert_eq!(test_data, loaded_data);
    }

    #[tokio::test]
    async fn test_load_nonexistent_file() {
        let (storage, _temp_dir) = create_test_storage();
        let result = storage.load_data(Path::new("nonexistent.txt")).await;

        assert!(result.is_err());
        match result {
            Err(FileSystemError::LoadError(msg)) => {
                assert_eq!(msg, "Path does not exist");
            }
            _ => panic!("Expected LoadError"),
        }
    }

    #[tokio::test]
    async fn test_delete_existing_file() {
        let (storage, _temp_dir) = create_test_storage();
        let test_data = b"File to delete".to_vec();
        let test_path = Path::new("delete_me.txt");

        // Create file
        storage
            .store_data(test_data, test_path)
            .await
            .expect("Failed to store data");

        // Delete file
        let result = storage
            .delete_data(test_path)
            .await
            .expect("Failed to delete file");

        assert!(result, "Should return true when file is deleted");

        // Verify file no longer exists
        let load_result = storage.load_data(test_path).await;
        assert!(load_result.is_err());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_file() {
        let (storage, _temp_dir) = create_test_storage();
        let result = storage
            .delete_data(Path::new("nonexistent.txt"))
            .await
            .expect("Delete should not fail for nonexistent file");

        assert!(!result, "Should return false when file doesn't exist");
    }

    #[tokio::test]
    async fn test_delete_directory() {
        let (storage, _temp_dir) = create_test_storage();
        let dir_path = Path::new("test_directory");
        let file_in_dir = Path::new("test_directory/file.txt");

        // Create directory with a file
        storage
            .store_data(b"content".to_vec(), file_in_dir)
            .await
            .expect("Failed to create file in directory");

        // Delete entire directory
        let result = storage
            .delete_data(dir_path)
            .await
            .expect("Failed to delete directory");

        assert!(result, "Should return true when directory is deleted");

        // Verify directory no longer exists
        let load_result = storage.load_data(file_in_dir).await;
        assert!(load_result.is_err());
    }

    #[tokio::test]
    async fn test_overwrite_existing_file() {
        let (storage, _temp_dir) = create_test_storage();
        let test_path = Path::new("overwrite_me.txt");

        // Store initial data
        storage
            .store_data(b"initial content".to_vec(), test_path)
            .await
            .expect("Failed to store initial data");

        // Overwrite with new data
        let new_data = b"new content".to_vec();
        storage
            .store_data(new_data.clone(), test_path)
            .await
            .expect("Failed to overwrite data");

        // Verify new data was stored
        let loaded_data = storage
            .load_data(test_path)
            .await
            .expect("Failed to load data");
        assert_eq!(new_data, loaded_data);
    }

    #[tokio::test]
    async fn test_store_empty_file() {
        let (storage, _temp_dir) = create_test_storage();
        let test_path = Path::new("empty.txt");
        let empty_data = Vec::new();

        storage
            .store_data(empty_data.clone(), test_path)
            .await
            .expect("Failed to store empty file");

        let loaded_data = storage
            .load_data(test_path)
            .await
            .expect("Failed to load empty file");
        assert_eq!(empty_data, loaded_data);
    }

    #[tokio::test]
    async fn test_store_binary_data() {
        let (storage, _temp_dir) = create_test_storage();
        let test_path = Path::new("binary.dat");
        let binary_data: Vec<u8> = (0..=255).collect();

        storage
            .store_data(binary_data.clone(), test_path)
            .await
            .expect("Failed to store binary data");

        let loaded_data = storage
            .load_data(test_path)
            .await
            .expect("Failed to load binary data");
        assert_eq!(binary_data, loaded_data);
    }

    #[tokio::test]
    async fn test_list_files_non_recursive() {
        let (storage, temp_dir) = create_test_storage();

        // Create test files structure
        storage
            .store_data(b"1".to_vec(), Path::new("file1.txt"))
            .await
            .unwrap();
        storage
            .store_data(b"2".to_vec(), Path::new("file2.txt"))
            .await
            .unwrap();
        storage
            .store_data(b"3".to_vec(), Path::new("subdir/file3.txt"))
            .await
            .unwrap();
        storage
            .store_data(b"4".to_vec(), Path::new("subdir/nested/file4.txt"))
            .await
            .unwrap();

        // List files at root level (non-recursive)
        let files = storage.list_files(Path::new(""), false).await.unwrap();

        // Should only return direct children (file1.txt, file2.txt, subdir)
        assert!(
            files.len() >= 2,
            "Expected at least 2 entries, got {}",
            files.len()
        );

        // Check that the direct files are included
        let file_names: Vec<String> = files
            .iter()
            .filter_map(|p| p.file_name().and_then(|n| n.to_str()).map(String::from))
            .collect();

        assert!(
            file_names.contains(&"file1.txt".to_string()),
            "file1.txt should be in the list"
        );
        assert!(
            file_names.contains(&"file2.txt".to_string()),
            "file2.txt should be in the list"
        );
    }

    #[tokio::test]
    async fn test_list_files_in_subdirectory() {
        let (storage, _temp_dir) = create_test_storage();

        // Create test files structure
        storage
            .store_data(b"1".to_vec(), Path::new("dir1/file1.txt"))
            .await
            .unwrap();
        storage
            .store_data(b"2".to_vec(), Path::new("dir1/file2.txt"))
            .await
            .unwrap();
        storage
            .store_data(b"3".to_vec(), Path::new("dir1/subdir/file3.txt"))
            .await
            .unwrap();
        storage
            .store_data(b"4".to_vec(), Path::new("dir2/file4.txt"))
            .await
            .unwrap();

        // List files in dir1 (non-recursive)
        let files = storage.list_files(Path::new("dir1"), false).await.unwrap();

        // Should only return direct children in dir1
        let file_names: Vec<String> = files
            .iter()
            .filter_map(|p| p.file_name().and_then(|n| n.to_str()).map(String::from))
            .collect();

        assert!(
            file_names.contains(&"file1.txt".to_string()),
            "file1.txt should be in dir1"
        );
        assert!(
            file_names.contains(&"file2.txt".to_string()),
            "file2.txt should be in dir1"
        );

        // Verify that files from other directories are not included
        assert!(
            !file_names.contains(&"file4.txt".to_string()),
            "file4.txt from dir2 should not be in the list"
        );
    }

    #[tokio::test]
    async fn test_list_files_empty_directory() {
        let (storage, temp_dir) = create_test_storage();

        // Create an empty subdirectory
        let empty_dir = temp_dir.path().join("empty_dir");
        std::fs::create_dir(&empty_dir).expect("Failed to create empty directory");

        // List files in the empty directory
        let files = storage
            .list_files(Path::new("empty_dir"), false)
            .await
            .unwrap();

        assert_eq!(files.len(), 0, "Empty directory should return no files");
    }

    #[tokio::test]
    async fn test_list_files_nonexistent_directory() {
        let (storage, _temp_dir) = create_test_storage();

        // Try to list files in a non-existent directory
        let result = storage.list_files(Path::new("nonexistent"), false).await;

        assert!(
            result.is_err(),
            "Listing non-existent directory should return an error"
        );
        match result {
            Err(NoDirError(_)) => {
                // Expected error type
            }
            Err(Other(_)) => {
                // Also acceptable (directory doesn't exist)
            }
            _ => panic!("Expected NoDirError or Other error"),
        }
    }

    #[tokio::test]
    async fn test_list_files_on_file_not_directory() {
        let (storage, _temp_dir) = create_test_storage();

        // Create a file
        storage
            .store_data(b"content".to_vec(), Path::new("file.txt"))
            .await
            .unwrap();

        // Try to list files on a file (not a directory)
        let result = storage.list_files(Path::new("file.txt"), false).await;

        assert!(result.is_err(), "Listing a file should return an error");
        match result {
            Err(NoDirError(_)) => {
                // Expected error type
            }
            _ => panic!("Expected NoDirError when listing a file"),
        }
    }

    #[tokio::test]
    async fn test_list_files_returns_relative_paths() {
        let (storage, _temp_dir) = create_test_storage();

        // Create test files
        storage
            .store_data(b"1".to_vec(), Path::new("dir/file1.txt"))
            .await
            .unwrap();
        storage
            .store_data(b"2".to_vec(), Path::new("dir/file2.txt"))
            .await
            .unwrap();

        // List files in dir
        let files = storage.list_files(Path::new("dir"), false).await.unwrap();

        // Check that returned paths are relative to base_path
        for file in &files {
            let path_str = file.to_str().unwrap();
            // The path should start with "dir/"
            assert!(
                path_str.starts_with("dir"),
                "Path should be relative and start with 'dir': {}",
                path_str
            );
            // Should not contain the temp directory path
            assert!(
                !path_str.contains("tmp"),
                "Path should not contain absolute temp path: {}",
                path_str
            );
        }
    }

    #[tokio::test]
    async fn test_list_files_with_nested_structure() {
        let (storage, _temp_dir) = create_test_storage();

        // Create a more complex structure
        storage
            .store_data(b"1".to_vec(), Path::new("root.txt"))
            .await
            .unwrap();
        storage
            .store_data(b"2".to_vec(), Path::new("level1/file1.txt"))
            .await
            .unwrap();
        storage
            .store_data(b"3".to_vec(), Path::new("level1/file2.txt"))
            .await
            .unwrap();
        storage
            .store_data(b"4".to_vec(), Path::new("level1/level2/file3.txt"))
            .await
            .unwrap();
        storage
            .store_data(b"5".to_vec(), Path::new("level1/level2/level3/file4.txt"))
            .await
            .unwrap();

        // List level1 directory (non-recursive)
        let files = storage
            .list_files(Path::new("level1"), false)
            .await
            .unwrap();

        // Count actual files (not subdirectories)
        let file_count = files
            .iter()
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.ends_with(".txt"))
                    .unwrap_or(false)
            })
            .count();

        // Should have file1.txt and file2.txt (direct children only)
        assert_eq!(
            file_count, 2,
            "Should list only direct file children in level1"
        );
    }

    // Tests for recursive listing

    #[tokio::test]
    async fn test_list_files_recursive_root() {
        let (storage, _temp_dir) = create_test_storage();

        // Create test files structure
        storage
            .store_data(b"1".to_vec(), Path::new("file1.txt"))
            .await
            .unwrap();
        storage
            .store_data(b"2".to_vec(), Path::new("file2.txt"))
            .await
            .unwrap();
        storage
            .store_data(b"3".to_vec(), Path::new("subdir/file3.txt"))
            .await
            .unwrap();
        storage
            .store_data(b"4".to_vec(), Path::new("subdir/nested/file4.txt"))
            .await
            .unwrap();

        // List files at root level (recursive)
        let files = storage.list_files(Path::new(""), true).await.unwrap();

        // Extract all file names for easier testing
        let file_paths: Vec<String> = files
            .iter()
            .map(|p| p.to_str().unwrap().to_string())
            .collect();

        // Should include all files recursively
        assert!(
            file_paths.iter().any(|p| p.contains("file1.txt")),
            "Should include file1.txt"
        );
        assert!(
            file_paths.iter().any(|p| p.contains("file2.txt")),
            "Should include file2.txt"
        );
        assert!(
            file_paths.iter().any(|p| p.contains("file3.txt")),
            "Should include file3.txt from subdir"
        );
        assert!(
            file_paths.iter().any(|p| p.contains("file4.txt")),
            "Should include file4.txt from nested"
        );
    }

    #[tokio::test]
    async fn test_list_files_recursive_subdirectory() {
        let (storage, _temp_dir) = create_test_storage();

        // Create test files structure
        storage
            .store_data(b"1".to_vec(), Path::new("dir1/file1.txt"))
            .await
            .unwrap();
        storage
            .store_data(b"2".to_vec(), Path::new("dir1/file2.txt"))
            .await
            .unwrap();
        storage
            .store_data(b"3".to_vec(), Path::new("dir1/subdir/file3.txt"))
            .await
            .unwrap();
        storage
            .store_data(b"4".to_vec(), Path::new("dir1/subdir/nested/file4.txt"))
            .await
            .unwrap();
        storage
            .store_data(b"5".to_vec(), Path::new("dir2/file5.txt"))
            .await
            .unwrap();

        // List files in dir1 (recursive)
        let files = storage.list_files(Path::new("dir1"), true).await.unwrap();

        let file_paths: Vec<String> = files
            .iter()
            .map(|p| p.to_str().unwrap().to_string())
            .collect();

        // Should include all files under dir1 recursively
        assert!(
            file_paths
                .iter()
                .any(|p| p.contains("dir1") && p.contains("file1.txt")),
            "Should include dir1/file1.txt"
        );
        assert!(
            file_paths
                .iter()
                .any(|p| p.contains("dir1") && p.contains("file2.txt")),
            "Should include dir1/file2.txt"
        );
        assert!(
            file_paths.iter().any(|p| p.contains("file3.txt")),
            "Should include file3.txt from subdir"
        );
        assert!(
            file_paths.iter().any(|p| p.contains("file4.txt")),
            "Should include file4.txt from nested"
        );

        // Should not include files from dir2
        assert!(
            !file_paths.iter().any(|p| p.contains("file5.txt")),
            "Should not include file5.txt from dir2"
        );
    }

    #[tokio::test]
    async fn test_list_files_recursive_deep_nesting() {
        let (storage, _temp_dir) = create_test_storage();

        // Create deeply nested structure
        storage
            .store_data(b"1".to_vec(), Path::new("level1/file1.txt"))
            .await
            .unwrap();
        storage
            .store_data(b"2".to_vec(), Path::new("level1/level2/file2.txt"))
            .await
            .unwrap();
        storage
            .store_data(b"3".to_vec(), Path::new("level1/level2/level3/file3.txt"))
            .await
            .unwrap();
        storage
            .store_data(
                b"4".to_vec(),
                Path::new("level1/level2/level3/level4/file4.txt"),
            )
            .await
            .unwrap();
        storage
            .store_data(
                b"5".to_vec(),
                Path::new("level1/level2/level3/level4/level5/file5.txt"),
            )
            .await
            .unwrap();

        // List level1 recursively
        let files = storage.list_files(Path::new("level1"), true).await.unwrap();

        let file_paths: Vec<String> = files
            .iter()
            .map(|p| p.to_str().unwrap().to_string())
            .collect();

        // Should find all 5 files at various depths
        assert!(
            file_paths.iter().any(|p| p.contains("file1.txt")),
            "Should include file1.txt"
        );
        assert!(
            file_paths.iter().any(|p| p.contains("file2.txt")),
            "Should include file2.txt"
        );
        assert!(
            file_paths.iter().any(|p| p.contains("file3.txt")),
            "Should include file3.txt"
        );
        assert!(
            file_paths.iter().any(|p| p.contains("file4.txt")),
            "Should include file4.txt"
        );
        assert!(
            file_paths.iter().any(|p| p.contains("file5.txt")),
            "Should include file5.txt"
        );
    }

    #[tokio::test]
    async fn test_list_files_recursive_vs_non_recursive() {
        let (storage, _temp_dir) = create_test_storage();

        // Create test structure
        storage
            .store_data(b"1".to_vec(), Path::new("dir/file1.txt"))
            .await
            .unwrap();
        storage
            .store_data(b"2".to_vec(), Path::new("dir/file2.txt"))
            .await
            .unwrap();
        storage
            .store_data(b"3".to_vec(), Path::new("dir/sub1/file3.txt"))
            .await
            .unwrap();
        storage
            .store_data(b"4".to_vec(), Path::new("dir/sub2/file4.txt"))
            .await
            .unwrap();
        storage
            .store_data(b"5".to_vec(), Path::new("dir/sub1/deep/file5.txt"))
            .await
            .unwrap();

        // Non-recursive listing
        let files_non_recursive = storage.list_files(Path::new("dir"), false).await.unwrap();
        let non_recursive_count = files_non_recursive
            .iter()
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.ends_with(".txt"))
                    .unwrap_or(false)
            })
            .count();

        // Recursive listing
        let files_recursive = storage.list_files(Path::new("dir"), true).await.unwrap();
        let recursive_count = files_recursive
            .iter()
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.ends_with(".txt"))
                    .unwrap_or(false)
            })
            .count();

        // Non-recursive should only have 2 direct files
        assert_eq!(
            non_recursive_count, 2,
            "Non-recursive should find 2 direct files"
        );

        // Recursive should find all 5 files
        assert_eq!(recursive_count, 5, "Recursive should find all 5 files");
    }

    #[tokio::test]
    async fn test_list_files_recursive_empty_directory() {
        let (storage, temp_dir) = create_test_storage();

        // Create an empty subdirectory
        let empty_dir = temp_dir.path().join("empty_dir");
        std::fs::create_dir(&empty_dir).expect("Failed to create empty directory");

        // List files in the empty directory (recursive)
        let files = storage
            .list_files(Path::new("empty_dir"), true)
            .await
            .unwrap();

        assert_eq!(
            files.len(),
            0,
            "Empty directory should return no files even with recursive=true"
        );
    }

    #[tokio::test]
    async fn test_list_files_recursive_mixed_content() {
        let (storage, _temp_dir) = create_test_storage();

        // Create structure with various file types
        storage
            .store_data(b"txt".to_vec(), Path::new("mixed/file.txt"))
            .await
            .unwrap();
        storage
            .store_data(b"json".to_vec(), Path::new("mixed/config.json"))
            .await
            .unwrap();
        storage
            .store_data(b"md".to_vec(), Path::new("mixed/readme.md"))
            .await
            .unwrap();
        storage
            .store_data(b"nested".to_vec(), Path::new("mixed/sub/data.dat"))
            .await
            .unwrap();
        storage
            .store_data(b"deep".to_vec(), Path::new("mixed/sub/deep/file.bin"))
            .await
            .unwrap();

        // List all files recursively
        let files = storage.list_files(Path::new("mixed"), true).await.unwrap();

        // Should find all 5 files
        let file_count = files
            .iter()
            .filter(|p| p.is_file() || p.to_str().unwrap().contains("."))
            .count();

        assert!(
            file_count >= 5,
            "Should find at least 5 files in recursive listing"
        );
    }

    #[tokio::test]
    async fn test_list_files_recursive_returns_relative_paths() {
        let (storage, _temp_dir) = create_test_storage();

        // Create nested structure
        storage
            .store_data(b"1".to_vec(), Path::new("base/file1.txt"))
            .await
            .unwrap();
        storage
            .store_data(b"2".to_vec(), Path::new("base/sub/file2.txt"))
            .await
            .unwrap();
        storage
            .store_data(b"3".to_vec(), Path::new("base/sub/deep/file3.txt"))
            .await
            .unwrap();

        // List recursively
        let files = storage.list_files(Path::new("base"), true).await.unwrap();

        // All paths should be relative and start with "base"
        for file in &files {
            let path_str = file.to_str().unwrap();
            assert!(
                path_str.starts_with("base"),
                "Path should start with 'base': {}",
                path_str
            );
            assert!(
                !path_str.contains("tmp"),
                "Path should not contain absolute temp path: {}",
                path_str
            );
        }
    }
}
