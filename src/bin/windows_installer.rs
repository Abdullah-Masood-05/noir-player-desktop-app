#[cfg(not(windows))]
fn main() {
    eprintln!("Windows installer generation requires a Windows host.");
    std::process::exit(1);
}

#[cfg(windows)]
fn main() {
    if let Err(error) = windows::run() {
        eprintln!("Installer generation failed: {error:#}");
        std::process::exit(1);
    }
}

#[cfg(windows)]
mod windows {
    use anyhow::{bail, ensure, Context, Result};
    use std::path::{Path, PathBuf};
    use tauri_bundler::{
        bundle_project, BundleBinary, BundleSettings, NsisSettings, PackageSettings, PackageType,
        SettingsBuilder, WindowsSettings,
    };
    use tauri_utils::config::WebviewInstallMode;

    pub fn run() -> Result<()> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut args = std::env::args_os().skip(1);
        let release = match (args.next(), args.next(), args.next()) {
            (None, None, None) => root.join("target").join("release"),
            (Some(flag), Some(path), None) if flag == "--release-dir" => {
                let p = PathBuf::from(path);
                if p.is_absolute() {
                    p
                } else {
                    std::env::current_dir()?.join(p)
                }
            }
            _ => bail!(
                "Usage: cargo installer [--release-dir <directory containing noir_player.exe>]"
            ),
        };
        if !release.is_dir() {
            bail!(
                "Release directory {} does not exist. Run cargo build --release first.",
                release.display()
            );
        }
        let binary = release.join("noir_player.exe");
        let original = std::fs::read(&binary).with_context(|| {
            format!(
                "Missing {}. Run cargo build --release first.",
                binary.display()
            )
        })?;
        ensure!(
            original.get(..2) == Some(b"MZ"),
            "Expected a Windows PE executable"
        );
        let offset = original.get(0x3c..0x40).context("Truncated PE header")?;
        let pe = u32::from_le_bytes(offset.try_into()?) as usize;
        let header = original.get(pe..).context("Invalid PE offset")?;
        ensure!(header.get(..4) == Some(b"PE\0\0"), "Invalid PE signature");
        ensure!(
            header.get(4..6) == Some(&[0x64, 0x86]),
            "Only Windows x64 executables are supported"
        );

        let icon = root.join("Noir_Player_Logo.ico");
        ensure!(icon.is_file(), "Missing icon: {}", icon.display());

        println!("WiX and NSIS tools will be downloaded automatically by tauri-bundler on first run (cached in %LOCALAPPDATA%\\tauri).");
        if !cfg!(debug_assertions) {
            println!("Hint: MSI packaging requires .NET Framework 4.x and the VBScript Windows feature. NSIS has no extra requirements.");
        }

        let stage = release.join("installer-stage");
        if stage.exists() {
            std::fs::remove_dir_all(&stage).with_context(|| {
                format!("Cannot clean stale staging directory {}", stage.display())
            })?;
        }
        std::fs::create_dir(&stage)
            .with_context(|| format!("Cannot create staging directory {}", stage.display()))?;

        let result = package(&root, &release, &stage, &icon);

        let _ = std::fs::remove_dir_all(&stage);
        result?;

        ensure!(
            std::fs::read(&binary)? == original,
            "Application executable unexpectedly changed during packaging"
        );
        Ok(())
    }

    #[allow(deprecated)]
    fn package(root: &Path, release: &Path, stage: &Path, icon: &Path) -> Result<()> {
        std::fs::copy(
            release.join("noir_player.exe"),
            stage.join("noir_player.exe"),
        )?;

        let output = std::process::Command::new("cargo")
            .args(["metadata", "--no-deps", "--format-version", "1", "--locked"])
            .current_dir(root)
            .output()
            .context("Cannot run cargo metadata")?;
        ensure!(
            output.status.success(),
            "Cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
        let metadata: serde_json::Value = serde_json::from_slice(&output.stdout)
            .context("Cargo metadata returned invalid JSON")?;
        let package = metadata["packages"]
            .as_array()
            .context("Missing Cargo packages")?
            .iter()
            .find(|p| p["name"] == env!("CARGO_PKG_NAME"))
            .context("Application package not found in Cargo metadata")?;
        let bundle = &package["metadata"]["bundle"];
        let name = bundle["name"]
            .as_str()
            .context("Missing bundle.name in Cargo.toml")?;
        let identifier = bundle["identifier"]
            .as_str()
            .context("Missing bundle.identifier in Cargo.toml")?;
        let authors: Vec<String> = package["authors"]
            .as_array()
            .context("Missing package.authors in Cargo.toml")?
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
        let publisher = authors
            .first()
            .context("At least one package author is required")?
            .split('<')
            .next()
            .unwrap_or_default()
            .trim()
            .to_owned();

        let windows = WindowsSettings {
            icon_path: PathBuf::new(),
            webview_install_mode: WebviewInstallMode::Skip,
            minimum_webview2_version: None,
            nsis: Some(NsisSettings {
                installer_icon: Some(icon.to_path_buf()),
                minimum_webview2_version: None,
                ..Default::default()
            }),
            ..Default::default()
        };

        let settings = SettingsBuilder::new()
            .project_out_directory(stage)
            .target("x86_64-pc-windows-msvc".into())
            .package_types(vec![PackageType::WindowsMsi, PackageType::Nsis])
            .package_settings(PackageSettings {
                product_name: name.into(),
                version: env!("CARGO_PKG_VERSION").into(),
                description: env!("CARGO_PKG_DESCRIPTION").into(),
                homepage: Some(env!("CARGO_PKG_HOMEPAGE").into()),
                authors: Some(authors),
                default_run: Some("noir_player".into()),
            })
            .bundle_settings(BundleSettings {
                identifier: Some(identifier.into()),
                publisher: Some(publisher),
                icon: Some(vec![icon.to_string_lossy().into_owned()]),
                windows,
                updater: None,
                ..Default::default()
            })
            .binaries(vec![BundleBinary::new("noir_player".into(), true)])
            .no_sign(true)
            .build()?;

        let bundles = bundle_project(&settings)
            .with_context(|| "tauri-bundler failed to generate installers. Check internet access for tool downloads, .NET Framework 4.x, and VBScript Windows feature.")?;

        let mut outputs = Vec::new();
        for bundle in bundles {
            for path in bundle.bundle_paths {
                let kind = match path.extension().and_then(|ext| ext.to_str()) {
                    Some("msi") => "msi",
                    Some("exe") => "nsis",
                    _ => continue,
                };
                ensure!(
                    std::fs::metadata(&path)?.len() > 0,
                    "Empty installer output"
                );
                outputs.push((kind, path));
            }
        }

        ensure!(
            outputs.iter().any(|(k, _)| *k == "msi") && outputs.iter().any(|(k, _)| *k == "nsis"),
            "Expected one MSI and one NSIS installer, got: {:?}",
            outputs
                .iter()
                .map(|(k, p)| format!("{k}: {}", p.display()))
                .collect::<Vec<_>>()
        );

        for (kind, path) in outputs {
            let dest_dir = release.join("bundle").join(kind);
            std::fs::create_dir_all(&dest_dir)?;
            let dest = dest_dir.join(path.file_name().context("Missing installer filename")?);
            std::fs::copy(&path, &dest)?;
            println!("{}", dest.display());
        }
        Ok(())
    }
}
