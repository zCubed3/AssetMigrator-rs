mod meta_file;
mod dropwatch;

use std::fs::{File, read_dir, create_dir};
use std::path::{Path, PathBuf};
use std::env;

use crate::dropwatch::Dropwatch;
use crate::meta_file::*;

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

fn main() {
    // Handle arguments
    let args: Vec<String> = env::args().collect();

    // Before we export, create the temp folder
    let _ = create_dir("ConversionOutput");

    // We read two projects worth of hash files
    // Any overlap between the two is eliminated (we assume the asset already exists properly)
    let mut missing_metas = Vec::<MetaFile>::new();

    {
        println!("Collecting source meta files...");
        let src_metas = collect_meta_files("F:/Plastic SCM/LakaVRCore/Assets");

        println!("Collecting destination meta files...");
        let dst_metas = collect_meta_files("E:/Unity Projects/ASVRP 2/Assets");

        //let drop = Dropwatch::new_begin("OVERLAPPING");

        println!("Determining missing meta files...");
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
