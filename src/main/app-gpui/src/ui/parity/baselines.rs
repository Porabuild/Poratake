use std::path::Path;

fn compare(actual: &image::RgbaImage, expected: &image::RgbaImage) -> anyhow::Result<()> {
    anyhow::ensure!(
        actual.dimensions() == expected.dimensions(),
        "image dimensions changed"
    );
    let changed = actual
        .pixels()
        .zip(expected.pixels())
        .filter(|(actual, expected)| actual != expected)
        .count();
    anyhow::ensure!(changed == 0, "{changed} pixels changed");
    Ok(())
}

pub(super) fn check(output: &Path) {
    let Some(baselines) = std::env::var_os("PORATAKE_PARITY_BASELINES") else {
        return;
    };
    let baselines = std::path::PathBuf::from(baselines);
    #[cfg(windows)]
    let baselines = baselines.join(super::windows_renderer::baseline_profile());
    let update = std::env::var("PORATAKE_PARITY_UPDATE_BASELINES").as_deref() == Ok("1");
    if update {
        std::fs::create_dir_all(&baselines).unwrap();
    }
    let mut files: Vec<_> = std::fs::read_dir(output)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension().is_some_and(|extension| extension == "png")
                && path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("gpui-")
        })
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no visual evidence was captured");
    if update {
        for entry in std::fs::read_dir(&baselines).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|extension| extension == "png")
                && path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("gpui-")
                && !files
                    .iter()
                    .any(|file| file.file_name() == path.file_name())
            {
                std::fs::remove_file(path).unwrap();
            }
        }
    } else {
        let mut expected: Vec<_> = std::fs::read_dir(&baselines).unwrap_or_else(|error| panic!("missing visual baselines {}: {error}; record and review them with --update-baselines", baselines.display()))
            .map(|entry| entry.unwrap().file_name())
            .filter(|name| name.to_string_lossy().starts_with("gpui-") && Path::new(name).extension().is_some_and(|extension| extension == "png"))
            .collect();
        expected.sort();
        assert_eq!(
            files
                .iter()
                .map(|path| path.file_name().unwrap().to_os_string())
                .collect::<Vec<_>>(),
            expected,
            "visual state coverage changed; review the captured and baseline state lists"
        );
    }
    let mut failures = Vec::new();
    for actual_path in &files {
        let expected_path = baselines.join(actual_path.file_name().unwrap());
        if update {
            std::fs::copy(actual_path, expected_path).unwrap();
            continue;
        }
        let actual = image::open(actual_path).unwrap().to_rgba8();
        let result = image::open(&expected_path)
            .map_err(anyhow::Error::from)
            .and_then(|expected| {
                let expected = expected.to_rgba8();
                let result = compare(&actual, &expected);
                if result.is_err() && actual.dimensions() == expected.dimensions() {
                    let diff =
                        image::RgbaImage::from_fn(actual.width(), actual.height(), |x, y| {
                            if actual.get_pixel(x, y) == expected.get_pixel(x, y) {
                                image::Rgba([32, 32, 32, 255])
                            } else {
                                image::Rgba([255, 0, 80, 255])
                            }
                        });
                    diff.save(output.join(format!(
                        "diff-{}",
                        actual_path.file_name().unwrap().to_string_lossy()
                    )))
                    .unwrap();
                }
                result
            });
        if let Err(error) = result {
            failures.push(format!("{}: {error}", expected_path.display()));
        }
    }
    assert!(
        failures.is_empty(),
        "visual regressions:\n{}",
        failures.join("\n")
    );
    println!(
        "{} visual baselines {}",
        files.len(),
        if update {
            "recorded for review"
        } else {
            "passed"
        }
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_single_changed_pixel_fails_the_visual_gate() {
        let expected = image::RgbaImage::from_pixel(12, 8, image::Rgba([0, 0, 0, 255]));
        let mut actual = expected.clone();
        assert!(compare(&actual, &expected).is_ok());
        actual.put_pixel(6, 4, image::Rgba([1, 0, 0, 255]));
        assert!(compare(&actual, &expected)
            .unwrap_err()
            .to_string()
            .contains("1 pixels changed"));
        assert!(compare(&image::RgbaImage::new(8, 12), &expected).is_err());
    }
}
