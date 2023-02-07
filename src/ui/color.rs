pub struct Palette {
    pub colors: &'static [[u8; 3]],
}

impl Palette {
    pub fn pick(&self, id: u32) -> egui::Color32 {
        let color_idx = id as usize % self.colors.len();
        let color = self.colors[color_idx];
        egui::Color32::from_rgb(color[0], color[1], color[2])
    }
}

// alphabet
// 26 visually distinct colors
// Source: "A Colour Alphabet and the Limits of Colour Coding", Paul Green-Armytage
// https://graphicdesign.stackexchange.com/questions/3682/where-can-i-find-a-large-palette-set-of-contrasting-colors-for-coloring-many-d
// https://github.com/Netflix/atlas/wiki/Color-Palettes#armytage (Apache 2.0 license)
pub const ALPHABET: [[u8; 3]; 26] = [
    [240, 163, 255],
    [0, 117, 220],
    [153, 63, 0],
    [76, 0, 92],
    [25, 25, 25],
    [0, 92, 49],
    [43, 206, 72],
    [255, 204, 153],
    [128, 128, 128],
    [148, 255, 181],
    [143, 124, 0],
    [157, 204, 0],
    [194, 0, 136],
    [0, 51, 128],
    [255, 164, 5],
    [255, 168, 187],
    [66, 102, 0],
    [255, 0, 16],
    [94, 241, 242],
    [0, 153, 143],
    [224, 255, 102],
    [116, 10, 255],
    [153, 0, 0],
    [255, 255, 128],
    [255, 255, 0],
    [255, 80, 5],
];
