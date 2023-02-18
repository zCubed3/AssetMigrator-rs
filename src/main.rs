mod meta_file;
mod dropwatch;

use std::collections::HashMap;
use std::fs::{File, read_dir, create_dir, copy, write, create_dir_all};
use std::path::{Path, PathBuf};
use std::{env, fs};
use std::hash::Hash;
use std::io::Read;
use std::sync::{Arc, Mutex, Condvar, RwLock};
use std::thread::{Thread, spawn, JoinHandle};

use crate::dropwatch::Dropwatch;
use crate::meta_file::*;

// TODO: Let the user decide this
const CONVERT_EXTENSIONS: &'static [&'static str] = &[
    ".prefab", ".mat", ".asset", ".unity"
];

#[derive(Default, Debug)]
struct AssetConversion {
    path: String,
    output_path: String,
}

impl PartialEq<AssetConversion> for AssetConversion {
    fn eq(&self, other: &AssetConversion) -> bool {
        return self.path == other.path;
    }
}

struct AssetConversionCollector {
    threads: Vec<JoinHandle<()>>,
    convert_queue: Arc<Mutex<Vec<AssetConversion>>>,
    missing_metas: Arc<RwLock<Vec<MetaFile>>>,
    remap_metas: Arc<RwLock<HashMap<String, MetaFile>>>
}

impl AssetConversionCollector {
    pub fn new(
        convertees: Vec<AssetConversion>,
        missing_metas: Vec<MetaFile>,
        remap_metas: HashMap<String, MetaFile>
    ) -> Self {
        let mut threads = Vec::<JoinHandle<()>>::new();
        let convert_queue = Arc::new(Mutex::new(convertees));
        let missing_metas = Arc::new(RwLock::new(missing_metas));
        let remap_metas = Arc::new(RwLock::new(remap_metas));

        let thread_count = std::thread::available_parallelism().unwrap().get();

        #[cfg(debug_assertions)]
        {
            println!("USING {} THREADS", thread_count);
        }

        for _ in 0usize .. thread_count {
            let convert_queue = Arc::clone(&convert_queue);
            let missing_metas = Arc::clone(&missing_metas);
            let remap_metas = Arc::clone(&remap_metas);

            threads.push(spawn(move || {
                Self::collector_loop(convert_queue, missing_metas, remap_metas);
            }));
        }

        return Self {
            threads,
            convert_queue,
            missing_metas,
            remap_metas
        }
    }

    fn collector_loop(
        convert_queue: Arc<Mutex<Vec<AssetConversion>>>,
        missing_metas: Arc<RwLock<Vec<MetaFile>>>,
        remap_metas: Arc<RwLock<HashMap<String, MetaFile>>>
    ) {
        loop {
            let mut convert: Option<AssetConversion> = None;

            {
                let mut lock = convert_queue.lock().unwrap();
                convert = lock.pop();
            }

            if let Some(convert) = convert {
                let remapped_metas = remap_metas.read();
                let missing_metas = missing_metas.read();

                let prefab_path = Path::new(&convert.path);
                let mut prefab_file = File::open(prefab_path).unwrap();

                // Copy over the meta file first (if it doesn't exist)
                let mut meta_path = prefab_path.display().to_string();
                meta_path.push_str(".meta");

                let mut contents = String::new();
                let size = prefab_file.read_to_string(&mut contents);

                let mut converted_contents = contents.clone();

                // Find all occurrences of "guid"
                for indice in contents.match_indices("guid: ") {
                    let guid: String = contents.chars().skip(indice.0 + 6).take(32).collect();

                    // Check if this has been remapped
                    if let Some(meta_file) = remapped_metas.get(&guid) {
                        converted_contents.replace_range(indice.0 + 6..indice.0 + 6 + 32, &meta_file.guid);
                        continue;
                    }

                    // Check if this is in our list of missing ones
                    // If so copy it
                    let mut need_delete = false;
                    let mut delete = 0usize;

                    for missing_meta in &missing_metas.unwrap() {
                        if missing_meta.guid == guid {
                            // Copy the asset (with and without the meta over)
                            // If the file doesn't exist already, copy it
                            let prefab_dir = PathBuf::from(&missing_meta.directory);
                            let mut relative_export_path = PathBuf::from(&export_path);
                            relative_export_path.push(prefab_dir.strip_prefix(&src_assets).unwrap());

                            let export_path = relative_export_path.display().to_string();
                            let _ = create_dir_all(&export_path);

                            let (asset_src_path, meta_src_path) = missing_meta.get_paths();
                            let (asset_dst_path, meta_dst_path) = missing_meta.get_paths_stem(&export_path);

                            if Path::new(&asset_src_path).exists() && !Path::new(&asset_dst_path).exists() {
                                copy(&asset_src_path, &asset_dst_path).unwrap();
                            }

                            if Path::new(&meta_src_path).exists() && !Path::new(&meta_dst_path).exists() {
                                copy(&meta_src_path, &meta_dst_path).unwrap();
                            }

                            // If this is a prefab, push it to the list of queued conversions
                            // If it hasn't been pushed already!
                            for ext in CONVERT_EXTENSIONS {
                                if missing_meta.base_name.ends_with(ext) {
                                    let mut convert = AssetConversion::default();
                                    convert.path = asset_src_path.clone();

                                    if !convert_queue.contains(&convert) {
                                        println!("Converting referenced asset {:?}", asset_src_path);

                                        convert.output_path = relative_export_path.display().to_string();
                                        convert_queue.push(convert);
                                    }

                                    break;
                                }
                            }

                            // After being successfully copied, this is removed from the missing list
                            // This prevents prefab duplication / overwriting
                            need_delete = true;
                            break;

                            //println!("MATCH: {} = {:?}", guid, missing_meta)
                        }

                        delete += 1;
                    }

                    if need_delete {
                        missing_metas.remove(delete);
                    }
                }

                let _ = create_dir_all(&convert.output_path);

                let mut file_path = PathBuf::from(convert.output_path);
                file_path.push(prefab_path.file_name().unwrap());

                write(&file_path, converted_contents).unwrap();

                let mut extension = file_path.extension().unwrap().to_str().unwrap().to_string();
                extension.push_str(".meta");

                file_path.set_extension(extension);
                let _ = copy(meta_path, &file_path);
            }
        }
    }
}

fn main() {
    // Handle arguments
    let args: Vec<String> = env::args().collect();

    let mut src_assets = String::new();
    let mut dst_assets = String::new();

    if args.len() > 1 {
        // Minimum is 4
        if args.len() < 3 {
            println!("Proper usage of prefab_converter.exe is as follows\n");
            println!("\t ./prefab_converter.exe [src assets path] [dst assets path] ...");
            println!("\n... = Any number of valid prefab paths (in the source assets path)!");

            panic!();
        }

        src_assets = args[1].clone();
        dst_assets = args[2].clone();
    } else {
        // TODO: ASK FOR ARGS!
        panic!("Improper arguments!");
    }

    // Before we export, create the temp folder
    let _ = create_dir("ConversionOutput");
    let export_path = "./ConversionOutput".to_string();

    //
    // Collection stage
    //

    // We read two projects worth of hash files
    // Any overlap between the two is eliminated (we assume the asset already exists properly)
    let mut missing_metas = Vec::<MetaFile>::new();
    let mut remapped_metas = HashMap::<String, MetaFile>::new();

    print!("[Collection Stage]: If this is the first time you've done this since rebooting");
    print!(" you might have to wait a second or two for the OS to cache files and directories!");
    println!(" Subsequent runs should be much faster though!");

    {
        println!("Collecting source meta files...");
        let src_metas = collect_meta_files(&src_assets);

        println!("Collecting destination meta files...");
        let dst_metas = collect_meta_files(&dst_assets);

        //let drop = Dropwatch::new_begin("OVERLAPPING");

        println!("Determining missing meta files...");
        for src_meta in &src_metas {
            //println!("{:?}", src_meta);

            let mut same_found = false;

            for dst_meta in &dst_metas {
                if src_meta.guid_hash == dst_meta.guid_hash {
                    same_found = true;
                    break;
                }

                // Is this the same asset but with a different GUID?
                if src_meta.base_hash == dst_meta.base_hash {
                    same_found = true;
                    remapped_metas.insert(src_meta.guid.clone(), dst_meta.clone());
                    break;
                }
            }

            if !same_found {
                missing_metas.push(src_meta.clone());
            }
        }
    }

    //
    // Conversion stage
    //
    println!("[Conversion Stage]");

    let mut convert_queue = Vec::<AssetConversion>::new();

    for a in 3 .. args.len() {
        let prefab_dir = PathBuf::from(&args[a]);
        let mut relative_export_path = PathBuf::from(&export_path);
        relative_export_path.push(prefab_dir.strip_prefix(&src_assets).unwrap());
        relative_export_path.pop();

        let mut convert = AssetConversion::default();
        convert.path = args[a].clone();
        convert.output_path = relative_export_path.display().to_string();

        convert_queue.push(convert);
    }
}
