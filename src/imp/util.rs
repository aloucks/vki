use crate::Extent3D;
use ash::vk;

pub fn has_zero_or_one_bits(bits: u32) -> bool {
    let bits = bits as i32;
    bits & (bits - 1) == 0
}

pub fn extent_3d(extent: Extent3D) -> vk::Extent3D {
    vk::Extent3D {
        width: extent.width,
        height: extent.height,
        depth: extent.depth,
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_has_zero_or_one_bits() {
        use crate::TextureUsageFlags;
        assert!(super::has_zero_or_one_bits(TextureUsageFlags::NONE.bits()));
        assert!(super::has_zero_or_one_bits(TextureUsageFlags::TRANSFER_SRC.bits()));
        assert!(!super::has_zero_or_one_bits(
            (TextureUsageFlags::TRANSFER_SRC | TextureUsageFlags::TRANSFER_DST).bits()
        ));
    }
}
