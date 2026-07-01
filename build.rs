use std::env;
use std::fs;
use std::path::Path;

fn main() {
    // Встраиваем иконку в .exe
    #[cfg(target_os = "windows")]
    {
        embed_resource::compile("resource/resource.rc", embed_resource::NONE);
    }
    copy_assets();
}

fn copy_assets() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir);
    let target_root = out_path.ancestors().nth(4).unwrap();

    let target_dir = if cfg!(debug_assertions) {
        target_root.join("debug")
    } else {
        target_root.join("release")
    };

    let dest_assets = target_dir.join("assets");
    let _ = fs::create_dir_all(&dest_assets);

    let assets_dir = Path::new("assets");
    if !assets_dir.exists() {
        return;
    }

    for entry in fs::read_dir(assets_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_file() {
            let fname = path.file_name().unwrap();
            if fname.to_str().map_or(false, |n| n.ends_with(".png") || n.ends_with(".txt")) {
                let dst = dest_assets.join(fname);
                if !dst.exists() {
                    let _ = fs::copy(&path, &dst);
                }
            }
        }
    }
}
