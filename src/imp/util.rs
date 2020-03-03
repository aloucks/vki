use crate::{Extent3d, Origin3d};
use ash::vk;

pub fn has_zero_or_one_bits(bits: u32) -> bool {
    let bits = bits as i32;
    bits & (bits - 1) == 0
}

pub fn extent_3d(extent: Extent3d) -> vk::Extent3D {
    vk::Extent3D {
        width: extent.width,
        height: extent.height,
        depth: extent.depth,
    }
}

pub fn offset_3d(origin: Origin3d) -> vk::Offset3D {
    vk::Offset3D {
        x: origin.x,
        y: origin.y,
        z: origin.z,
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
