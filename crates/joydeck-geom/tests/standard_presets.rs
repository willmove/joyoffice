//! Text rectangles and ordered glue points evaluated independently from the
//! unmodified ECMA-376 attachment by scripts/joydeck/check-standard-presets.py.
use std::error::Error;

fn values(column: &str) -> Result<Vec<f64>, Box<dyn Error>> {
    if column == "-" {
        return Ok(Vec::new());
    }
    column.split(',').map(|word| Ok(word.parse()?)).collect()
}

#[test]
fn text_rectangles_and_connection_sites_match_the_standard() -> Result<(), Box<dyn Error>> {
    let reference = match std::env::var_os("JOYDECK_PRESET_GEOMETRY_REFERENCE") {
        Some(path) => std::fs::read_to_string(path)?,
        None => include_str!("gallery-geometry.txt").to_owned(),
    };
    for row in reference.lines() {
        let columns: Vec<_> = row.split_ascii_whitespace().collect();
        let [name, size, adjustments, rect, sites] = columns.as_slice() else {
            return Err(format!("Invalid reference row: {row}").into());
        };
        let size = values(size)?;
        let adjustments = values(adjustments)?;
        let g = joydeck_geom::preset::build(name, size[0], size[1], &adjustments)
            .ok_or_else(|| format!("Missing preset {name}"))?;
        let rect = values(rect)?;
        let actual_rect = [
            g.text_rect.x0,
            g.text_rect.y0,
            g.text_rect.x1,
            g.text_rect.y1,
        ];
        let sites = values(sites)?;
        let actual_sites = g.sites.iter().flat_map(|p| [p.x, p.y]).collect::<Vec<_>>();
        assert_eq!(rect.len(), actual_rect.len());
        assert_eq!(sites.len(), actual_sites.len(), "{name}");
        for (a, b) in actual_rect
            .into_iter()
            .chain(actual_sites)
            .zip(rect.into_iter().chain(sites))
        {
            assert!(
                (a - b).abs() <= 0.0001,
                "{name} {size:?} {adjustments:?}: {a} vs {b}"
            );
        }
    }
    Ok(())
}
