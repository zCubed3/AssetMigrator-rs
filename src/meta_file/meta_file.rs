use std::io::{BufReader, BufRead};
use std::fs::{File};
use std::path::{PathBuf};
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

/// Unity meta file (GUID only)
#[derive(Debug, Default, Clone)]
pub struct MetaFile {
    /// Where this meta file is located
    pub path: String,

    /// Unity's GUID for this asset
    pub guid: String,

    /// Hash of the GUID (for faster checking)
    pub hash: u64
}

impl MetaFile {
    // Reads a meta file, grabs the GUID and returns it
    pub fn read_from_path(path: &PathBuf) -> Option<Self> {
        if let Ok(file) = File::open(path) {
            let mut hasher = DefaultHasher::new();
            let reader = BufReader::new(file);

            let mut meta_file = Self::default();
            meta_file.path = path.display().to_string();

            for line in reader.lines() {
                if let Ok(contents) = line {
                    if contents.contains("guid: ") {
                        meta_file.guid = contents.replace("guid:", "").trim().to_string();

                        // Hashing the GUID makes overlap comparison BLAZING FAST :P
                        meta_file.guid.hash(&mut hasher);
                        meta_file.hash = hasher.finish();

                        break;
                    }
                }
            }

            if !meta_file.guid.is_empty() {
                return Some(meta_file)
            }
        }

        return None;
    }
}