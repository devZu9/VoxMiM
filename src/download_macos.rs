use std::path::{Path, PathBuf};
use std::process::Command;

const WHISPER_VERSION: &str = "v1.9.1";

fn bins_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join("bins")))
        .unwrap_or_else(|| Path::new("bins").to_path_buf())
}

fn whisper_cli_path() -> PathBuf {
    bins_dir().join("whisper-cli")
}

fn has_cmake() -> bool {
    Command::new("cmake")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn install_cmake() -> Result<(), String> {
    log::info!("Устанавливаю cmake через Homebrew...");
    let status = Command::new("brew")
        .args(["install", "cmake"])
        .status()
        .map_err(|e| format!("Homebrew не найден. Установите: /bin/bash -c \"$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\"\nОшибка: {e}"))?;
    if !status.success() {
        return Err("Не удалось установить cmake через brew".to_string());
    }
    Ok(())
}

fn build_whisper() -> Result<(), String> {
    let bins = bins_dir();
    std::fs::create_dir_all(&bins).map_err(|e| format!("Не удалось создать bins/: {e}"))?;

    if !has_cmake() {
        install_cmake()?;
    }

    let build_dir = std::env::temp_dir().join("voxmim-whisper-build");
    let _ = std::fs::remove_dir_all(&build_dir);

    log::info!("Клонирую whisper.cpp {}...", WHISPER_VERSION);
    let status = Command::new("git")
        .args([
            "clone", "--branch", WHISPER_VERSION, "--depth", "1",
            "https://github.com/ggml-org/whisper.cpp.git",
            build_dir.to_str().unwrap_or("/tmp/voxmim-whisper-build"),
        ])
        .status()
        .map_err(|e| format!("git не найден: {e}"))?;
    if !status.success() {
        return Err("Не удалось клонировать whisper.cpp".to_string());
    }

    let build_subdir = build_dir.join("build");
    std::fs::create_dir_all(&build_subdir).map_err(|e| format!("mkdir build: {e}"))?;

    log::info!("Конфигурирую cmake (Metal ON)...");
    let status = Command::new("cmake")
        .args([
            "-B", build_subdir.to_str().unwrap(),
            "-DCMAKE_BUILD_TYPE=Release",
        "-DGGML_METAL=ON",
        "-DWHISPER_BUILD_EXAMPLES=ON",
        "-DWHISPER_BUILD_SERVER=ON",
        "-DWHISPER_BUILD_TESTS=OFF",
            "-S", build_dir.to_str().unwrap(),
        ])
        .status()
        .map_err(|e| format!("cmake configure: {e}"))?;
    if !status.success() {
        return Err("cmake configure failed".to_string());
    }

    let num_cpus = std::thread::available_parallelism()
        .map(|n| n.get().to_string())
        .unwrap_or_else(|_| "4".to_string());

    log::info!("Собираю whisper-cli и whisper-server ({} потоков)...", num_cpus);
    let status = Command::new("cmake")
        .args([
            "--build", build_subdir.to_str().unwrap(),
            "--config", "Release",
            "--target", "whisper-cli", "whisper-server",
            "-j", &num_cpus,
        ])
        .status()
        .map_err(|e| format!("cmake build: {e}"))?;
    if !status.success() {
        return Err("cmake build failed".to_string());
    }

    let built_bin = build_subdir.join("bin");
    for name in &["whisper-cli", "whisper-server"] {
        let src = built_bin.join(name);
        if src.exists() {
            std::fs::copy(&src, bins.join(name))
                .map_err(|e| format!("copy {}: {e}", name))?;
            log::info!("  -> bins/{}", name);
        } else {
            log::warn!("  {} не найден в {}", name, built_bin.display());
        }
    }

    // Копируем .dylib из build/bin/ и build/src/
    let dylib_dirs = [
        built_bin.clone(),
        build_subdir.join("src"),
        build_subdir.join("ggml").join("src"),
    ];
    for dir in &dylib_dirs {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "dylib") {
                    let name = path.file_name().unwrap();
                    if let Err(e) = std::fs::copy(&path, bins.join(name)) {
                        log::warn!("copy dylib {}: {e}", name.to_string_lossy());
                    } else {
                        log::info!("  -> bins/{}", name.to_string_lossy());
                    }
                }
            }
        }
    }

    // fix_rpath: меняем @rpath на @loader_path у бинарников и dylib
    let bins_abs = bins.canonicalize().unwrap_or(bins);
    for entry in std::fs::read_dir(&bins_abs).unwrap_or(std::fs::read_dir(".").unwrap()) {
        let path = match entry {
            Ok(e) => e.path(),
            _ => continue,
        };
        if !path.is_file() { continue; }
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let is_bin = name == "whisper-cli" || name == "whisper-server";
        let is_dylib = path.extension().map_or(false, |e| e == "dylib");
        if !is_bin && !is_dylib { continue; }

        // Получаем список зависимостей через otool
        let out = Command::new("otool")
            .args(["-L", &path.to_string_lossy()])
            .output();
        let out = match out {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
            _ => continue,
        };

        for line in out.lines() {
            let dep = line.trim().split_whitespace().next().unwrap_or("");
            if !dep.starts_with("@rpath/") { continue; }
            let lib_name = dep.strip_prefix("@rpath/").unwrap();
            let new_ref = format!("@loader_path/{}", lib_name);
            let _ = Command::new("install_name_tool")
                .args(["-change", dep, &new_ref, &path.to_string_lossy()])
                .status();
            log::info!("  fix_rpath: {} → {}", name, new_ref);
        }
    }

    let _ = std::fs::remove_dir_all(&build_dir);
    log::info!("Сборка whisper завершена. Временные файлы удалены.");
    Ok(())
}

pub fn ensure_whisper_bins(preferred: Option<&str>) -> Result<String, String> {
    if let Some(pref) = preferred {
        let path = Path::new(pref);
        let cli = path.join("whisper-cli");
        if cli.exists() {
            log::info!("whisper-cli: {pref}");
            return Ok(pref.to_string());
        }
    }

    let cli = whisper_cli_path();
    if cli.exists() {
        let path = bins_dir();
        log::info!("whisper-cli: {}", path.display());
        return Ok(path.to_string_lossy().to_string());
    }

    log::info!("Бинарники whisper не найдены. Собираю из исходников...");
    build_whisper()?;

    if !whisper_cli_path().exists() {
        return Err("Сборка whisper завершилась, но whisper-cli не найден в bins/".to_string());
    }

    let path = bins_dir();
    Ok(path.to_string_lossy().to_string())
}
