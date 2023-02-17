mod dropwatch;

use std::collections::hash_map::DefaultHasher;
use std::fs::{File, FileType, read_dir};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::thread::{Thread, JoinHandle, spawn};
use std::sync::{Arc, Mutex, Condvar};
use std::hash::{Hash, Hasher};

use crate::dropwatch::Dropwatch;

#[derive(Debug, Default, Clone)]
struct MetaFile {
    path: String,
    guid: String,
    hash: u64
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

// Spawns threads and collects meta files from an internal worklist
struct MetaFileCollector {
    threads: Vec<JoinHandle<()>>,
    work_paths: Arc<Mutex<Vec<PathBuf>>>,
    meta_files: Arc<Mutex<Vec<MetaFile>>>,
    condvar: Arc<(Mutex<bool>, Condvar)>
}

impl MetaFileCollector {
    pub fn new(paths: Vec<PathBuf>) -> Self {
        let mut threads = Vec::<JoinHandle<()>>::new();
        let work_paths = Arc::new(Mutex::new(paths));
        let meta_files = Arc::new(Mutex::new(Vec::<MetaFile>::new()));
        let condvar = Arc::new((Mutex::new(false), Condvar::new()));

        // TODO: Get hardware concurrency?
        for _ in 0 .. 16 {
            let work_paths = Arc::clone(&work_paths);
            let meta_files = Arc::clone(&meta_files);
            let condvar = Arc::clone(&condvar);

            threads.push(spawn(move || {
                MetaFileCollector::collector_loop(condvar, work_paths, meta_files)
            }));
        }

        return Self {
            threads,
            work_paths,
            meta_files,
            condvar
        }
    }

    pub fn wait(&self) {
        {
            let (lock, cvar) = &*self.condvar;
            let mut notified = lock.lock().unwrap();

            notified = cvar.wait(notified).unwrap();
        }
    }

    pub fn consume(self) -> Vec<MetaFile> {
        // Ensure all the threads have exited first
        loop {
            let mut all_finished = true;
            for thread in &self.threads {
                if !thread.is_finished() {
                    all_finished = false;
                    break;
                }
            }

            if all_finished {
                break;
            }
        }

        return Arc::try_unwrap(self.meta_files).unwrap().into_inner().unwrap();
    }

    fn collector_loop(condvar: Arc<(Mutex<bool>, Condvar)>, work_paths: Arc<Mutex<Vec<PathBuf>>>, meta_files: Arc<Mutex<Vec<MetaFile>>>) {
        loop {
            let mut path: Option<PathBuf> = None;
            let mut notify: bool = false;

            {
                let mut lock = work_paths.lock().unwrap();
                path = lock.pop();
                notify = lock.is_empty();
            }

            if let Some(path) = path {
                // Read the files first
                let mut metas = Vec::<MetaFile>::new();

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

                {
                    let mut lock = meta_files.lock().unwrap();
                    lock.append(&mut metas);
                }

                if notify {
                    let (lock, cvar) = &*condvar;

                    let mut notified = lock.lock().unwrap();
                    *notified = true;

                    cvar.notify_one();
                }
            } else {
                break;
            }
        }
    }
}

fn collect_recurse<P: AsRef<Path>>(path: P, dirs: &mut Vec<PathBuf>) {
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

fn collect_meta_files(path: &str) -> Vec<MetaFile> {
    // First fetch all the directories within a project
    let mut dirs = Vec::<PathBuf>::new();
    collect_recurse(path, &mut dirs);

    // Then collect them
    println!("Collecting meta files...");
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

fn main() {
    // We read two projects worth of hash files
    // Any overlap between the two is eliminated (we assume the asset already exists properly)
    let mut missing_metas = Vec::<MetaFile>::new();

    {
        let src_metas = collect_meta_files("F:/Plastic SCM/LakaVRCore/Assets");
        let dst_metas = collect_meta_files("E:/Unity Projects/ASVRP 2/Assets");

        //let drop = Dropwatch::new_begin("OVERLAPPING");

        for src_meta in &src_metas {
            let mut same_found = false;

            for dst_meta in &dst_metas {
                if src_meta.hash == dst_meta.hash {
                    same_found = true;
                    break;
                }
            }

            if !same_found {
                missing_metas.push(src_meta.clone());
            }
        }
    }
}
