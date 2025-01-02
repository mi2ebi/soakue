use std::fs;

use itertools::Itertools;
use notoize::{Font, NotoizeClient};
use reqwest::blocking::Client;

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
    fonts.retain(|f| !["Noto Color Emoji", "Noto Serif Tangut"].contains(&f.fontname.as_str()));
    let client = Client::new();
    #[allow(clippy::single_element_loop)]
    for (filename, fontname, url) in [(
        "TangutYinchuan.ttf",
        "Tangut Yinchuan",
        "https://www.babelstone.co.uk/Fonts/Download/TangutYinchuan.ttf",
    )] {
        fonts.push(Font {
            filename: filename.to_string(),
            fontname: fontname.to_string(),
            bytes: client.get(url).send().unwrap().bytes().unwrap().to_vec(),
        });
    }
    let mut css = String::new();
    for font in fs::read_dir("fonts").unwrap() {
        fs::remove_file(font.unwrap().path()).unwrap();
    }
    for font in fonts.clone() {
        fs::write(format!("fonts/{}", font.filename), font.bytes).unwrap();
        css += &format!(
            "@font-face {{\r\n    font-family: \"{}\";\r\n    src: \
             url(\"fonts/{}\");\r\n{}{}}}\r\n",
            font.fontname,
            font.filename,
            if font.fontname == "Noto Sans" {
                "    font-display: swap;\r\n"
            } else {
                ""
            },
            if ["Noto Sans Symbols 2", "Noto Sans CJK HK"].contains(&font.fontname.as_str()) {
                "    unicode-range: 0000-269f, 26a1-10ffff;\r\n"
            } else {
                ""
            }
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
