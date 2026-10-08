use bevy::render::render_resource::{TextureDimension, TextureFormat, TextureViewDimension};
use bevy_image::{CompressedImageFormats, Image, ImageSampler, ImageType};

const DIFFUSE: &[u8] = include_bytes!("../assets/environment_maps/diffuse_rgb9e5_zstd.ktx2");
const SPECULAR: &[u8] = include_bytes!("../assets/environment_maps/specular_rgb9e5_zstd.ktx2");

fn load(buffer: &[u8]) -> Result<Image, bevy_image::TextureError> {
    Image::from_buffer(
        buffer,
        ImageType::Extension("ktx2"),
        CompressedImageFormats::NONE,
        false,
        ImageSampler::default(),
        Default::default(),
    )
}

fn check_original_map(buffer: &[u8], mip_levels: u32, data_length: usize, checksum: u64) {
    let image = load(buffer).unwrap();
    let descriptor = &image.texture_descriptor;
    assert_eq!(descriptor.size.width, 1024);
    assert_eq!(descriptor.size.height, 1024);
    assert_eq!(descriptor.size.depth_or_array_layers, 6);
    assert_eq!(descriptor.dimension, TextureDimension::D2);
    assert_eq!(descriptor.format, TextureFormat::Rgb9e5Ufloat);
    assert_eq!(descriptor.mip_level_count, mip_levels);
    assert_eq!(
        image.texture_view_descriptor.unwrap().dimension,
        Some(TextureViewDimension::Cube),
    );
    let data = image.data.unwrap();
    assert_eq!(data.len(), data_length);
    let actual_checksum = data.iter().fold(0xcbf29ce484222325_u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    });
    assert_eq!(actual_checksum, checksum);
}

#[test]
fn original_diffuse_map_loads() {
    check_original_map(DIFFUSE, 1, 25_165_824, 0x101ccbd84baf7b5b);
}

#[test]
fn original_specular_map_loads() {
    check_original_map(SPECULAR, 11, 33_554_424, 0x5ac017e7651db668);
}

#[test]
fn zero_sized_descriptor_is_rejected() {
    let mut buffer = DIFFUSE.to_vec();
    let descriptor_offset = u32::from_le_bytes(buffer[48..52].try_into().unwrap()) as usize;
    buffer[descriptor_offset + 10..descriptor_offset + 12].fill(0);
    assert!(load(&buffer).is_err());
}
