use crate::{
    file_system::FileSystem,
    smart_xml::{self, BGData, Screen},
};
use anyhow::{Context, Result};
use std::path::Path;

type Color = [u8; 3];

#[derive(Debug, Clone)]
pub struct Image {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

impl Image {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; width * height * 4],
        }
    }

    pub fn get_pixel(&self, x: usize, y: usize) -> Color {
        let i = (y * self.width + x) * 4;
        [self.pixels[i], self.pixels[i + 1], self.pixels[i + 2]]
    }

    pub fn set_pixel(&mut self, x: usize, y: usize, color: Color) {
        let i = (y * self.width + x) * 4;
        self.pixels[i] = color[0];
        self.pixels[i + 1] = color[1];
        self.pixels[i + 2] = color[2];
        self.pixels[i + 3] = 255;
    }
}

#[derive(Clone)]
pub struct RoomImages {
    pub room_state_names: Vec<String>,
    pub layer1: Vec<Image>,
    pub layer2: Vec<Image>,
}

#[derive(Copy, Clone)]
struct Tile8x8 {
    idx: usize,
    palette: usize,
    flip_x: bool,
    flip_y: bool,
    _priority: bool,
}

#[derive(Copy, Clone)]
struct Tile16x16 {
    top_left: Tile8x8,
    top_right: Tile8x8,
    bottom_left: Tile8x8,
    bottom_right: Tile8x8,
}

struct CRETileset {
    gfx: Vec<[[u8; 8]; 8]>,
    tiles: Vec<Tile16x16>,
}

struct SCETileset {
    palette: Vec<Color>,
    gfx: Vec<[[u8; 8]; 8]>,
    tiles: Vec<Tile16x16>,
}

fn decode_8x8_tile_data_4bpp(data: &[u8]) -> Result<[[u8; 8]; 8]> {
    let mut out: [[u8; 8]; 8] = [[0; 8]; 8];
    for y in 0..8 {
        let addr = y * 2;
        let data_0 = data[addr];
        let data_1 = data[addr + 1];
        let data_2 = data[addr + 16];
        let data_3 = data[addr + 17];
        for x in 0..8 {
            let bit_0 = (data_0 >> (7 - x)) & 1;
            let bit_1 = (data_1 >> (7 - x)) & 1;
            let bit_2 = (data_2 >> (7 - x)) & 1;
            let bit_3 = (data_3 >> (7 - x)) & 1;
            let c = bit_0 | (bit_1 << 1) | (bit_2 << 2) | (bit_3 << 3);
            out[y][x] = c as u8;
        }
    }
    Ok(out)
}

fn decode_8x8_tile(x: u16) -> Tile8x8 {
    Tile8x8 {
        idx: (x & 0x3FF) as usize,
        palette: ((x >> 10) & 7) as usize,
        _priority: ((x >> 13) & 1) == 1,
        flip_x: ((x >> 14) & 1) == 1,
        flip_y: ((x >> 15) & 1) == 1,
    }
}

fn decode_16x16_tile(data: &[u8]) -> Tile16x16 {
    Tile16x16 {
        top_left: decode_8x8_tile(u16::from_le_bytes([data[0], data[1]])),
        top_right: decode_8x8_tile(u16::from_le_bytes([data[2], data[3]])),
        bottom_left: decode_8x8_tile(u16::from_le_bytes([data[4], data[5]])),
        bottom_right: decode_8x8_tile(u16::from_le_bytes([data[6], data[7]])),
    }
}

fn decode_color(data: u16) -> Color {
    let r = data & 0x1f;
    let g = (data >> 5) & 0x1f;
    let b = (data >> 10) & 0x1f;
    [(r * 8) as u8, (g * 8) as u8, (b * 8) as u8]
}

fn load_palette<T: FileSystem>(palette_path: &Path, file_system: &T) -> Result<Vec<Color>> {
    let palette_bytes = file_system
        .load(palette_path)
        .with_context(|| format!("Unable to load palette at {}", palette_path.display()))?;
    let mut palette: Vec<Color> = vec![];
    let mut i = 0;
    while i < palette_bytes.len() {
        let data = (palette_bytes[i] as u16) | ((palette_bytes[i + 1] as u16) << 8);
        let color = decode_color(data);
        palette.push(color);
        i += 2;
    }
    Ok(palette)
}

fn load_8x8_gfx<T: FileSystem>(gfx8x8_path: &Path, file_system: &T) -> Result<Vec<[[u8; 8]; 8]>> {
    let gfx8x8_bytes = file_system
        .load(&gfx8x8_path)
        .with_context(|| format!("Unable to load CRE 8x8 gfx at {}", gfx8x8_path.display()))?;
    let mut gfx: Vec<[[u8; 8]; 8]> = vec![];
    let mut i = 0;
    while i < gfx8x8_bytes.len() {
        let tile = decode_8x8_tile_data_4bpp(&gfx8x8_bytes[i..(i + 32)])?;
        gfx.push(tile);
        i += 32;
    }
    Ok(gfx)
}

fn load_16x16_gfx<T: FileSystem>(gfx16x16_path: &Path, file_system: &T) -> Result<Vec<Tile16x16>> {
    let gfx16x16_bytes = file_system.load(&gfx16x16_path).with_context(|| {
        format!(
            "Unable to load CRE 16x16 tiles at {}",
            gfx16x16_path.display()
        )
    })?;
    let mut tiles: Vec<Tile16x16> = vec![];
    let mut i = 0;
    while i < gfx16x16_bytes.len() {
        let tile = decode_16x16_tile(&gfx16x16_bytes[i..(i + 8)]);
        tiles.push(tile);
        i += 8;
    }
    Ok(tiles)
}

fn load_cre_tileset<T: FileSystem>(tileset_path: &Path, file_system: &T) -> Result<CRETileset> {
    let gfx = load_8x8_gfx(&tileset_path.join("8x8tiles.gfx"), file_system)?;
    let tiles = load_16x16_gfx(&tileset_path.join("16x16tiles.ttb"), file_system)?;
    Ok(CRETileset { gfx, tiles })
}

fn load_sce_tileset<T: FileSystem>(
    tileset_path: &Path,
    cre_tileset: &CRETileset,
    file_system: &T,
) -> Result<SCETileset> {
    let palette = load_palette(&tileset_path.join("palette.snes"), file_system)?;
    let mut gfx = load_8x8_gfx(&tileset_path.join("8x8tiles.gfx"), file_system)?;
    let sce_tiles = load_16x16_gfx(&tileset_path.join("16x16tiles.ttb"), file_system)?;

    gfx.extend(cre_tileset.gfx.clone());
    let mut tiles = cre_tileset.tiles.clone();
    tiles.extend(sce_tiles);
    Ok(SCETileset {
        palette,
        gfx,
        tiles,
    })
}

fn render_tile_8x8(image: &mut Image, x0: usize, y0: usize, tile: Tile8x8, tileset: &SCETileset) {
    let gfx = &tileset.gfx[tile.idx];
    for y in 0..8 {
        for x in 0..8 {
            let x1 = if tile.flip_x { 7 - x } else { x };
            let y1 = if tile.flip_y { 7 - y } else { y };
            if gfx[y1][x1] == 0 {
                // Skip transparent pixels
                continue;
            }
            let color_idx = tile.palette * 16 + (gfx[y1][x1] as usize);
            let color = tileset.palette[color_idx];
            image.set_pixel(x0 + x, y0 + y, color);
        }
    }
}

fn render_tile_16x16(
    image: &mut Image,
    x0: usize,
    y0: usize,
    tile: Tile16x16,
    tileset: &SCETileset,
) {
    render_tile_8x8(image, x0, y0, tile.top_left, tileset);
    render_tile_8x8(image, x0 + 8, y0, tile.top_right, tileset);
    render_tile_8x8(image, x0, y0 + 8, tile.bottom_left, tileset);
    render_tile_8x8(image, x0 + 8, y0 + 8, tile.bottom_right, tileset);
}

fn render_bgdata(bgdata: &BGData, image: &mut Image, tileset: &SCETileset) -> Result<()> {
    for data in &bgdata.data {
        if data.type_ != "DECOMP" {
            continue;
        }
        let mut tiles = vec![];
        for &word in &data.source {
            let tile = decode_8x8_tile(word as u16);
            tiles.push(tile);
        }
        if tiles.len() == 1024 {
            for screen_y in 0..image.height / 256 {
                for screen_x in 0..image.width / 256 {
                    for (i, &tile) in tiles.iter().enumerate() {
                        let x = screen_x * 256 + (i % 32) * 8;
                        let y = screen_y * 256 + (i / 32) * 8;
                        render_tile_8x8(image, x, y, tile, tileset);
                    }
                }
            }
        } else if tiles.len() == 2048 {
            for screen_y in 0..image.height / 256 {
                for screen_x2 in 0..image.width / 512 {
                    for (i, &tile) in tiles.iter().enumerate() {
                        if i < 1024 {
                            let x = screen_x2 * 512 + (i % 32) * 8;
                            let y = screen_y * 256 + (i / 32) * 8;
                            render_tile_8x8(image, x, y, tile, tileset);
                        } else {
                            let x = screen_x2 * 512 + 256 + (i % 32) * 8;
                            let y = screen_y * 256 + ((i - 1024) / 32) * 8;
                            render_tile_8x8(image, x, y, tile, tileset);
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn render_screens(screens: &[Screen], image: &mut Image, tileset: &SCETileset) {
    for screen in screens {
        let x0 = screen.x * 16;
        let y0 = screen.y * 16;

        for (i, data) in screen.data.iter().enumerate() {
            let x = (i % 16) + x0;
            let y = (i / 16) + y0;
            let tile_idx = (data & 0x3FF) as usize;
            let tile_flip_x = (data & 0x400) != 0;
            let tile_flip_y = (data & 0x800) != 0;
            let mut tile = tileset.tiles[tile_idx];
            if tile_flip_x {
                (tile.top_left, tile.top_right) = (tile.top_right, tile.top_left);
                (tile.bottom_left, tile.bottom_right) = (tile.bottom_right, tile.bottom_left);
                tile.top_left.flip_x = !tile.top_left.flip_x;
                tile.top_right.flip_x = !tile.top_right.flip_x;
                tile.bottom_left.flip_x = !tile.bottom_left.flip_x;
                tile.bottom_right.flip_x = !tile.bottom_right.flip_x;
            }
            if tile_flip_y {
                (tile.top_left, tile.bottom_left) = (tile.bottom_left, tile.top_left);
                (tile.top_right, tile.bottom_right) = (tile.bottom_right, tile.top_right);
                tile.top_left.flip_y = !tile.top_left.flip_y;
                tile.top_right.flip_y = !tile.top_right.flip_y;
                tile.bottom_left.flip_y = !tile.bottom_left.flip_y;
                tile.bottom_right.flip_y = !tile.bottom_right.flip_y;
            }
            render_tile_16x16(image, x * 16, y * 16, tile, tileset);
        }
    }
}

pub fn render_room<F: FileSystem>(
    project_dir: &Path,
    room_name: &str,
    file_system: &F,
) -> Result<RoomImages> {
    let room_path = project_dir.join(format!("Export/Rooms/{}.xml", room_name));
    let room_bytes = file_system
        .load(&room_path)
        .with_context(|| format!("Unable to load room at {}", room_path.display()))?;
    let room_str = String::from_utf8(room_bytes)?;
    let room: smart_xml::Room = serde_xml_rs::from_str(room_str.as_str())
        .with_context(|| format!("Unable to parse XML in {}", room_path.display()))?;

    let cre_tileset_dir = project_dir.join("Export/Tileset/CRE/00/");
    let cre_tileset = load_cre_tileset(&cre_tileset_dir, file_system)?;

    let sce_tilesets_dir = project_dir.join("Export/Tileset/SCE");

    let mut room_state_name_list: Vec<String> = vec![];
    let mut layer1_list: Vec<Image> = vec![];
    let mut layer2_list: Vec<Image> = vec![];

    for state_xml in room.states.state.iter() {
        let room_state_name = format!("{}: {}", state_xml.condition, state_xml.arg);
        room_state_name_list.push(room_state_name);

        let gfx_set_str = format!("{:02X}", state_xml.gfx_set);
        let tileset_path = sce_tilesets_dir.join(gfx_set_str);
        let tileset = load_sce_tileset(&tileset_path, &cre_tileset, file_system)?;
        let width = room.width * 256;
        let height = room.height * 256;

        let mut layer1 = Image::new(width, height);
        render_screens(&state_xml.level_data.layer_1.screen, &mut layer1, &tileset);
        layer1_list.push(layer1);

        let mut layer2 = Image::new(width, height);
        render_bgdata(&state_xml.bg_data, &mut layer2, &tileset)?;
        render_screens(&state_xml.level_data.layer_2.screen, &mut layer2, &tileset);
        layer2_list.push(layer2);
    }
    Ok(RoomImages {
        room_state_names: room_state_name_list,
        layer1: layer1_list,
        layer2: layer2_list,
    })
}
