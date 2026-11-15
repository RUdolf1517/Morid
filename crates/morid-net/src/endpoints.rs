//! Exact release endpoints from shipped descriptions, plus the one D-021 exception.
//! This table is not extended by user extension JSON or a redirect.
#[derive(Debug)]
pub struct Endpoint {
    pub url: &'static str,
    pub source: &'static str,
}

pub const RELEASE_ENDPOINTS: &[Endpoint] = &[
    // extensions/deno.json::DownloadUrl
    Endpoint {
        url: "https://api.github.com/repos/denoland/deno/releases/latest",
        source: "extensions/deno.json::DownloadUrl",
    },
    // extensions/gallery-dl.json::DownloadUrl
    Endpoint {
        url: "https://codeberg.org/api/v1/repos/mikf/gallery-dl/releases",
        source: "extensions/gallery-dl.json::DownloadUrl",
    },
    // extensions/gallery-dl.json::DownloadUrlMAC
    Endpoint {
        url: "https://api.github.com/repos/gdl-org/builds/releases/latest",
        source: "extensions/gallery-dl.json::DownloadUrlMAC",
    },
    // extensions/getsauce.json::DownloadUrl
    Endpoint {
        url: "https://api.github.com/repos/gan-of-culture/get-sauce/releases/latest",
        source: "extensions/getsauce.json::DownloadUrl",
    },
    // extensions/lux.json::DownloadUrl
    Endpoint {
        url: "https://api.github.com/repos/iawia002/lux/releases/latest",
        source: "extensions/lux.json::DownloadUrl",
    },
    // extensions/quickjs.json::DownloadUrl
    Endpoint {
        url: "https://bellard.org/quickjs/binary_releases/LATEST.json",
        source: "extensions/quickjs.json::DownloadUrl",
    },
    // extensions/quickjs_ng.json::DownloadUrl
    Endpoint {
        url: "https://api.github.com/repos/quickjs-ng/quickjs/releases/latest",
        source: "extensions/quickjs_ng.json::DownloadUrl",
    },
    // extensions/safaribooks.json::DownloadUrl
    Endpoint {
        url: "https://api.github.com/repos/mhogomchungu/safaribooks/releases/latest",
        source: "extensions/safaribooks.json::DownloadUrl",
    },
    // extensions/you-get.json::DownloadUrl
    Endpoint {
        url: "https://api.github.com/repos/soimort/you-get/releases/latest",
        source: "extensions/you-get.json::DownloadUrl",
    },
    // extensions/yt-dlp-aria2c-native.json::DownloadUrl, extensions/yt-dlp.json::DownloadUrl
    Endpoint {
        url: "https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest",
        source:
            "extensions/yt-dlp-aria2c-native.json::DownloadUrl, extensions/yt-dlp.json::DownloadUrl",
    },
    // extensions/ytdl-patched.json::DownloadUrl
    Endpoint {
        url: "https://api.github.com/repos/ytdl-patched/ytdl-patched/releases/latest",
        source: "extensions/ytdl-patched.json::DownloadUrl",
    },
    // D-021; legacy/src/engines/ffmpeg.cpp:96 and python.cpp:130 (Windows).
    Endpoint {
        url: "https://api.github.com/repos/mhogomchungu/packages/releases/latest",
        source: "D-021: legacy/src/engines/ffmpeg.cpp:96, python.cpp:130",
    },
];
