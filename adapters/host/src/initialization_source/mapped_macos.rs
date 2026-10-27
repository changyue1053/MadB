//! A POD-only implementation of libproc 0.14.11's safe PIDInfo extension point.
//! Layouts follow the pinned macOS SDK sys/proc_info.h. No pointer, union or
//! Rust enum is passed in the zero-initialized kernel output buffer.
use super::{Result, Stamp, conflict, host};
use libproc::net_info::VInfoStat;
use libproc::proc_pid::{PIDInfo, PidInfoFlavor, pidinfo};

#[repr(C)]
struct RegionInfo {
    protection: u32,
    max_protection: u32,
    inheritance: u32,
    flags: u32,
    offset: u64,
    behavior: u32,
    user_wired_count: u32,
    user_tag: u32,
    pages_resident: u32,
    pages_shared_now_private: u32,
    pages_swapped_out: u32,
    pages_dirtied: u32,
    ref_count: u32,
    shadow_depth: u32,
    share_mode: u32,
    private_pages_resident: u32,
    shared_pages_resident: u32,
    obj_id: u32,
    depth: u32,
    address: u64,
    size: u64,
}
#[repr(C)]
struct VnodeInfo {
    stat: VInfoStat,
    kind: i32,
    pad: i32,
    fsid: [i32; 2],
}
#[repr(C)]
struct VnodePath {
    vnode: VnodeInfo,
    path: [i8; 1024],
}
#[repr(C)]
struct RegionPath {
    region: RegionInfo,
    vnode_path: VnodePath,
}
impl PIDInfo for RegionPath {
    fn flavor() -> PidInfoFlavor {
        PidInfoFlavor::RegionPathInfo
    }
}

// Independently compared with sizeof/offsetof from the fixed SDK in the
// source-identity review. An incompatible dependency layout cannot compile.
const _: () = {
    use std::mem::{offset_of, size_of};
    assert!(size_of::<RegionInfo>() == 96);
    assert!(size_of::<VInfoStat>() == 136);
    assert!(size_of::<VnodeInfo>() == 152);
    assert!(size_of::<VnodePath>() == 1176);
    assert!(size_of::<RegionPath>() == 1272);
    assert!(offset_of!(RegionInfo, address) == 80);
    assert!(offset_of!(VInfoStat, vst_size) == 88);
    assert!(offset_of!(RegionPath, vnode_path) == 96);
    assert!(offset_of!(VnodePath, path) == 152);
};

pub(super) fn require_running_image(stamp: &Stamp) -> Result<()> {
    let address = super::FrozenInitializationTool::capture as *const () as usize as u64;
    let pid = i32::try_from(std::process::id()).map_err(|e| host("tool.pid", e))?;
    let observed = pidinfo::<RegionPath>(pid, address).map_err(|e| host("tool.mapping", e))?;
    let region = observed.region;
    let vnode = observed.vnode_path.vnode;
    let stat = vnode.stat;
    // pidinfo's fixed wrapper checks only positive return length. Missing or
    // partial zero-filled observations cannot establish these required facts.
    if region.address > address
        || region.address.checked_add(region.size).is_none_or(|end| end <= address)
        || region.protection & 4 == 0
        || vnode.kind != 1 // VREG
        || stat.vst_ino == 0
        || stat.vst_size <= 0
        || u64::try_from(stat.vst_size).ok() != Some(stamp.size)
        || (u64::from(stat.vst_dev), stat.vst_ino, stat.vst_mtime, stat.vst_mtimensec,
            stat.vst_ctime, stat.vst_ctimensec, u32::from(stat.vst_mode)) != stamp.identity
    {
        return Err(conflict(
            "named image does not match the running executable mapping",
        ));
    }
    Ok(())
}
