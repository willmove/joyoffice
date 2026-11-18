use std::path::{Path, PathBuf};
use std::process::Command;

const BOOTSTRAPPED: &str = "JOYOFFICE_GRAPHICS_BOOTSTRAPPED";
const FORCE: &str = "JOYOFFICE_SOFTWARE_GPU";
const DISABLE: &str = "JOYOFFICE_DISABLE_SOFTWARE_GPU";

/// Add the user-installed Vulkan software runtime on hosts without a render node.
/// Hardware drivers stay ahead of lavapipe, and an explicit driver selection is preserved.
pub fn prepare_desktop_graphics() -> Result<(), String> {
    if env_flag(BOOTSTRAPPED)
        || env_flag(DISABLE)
        || std::env::var_os(FORCE).is_some_and(|value| value == "0")
        || (std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none())
    {
        return Ok(());
    }
    if !env_flag(FORCE)
        && (std::env::var_os("VK_ICD_FILENAMES").is_some()
            || std::env::var_os("VK_DRIVER_FILES").is_some()
            || has_render_node())
    {
        return Ok(());
    }
    let Some(runtime) = Runtime::discover() else {
        if env_flag(FORCE) {
            return Err(
                "JOYOFFICE_SOFTWARE_GPU=1 requires a complete JoyOffice graphics runtime".into(),
            );
        }
        return Ok(());
    };
    eprintln!("JoyOffice: no render node found; enabling the Vulkan software fallback");
    runtime.exec()
}

fn env_flag(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|value| value == "1")
}

fn has_render_node() -> bool {
    std::fs::read_dir("/dev/dri")
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .any(|entry| is_render_node(&entry.file_name().to_string_lossy()))
}

fn is_render_node(name: &str) -> bool {
    name.strip_prefix("renderD").is_some_and(|suffix| {
        !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
    })
}

struct Runtime {
    loader: PathBuf,
    glibc: PathBuf,
    graphics: PathBuf,
    mesa: PathBuf,
    icd: PathBuf,
}

impl Runtime {
    fn discover() -> Option<Self> {
        let root = std::env::var_os("JOYOFFICE_RUNTIME_DIR")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(|home| PathBuf::from(home).join(".local/share/joyoffice/runtime"))
            })?;
        Self::at(root)
    }

    fn at(root: PathBuf) -> Option<Self> {
        let lib = root.join("glibc2.39/usr/lib/x86_64-linux-gnu");
        let runtime = Runtime {
            loader: lib.join("ld-linux-x86-64.so.2"),
            glibc: lib,
            graphics: root.join("graphics/usr/lib/x86_64-linux-gnu"),
            mesa: root.join("mesa-vulkan/usr/lib/x86_64-linux-gnu"),
            icd: root.join("lvp_icd.json"),
        };
        let complete = [
            &runtime.loader,
            &runtime.glibc.join("libc.so.6"),
            &runtime.graphics.join("libvulkan.so.1"),
            &runtime.mesa.join("libvulkan_lvp.so"),
            &runtime.icd,
        ]
        .into_iter()
        .all(|path| path.is_file());
        complete.then_some(runtime)
    }

    fn exec(self) -> Result<(), String> {
        use std::os::unix::process::CommandExt;

        let library_path = std::env::join_paths(self.library_paths())
            .map_err(|error| format!("invalid software graphics library path: {error}"))?;
        let drivers = std::env::join_paths(icd_files(&self.icd))
            .map_err(|error| format!("invalid Vulkan driver path: {error}"))?;
        let executable = std::env::current_exe()
            .map_err(|error| format!("cannot locate the application executable: {error}"))?;
        let mut command = Command::new(&self.loader);
        command
            .arg("--library-path")
            .arg(&library_path)
            .arg(executable)
            .args(std::env::args_os().skip(1))
            .env(BOOTSTRAPPED, "1")
            .env("GCONV_PATH", self.glibc.join("gconv"))
            .env("LD_LIBRARY_PATH", &library_path)
            .env("VK_ICD_FILENAMES", drivers)
            .env("LIBGL_ALWAYS_SOFTWARE", "1")
            .env("GALLIUM_DRIVER", "llvmpipe")
            .env("RAYON_NUM_THREADS", "1");
        let mesa_egl = Path::new("/usr/share/glvnd/egl_vendor.d/50_mesa.json");
        if mesa_egl.is_file() {
            command
                .env("__EGL_VENDOR_LIBRARY_FILENAMES", mesa_egl)
                .env("__GLX_VENDOR_LIBRARY_NAME", "mesa");
        }
        let error = command.exec();
        Err(format!(
            "cannot start the software graphics runtime: {error}"
        ))
    }

    fn library_paths(&self) -> Vec<PathBuf> {
        let mut paths = vec![
            self.glibc.clone(),
            self.graphics.clone(),
            self.mesa.clone(),
            PathBuf::from("/lib/x86_64-linux-gnu"),
            PathBuf::from("/usr/lib/x86_64-linux-gnu"),
        ];
        if let Some(existing) = std::env::var_os("LD_LIBRARY_PATH") {
            paths.extend(std::env::split_paths(&existing));
        }
        paths
    }
}

fn icd_files(software: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for directory in [
        PathBuf::from("/usr/local/share/vulkan/icd.d"),
        PathBuf::from("/usr/share/vulkan/icd.d"),
    ] {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(directory)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "json")
            })
            .collect();
        entries.sort();
        paths.extend(entries);
    }
    paths.push(software.to_path_buf());
    paths
}

#[cfg(test)]
mod tests {
    #[test]
    fn render_nodes_are_distinct_from_card_devices() {
        assert!(super::is_render_node("renderD128"));
        assert!(!super::is_render_node("card0"));
        assert!(!super::is_render_node("renderD"));
        assert!(!super::is_render_node("renderDvirtual"));
    }

    #[test]
    fn software_driver_is_the_last_candidate() {
        let software = std::path::Path::new("/tmp/joyoffice-lvp.json");
        assert_eq!(
            super::icd_files(software).last(),
            Some(&software.to_path_buf())
        );
    }

    #[test]
    fn runtime_rejects_an_incomplete_tree() {
        assert!(super::Runtime::at(std::path::PathBuf::from("/definitely/missing")).is_none());
    }
}
