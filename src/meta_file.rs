pub mod meta_file;
pub mod collector;

pub use meta_file::*;
pub use collector::*;

use std::fs::{read_dir};
use std::path::{Path, PathBuf};

pub fn collect_recurse<P: AsRef<Path>>(path: P, dirs: &mut Vec<PathBuf>) {
    for entry_result in read_dir(path).expect("Failed to read given path!") {
        // If we can't read a meta file we probably shouldn't be in here
        let entry = entry_result.expect("Failed to read file in given path!");

        if let Ok(file_type) = entry.file_type() {
            if file_type.is_dir() {
                dirs.push(entry.path());
                collect_recurse(entry.path(), dirs);
            }
        }
    }
}

pub fn collect_meta_files(path: &String) -> Vec<MetaFile> {
    // First fetch all the directories within a project
    let mut dirs = Vec::<PathBuf>::new();
    collect_recurse(path, &mut dirs);

    // Then collect them
    //println!("Collecting meta files...");
    let collect_multi = true;

    return if collect_multi {
        //let drop = dropwatch::Dropwatch::new_begin("META_COLLECT");

        let collector = MetaFileCollector::new(dirs);
        collector.wait();

        collector.consume()
    } else {
        //let drop = dropwatch::Dropwatch::new_begin("META_COLLECT");

        let mut metas = Vec::<MetaFile>::new();
        for path in dirs {
            for entry_result in read_dir(path).expect("Failed to read given path!") {
                // If we can't read a meta file we probably shouldn't be in here
                let entry = entry_result.expect("Failed to read file in given path!");

                if let Some(extension) = entry.path().extension() {
                    if extension == "meta" {
                        let meta = MetaFile::read_from_path(&entry.path()).unwrap();

                        //println!("{:?}", meta);
                        metas.push(meta);
                    }
                }
            }
        }

        metas
    }
}