use std::fs;

use itertools::Itertools;
use notoize::NotoizeClient;

fn main() {
    let json = fs::read_to_string("data/toakue.js").unwrap();
    let chars = json.chars().sorted().dedup().collect::<String>();
    fs::write("data/chars.txt", chars).unwrap();
    let mut client = NotoizeClient::new();
    let mut fonts = client
        .notoize(&json)
        .files()
        .into_iter()
        .sorted_by_key(|x| x.fontname.contains("CJK"))
        .collect_vec();
    fonts.retain(|f| !["Noto Color Emoji"].contains(&f.fontname.as_str()));
    let mut css = String::new();
    for font in fonts.clone() {
        fs::write(format!("fonts/{}", font.filename), font.bytes).unwrap();
        css += &format!(
            "@font-face {{\r\n    font-family: \"{}\";\r\n    src: url(\"fonts/{}\");\r\n    \
             font-display: swap;\r\n}}\r\n",
            font.fontname, font.filename
        );
    }
    css += &format!(
        ":root {{\r\n    --sans: {}, ui-sans-serif, sans-serif;\r\n}}",
        fonts
            .iter()
            .map(|f| format!("\"{}\"", f.fontname))
            .collect::<Vec<_>>()
            .join(", ")
    );
    fs::write("noto.css", css).unwrap();
}
