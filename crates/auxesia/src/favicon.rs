use crate::{content::Site, render::Route};
use image::{
    ExtendedColorType, ImageEncoder, ImageReader, RgbaImage,
    codecs::{
        ico::{IcoEncoder, IcoFrame},
        png::PngEncoder,
    },
    imageops::{FilterType, overlay},
};
use std::{collections::BTreeMap, io::Cursor};

pub(crate) const LINKS: &str = concat!(
    "<link rel=\"icon\" href=\"/.auxesia/gen/favicon.ico\" sizes=\"16x16 32x32 48x48\">",
    "<link rel=\"icon\" type=\"image/png\" sizes=\"16x16\" href=\"/.auxesia/gen/favicon-16x16.png\">",
    "<link rel=\"icon\" type=\"image/png\" sizes=\"32x32\" href=\"/.auxesia/gen/favicon-32x32.png\">",
    "<link rel=\"apple-touch-icon\" sizes=\"180x180\" href=\"/.auxesia/gen/apple-touch-icon.png\">",
);

pub(crate) fn generate(site: &Site, routes: &mut BTreeMap<String, Route>) -> Result<(), String> {
    let Some(path) = &site.config.favicon else {
        return Ok(());
    };
    let result = (|| -> image::ImageResult<()> {
        let mut reader =
            ImageReader::new(Cursor::new(&site.files[path].bytes)).with_guessed_format()?;
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(4096);
        limits.max_image_height = Some(4096);
        limits.max_alloc = Some(64 * 1024 * 1024);
        reader.limits(limits);
        let source = reader.decode()?;
        let mut frames = Vec::new();
        for size in [16, 32, 48, 180] {
            let resized = source.resize(size, size, FilterType::Lanczos3).to_rgba8();
            let mut square = RgbaImage::new(size, size);
            overlay(
                &mut square,
                &resized,
                ((size - resized.width()) / 2).into(),
                ((size - resized.height()) / 2).into(),
            );
            let mut png = Vec::new();
            PngEncoder::new(&mut png).write_image(&square, size, size, ExtendedColorType::Rgba8)?;
            if size <= 48 {
                frames.push(IcoFrame::with_encoded(
                    png.clone(),
                    size,
                    size,
                    ExtendedColorType::Rgba8,
                )?);
            }
            let name = match size {
                16 => "favicon-16x16.png",
                32 => "favicon-32x32.png",
                180 => "apple-touch-icon.png",
                _ => continue,
            };
            insert(routes, name, png, "image/png");
        }
        let mut ico = Vec::new();
        IcoEncoder::new(&mut ico).encode_images(&frames)?;
        insert(routes, "favicon.ico", ico, "image/x-icon");
        Ok(())
    })();
    result.map_err(|e| format!(".auxesia.yaml: favicon {path:?}: {e}"))
}

fn insert(routes: &mut BTreeMap<String, Route>, name: &str, bytes: Vec<u8>, mime: &str) {
    routes.insert(
        format!("/.auxesia/gen/{name}"),
        Route::Blob {
            bytes: bytes.into(),
            content_type: mime.into(),
            download: None,
        },
    );
}
