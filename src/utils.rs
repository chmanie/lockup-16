use midi2::ux::u7;

/// Scale from 4096 to 127
pub fn u16_to_u7(value: u16) -> u7 {
    u7::new(((value as u32 * 127) / 4095) as u8)
}
