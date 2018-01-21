extern crate image;
extern crate walkdir;
extern crate rayon;
extern crate structopt;
#[macro_use]
extern crate structopt_derive;

use structopt::StructOpt;
use image::{DynamicImage, RgbImage, FilterType};
use walkdir::WalkDir;
use rayon::iter::IntoParallelRefMutIterator;
use rayon::iter::ParallelIterator;

use std::path::PathBuf;
use std::fs::remove_file;

#[derive(StructOpt)]
struct Options {
    #[structopt(help = "Input directory")]
    directory: String,
    #[structopt(short = "t", long = "threshold", help = "Theshold for image grouping", default_value = "10")]
    threshold: f32,
    #[structopt(short = "d", long = "delete", help = "Delete similar files to the first one")]
    delete: bool
}

fn main() {
    let opt = Options::from_args();
    let mut images: Vec<(ImageThumbnail, Vec<PathBuf>)> = Vec::new();

    // Iterate over files in the directory
    WalkDir::new(&opt.directory).into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
        // Map to and print out the path
        .map(|entry| {
            let path = entry.path().to_path_buf();
            println!("{}", path.display());
            path
        })
        // Filter map to the thumbnails of readable images
        .filter_map(|path| 
            image::open(&path).ok()
                .map(|image| (path, ImageThumbnail::new(&image)))
        )
        .for_each(|(path, image)| {
            let found = {
                // Find the paths of a similar-enough image in parallel
                let mut paths = images.par_iter_mut()
                    .find_any(|&&mut (ref original, _)| original.difference(&image) <= opt.threshold)
                    .map(|&mut (_, ref mut paths)| paths);

                // Push the path to the paths if they exist
                if let Some(ref mut paths) = paths {
                    paths.push(path.clone());
                }

                paths.is_some()
            };

            // Or push the image and a new path vector
            if !found {
                images.push((image, vec![path]));
            }
        });

    images.iter()
        // Filter to image groups with more than one path
        .filter(|&&(_, ref paths)| paths.len() > 1)
        // Flat map to the paths in the group along with their index
        .flat_map(|&(_, ref paths)| {
            println!("-----");
            paths.iter().enumerate()
        })
        .for_each(|(i, path)| {
            // Print each path
            println!("{}", path.display());

            // and remove it if it's not the first
            if opt.delete && i > 0 {
                remove_file(path).unwrap();
            }
        });
}

struct ImageThumbnail {
    inner: RgbImage
}

impl ImageThumbnail {
    const SIZE: u32 = 32;
    // The total number of channels in the image
    const TOTAL_CHANNELS: u32 = Self::SIZE * Self::SIZE * 3;

    fn new(dynamic: &DynamicImage) -> Self {
        Self {
            inner: dynamic.resize_exact(Self::SIZE, Self::SIZE, FilterType::Triangle).to_rgb()
        }
    }
    
    // Calculate the the difference between two thumbnails
    fn difference(&self, other: &ImageThumbnail) -> f32 {
        // Zip two iterators of pixels together
        self.inner.pixels()
            .zip(other.inner.pixels())
            // Flat map to pixel channels
            .flat_map(|(a, b)| a.data.iter().zip(b.data.iter()))
            // Map to channel difference as f32
            .map(|(a, b)| (f32::from(*a) - f32::from(*b)).abs())
            // Divide by number of channels and rescale to 0 -> 100
            .sum::<f32>() / Self::TOTAL_CHANNELS as f32 / 255.0 * 100.0
    }
}
