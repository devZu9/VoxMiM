use std::env;
use std::fs;
use std::path::Path;

fn main() {
    #[cfg(target_os = "windows")]
    {
        let _ = embed_resource::compile("resource/resource.rc", embed_resource::NONE);
    }
    sync_from_target();
    restore_config();
    copy_dicts();
    copy_lang();
}

/// Безусловная синхронизация пользовательских данных из target в корень.
/// Забираем свежие файлы из рабочей копии ДО того, как build.rs перезапишет
/// их из корня — иначе правки, сделанные в приложении, пропадут при cargo clean.
fn sync_from_target() {
    let src = target_dir();

    let pairs = [
        (src.join("dicts/user_dict.json"), Path::new("dicts/user_dict.json")),
        (src.join("dicts/hallucinations.txt"), Path::new("dicts/hallucinations.txt")),
        (src.join("config.json"), Path::new("config.json")),
    ];
    for (from, to) in &pairs {
        if from.is_file() {
            if let Some(parent) = to.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::copy(from, to);
        }
    }
}

/// Возврат настроек в target после cargo clean:
/// если config.json в target отсутствует, а корневой бэкап есть — копируем обратно.
fn restore_config() {
    let to = target_dir().join("config.json");
    if to.is_file() {
        return;
    }
    let from = Path::new("config.json");
    if from.is_file() {
        let _ = fs::copy(from, &to);
    }
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
