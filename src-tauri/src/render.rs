use std::io::Cursor;
use std::path::Path;

use image::imageops::FilterType;
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

pub const PORTRAIT_SIZE: (u32, u32) = (1200, 1800);
pub const LANDSCAPE_SIZE: (u32, u32) = (1800, 1200);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PrintLayout {
    #[default]
    Cover,
    Fit,
}

impl PrintLayout {
    pub fn as_str(self) -> &'static str {
        match self {
            PrintLayout::Cover => "cover",
            PrintLayout::Fit => "fit",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewPaths {
    pub portrait_cover: String,
    pub portrait_fit: String,
    pub landscape_cover: String,
    pub landscape_fit: String,
}

pub async fn render_previews(
    prep_id: &str,
    cover: Vec<u8>,
    previews_dir: &Path,
) -> AppResult<PreviewPaths> {
    tokio::fs::create_dir_all(previews_dir).await?;
    let portrait_cover = previews_dir.join(format!("{prep_id}-portrait-cover.png"));
    let portrait_fit = previews_dir.join(format!("{prep_id}-portrait-fit.png"));
    let landscape_cover = previews_dir.join(format!("{prep_id}-landscape-cover.png"));
    let landscape_fit = previews_dir.join(format!("{prep_id}-landscape-fit.png"));

    let targets = [
        portrait_cover.clone(),
        portrait_fit.clone(),
        landscape_cover.clone(),
        landscape_fit.clone(),
    ];
    tokio::task::spawn_blocking(move || {
        render_to_files(&cover, &targets[0], &targets[1], &targets[2], &targets[3])
    })
    .await
    .map_err(join_error)??;

    Ok(PreviewPaths {
        portrait_cover: portrait_cover.to_string_lossy().to_string(),
        portrait_fit: portrait_fit.to_string_lossy().to_string(),
        landscape_cover: landscape_cover.to_string_lossy().to_string(),
        landscape_fit: landscape_fit.to_string_lossy().to_string(),
    })
}

fn join_error(err: tokio::task::JoinError) -> AppError {
    AppError::internal(format!("render task failed: {err}"))
}

fn render_to_files(
    bytes: &[u8],
    portrait_cover: &Path,
    portrait_fit: &Path,
    landscape_cover: &Path,
    landscape_fit: &Path,
) -> AppResult<()> {
    let image = decode(bytes)?;
    save_image(&resize_to_fill(&image, PORTRAIT_SIZE), portrait_cover)?;
    save_image(&fit_to_canvas(&image, PORTRAIT_SIZE), portrait_fit)?;
    save_image(&resize_to_fill(&image, LANDSCAPE_SIZE), landscape_cover)?;
    save_image(&fit_to_canvas(&image, LANDSCAPE_SIZE), landscape_fit)?;
    Ok(())
}

fn save_image(image: &DynamicImage, path: &Path) -> AppResult<()> {
    image
        .save_with_format(path, ImageFormat::Png)
        .map_err(|e| AppError::internal(format!("could not write preview: {e}")))
}

pub fn decode(bytes: &[u8]) -> AppResult<DynamicImage> {
    image::load_from_memory(bytes)
        .map_err(|e| AppError::internal(format!("could not decode image: {e}")))
}

pub fn resize_to_fill(image: &DynamicImage, size: (u32, u32)) -> DynamicImage {
    image.resize_to_fill(size.0, size.1, FilterType::Lanczos3)
}

pub fn fit_to_canvas(image: &DynamicImage, size: (u32, u32)) -> DynamicImage {
    let fitted = image.resize(size.0, size.1, FilterType::Lanczos3);
    let mut canvas = DynamicImage::ImageRgba8(RgbaImage::from_pixel(
        size.0,
        size.1,
        Rgba([255, 255, 255, 255]),
    ));
    let x = (size.0.saturating_sub(fitted.width()) / 2) as i64;
    let y = (size.1.saturating_sub(fitted.height()) / 2) as i64;
    image::imageops::overlay(&mut canvas, &fitted, x, y);
    canvas
}

pub fn image_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()
}

pub fn save_bytes_as_png(bytes: &[u8], path: &Path) -> AppResult<()> {
    decode(bytes)?
        .save_with_format(path, ImageFormat::Png)
        .map_err(|e| AppError::internal(format!("could not write image: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic() -> Vec<u8> {
        let mut img = RgbaImage::new(100, 50);
        for (x, _y, pixel) in img.enumerate_pixels_mut() {
            *pixel = if x < 50 {
                Rgba([255, 0, 0, 255])
            } else {
                Rgba([0, 0, 255, 255])
            };
        }
        let mut bytes = Vec::new();
        DynamicImage::ImageRgba8(img)
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        bytes
    }

    #[test]
    fn exact_output_dimensions() {
        let bytes = synthetic();
        let portrait = resize_to_fill(&decode(&bytes).unwrap(), PORTRAIT_SIZE);
        assert_eq!((portrait.width(), portrait.height()), PORTRAIT_SIZE);
        let landscape = resize_to_fill(&decode(&bytes).unwrap(), LANDSCAPE_SIZE);
        assert_eq!((landscape.width(), landscape.height()), LANDSCAPE_SIZE);
        let portrait_fit = fit_to_canvas(&decode(&bytes).unwrap(), PORTRAIT_SIZE);
        assert_eq!((portrait_fit.width(), portrait_fit.height()), PORTRAIT_SIZE);
        let landscape_fit = fit_to_canvas(&decode(&bytes).unwrap(), LANDSCAPE_SIZE);
        assert_eq!(
            (landscape_fit.width(), landscape_fit.height()),
            LANDSCAPE_SIZE
        );
    }

    #[test]
    fn centered_crop_keeps_colour_symmetry() {
        let bytes = synthetic();
        let cropped = resize_to_fill(&decode(&bytes).unwrap(), (20, 20)).to_rgba8();
        let left = cropped.get_pixel(2, 10).0;
        let right = cropped.get_pixel(17, 10).0;
        assert!(left[0] > 200 && left[2] < 40, "left edge should stay red");
        assert!(
            right[2] > 200 && right[0] < 40,
            "right edge should stay blue"
        );
    }

    #[test]
    fn fit_keeps_the_full_cover_on_a_white_canvas() {
        let bytes = synthetic();
        let fitted = fit_to_canvas(&decode(&bytes).unwrap(), (20, 20)).to_rgba8();
        assert_eq!(fitted.get_pixel(10, 1).0, [255, 255, 255, 255]);
        let left = fitted.get_pixel(2, 10).0;
        let right = fitted.get_pixel(17, 10).0;
        assert!(left[0] > 200 && left[2] < 40, "left edge should stay red");
        assert!(
            right[2] > 200 && right[0] < 40,
            "right edge should stay blue"
        );
    }

    #[tokio::test]
    async fn renders_all_four_previews_to_disk() {
        let dir = tempfile::tempdir().unwrap();
        let paths = render_previews("abc", synthetic(), dir.path())
            .await
            .unwrap();
        let checks = [
            (&paths.portrait_cover, PORTRAIT_SIZE),
            (&paths.portrait_fit, PORTRAIT_SIZE),
            (&paths.landscape_cover, LANDSCAPE_SIZE),
            (&paths.landscape_fit, LANDSCAPE_SIZE),
        ];
        for (path, expected) in checks {
            let image = image::open(path).unwrap();
            assert_eq!((image.width(), image.height()), expected);
        }
    }

    #[test]
    fn rejects_garbage() {
        assert!(decode(b"not an image").is_err());
    }
}
