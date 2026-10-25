//! Test-support only: exercise native scalar ABI widths without record serialization.
#[uniffi::export(callback_interface)]
pub trait UnsignedHost: Send + Sync {
    fn echo_u8(&self, value: u8) -> u8;
    fn echo_u16(&self, value: u16) -> u16;
    fn echo_u32(&self, value: u32) -> u32;
    fn echo_u64(&self, value: u64) -> u64;
}

#[uniffi::export]
pub fn unsigned_u8(value: u8) -> u8 {
    value
}
#[uniffi::export]
pub fn unsigned_u16(value: u16) -> u16 {
    value
}
#[uniffi::export]
pub fn unsigned_u32(value: u32) -> u32 {
    value
}
#[uniffi::export]
pub fn unsigned_u64(value: u64) -> u64 {
    value
}

#[uniffi::export]
pub fn unsigned_via_host_u8(value: u8, host: Box<dyn UnsignedHost>) -> u8 {
    host.echo_u8(value)
}
#[uniffi::export]
pub fn unsigned_via_host_u16(value: u16, host: Box<dyn UnsignedHost>) -> u16 {
    host.echo_u16(value)
}
#[uniffi::export]
pub fn unsigned_via_host_u32(value: u32, host: Box<dyn UnsignedHost>) -> u32 {
    host.echo_u32(value)
}
#[uniffi::export]
pub fn unsigned_via_host_u64(value: u64, host: Box<dyn UnsignedHost>) -> u64 {
    host.echo_u64(value)
}
