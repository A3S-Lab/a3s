//! Image preview helpers: half-block terminal rendering + clipboard capture.

use std::io::{self, Cursor};
use std::path::Path;
use std::process::Command;

use a3s_tui::style::{Color, Style};

/// A validated clipboard image backed by a unique temporary PNG.
///
/// Keeping the [`tempfile::TempPath`] alive lets the composer offer a RemoteUI
/// preview. Dropping the value removes the file automatically.
pub(super) struct CapturedClipboardImage {
    pub(super) path: tempfile::TempPath,
    pub(super) width: u32,
    pub(super) height: u32,
}

/// True if `path` looks like a previewable raster image.
pub(crate) fn is_image_path(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .as_deref(),
        Some("png" | "jpg" | "jpeg" | "gif" | "webp")
    )
}

/// Render an image as Unicode half-block lines — each line is one text row with
/// fg = the upper pixel and bg = the lower pixel, so it drops cleanly into the
/// line-based renderer and works in any truecolor terminal.
pub(crate) fn render_image_blocks(
    img: &image::DynamicImage,
    max_cols: usize,
    max_rows: usize,
) -> Vec<String> {
    use image::GenericImageView;
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 || max_cols == 0 || max_rows == 0 {
        return Vec::new();
    }
    // Two vertical pixels per text row.
    let scale = (max_cols as f32 / w as f32)
        .min((max_rows * 2) as f32 / h as f32)
        .clamp(0.001, 1.0);
    let nw = ((w as f32 * scale) as u32).max(1);
    let nh = ((h as f32 * scale) as u32).max(2);
    let rgba = img
        .resize_exact(nw, nh, image::imageops::FilterType::Triangle)
        .to_rgba8();
    let mut lines = Vec::new();
    let mut y = 0;
    while y < nh {
        let mut line = String::new();
        for x in 0..nw {
            let t = *rgba.get_pixel(x, y);
            let b = if y + 1 < nh {
                *rgba.get_pixel(x, y + 1)
            } else {
                t
            };
            line.push_str(
                &Style::new()
                    .fg(Color::Rgb(t[0], t[1], t[2]))
                    .bg(Color::Rgb(b[0], b[1], b[2]))
                    .render("▀"),
            );
        }
        lines.push(line);
        y += 2;
    }
    lines
}

/// Half-block preview of an image file, or `None` if it can't be decoded.
pub(crate) fn render_image_file(
    path: &Path,
    max_cols: usize,
    max_rows: usize,
) -> Option<Vec<String>> {
    let img = image::open(path).ok()?;
    Some(render_image_blocks(&img, max_cols, max_rows))
}

/// Capture and validate the native clipboard image as a uniquely named PNG.
pub(super) fn capture_clipboard_image() -> io::Result<CapturedClipboardImage> {
    let file = tempfile::Builder::new()
        .prefix("a3s-code-paste-")
        .suffix(".png")
        .tempfile()?;
    let path = file.into_temp_path();
    write_clipboard_image(&path)?;

    let (width, height) = normalize_clipboard_image(&path)?;
    Ok(CapturedClipboardImage {
        path,
        width,
        height,
    })
}

fn normalize_clipboard_image(path: &Path) -> io::Result<(u32, u32)> {
    let source = std::fs::read(path)?;
    let image = image::load_from_memory(&source).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("clipboard image could not be decoded: {error}"),
        )
    })?;
    if image.width() == 0 || image.height() == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "clipboard image has no pixels",
        ));
    }

    // Every provider receives the same well-defined media type, even when a
    // Linux clipboard helper returned a convertible source format.
    let mut encoded = Cursor::new(Vec::new());
    image
        .write_to(&mut encoded, image::ImageFormat::Png)
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("clipboard image could not be encoded as PNG: {error}"),
            )
        })?;
    let bytes = encoded.into_inner();
    std::fs::write(path, &bytes)?;
    Ok((image.width(), image.height()))
}

/// Compatibility helper retained for the focused image tests.
///
/// The destination is always truncated before capture and removed after any
/// failure, so stale bytes can never masquerade as a fresh clipboard image.
#[cfg(test)]
pub(crate) fn clipboard_image_to(dest: &Path) -> bool {
    write_clipboard_image(dest).is_ok()
}

fn write_clipboard_image(dest: &Path) -> io::Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    drop(
        std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(dest)?,
    );

    let result = write_native_clipboard_png(dest).and_then(|()| {
        let len = std::fs::metadata(dest)?.len();
        if len == 0 {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "clipboard does not contain an image",
            ))
        } else {
            Ok(())
        }
    });
    if result.is_err() {
        let _ = std::fs::remove_file(dest);
    }
    result
}

#[cfg(target_os = "macos")]
fn write_native_clipboard_png(dest: &Path) -> io::Result<()> {
    // Pass the destination as argv instead of interpolating it into AppleScript;
    // this is safe for spaces, quotes, and non-ASCII paths. Resetting EOF is
    // essential because `open for access` does not truncate an existing file.
    const SCRIPT: &str = r#"
on run argv
    set destination to POSIX file (item 1 of argv)
    set fileHandle to missing value
    try
        set pngData to the clipboard as «class PNGf»
        set fileHandle to open for access destination with write permission
        set eof fileHandle to 0
        write pngData to fileHandle
        close access fileHandle
    on error errorMessage number errorNumber
        if fileHandle is not missing value then
            try
                close access fileHandle
            end try
        end if
        error errorMessage number errorNumber
    end try
end run
"#;
    let output = Command::new("osascript")
        .arg("-e")
        .arg(SCRIPT)
        .arg(dest)
        .output()?;
    command_succeeded(output, "macOS clipboard does not contain a PNG image")
}

#[cfg(target_os = "linux")]
fn write_native_clipboard_png(dest: &Path) -> io::Result<()> {
    let candidates: [(&str, &[&str]); 2] = [
        ("wl-paste", &["--type", "image/png"]),
        (
            "xclip",
            &["-selection", "clipboard", "-t", "image/png", "-o"],
        ),
    ];
    let mut last_error = None;
    for (program, args) in candidates {
        match Command::new(program).args(args).output() {
            Ok(output) if output.status.success() && !output.stdout.is_empty() => {
                return std::fs::write(dest, output.stdout);
            }
            Ok(output) => {
                last_error = Some(command_error(
                    &output.stderr,
                    "clipboard does not contain a PNG image",
                ));
            }
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "install wl-clipboard or xclip to paste clipboard images",
        )
    }))
}

#[cfg(target_os = "windows")]
fn write_native_clipboard_png(dest: &Path) -> io::Result<()> {
    const SCRIPT: &str = r#"
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
if (-not [System.Windows.Forms.Clipboard]::ContainsImage()) { exit 2 }
$image = [System.Windows.Forms.Clipboard]::GetImage()
try { $image.Save($args[0], [System.Drawing.Imaging.ImageFormat]::Png) }
finally { $image.Dispose() }
"#;
    let mut last_error = None;
    for program in ["pwsh", "powershell"] {
        match Command::new(program)
            .args(["-NoProfile", "-STA", "-Command", SCRIPT])
            .arg(dest)
            .output()
        {
            Ok(output) if output.status.success() => return Ok(()),
            Ok(output) => {
                last_error = Some(command_error(
                    &output.stderr,
                    "Windows clipboard does not contain an image",
                ));
            }
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "PowerShell is required to paste clipboard images",
        )
    }))
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn write_native_clipboard_png(_dest: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "clipboard image paste is not supported on this platform",
    ))
}

#[cfg(target_os = "macos")]
fn command_succeeded(output: std::process::Output, fallback: &str) -> io::Result<()> {
    if output.status.success() {
        Ok(())
    } else {
        Err(command_error(&output.stderr, fallback))
    }
}

fn command_error(stderr: &[u8], fallback: &str) -> io::Error {
    let detail = String::from_utf8_lossy(stderr);
    let detail = detail.trim();
    io::Error::new(
        io::ErrorKind::InvalidData,
        if detail.is_empty() {
            fallback.to_string()
        } else {
            detail.to_string()
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipboard_normalization_validates_and_converts_to_png() {
        let file = tempfile::Builder::new().suffix(".png").tempfile().unwrap();
        let source = ::image::DynamicImage::ImageRgb8(::image::RgbImage::from_pixel(
            7,
            5,
            ::image::Rgb([10, 20, 30]),
        ));
        let mut jpeg = Cursor::new(Vec::new());
        source
            .write_to(&mut jpeg, ::image::ImageFormat::Jpeg)
            .unwrap();
        std::fs::write(file.path(), jpeg.into_inner()).unwrap();

        assert_eq!(normalize_clipboard_image(file.path()).unwrap(), (7, 5));
        let normalized = std::fs::read(file.path()).unwrap();
        assert_eq!(
            ::image::guess_format(&normalized).unwrap(),
            ::image::ImageFormat::Png
        );
    }

    #[test]
    fn clipboard_normalization_rejects_non_image_bytes() {
        let file = tempfile::Builder::new().suffix(".png").tempfile().unwrap();
        std::fs::write(file.path(), b"not an image").unwrap();
        assert_eq!(
            normalize_clipboard_image(file.path()).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }
}
