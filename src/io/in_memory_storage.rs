use crate::io::data_storage::{DataStorage, FileSystemError};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// In-memory implementation of DataStorage for testing purposes
/// Stores all data in RAM using a HashMap
pub struct InMemoryDataStorage {
    storage: Arc<Mutex<HashMap<PathBuf, Vec<u8>>>>,
}

impl InMemoryDataStorage {
    /// Creates a new empty in-memory storage
    pub fn new() -> Self {
        Self {
            storage: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Returns all stored paths (useful for testing)
    pub fn list_paths(&self) -> Vec<PathBuf> {
        let storage = self.storage.lock().unwrap();
        storage.keys().cloned().collect()
    }

    /// Clears all stored data (useful for testing)
    pub fn clear(&self) {
        let mut storage = self.storage.lock().unwrap();
        storage.clear();
    }

    /// Returns the number of stored entries
    pub fn len(&self) -> usize {
        let storage = self.storage.lock().unwrap();
        storage.len()
    }

    /// Checks if the storage is empty
    pub fn is_empty(&self) -> bool {
        let storage = self.storage.lock().unwrap();
        storage.is_empty()
    }
}

impl Default for InMemoryDataStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl DataStorage for InMemoryDataStorage {
    async fn load_data(&self, path: &Path) -> Result<Vec<u8>, FileSystemError> {
        let storage = self.storage.lock().unwrap();
        storage
            .get(path)
            .cloned()
            .ok_or_else(|| FileSystemError::LoadError("Path does not exist".to_string()))
    }

    async fn store_data(&self, data: Vec<u8>, path: &Path) -> Result<(), FileSystemError> {
        let mut storage = self.storage.lock().unwrap();
        storage.insert(path.to_path_buf(), data);
        Ok(())
    }

    async fn delete_data(&self, path: &Path) -> Result<bool, FileSystemError> {
        let mut storage = self.storage.lock().unwrap();
        Ok(storage.remove(path).is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_store_and_load() {
        let storage = InMemoryDataStorage::new();
        let test_data = b"Hello, Memory!".to_vec();
        let test_path = Path::new("test.txt");

        storage
            .store_data(test_data.clone(), test_path)
            .await
            .expect("Failed to store data");

        let loaded_data = storage
            .load_data(test_path)
            .await
            .expect("Failed to load data");
        assert_eq!(test_data, loaded_data);
    }

    #[tokio::test]
    async fn test_load_nonexistent() {
        let storage = InMemoryDataStorage::new();
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
    async fn test_delete() {
        let storage = InMemoryDataStorage::new();
        let test_path = Path::new("delete.txt");

        // Store data
        storage
            .store_data(b"data".to_vec(), test_path)
            .await
            .expect("Failed to store data");

        // Delete should return true
        let result = storage
            .delete_data(test_path)
            .await
            .expect("Failed to delete");
        assert!(result);

        // Delete non-existent should return false
        let result2 = storage
            .delete_data(test_path)
            .await
            .expect("Failed to delete");
        assert!(!result2);
    }

    #[tokio::test]
    async fn test_overwrite() {
        let storage = InMemoryDataStorage::new();
        let test_path = Path::new("overwrite.txt");

        storage
            .store_data(b"old".to_vec(), test_path)
            .await
            .expect("Failed to store");

        storage
            .store_data(b"new".to_vec(), test_path)
            .await
            .expect("Failed to overwrite");

        let loaded = storage.load_data(test_path).await.expect("Failed to load");
        assert_eq!(b"new".to_vec(), loaded);
    }

    #[tokio::test]
    async fn test_list_paths() {
        let storage = InMemoryDataStorage::new();

        storage
            .store_data(b"1".to_vec(), Path::new("file1.txt"))
            .await
            .expect("Failed to store");
        storage
            .store_data(b"2".to_vec(), Path::new("file2.txt"))
            .await
            .expect("Failed to store");

        let paths = storage.list_paths();
        assert_eq!(paths.len(), 2);
        assert!(paths.contains(&PathBuf::from("file1.txt")));
        assert!(paths.contains(&PathBuf::from("file2.txt")));
    }

    #[tokio::test]
    async fn test_clear() {
        let storage = InMemoryDataStorage::new();

        storage
            .store_data(b"data".to_vec(), Path::new("file.txt"))
            .await
            .expect("Failed to store");

        assert_eq!(storage.len(), 1);
        assert!(!storage.is_empty());

        storage.clear();

        assert_eq!(storage.len(), 0);
        assert!(storage.is_empty());
    }

    #[tokio::test]
    async fn test_nested_paths() {
        let storage = InMemoryDataStorage::new();
        let nested_path = Path::new("dir1/dir2/file.txt");

        // InMemory treats nested paths simply as string keys
        storage
            .store_data(b"nested".to_vec(), nested_path)
            .await
            .expect("Failed to store");

        let loaded = storage
            .load_data(nested_path)
            .await
            .expect("Failed to load");
        assert_eq!(b"nested".to_vec(), loaded);
    }

    #[tokio::test]
    async fn test_multiple_storages_independent() {
        let storage1 = InMemoryDataStorage::new();
        let storage2 = InMemoryDataStorage::new();
        let test_path = Path::new("test.txt");

        storage1
            .store_data(b"storage1".to_vec(), test_path)
            .await
            .expect("Failed to store");
        storage2
            .store_data(b"storage2".to_vec(), test_path)
            .await
            .expect("Failed to store");

        let data1 = storage1.load_data(test_path).await.expect("Failed to load");
        let data2 = storage2.load_data(test_path).await.expect("Failed to load");

        assert_eq!(data1, b"storage1".to_vec());
        assert_eq!(data2, b"storage2".to_vec());
    }

    #[tokio::test]
    async fn test_empty_file() {
        let storage = InMemoryDataStorage::new();
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
    async fn test_binary_data() {
        let storage = InMemoryDataStorage::new();
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
}
