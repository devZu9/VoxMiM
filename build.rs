use std::env;
use std::fs;
use std::path::Path;

fn main() {
    // Slint UI
    let slint_path = std::path::Path::new("src/ui/settings.slint");
    if slint_path.exists() {
        slint_build::compile(slint_path).unwrap();
    }

    #[cfg(target_os = "windows")]
    {
        let _ = embed_resource::compile("resource/resource.rc", embed_resource::NONE);
    }
    copy_dicts();
    copy_lang();
    copy_bins();
}

fn target_dir() -> std::path::PathBuf {
    let out_dir = env::var("OUT_DIR").unwrap();
    let target_root = Path::new(&out_dir).ancestors().nth(4).unwrap();
    if cfg!(debug_assertions) {
        target_root.join("debug")
    } else {
        target_root.join("release")
    }
}

fn copy_dicts() {
    let dest = target_dir().join("dicts");
    let _ = fs::create_dir_all(&dest);

    let src_dicts = Path::new("dicts");
    if src_dicts.exists() {
        for entry in fs::read_dir(src_dicts).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_file() {
                let dst = dest.join(path.file_name().unwrap());
                let _ = fs::copy(&path, &dst);
            }
        }
    }

    let src_assets = Path::new("assets");
    if src_assets.exists() {
        for entry in fs::read_dir(src_assets).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_file() && path.extension().map(|e| e == "txt").unwrap_or(false) {
                let dst = dest.join(path.file_name().unwrap());
                if !dst.exists() {
                    let _ = fs::copy(&path, &dst);
                }
            }
        }
    }
}

fn copy_lang() {
    let dest = target_dir().join("lang");
    let _ = fs::create_dir_all(&dest);

    let src = Path::new("lang");
    if !src.exists() { return; }

    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_file() {
            let fname = path.file_name().unwrap();
            let dst = dest.join(&fname);
            let _ = fs::copy(&path, &dst);
        }
    }
}

fn copy_bins() {
    let dest = target_dir().join("bins");
    let _ = fs::create_dir_all(&dest);

    let src = Path::new("bins");
    if !src.exists() { return; }

    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_file() {
            let fname = path.file_name().unwrap();
            let dst = dest.join(fname);
            let _ = fs::copy(&path, &dst);
        }
    }
}
