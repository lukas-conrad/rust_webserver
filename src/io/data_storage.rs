use crate::io::data_storage::FileSystemError::{LoadError, StoreError};
use std::fs;
use std::path::Path;

#[derive(Debug)]
pub enum FileSystemError {
    LoadError(String),
    StoreError(String),
    DeleteError(String),
}
pub trait DataStorage: Send {
    fn load_data(&self, path: &Path) -> Result<Vec<u8>, FileSystemError>;
    fn store_data(&self, data: Vec<u8>, path: &Path) -> Result<(), FileSystemError>;
    fn delete_data(&self, path: &Path) -> Result<bool, FileSystemError>;
}

pub struct FSDataStorage {
    base_path: Box<Path>,
}

impl FSDataStorage {
    pub fn new(base_path: Box<Path>) -> Self {
        Self { base_path }
    }
}
impl DataStorage for FSDataStorage {
    fn load_data(&self, path: &Path) -> Result<Vec<u8>, FileSystemError> {
        let full_path = self.base_path.join(path);
        if !full_path.exists() {
            return Err(LoadError("Path does not exist".to_string()));
        }

        fs::read(full_path.as_path()).map_err(move |e| LoadError(e.to_string()))
    }

    fn store_data(&self, data: Vec<u8>, path: &Path) -> Result<(), FileSystemError> {
        let full_path = self.base_path.join(path);
        if !full_path.exists() {
            // TODO: remove unwrap
            let option = full_path.as_path().parent();
            if let None = option {
                return Err(StoreError("Path has no parent directory".to_string()));
            }
            let create_dir = fs::create_dir_all(option.unwrap());
            if let Err(error) = create_dir {
                return Err(StoreError(error.to_string()));
            }
        }

        fs::write(full_path, data).map_err(move |e| StoreError(e.to_string()))
    }

    fn delete_data(&self, path: &Path) -> Result<bool, FileSystemError> {
        let full_path = self.base_path.join(path);
        if !full_path.exists() {
            return Ok(false);
        }
        if full_path.is_dir() {
            fs::remove_dir_all(&full_path)
                .map_err(|e| FileSystemError::DeleteError(e.to_string()))?;
        } else {
            fs::remove_file(&full_path).map_err(|e| FileSystemError::DeleteError(e.to_string()))?;
        }

        Ok(true)
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

    #[test]
    fn test_store_and_load_data() {
        let (storage, _temp_dir) = create_test_storage();
        let test_data = b"Hello, World!".to_vec();
        let test_path = Path::new("test_file.txt");

        // Store data
        storage
            .store_data(test_data.clone(), test_path)
            .expect("Failed to store data");

        // Load data
        let loaded_data = storage.load_data(test_path).expect("Failed to load data");

        assert_eq!(test_data, loaded_data);
    }

    #[test]
    fn test_store_data_creates_parent_directories() {
        let (storage, _temp_dir) = create_test_storage();
        let test_data = b"Nested file content".to_vec();
        let nested_path = Path::new("subdir1/subdir2/nested_file.txt");

        // Store data in nested path
        storage
            .store_data(test_data.clone(), nested_path)
            .expect("Failed to store data in nested path");

        // Verify file was created and can be loaded
        let loaded_data = storage
            .load_data(nested_path)
            .expect("Failed to load nested file");

        assert_eq!(test_data, loaded_data);
    }

    #[test]
    fn test_load_nonexistent_file() {
        let (storage, _temp_dir) = create_test_storage();
        let result = storage.load_data(Path::new("nonexistent.txt"));

        assert!(result.is_err());
        match result {
            Err(FileSystemError::LoadError(msg)) => {
                assert_eq!(msg, "Path does not exist");
            }
            _ => panic!("Expected LoadError"),
        }
    }

    #[test]
    fn test_delete_existing_file() {
        let (storage, _temp_dir) = create_test_storage();
        let test_data = b"File to delete".to_vec();
        let test_path = Path::new("delete_me.txt");

        // Create file
        storage
            .store_data(test_data, test_path)
            .expect("Failed to store data");

        // Delete file
        let result = storage
            .delete_data(test_path)
            .expect("Failed to delete file");

        assert!(result, "Should return true when file is deleted");

        // Verify file no longer exists
        let load_result = storage.load_data(test_path);
        assert!(load_result.is_err());
    }

    #[test]
    fn test_delete_nonexistent_file() {
        let (storage, _temp_dir) = create_test_storage();
        let result = storage
            .delete_data(Path::new("nonexistent.txt"))
            .expect("Delete should not fail for nonexistent file");

        assert!(!result, "Should return false when file doesn't exist");
    }

    #[test]
    fn test_delete_directory() {
        let (storage, _temp_dir) = create_test_storage();
        let dir_path = Path::new("test_directory");
        let file_in_dir = Path::new("test_directory/file.txt");

        // Create directory with a file
        storage
            .store_data(b"content".to_vec(), file_in_dir)
            .expect("Failed to create file in directory");

        // Delete entire directory
        let result = storage
            .delete_data(dir_path)
            .expect("Failed to delete directory");

        assert!(result, "Should return true when directory is deleted");

        // Verify directory no longer exists
        let load_result = storage.load_data(file_in_dir);
        assert!(load_result.is_err());
    }

    #[test]
    fn test_overwrite_existing_file() {
        let (storage, _temp_dir) = create_test_storage();
        let test_path = Path::new("overwrite_me.txt");

        // Store initial data
        storage
            .store_data(b"initial content".to_vec(), test_path)
            .expect("Failed to store initial data");

        // Overwrite with new data
        let new_data = b"new content".to_vec();
        storage
            .store_data(new_data.clone(), test_path)
            .expect("Failed to overwrite data");

        // Verify new data was stored
        let loaded_data = storage.load_data(test_path).expect("Failed to load data");
        assert_eq!(new_data, loaded_data);
    }

    #[test]
    fn test_store_empty_file() {
        let (storage, _temp_dir) = create_test_storage();
        let test_path = Path::new("empty.txt");
        let empty_data = Vec::new();

        storage
            .store_data(empty_data.clone(), test_path)
            .expect("Failed to store empty file");

        let loaded_data = storage.load_data(test_path).expect("Failed to load empty file");
        assert_eq!(empty_data, loaded_data);
    }

    #[test]
    fn test_store_binary_data() {
        let (storage, _temp_dir) = create_test_storage();
        let test_path = Path::new("binary.dat");
        let binary_data: Vec<u8> = (0..=255).collect();

        storage
            .store_data(binary_data.clone(), test_path)
            .expect("Failed to store binary data");

        let loaded_data = storage
            .load_data(test_path)
            .expect("Failed to load binary data");
        assert_eq!(binary_data, loaded_data);
    }
}

