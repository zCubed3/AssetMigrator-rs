use std::fs::{File, FileType, read_dir};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::thread::{Thread, spawn};
use std::sync::{Arc, Mutex, Condvar};

#[derive(Debug, Default)]
struct MetaFile {
    guid: String
}

impl MetaFile {
    // Reads a meta file, grabs the GUID and returns it
    pub fn read_from_path<P: AsRef<Path>>(path: P) -> Option<Self> {
        if let Ok(file) = File::open(path) {
            let reader = BufReader::new(file);
            let mut meta_file = Self::default();

            for line in reader.lines() {
                if let Ok(contents) = line {
                    if contents.contains("guid: ") {
                        meta_file.guid = contents.replace("guid:", "").trim().to_string();
                        break;
                    }
                }
            }

            return Some(meta_file);
        }

        return None;
    }
}

// Spawns threads and collects meta files from an internal worklist
struct MetaFileCollector {
    threads: Vec<Thread>,
    work_paths: Arc<Mutex<Vec<PathBuf>>>,
    meta_files: Arc<Mutex<Vec<MetaFile>>>,
    condvar: Arc<(Mutex<bool>, Condvar)>
}

impl MetaFileCollector {
    pub fn new(paths: Vec<PathBuf>) -> Self {
        let mut threads = Vec::<Thread>::new();
        let work_paths = Arc::new(Mutex::new(paths));
        let meta_files = Arc::new(Mutex::new(Vec::<MetaFile>::new()));
        let condvar = Arc::new((Mutex::new(false), Condvar::new()));

        // TODO: Get hardware concurrency?
        for _ in 0 .. 4 {
            let work_paths = Arc::clone(&work_paths);
            let meta_files = Arc::clone(&meta_files);
            let condvar = Arc::clone(&condvar);

            let thread = spawn(move || {
                MetaFileCollector::collector_loop(condvar, work_paths, meta_files)
            });
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
                            let meta = MetaFile::read_from_path(entry.path()).unwrap();

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

    for dir in &dirs {
        println!("{:?}", dir);
    }

    let mut collector = MetaFileCollector::new(dirs);
    collector.wait();

    return vec!();
}

fn main() {
    // Read a test meta file
    for ent in collect_meta_files("F:/Plastic SCM/LakaVRCore/Assets") {
        println!("{:?}", ent);
    }
}
