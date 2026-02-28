pub const RAYDSL_NAME: &str = "raydsl";
pub const RAYDSL_VERSION: u32 = 1;

pub fn normalize_source(source: &str) -> String {
    source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}
