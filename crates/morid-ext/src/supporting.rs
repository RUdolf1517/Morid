//! Built-in helper descriptors (D-020), never downloader extension files.
use crate::Target;

/// Source shared by Windows ffmpeg and Python, approved explicitly in D-021.
pub const PACKAGES_ENDPOINT: &str =
    "https://api.github.com/repos/mhogomchungu/packages/releases/latest";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupportingEngine {
    pub name: &'static str,
    pub executable: &'static str,
    pub version_argument: &'static str,
    pub version_line: usize,
    pub version_position: usize,
    pub release_endpoint: Option<&'static str>,
}

/// Product platform rules from D-020, including deno on Linux x86 as well as
/// quickjs-ng. Python's condition is presence of you-get, not merely its JSON.
pub fn descriptors(target: &Target<'_>, you_get_present: bool) -> Vec<SupportingEngine> {
    let windows = target.platform == "Windows";
    let mut helpers = vec![
        SupportingEngine {
            name: "ffmpeg",
            executable: if windows { "ffmpeg.exe" } else { "ffmpeg" },
            version_argument: "-version",
            version_line: 0,
            version_position: 2,
            release_endpoint: windows.then_some(PACKAGES_ENDPOINT),
        },
        SupportingEngine {
            name: "deno",
            executable: if windows { "deno.exe" } else { "deno" },
            version_argument: "--version",
            version_line: 0,
            version_position: 1,
            release_endpoint: Some("https://api.github.com/repos/denoland/deno/releases/latest"),
        },
    ];
    helpers.push(SupportingEngine {
        name: if windows { "bsdtar" } else { "tar" },
        executable: if windows { "bsdtar.exe" } else { "tar" },
        version_argument: "--version",
        version_line: 0,
        version_position: if target.platform == "Generic" { 3 } else { 1 },
        // engines.cpp::getSupportingEngineByName declares no download URL.
        release_endpoint: None,
    });
    if target.platform == "Generic" && target.arch == "x86" {
        helpers.push(SupportingEngine {
            name: "quickjs-ng",
            executable: "qjs-linux-x86",
            version_argument: "--version",
            version_line: 0,
            version_position: 0,
            release_endpoint: Some(
                "https://api.github.com/repos/quickjs-ng/quickjs/releases/latest",
            ),
        });
    }
    if you_get_present {
        helpers.push(SupportingEngine {
            name: "python",
            executable: if windows { "python.exe" } else { "python3" },
            version_argument: "--version",
            version_line: 0,
            version_position: 1,
            release_endpoint: windows.then_some(PACKAGES_ENDPOINT),
        });
    }
    helpers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supporting_sets_follow_d020_on_every_platform() {
        for (platform, arch, expected) in [
            ("Windows", "amd64", vec!["ffmpeg", "deno", "bsdtar"]),
            ("Windows", "x86", vec!["ffmpeg", "deno", "bsdtar"]),
            ("MacOS", "amd64", vec!["ffmpeg", "deno", "tar"]),
            ("MacOS", "aarch64", vec!["ffmpeg", "deno", "tar"]),
            ("Generic", "amd64", vec!["ffmpeg", "deno", "tar"]),
            ("Generic", "aarch64", vec!["ffmpeg", "deno", "tar"]),
            (
                "Generic",
                "x86",
                vec!["ffmpeg", "deno", "tar", "quickjs-ng"],
            ),
        ] {
            for present in [false, true] {
                let rows = descriptors(&Target { platform, arch }, present);
                let names: Vec<_> = rows.iter().map(|row| row.name).collect();
                let mut wanted = expected.clone();
                if present {
                    wanted.push("python");
                }
                assert_eq!(names, wanted);
                println!("SUPPORTING_SET {platform}/{arch} you_get_present={present}: {names:?}");
                assert!(rows.iter().all(|row| !row.version_argument.is_empty()));
                if platform != "Windows" {
                    assert!(rows
                        .iter()
                        .filter(|row| matches!(row.name, "ffmpeg" | "python"))
                        .all(|row| row.release_endpoint.is_none()));
                }
            }
        }
    }
}
