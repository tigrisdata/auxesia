use crate::{content::Site, render::Route};
use image::{
    ExtendedColorType, ImageEncoder, ImageReader, RgbaImage,
    codecs::{
        ico::{IcoEncoder, IcoFrame},
        png::PngEncoder,
    },
    imageops::{FilterType, overlay},
};
use std::{
    collections::BTreeMap,
    io::Cursor,
    sync::atomic::{AtomicBool, Ordering},
};

pub(crate) fn links() -> maud::Markup {
    maud::html! {
        link rel="icon" href="/_auxesia/gen/favicon.ico" sizes="16x16 32x32 48x48";
        link rel="icon" type="image/png" sizes="16x16" href="/_auxesia/gen/favicon-16x16.png";
        link rel="icon" type="image/png" sizes="32x32" href="/_auxesia/gen/favicon-32x32.png";
        link rel="apple-touch-icon" sizes="180x180" href="/_auxesia/gen/apple-touch-icon.png";
    }
}

pub(crate) fn generate(site: &Site, routes: &mut BTreeMap<String, Route>) -> Result<(), String> {
    let Some(path) = &site.config.favicon else {
        return Ok(());
    };
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let mut reader =
            ImageReader::new(Cursor::new(&site.files[path].bytes)).with_guessed_format()?;
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(4096);
        limits.max_image_height = Some(4096);
        limits.max_alloc = Some(64 * 1024 * 1024);
        reader.limits(limits.clone());
        let (source, svg) = if reader.format().is_some() {
            (Some(reader.decode()?), None)
        } else {
            let invalid_image = AtomicBool::new(false);
            let rejected = &invalid_image;
            let mut options = resvg::usvg::Options::default();
            let resolve_data = resvg::usvg::ImageHrefResolver::default_data_resolver();
            options.image_href_resolver.resolve_data = Box::new(move |mime, data, options| {
                let kind = resolve_data(mime, data, options)?;
                let bytes = match &kind {
                    resvg::usvg::ImageKind::JPEG(bytes)
                    | resvg::usvg::ImageKind::PNG(bytes)
                    | resvg::usvg::ImageKind::GIF(bytes)
                    | resvg::usvg::ImageKind::WEBP(bytes) => bytes,
                    resvg::usvg::ImageKind::SVG(_) => return Some(kind),
                };
                let decoded = (|| -> image::ImageResult<()> {
                    let mut reader =
                        ImageReader::new(Cursor::new(bytes.as_slice())).with_guessed_format()?;
                    reader.limits(limits.clone());
                    reader.decode()?;
                    Ok(())
                })();
                if decoded.is_err() {
                    rejected.store(true, Ordering::Relaxed);
                    return None;
                }
                Some(kind)
            });
            // Only captured bytes may supply images, never files from the host.
            options.image_href_resolver.resolve_string = Box::new(|_, _| None);
            options.fontdb_mut().load_system_fonts();
            let tree = resvg::usvg::Tree::from_data(&site.files[path].bytes, &options)?;
            if invalid_image.load(Ordering::Relaxed) {
                return Err("embedded SVG image is invalid or exceeds decoding limits".into());
            }
            (None, Some(tree))
        };
        let mut frames = Vec::new();
        for size in [16, 32, 48, 180] {
            let square = if let Some(tree) = &svg {
                let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size)
                    .ok_or("could not allocate favicon image")?;
                let width = tree.size().width();
                let height = tree.size().height();
                let scale = (size as f32 / width).min(size as f32 / height);
                let transform = resvg::tiny_skia::Transform::from_row(
                    scale,
                    0.0,
                    0.0,
                    scale,
                    (size as f32 - width * scale) / 2.0,
                    (size as f32 - height * scale) / 2.0,
                );
                resvg::render(tree, transform, &mut pixmap.as_mut());
                // tiny-skia stores premultiplied alpha; PNG needs straight alpha.
                RgbaImage::from_fn(size, size, |x, y| {
                    let pixel = pixmap.pixel(x, y).unwrap().demultiply();
                    image::Rgba([pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()])
                })
            } else {
                let resized = source
                    .as_ref()
                    .unwrap()
                    .resize(size, size, FilterType::Lanczos3)
                    .to_rgba8();
                let mut square = RgbaImage::new(size, size);
                overlay(
                    &mut square,
                    &resized,
                    ((size - resized.width()) / 2).into(),
                    ((size - resized.height()) / 2).into(),
                );
                square
            };
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
        format!("/_auxesia/gen/{name}"),
        Route::Blob {
            bytes: bytes.into(),
            content_type: mime.into(),
            download: None,
        },
    );
}
