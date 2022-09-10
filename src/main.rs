use std::{
    fs::{self, create_dir_all, File},
    io,
    path::PathBuf, fmt::Debug,
};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use lapce_plugin::{register_plugin, send_notification, start_lsp, LapcePlugin};

#[derive(Default)]
struct State {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    arch: String,
    os: String,
    configuration: Configuration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Configuration {
    language_id: String,
    options: Option<Value>,
}

register_plugin!(State);

const LSP_VER: &str = "3.4.4";

impl LapcePlugin for State {
    fn initialize(&mut self, info: serde_json::Value) {
        eprintln!("Starting lapce-cpp");
        let mut info = serde_json::from_value::<PluginInfo>(info).unwrap();

        let _ = match info.arch.as_str() {
            "x86_64" => "x86_64",
            // "aarch64" => "aarch64",
            _ => return,
        };

        // ! We need permission system so we can do stuff like HTTP requests to grab info about
        // ! latest releases, and download them or notify user about updates

        // let response =
        //     futures::executor::block_on(reqwest::get("https://api.github.com/repos/clangd/clangd/releases/latest")).expect("request failed");

        // let api_resp = futures::executor::block_on(response
        //     .json::<GHAPIResponse>()).expect("failed to deserialise api response");

        // let mut download_asset = Asset {
        //     ..Default::default()
        // };
        // for asset in api_resp.assets {
        //     match asset.name.strip_prefix("clangd-") {
        //         Some(asset_name) => match asset_name.starts_with(info.os.as_str()) {
        //             true => download_asset = asset,
        //             false => continue,
        //         },
        //         None => continue,
        //     }
        // }

        // if download_asset.browser_download_url.is_empty() || download_asset.name.is_empty() {
        //     panic!("failed to find clangd in release")
        // }

        // let zip_file = PathBuf::from(download_asset.name);

        let mut lsp_args = String::new();

        if let Some(opts) = &info.configuration.options {
            if let Some(bin) = opts.get("binary") {
                if let Some(args) = bin.get("args") {
                    if let Some(args) = args.as_str() {
                        if !args.is_empty() {
                            lsp_args = String::from(args)
                        }
                    }
                }
            }
        }

        let zip_file = String::from("PowerShellEditorServices.zip");

        let zip_file = PathBuf::from(zip_file);

        let download_url = format!("https://github.com/PowerShell/PowerShellEditorServices/releases/download/v{LSP_VER}/{}", zip_file.display());

        let mut server_path = PathBuf::from(format!("PSEditorServices"));

        create_dir_all(&server_path).expect("failed to create lsp dir");

        let exec_path = format!("{}", server_path.display());

        let lock_file = PathBuf::from("download.lock");
        send_notification(
            "lock_file",
            &json!({
                "path": &lock_file,
            }),
        );

        if !PathBuf::from(&server_path).exists() {
            send_notification(
                "download_file",
                &json!({
                    // "url": download_asset.browser_download_url,
                    "url": download_url,
                    "path": zip_file,
                }),
            );

            if !zip_file.exists() {
                eprintln!("clangd download failed");
                return;
            }

            let mut zip =
                zip::ZipArchive::new(File::open(&zip_file).unwrap()).expect("failed to open zip");

            for i in 0..zip.len() {
                let mut file = zip.by_index(i).unwrap();
                let outpath = match file.enclosed_name() {
                    Some(path) => path.to_owned(),
                    None => continue,
                };

                if (*file.name()).ends_with('/') {
                    fs::create_dir_all(&outpath).unwrap();
                } else {
                    if let Some(p) = outpath.parent() {
                        if !p.exists() {
                            fs::create_dir_all(&p).unwrap();
                        }
                    }
                    let mut outfile = fs::File::create(&outpath).unwrap();
                    io::copy(&mut file, &mut outfile).unwrap();
                }
            }

            send_notification(
                "make_file_executable",
                &json!({
                    "path": exec_path,
                }),
            );

            _ = std::fs::remove_file(&zip_file);
        }
        _ = std::fs::remove_file(&lock_file);

        // ! Need to figure out how the sandbox works to use clangd
        // ! provided by system (package managers, etc.)

        // match env::var_os("PATH") {
        //     Some(paths) => {
        //         for path in env::split_paths(&paths) {
        //             if let Ok(dir) = std::path::Path::new(path.as_path()).read_dir() {
        //                 for file in dir.flatten() {
        //                     if let Ok(server) = file.file_name().into_string() {
        //                         if server == server_path {
        //                             server_path = format!("{}", file.path().display())
        //                         }
        //                     }
        //                 }
        //             }
        //         }
        //     }
        //     None => {}
        // }

        let mut session_path = std::env::temp_dir();
        session_path = session_path.join(format!("pses-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()));


        let lsp_args = vec![];
        if lsp_args.is_empty() {
            lsp_args = vec![
                "-NoLogo".to_string(),
                "-NoProfile".to_string(),
                "-Command".to_string(),
                format!("{}/PSEditorServices/Start-EditorServices.ps1", server_path.display()),
                "-BundleModulesPath".to_string(),
                format!("{}", server_path.display()),
                "-LogPath".to_string(),
                format!("{}/logs.log", session_path.display()),
                "-SessionDetailsPath".to_string(),
                format!("{}/session.json", session_path.display()),
                "-FeatureFlags".to_string(),
                "@()".to_string(),
                "-AdditionalModules".to_string(),
                "@()".to_string(),
                "-HostName".to_string(),
                "'lapce'".to_string(),
                "-HostProfileId".to_string(),
                "'lapce'".to_string(),
                "-HostVersion".to_string(),
                "1.0.0".to_string(),
                "-Stdio".to_string(),
                "-LogLevel".to_string(),
                "Normal".to_string(),
            ];
        }

        if let Some(ref mut opts) = info.configuration.options {
            if let Some(bin) = opts.get_mut("binary") {
                if let Some(mut args) = bin.get_mut("args") {
                    args = &mut json!(lsp_args)
                }
            }
        }

        let exec_path = match info.os.as_str() {
            "windows" => "powershell.exe",
            _ => "pwsh",
        };

        eprintln!("LSP server path: {}", exec_path);

        start_lsp("pwsh", "pwsh", info.configuration.options, true)
    }
}
