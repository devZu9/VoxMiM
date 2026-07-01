use std::env;
use std::fs;
use std::path::Path;

fn main() {
    copy_cuda_dlls();
    copy_assets();
}

fn copy_cuda_dlls() {
    let dll_dir = Path::new(r"C:\_workPortable\WhisperCpp\bins\cu-bin-blas12.4");
    if !dll_dir.exists() {
        return;
    }

    let out_dir = env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir);
    let target_root = out_path.ancestors().nth(4).unwrap();

    let needed = [
        "ggml-cuda.dll",
        "cublas64_12.dll",
        "cublasLt64_12.dll",
        "cudart64_12.dll",
        "nvrtc64_120_0.dll",
        "nvrtc-builtins64_124.dll",
        "nvblas64_12.dll",
    ];

    for dir in [target_root.join("debug"), target_root.join("release")] {
        if !dir.exists() { continue; }
        for file in &needed {
            let src = dll_dir.join(file);
            if src.exists() {
                let dst = dir.join(file);
                if !dst.exists() {
                    let _ = fs::copy(&src, &dst);
                }
            }
        }
    }
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
            let dst = dest_assets.join(fname);
            if !dst.exists() {
                let _ = fs::copy(&path, &dst);
            }
        }
    }
}
