use std::fs::{read_dir};
use std::path::{PathBuf};
use std::sync::{Arc, Mutex, Condvar};
use std::thread::{JoinHandle, spawn};

use super::meta_file::MetaFile;

/// Spawns threads and collects meta files from an internal worklist
pub struct MetaFileCollector {
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
        let thread_count = std::thread::available_parallelism().unwrap().get();

        #[cfg(debug_assertions)]
        {
            println!("USING {} THREADS", thread_count);
        }

        for _ in 0usize .. thread_count {



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