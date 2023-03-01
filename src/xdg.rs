use cached::proc_macro::cached;
use color_eyre::eyre::Result;
use linicon;
use regex::Regex;
use resvg::render;
use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{FitTo, Tree};
use std::ffi::OsString;
use std::fs::create_dir_all;
use std::io::{BufRead, BufReader, ErrorKind};
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::{env, fs};
use walkdir::WalkDir;
fn get_data_dirs() -> Vec<PathBuf> {
	let xdg_data_dirs_str = std::env::var("XDG_DATA_DIRS").unwrap_or_default();

	let xdg_data_dirs = xdg_data_dirs_str
		.split(":")
		.filter_map(|dir| PathBuf::from_str(dir).ok());

	let data_home = dirs::home_dir()
		.unwrap_or(PathBuf::from_str("/usr/share/").expect(
			"No XDG_DATA_DIR set, no HOME directory found and no /usr/share direcotry found",
		))
		.join(".local")
		.join("share");

	xdg_data_dirs
		.chain([data_home].into_iter())
		.filter(|dir| dir.exists() && dir.is_dir())
		.collect()
}

fn get_app_dirs() -> Vec<PathBuf> {
	get_data_dirs()
		.into_iter()
		.map(|dir| dir.join("applications"))
		.filter(|dir| dir.exists() && dir.is_dir())
		.collect()
}

pub fn get_desktop_files() -> Vec<PathBuf> {
	let desktop_extension = OsString::from_str("desktop").unwrap();
	// Get the list of directories to search
	let app_dirs = get_app_dirs();
	app_dirs
		.into_iter()
		.flat_map(|dir| {
			// Follow symlinks and recursively search directories
			WalkDir::new(dir)
				.follow_links(true)
				.into_iter()
				.filter_map(|entry| entry.ok())
				.filter(|entry| entry.file_type().is_file())
				.map(|entry| entry.path().to_path_buf())
		})
		.filter(|path| path.extension() == Some(&desktop_extension))
		.collect::<Vec<PathBuf>>()
}

#[test]
fn test_get_desktop_files() {
	let desktop_files = get_desktop_files();
	dbg!(&desktop_files);
	assert!(desktop_files
		.iter()
		.any(|file| file.ends_with("gimp.desktop")));
}

pub fn parse_desktop_file(path: PathBuf) -> Result<DesktopFile, String> {
	// Open the file in read-only mode
	let file = match fs::File::open(
		env::current_dir()
			.map_err(|e| e.to_string())?
			.join(path.clone()),
	) {
		Ok(file) => file,
		Err(err) => return Err(format!("Failed to open file: {}", err)),
	};

	let reader = BufReader::new(file);

	// Create temporary variables to hold the parsed values
	let mut name = None;
	let mut command = None;
	let mut categories = Vec::new();
	let mut icon = None;
	let mut no_display = false;
	let mut desktop_entry_found = false;

	let re = Regex::new(r"^\[([^\]]*)\]$").unwrap();

	// Loop through each line of the file
	for line in reader.lines() {
		let line = match line {
			Ok(line) => line,
			Err(err) => return Err(format!("Failed to read line: {}", err)),
		};

		// Skip empty lines and lines that start with "#" (comments)
		if line.is_empty() || line.starts_with('#') {
			continue;
		}

		if let Some(captures) = re.captures(&line) {
			let entry = captures.get(1).unwrap();
			desktop_entry_found = entry.as_str().contains("Desktop Entry");
		}

		if !desktop_entry_found {
			continue;
		}
		// Split the line into a key-value pair by looking for the first "=" character
		let parts = line.split_once('=');
		let (key, value) = match parts {
			Some((key, value)) => (key, value),
			None => continue,
		};

		// Parse the key-value pair based on the key
		match key {
			"Name" => name = Some(value.to_string()),
			"Exec" => command = Some(value.to_string()),
			"Categories" => {
				categories = value
					.split(';')
					.map(|s| s.to_string())
					.filter(|s| !s.is_empty())
					.collect()
			}
			"Icon" => icon = Some(value.to_string()),
			"NoDisplay" => {
				no_display = match value {
					"true" => true,
					_ => false,
				}
			}
			_ => (), // Ignore unknown keys
		}
	}

	// Create and return a new DesktopFile instance with the parsed values
	Ok(DesktopFile {
		path,
		name,
		command,
		categories,
		icon,
		no_display,
	})
}

#[test]
fn test_parse_desktop_file() {
	// Create a temporary directory and a test desktop file
	let dir = tempdir::TempDir::new("test").unwrap();
	let file = dir.path().join("test.desktop");
	let data = "[Desktop Entry]\nName=Test\nExec=test\nCategories=A;B;C\nIcon=test.png";
	fs::write(&file, data).unwrap();

	// Parse the test desktop file
	let desktop_file = parse_desktop_file(file).unwrap();

	// Check the parsed values
	assert_eq!(desktop_file.name, Some("Test".to_string()));
	assert_eq!(desktop_file.command, Some("test".to_string()));
	assert_eq!(
		desktop_file.categories,
		vec!["A".to_string(), "B".to_string(), "C".to_string()]
	);
	assert_eq!(desktop_file.icon, Some("test.png".to_string()));
}

#[derive(Debug, Clone)]
pub struct DesktopFile {
	path: PathBuf,
	pub name: Option<String>,
	pub command: Option<String>,
	pub categories: Vec<String>,
	pub icon: Option<String>,
	pub no_display: bool,
}
impl DesktopFile {
	pub fn get_raw_icons(&self) -> Vec<Icon> {
		// Get the name of the icon from the DesktopFile struct
		let Some(icon_name) = self.icon.as_ref() else { return Vec::new(); };
		let test_icon_path = self.path.join(Path::new(icon_name));
		if test_icon_path.exists() {
			if let Some(icon) = Icon::from_path(test_icon_path, 128) {
				return vec![icon];
			}
		}

		let cache_icon_path = get_image_cache_dir().join(icon_name).canonicalize();
		if cache_icon_path.is_ok() {
			return vec![Icon::from_path(cache_icon_path.unwrap(), 128).unwrap()];
		}

		let mut icons_iter = linicon::lookup_icon(icon_name)
			.use_fallback_themes(false)
			.peekable();

		if icons_iter.peek().is_none() {
			//dbg!("No icons found in current theme");
			icons_iter = linicon::lookup_icon(icon_name).peekable();
		}

		let sized_png: Vec<Icon> = icons_iter
			.filter_map(|i| i.ok())
			.filter(|i| i.icon_type != linicon::IconType::XMP) //TODO: support XMP
			.map(|i| Icon::from_path(i.path, i.max_size - 2).unwrap())
			.collect();
		sized_png
	}
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Icon {
	pub icon_type: IconType,
	pub path: PathBuf,
	pub size: u16,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum IconType {
	Png,
	Svg,
	Gltf,
}
impl Icon {
	pub fn from_path(path: PathBuf, size: u16) -> Option<Icon> {
		let icon_type = match path.extension().and_then(|ext| ext.to_str()) {
			Some("png") => Some(IconType::Png),
			Some("svg") => Some(IconType::Svg),
			Some("glb") | Some("gltf") => Some(IconType::Gltf),
			_ => return None,
		}
		.unwrap();
		return Some(Icon {
			icon_type,
			path,
			size,
		});
	}

	pub fn cached_process(self, size: u16) -> Result<Icon, std::io::Error> {
		let new_path =
			get_image_cache_dir().join(self.path.with_extension("").file_name().unwrap());
		if !new_path.exists() {
			_ = symlink(self.path.clone(), new_path);
		}
		match self.icon_type {
			IconType::Svg => Ok(Icon::from_path(get_png_from_svg(self.path, size)?, size).unwrap()),
			_ => Ok(self),
		}
	}
}

#[test]
fn test_get_icon_path() {
	// Create an instance of the DesktopFile struct with some dummy data
	let desktop_file = DesktopFile {
		path: PathBuf::new(),
		name: None,
		command: None,
		categories: vec![],
		icon: Some("krita".into()),
		no_display: false,
	};

	// Call the get_icon_path() function with a size argument and store the result
	let icon_paths = desktop_file.get_raw_icons();
	dbg!(&icon_paths);

	// Assert that the get_icon_path() function returns the expected result
	assert!(icon_paths.contains(
		&Icon::from_path(
			PathBuf::from("/usr/share/icons/hicolor/32x32/apps/krita.png"),
			32
		)
		.unwrap()
	));
}

#[cached]
pub fn get_image_cache_dir() -> PathBuf {
	let cache_dir;
	if let Ok(xdg_cache_home) = std::env::var("XDG_CACHE_HOME") {
		cache_dir =
			PathBuf::from_str(&xdg_cache_home).unwrap_or(dirs::home_dir().unwrap().join(".cache"))
	} else {
		cache_dir = dirs::home_dir().unwrap().join(".cache");
	}
	let image_cache_dir = cache_dir.join("protostar_icon_cache");
	create_dir_all(&image_cache_dir).expect("Could not create image cache directory");
	return image_cache_dir;
}

pub fn get_png_from_svg(svg_path: impl AsRef<Path>, size: u16) -> Result<PathBuf, std::io::Error> {
	let svg_path = fs::canonicalize(svg_path)?;
	let svg_data = fs::read(svg_path.as_path())?;
	let tree = Tree::from_data(svg_data.as_slice(), &resvg::usvg::Options::default())
		.map_err(|_| ErrorKind::InvalidData)?;

	let png_path = get_image_cache_dir().join(format!(
		"{}-{}.png",
		svg_path.file_name().unwrap().to_str().unwrap(),
		svg_data.len()
	));

	if png_path.exists() {
		return Ok(png_path);
	}

	let mut pixmap = Pixmap::new(size.into(), size.into()).unwrap();
	render(
		&tree,
		FitTo::Width(size.into()),
		Transform::identity(),
		pixmap.as_mut(),
	);
	pixmap
		.save_png(&png_path)
		.map_err(|_| ErrorKind::InvalidData)?;
	Ok(png_path)
}
#[test]
fn test_render_svg_to_png() {
	use image::GenericImageView;
	// Create temporary input and output paths
	let svg_path = env::current_dir().unwrap().join("test_input.svg");

	// Write some test SVG data to the input path
	let test_svg_data = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 100 100\">
	<ellipse cx=\"50\" cy=\"80\" rx=\"46\" ry=\"19\" fill=\"#07c\"/>
	<path d=\"M43,0c-6,25,16,22,1,52c11,3,19,0,19-22c38,18,16,63-12,64c-25,2-55-39-8-94\" fill=\"#e34\"/>
	<path d=\"M34,41c-6,39,29,32,33,7c39,42-69,63-33-7\" fill=\"#fc2\"/>
</svg>";
	fs::write(&svg_path, test_svg_data).unwrap();

	// Call the function with the test input and output paths and a size of 200
	let png_path = get_png_from_svg(&svg_path, 200).unwrap();
	dbg!(&png_path);

	// Check that the output file exists
	assert!(png_path.exists());

	// Check that the output file is a PNG file
	assert_eq!(png_path.extension().unwrap(), "png");

	// Check that the output file has the expected dimensions
	let output_image = image::open(&png_path).unwrap();
	assert_eq!(output_image.dimensions(), (200, 200));

	// Delete the temporary input and output files
	fs::remove_file(&svg_path).unwrap();
	fs::remove_file(&png_path).unwrap();
}
