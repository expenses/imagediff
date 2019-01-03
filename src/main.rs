extern crate image;
extern crate walkdir;
extern crate rayon;
extern crate structopt;
extern crate structopt_derive;

use structopt::StructOpt;
use image::{DynamicImage, RgbImage, FilterType};
use walkdir::WalkDir;
use rayon::iter::{ParallelIterator, ParallelBridge};

use std::path::PathBuf;
use std::fs::remove_file;
use std::sync::*;

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
    let images: RwLock<Vec<(ImageThumbnail, Vec<PathBuf>)>> = RwLock::new(Vec::new());

    // Iterate over files in the directory
    WalkDir::new(&opt.directory).into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
        .enumerate()
        .par_bridge()
        // Map to and print out the path
        .map(|(i, entry)| {
            let path = entry.path().to_path_buf();
            println!("{} {}", i, path.display());
            path
        })
        // Filter map to the thumbnails of readable images
        .filter_map(|path| 
            image::open(&path).ok()
                .map(|image| (path, ImageThumbnail::new(&image)))
        )
        .for_each(|(path, image)| {
            // Find the paths of a similar-enough image in parallel
            let index = images.read().unwrap().iter()
                .enumerate()
                .find(|(_, &(ref original, _))| original.difference(&image) <= opt.threshold)
                .map(|(index, _)| index);

            // Push the path to the paths if they exist
            if let Some(index) = index {
                images.write().unwrap()[index].1.push(path.clone());
            }

            let found = index.is_some();

            // Or push the image and a new path vector
            if !found {
                images.write().unwrap().push((image, vec![path]));
            }
        });

    images.read().unwrap().iter()
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
        // Zip two iterators of pixel channels together
        self.inner.iter()
            .zip(other.inner.iter())
            // Map to channel difference as f32
            .map(|(&a, &b)| (f32::from(a) - f32::from(b)).abs())
            // Divide by number of channels and rescale to 0 -> 100
            .sum::<f32>() / Self::TOTAL_CHANNELS as f32 / 255.0 * 100.0
    }
}
