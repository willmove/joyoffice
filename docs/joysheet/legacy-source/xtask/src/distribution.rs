use std::{
    path::{Path, PathBuf},
    process::Command,
};
fn run(mut c: Command) -> Result<(), String> {
    let status = c.status().map_err(|e| e.to_string())?;
    if status.success() { Ok(()) } else { Err(format!("Packaging failed: {status}")) }
}
fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::create_dir_all(to).map_err(|e| e.to_string())?;
    for file in std::fs::read_dir(from).map_err(|e| e.to_string())? {
        let file = file.map_err(|e| e.to_string())?;
        let path = file.path();
        let name = file.file_name();
        if name == "Joysheet.app" || name == "Joysheet.AppDir" || name == "debian" {
            continue;
        }
        let dest = to.join(name);
        if path.is_dir() {
            copy_tree(&path, &dest)?;
        } else {
            std::fs::copy(path, dest).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
fn write(path: PathBuf, text: &str) -> Result<(), String> {
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, text).map_err(|e| e.to_string())
}
pub fn native(root: &Path, directory: &Path, label: &str) -> Result<(), String> {
    match std::env::consts::OS {
        "windows" => {
            let mut c = Command::new("pwsh.exe");
            c.args(["-NoProfile", "-File"])
                .arg(root.join("scripts/package-windows.ps1"))
                .arg("-Directory")
                .arg(directory)
                .arg("-Output")
                .arg(root.join("dist").join(format!("{label}-setup.exe")));
            run(c)?;
        }
        "macos" => {
            if std::env::consts::ARCH != "aarch64" {
                return Err("Initial macOS distribution requires Apple Silicon".into());
            }
            let app = directory.join("Joysheet.app");
            let contents = app.join("Contents");
            let bin = contents.join("MacOS");
            std::fs::create_dir_all(&bin).map_err(|e| e.to_string())?;
            for name in ["joysheet", "joysheet-cli"] {
                std::fs::copy(directory.join(name), bin.join(name)).map_err(|e| e.to_string())?;
            }
            copy_tree(directory, &contents.join("Resources"))?;
            std::fs::copy(root.join("assets/joysheet.icns"), contents.join("Resources/joysheet.icns")).map_err(|e| e.to_string())?;
            write(
                contents.join("Info.plist"),
                r#"<?xml version="1.0" encoding="UTF-8"?><!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd"><plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>org.joysheet.desktop</string><key>CFBundleName</key><string>Joysheet</string><key>CFBundleExecutable</key><string>joysheet</string><key>CFBundlePackageType</key><string>APPL</string><key>CFBundleShortVersionString</key><string>0.1.0</string><key>CFBundleVersion</key><string>1</string><key>LSMinimumSystemVersion</key><string>15.0</string><key>NSHighResolutionCapable</key><true/><key>CFBundleIconFile</key><string>joysheet.icns</string><key>CFBundleDocumentTypes</key><array><dict><key>CFBundleTypeName</key><string>Spreadsheet workbook</string><key>CFBundleTypeRole</key><string>Editor</string><key>LSHandlerRank</key><string>Alternate</string><key>CFBundleTypeExtensions</key><array><string>xlsx</string><string>xls</string><string>xlsb</string><string>ods</string><string>csv</string><string>tsv</string></array></dict></array></dict></plist>"#,
            )?;
            let identity = std::env::var("JOYSHEET_MAC_SIGN_IDENTITY").unwrap_or_else(|_| "-".into());
            let mut c = Command::new("codesign");
            c.args(["--force", "--deep", "--sign", &identity]).arg(&app);
            if identity != "-" {
                c.args(["--options", "runtime", "--timestamp"]);
            }
            run(c)?;
            let image = root.join("dist").join(format!("{label}.dmg"));
            let mut c = Command::new("hdiutil");
            c.args(["create", "-ov", "-format", "UDZO", "-volname", "Joysheet", "-srcfolder"]).arg(&app).arg(&image);
            run(c)?;
            if let Ok(profile) = std::env::var("JOYSHEET_NOTARY_PROFILE") {
                let mut c = Command::new("xcrun");
                c.args(["notarytool", "submit"]).arg(&image).args(["--keychain-profile", &profile, "--wait"]);
                run(c)?;
                let mut c = Command::new("xcrun");
                c.args(["stapler", "staple"]).arg(&image);
                run(c)?;
            }
        }
        "linux" => {
            let app = directory.join("Joysheet.AppDir");
            let deb = directory.join("debian");
            for staging in [&app, &deb] {
                for name in ["joysheet", "joysheet-cli"] {
                    let path = staging.join("usr/bin").join(name);
                    if let Some(p) = path.parent() {
                        std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
                    }
                    std::fs::copy(directory.join(name), path).map_err(|e| e.to_string())?;
                }
                copy_tree(&directory.join("licenses"), &staging.join("usr/share/doc/joysheet/licenses"))?;
                let icon = staging.join("usr/share/icons/hicolor/512x512/apps/joysheet.png");
                if let Some(p) = icon.parent() {
                    std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
                }
                std::fs::copy(root.join("assets/joysheet.png"), icon).map_err(|e| e.to_string())?;
                write(
                    staging.join("usr/share/applications/joysheet.desktop"),
                    "[Desktop Entry]\nType=Application\nName=Joysheet\nComment=Spreadsheet desktop\nExec=joysheet %f\nIcon=joysheet\nTerminal=false\nCategories=Office;Spreadsheet;\nMimeType=application/vnd.openxmlformats-officedocument.spreadsheetml.sheet;application/vnd.ms-excel;application/vnd.oasis.opendocument.spreadsheet;text/csv;text/tab-separated-values;\n",
                )?;
            }
            write(app.join("AppRun"), "#!/bin/sh\nHERE=$(CDPATH= cd -- \"$(dirname -- \"$0\")\" && pwd)\nexec \"$HERE/usr/bin/joysheet\" \"$@\"\n")?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(app.join("AppRun"), std::fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
            }
            write(
                deb.join("DEBIAN/control"),
                "Package: joysheet\nVersion: 0.1.0\nArchitecture: amd64\nMaintainer: Joysheet contributors\nSection: editors\nPriority: optional\nDepends: libc6 (>= 2.39), libfontconfig1, libx11-xcb1, libxkbcommon-x11-0, libssl3t64, libvulkan1, libwayland-client0, libzstd1, poppler-utils, xdg-utils\nDescription: Joysheet GPUI spreadsheet desktop preview\n",
            )?;
            let mut c = Command::new("dpkg-deb");
            c.args(["--build", "--root-owner-group"]).arg(&deb).arg(root.join("dist").join(format!("{label}.deb")));
            run(c)?;
            let mut c = Command::new("tar");
            c.args(["-czf"]).arg(root.join("dist").join(format!("{label}-appdir.tar.gz"))).arg("-C").arg(directory).arg("Joysheet.AppDir");
            run(c)?;
        }
        _ => return Err("Unsupported native distribution platform".into()),
    }
    Ok(())
}
