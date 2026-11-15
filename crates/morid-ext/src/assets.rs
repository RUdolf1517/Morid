//! Asset predicates ported from original foundNetworkUrl overrides. The first
//! match wins in API order, not by lexicographic version or publication date.
use crate::{Extension, Target};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rule {
    Exact(String),
    Contains(String),
    YouGetTar,
}
impl Rule {
    pub fn matches(&self, name: &str) -> bool {
        match self {
            Self::Exact(wanted) => name == wanted,
            Self::Contains(wanted) => !wanted.is_empty() && name.contains(wanted),
            Self::YouGetTar => {
                (name.starts_with("you_get") || name.starts_with("you-get"))
                    && name.ends_with(".tar.gz")
            }
        }
    }
}

pub fn for_extension(extension: &Extension) -> Rule {
    match extension.id.as_str() {
        "you-get" => Rule::YouGetTar, // legacy/src/engines/you-get.cpp:37
        "yt-dlp-nightly" => Rule::Exact(extension.command.name.replace("-nightly", "")), // yt-dlp.cpp:524
        _ => Rule::Exact(extension.command.name.clone()), // engines.cpp:1964
    }
}
pub fn endpoint<'a>(extension: &'a Extension, target: &Target<'_>) -> &'a str {
    if target.platform == "MacOS" && !extension.metadata.download_url_mac.is_empty() {
        &extension.metadata.download_url_mac
    } else {
        &extension.metadata.download_url
    }
}

pub fn for_helper(name: &str, target: &Target<'_>) -> Option<Rule> {
    let exact = |name: &str| Some(Rule::Exact(name.into()));
    match name {
        "deno" => {
            // deno.cpp:116 uses contains, not equality; retain that observable rule.
            let filename = match (target.platform, target.arch) {
                ("Windows", "amd64") => "deno-x86_64-pc-windows-msvc.zip",
                ("Generic", "amd64") => "deno-x86_64-unknown-linux-gnu.zip",
                ("Generic", "aarch64") => "deno-aarch64-unknown-linux-gnu.zip",
                ("MacOS", "amd64") => "deno-x86_64-apple-darwin.zip",
                ("MacOS", "aarch64") => "deno-aarch64-apple-darwin.zip",
                _ => return None,
            };
            Some(Rule::Contains(filename.into()))
        }
        "quickjs-ng" if target.platform == "Generic" && target.arch == "x86" => {
            exact("qjs-linux-x86")
        } // quickjs_ng.cpp:138
        "ffmpeg" if target.platform == "Windows" && target.arch == "amd64" => {
            exact("ffmpeg-n9.0-win64-gpl-shared.zip")
        } // ffmpeg.cpp:24
        "ffmpeg" if target.platform == "Windows" && target.arch == "x86" => {
            exact("ffmpeg-n8.1-win32-gpl-shared.zip")
        }
        "python" if target.platform == "Windows" => exact("python-3.8.10-win32.zip"), // python.cpp:121
        _ => None, // tar/bsdtar and system-only helpers have no release source.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn synthetic_platform_asset_table_follows_original_predicates() {
        for (platform, arch, helper, asset) in [
            (
                "Windows",
                "amd64",
                "deno",
                "deno-x86_64-pc-windows-msvc.zip",
            ),
            (
                "Generic",
                "amd64",
                "deno",
                "deno-x86_64-unknown-linux-gnu.zip",
            ),
            (
                "Generic",
                "aarch64",
                "deno",
                "deno-aarch64-unknown-linux-gnu.zip",
            ),
            ("MacOS", "amd64", "deno", "deno-x86_64-apple-darwin.zip"),
            ("MacOS", "aarch64", "deno", "deno-aarch64-apple-darwin.zip"),
            ("Generic", "x86", "quickjs-ng", "qjs-linux-x86"),
            (
                "Windows",
                "amd64",
                "ffmpeg",
                "ffmpeg-n9.0-win64-gpl-shared.zip",
            ),
            (
                "Windows",
                "x86",
                "ffmpeg",
                "ffmpeg-n8.1-win32-gpl-shared.zip",
            ),
            ("Windows", "amd64", "python", "python-3.8.10-win32.zip"),
        ] {
            let rule = for_helper(helper, &Target { platform, arch }).unwrap();
            assert!(rule.matches(asset));
            assert!(!rule.matches("different-platform"));
            println!("SYNTHETIC_ASSET_RULE {platform}/{arch} helper={helper} rule={rule:?} selected={asset}");
        }
        assert!(for_helper(
            "deno",
            &Target {
                platform: "Generic",
                arch: "x86"
            }
        )
        .is_none());
        assert!(Rule::YouGetTar.matches("you_get-1.0.tar.gz"));
        assert!(!Rule::YouGetTar.matches("you_get-1.0.zip"));
        let deno = for_helper(
            "deno",
            &Target {
                platform: "Generic",
                arch: "amd64",
            },
        )
        .unwrap();
        assert!(deno.matches("prefix-deno-x86_64-unknown-linux-gnu.zip-suffix"));
        println!("ORIGINAL_ASSET_DIFFERENCE deno=contains-not-equality you_get=prefix-plus-tar.gz nightly=strips-nightly first-match-is-API-order");
    }
}
