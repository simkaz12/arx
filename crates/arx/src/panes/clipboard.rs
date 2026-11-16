use std::process::Command;

#[derive(Debug, Clone, PartialEq)]
pub enum Clip {
    Text(String),
    Image { media_type: String, bytes: Vec<u8> },
}

pub trait Clipboard {
    fn read(&self) -> Result<Clip, String>;
}

const IMAGE_TYPES: [&str; 4] = ["image/png", "image/jpeg", "image/gif", "image/webp"];

pub struct WlPasteClipboard;

impl Clipboard for WlPasteClipboard {
    fn read(&self) -> Result<Clip, String> {
        let list = Command::new("wl-paste")
            .arg("--list-types")
            .output()
            .map_err(|err| err.to_string())?;
        if !list.status.success() {
            return Err("wl-paste not available".to_string());
        }
        let available = String::from_utf8_lossy(&list.stdout);
        let available: Vec<&str> = available.lines().map(str::trim).collect();

        if let Some(media_type) = IMAGE_TYPES.iter().find(|t| available.contains(t)) {
            let out =
                Command::new("wl-paste").arg("--type").arg(media_type).output().map_err(|err| err.to_string())?;
            if !out.status.success() {
                return Err(format!("wl-paste --type {media_type} failed"));
            }
            return Ok(Clip::Image { media_type: media_type.to_string(), bytes: out.stdout });
        }

        let out = Command::new("wl-paste").arg("--no-newline").output().map_err(|err| err.to_string())?;
        if !out.status.success() {
            return Err("clipboard empty".to_string());
        }
        let text = String::from_utf8_lossy(&out.stdout).into_owned();
        if text.is_empty() {
            return Err("clipboard empty".to_string());
        }
        Ok(Clip::Text(text))
    }
}

pub fn image_extension(media_type: &str) -> &'static str {
    match media_type {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => "bin",
    }
}

#[cfg(test)]
pub struct FakeClipboard(pub Result<Clip, String>);

#[cfg(test)]
impl Clipboard for FakeClipboard {
    fn read(&self) -> Result<Clip, String> {
        self.0.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_extension_maps_the_four_supported_types_and_falls_back_for_anything_else() {
        assert_eq!(image_extension("image/png"), "png");
        assert_eq!(image_extension("image/jpeg"), "jpg");
        assert_eq!(image_extension("image/gif"), "gif");
        assert_eq!(image_extension("image/webp"), "webp");
        assert_eq!(image_extension("image/tiff"), "bin");
    }
}
