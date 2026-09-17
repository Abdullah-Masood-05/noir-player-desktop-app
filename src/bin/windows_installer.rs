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
    use std::process::Command;
    use tauri_bundler::{
        bundle_project, BundleBinary, BundleSettings, NsisSettings, PackageSettings, PackageType,
        SettingsBuilder, WindowsSettings,
    };
    use tauri_utils::config::WebviewInstallMode;

    pub fn run() -> Result<()> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut args = std::env::args_os().skip(1);
        let release = match (args.next(), args.next(), args.next()) {
            (None, None, None) => root.join("target/release"),
            (Some(flag), Some(path), None) if flag == "--release-dir" => PathBuf::from(path),
            _ => bail!(
                "Usage: cargo installer [--release-dir <directory containing noir_player.exe>]"
            ),
        };
        let release = release.canonicalize().with_context(|| {
            format!(
                "Release directory {} is missing. Run cargo build --release first.",
                release.display()
            )
        })?;
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
        prerequisites()?;

        let icon = root.join("Noir_Player_Logo.ico");
        ensure!(icon.is_file(), "Missing icon: {}", icon.display());
        let tools = root.join("target/installer-tools");
        for (name, relative) in [
            ("WiX", ".tauri/WixTools314/candle.exe"),
            ("NSIS", ".tauri/NSIS/makensis.exe"),
        ] {
            println!(
                "{name}: {}",
                if tools.join(relative).is_file() {
                    "cached; bundler will validate tool files"
                } else {
                    "not cached; bundler will download its pinned tools (internet required)"
                }
            );
        }
        let stage = release.join(format!("installer-stage-{}", std::process::id()));
        std::fs::create_dir(&stage).context("Cannot create isolated staging directory; remove a stale directory with this name and retry")?;
        let result = package(&root, &release, &stage, &icon, &tools);
        if let Err(error) = std::fs::remove_dir_all(&stage) {
            eprintln!(
                "Could not remove staging directory {}: {error}",
                stage.display()
            );
        }
        result?;
        ensure!(
            std::fs::read(&binary)? == original,
            "Application executable unexpectedly changed"
        );
        Ok(())
    }

    fn prerequisites() -> Result<()> {
        let script = r#"$ErrorActionPreference = 'Stop'; $release = (Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\NET Framework Setup\NDP\v4\Full' -Name Release).Release; if ($release -lt 378389) { [Console]::Error.WriteLine('.NET Framework 4.5 or later is required by WiX 3'); exit 1 }; $type = [Type]::GetTypeFromCLSID([Guid]'B54F3741-5B07-11cf-A4B0-00AA004A55E8', $true); $engine = [Activator]::CreateInstance($type); [void][Runtime.InteropServices.Marshal]::ReleaseComObject($engine)"#;
        let output = Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .output()
            .context(
                "Cannot run Windows PowerShell to check .NET Framework and VBScript prerequisites",
            )?;
        ensure!(output.status.success(), "MSI prerequisites unavailable. Install .NET Framework 4.x and enable VBScript in Windows Optional Features, then retry. Details: {}", String::from_utf8_lossy(&output.stderr).trim());
        Ok(())
    }

    #[allow(deprecated)]
    fn package(root: &Path, release: &Path, stage: &Path, icon: &Path, tools: &Path) -> Result<()> {
        std::fs::copy(
            release.join("noir_player.exe"),
            stage.join("noir_player.exe"),
        )?;
        let output = Command::new("cargo")
            .args(["metadata", "--no-deps", "--format-version", "1", "--locked"])
            .current_dir(root)
            .output()
            .context("Cannot read Cargo package metadata")?;
        ensure!(output.status.success(), "Cargo metadata failed: {}", String::from_utf8_lossy(&output.stderr).trim());
        let metadata: serde_json::Value = serde_json::from_slice(&output.stdout)
            .context("Cargo metadata returned invalid JSON")?;
        let package = metadata["packages"]
            .as_array()
            .context("Missing Cargo packages")?
            .iter()
            .find(|package| package["name"] == env!("CARGO_PKG_NAME"))
            .context("Application package not found")?;
        let bundle = &package["metadata"]["bundle"];
        let name = bundle["name"].as_str().context("Missing bundle.name")?;
        let identifier = bundle["identifier"]
            .as_str()
            .context("Missing bundle.identifier")?;
        let authors: Vec<String> = package["authors"]
            .as_array()
            .context("Missing package.authors")?
            .iter()
            .filter_map(|author| author.as_str().map(String::from))
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
            .local_tools_directory(tools)
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
        let bundles = bundle_project(&settings).with_context(|| format!("tauri-bundler could not generate installers. WiX/NSIS are downloaded automatically into {}. Check HTTPS access, tool quarantine, .NET Framework and VBScript. No system WiX or NSIS installation is required.", tools.display()))?;
        let mut outputs = Vec::new();
        for bundle in bundles {
            for path in bundle.bundle_paths {
                let kind = match path.extension().and_then(|ext| ext.to_str()) {
                    Some("msi") => "msi",
                    Some("exe") => "nsis",
                    _ => bail!("Unexpected installer output: {}", path.display()),
                };
                ensure!(
                    std::fs::metadata(&path)?.len() > 0,
                    "Empty installer output"
                );
                outputs.push((kind, path));
            }
        }
        ensure!(
            outputs.len() == 2
                && outputs.iter().any(|(kind, _)| *kind == "msi")
                && outputs.iter().any(|(kind, _)| *kind == "nsis"),
            "Expected one MSI and one NSIS installer"
        );
        for (kind, path) in outputs {
            let destination = release.join("bundle").join(kind);
            std::fs::create_dir_all(&destination)?;
            let destination =
                destination.join(path.file_name().context("Missing installer filename")?);
            std::fs::copy(&path, &destination)?;
            println!("{}", destination.display());
        }
        Ok(())
    }
}
