use std::path::{Path, PathBuf};
use std::process::Command;

const WHISPER_VERSION: &str = "v1.9.1";

struct Variant {
    name: &'static str,
    zip_pattern: &'static str,
    needed_files: &'static [&'static str],
}

static VARIANTS: &[Variant] = &[
    Variant {
        name: "cu12.4",
        zip_pattern: "whisper-cublas-12.4.0-bin-x64.zip",
        needed_files: &[
            "whisper-cli.exe",
            "whisper.dll",
            "ggml-cuda.dll",
            "cublas64_12.dll",
            "cublasLt64_12.dll",
            "cudart64_12.dll",
            "nvrtc64_120_0.dll",
            "nvrtc-builtins64_124.dll",
            "nvblas64_12.dll",
        ],
    },
    Variant {
        name: "cu11.8",
        zip_pattern: "whisper-cublas-11.8.0-bin-x64.zip",
        needed_files: &[
            "whisper-cli.exe",
            "whisper.dll",
            "ggml-cuda.dll",
            "cublas64_11.dll",
            "cublasLt64_11.dll",
            "cudart64_110.dll",
            "cudart32_110.dll",
            "nvrtc-builtins64_118.dll",
            "nvrtc64_112_0.dll",
        ],
    },
    Variant {
        name: "blas",
        zip_pattern: "whisper-blas-bin-x64.zip",
        needed_files: &[
            "whisper-cli.exe",
            "libopenblas.dll",
        ],
    },
    Variant {
        name: "cpu",
        zip_pattern: "whisper-bin-x64.zip",
        needed_files: &[
            "whisper-cli.exe",
        ],
    },
];

fn bins_dir() -> PathBuf {
    let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".into());
    PathBuf::from(appdata).join("VoxMiM").join("bin")
}

fn zip_url(variant: &Variant) -> String {
    format!(
        "https://github.com/ggml-org/whisper.cpp/releases/download/{}/{}",
        WHISPER_VERSION, variant.zip_pattern
    )
}

fn has_nvidia_gpu() -> bool {
    // Пытаемся найти nvidia-smi
    let paths = [
        r"C:\Program Files\NVIDIA Corporation\NVSMI\nvidia-smi.exe",
        r"C:\Windows\System32\nvidia-smi.exe",
    ];
    for p in &paths {
        if Path::new(p).exists() {
            return true;
        }
    }
    // Пробуем запустить
    Command::new("nvidia-smi")
        .arg("--version")
        .output()
        .is_ok()
}

/// Выбирает наилучший вариант из доступных
fn pick_variant(downloaded: &[&str]) -> Option<&'static Variant> {
    for v in VARIANTS {
        if downloaded.contains(&v.name) || bins_dir().join(v.name).join("whisper-cli.exe").exists() {
            return Some(v);
        }
    }
    None
}

/// Проверяет, какие варианты уже скачаны
fn list_downloaded() -> Vec<String> {
    let dir = bins_dir();
    if !dir.exists() {
        return Vec::new();
    }
    let mut result = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            if entry.path().is_dir() && entry.path().join("whisper-cli.exe").exists() {
                if let Ok(name) = entry.file_name().into_string() {
                    result.push(name);
                }
            }
        }
    }
    result
}

/// Скачивает и распаковывает whisper-cli + DLL
fn download_variant(variant: &Variant) -> Result<(), String> {
    let dest = bins_dir().join(variant.name);
    if dest.join("whisper-cli.exe").exists() {
        return Ok(()); // уже есть
    }

    let url = zip_url(variant);
    let temp_zip = bins_dir().join("temp.zip");
    let temp_dir = bins_dir().join("_tmp_extract");

    std::fs::create_dir_all(&bins_dir()).map_err(|e| format!("Не удалось создать папку: {e}"))?;
    let _ = std::fs::remove_dir_all(&temp_dir);

    log::info!("Скачиваю {} ({})...", variant.name, variant.zip_pattern);

    // Скачивание через curl.exe (встроен в Windows 10+)
    let status = Command::new("curl.exe")
        .args([
            "-L", "-o",
            temp_zip.to_str().unwrap(),
            &url,
        ])
        .status()
        .map_err(|e| format!("curl не найден: {e}"))?;

    if !status.success() {
        let _ = std::fs::remove_file(&temp_zip);
        return Err(format!("Ошибка скачивания {}", url));
    }

    log::info!("Распаковываю...");

    // Распаковка через PowerShell Expand-Archive
    let ps_script = format!(
        "Expand-Archive -Path '{}' -DestinationPath '{}' -Force",
        temp_zip.to_str().unwrap_or(""),
        temp_dir.to_str().unwrap_or("")
    );
    let status = Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps_script])
        .status()
        .map_err(|e| format!("PowerShell не запущен: {e}"))?;

    if !status.success() {
        let _ = std::fs::remove_file(&temp_zip);
        let _ = std::fs::remove_dir_all(&temp_dir);
        return Err("Ошибка распаковки ZIP".to_string());
    }

    // Копируем нужные файлы
    std::fs::create_dir_all(&dest).map_err(|e| format!("Не удалось создать {}: {e}", variant.name))?;

    let found = collect_files(&temp_dir);
    let mut copied = 0;
    for needed in variant.needed_files {
        if let Some(src) = find_file(&found, needed) {
            let dst = dest.join(needed);
            let _ = std::fs::copy(&src, &dst);
            copied += 1;
        } else {
            log::warn!("Файл {needed} не найден в архиве");
        }
    }

    // Удаляем временные файлы
    let _ = std::fs::remove_file(&temp_zip);
    let _ = std::fs::remove_dir_all(&temp_dir);

    log::info!("Загружено {}/{} файлов в {}", copied, variant.needed_files.len(), dest.display());
    Ok(())
}

fn collect_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.extend(collect_files(&path));
            } else {
                files.push(path);
            }
        }
    }
    files
}

fn find_file(files: &[PathBuf], name: &str) -> Option<PathBuf> {
    files.iter()
        .find(|p| p.file_name().and_then(|n| n.to_str()) == Some(name))
        .cloned()
}

/// Главная функция: проверяет наличие, при необходимости скачивает.
/// Возвращает путь к папке с whisper-cli.exe
pub fn ensure_whisper_bins(preferred: Option<&str>) -> Result<String, String> {
    // 1. Проверяем путь из конфига
    if let Some(pref) = preferred {
        let path = Path::new(pref);
        if path.join("whisper-cli.exe").exists() {
            log::info!("whisper-cli: {pref}");
            return Ok(pref.to_string());
        }
    }

    // 2. Проверяем уже скачанные ранее
    let downloaded = list_downloaded();
    let downloaded_refs: Vec<&str> = downloaded.iter().map(|s| s.as_str()).collect();
    if let Some(v) = pick_variant(&downloaded_refs) {
        let path = bins_dir().join(v.name);
        log::info!("whisper-cli: {}", path.display());
        return Ok(path.to_string_lossy().to_string());
    }

    // 3. Проверяем известные локальные пути
    let local = [
        (r"C:\_workPortable\WhisperCpp\bins\cu-bin-blas12.4", "cu12.4"),
        (r"C:\_workPortable\WhisperCpp\bins\cu-bin-blas11.8", "cu11.8"),
        (r"C:\_workPortable\WhisperCpp\bins\bin-blas", "blas"),
        (r"C:\_workPortable\WhisperCpp\bins\bin", "cpu"),
    ];
    for (path, name) in &local {
        if Path::new(path).join("whisper-cli.exe").exists() {
            log::info!("whisper-cli (локальный): {name}");
            return Ok(path.to_string());
        }
    }

    // 4. Определяем, что скачивать
    let has_cuda = has_nvidia_gpu();
    let to_download: &[&Variant] = if has_cuda {
        &[&VARIANTS[0], &VARIANTS[1], &VARIANTS[2], &VARIANTS[3]]
    } else {
        &[&VARIANTS[2], &VARIANTS[3]]
    };

    let mut last_err = String::new();
    for v in to_download {
        match download_variant(v) {
            Ok(()) => {
                let path = bins_dir().join(v.name);
                log::info!("whisper-cli скачан: {}", path.display());
                return Ok(path.to_string_lossy().to_string());
            }
            Err(e) => {
                log::warn!("Не удалось загрузить {}: {e}", v.name);
                last_err = e;
            }
        }
    }

    Err(format!("Не удалось получить whisper-cli: {last_err}"))
}
