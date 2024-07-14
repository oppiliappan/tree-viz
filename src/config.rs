use std::default::Default;

#[derive(Clone, Copy)]
pub struct Config {
    pub indent_level: usize,
    pub show_ranges: bool,
    pub show_src: bool,
    pub show_field_name: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config::new()
    }
}

impl Config {
    fn new() -> Self {
        Self {
            indent_level: 2,
            show_ranges: true,
            show_src: true,
            show_field_name: true,
        }
    }
}
