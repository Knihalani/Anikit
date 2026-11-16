//! SPS / PPS / slice header writers.

use crate::bits::BitWriter;

pub(crate) const LOG2_MAX_FRAME_NUM: u32 = 8;

pub(crate) struct SeqInfo {
    pub mbw: usize,
    pub mbh: usize,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub full_range: bool,
    pub cabac: bool,
}

/// (level_idc, MaxMBPS, MaxFS)
const LEVELS: [(u8, u64, u64); 17] = [
    (10, 1485, 99),
    (11, 3000, 396),
    (12, 6000, 396),
    (13, 11880, 396),
    (21, 19800, 792),
    (22, 20250, 1620),
    (30, 40500, 1620),
    (31, 108000, 3600),
    (32, 216000, 5120),
    (40, 245760, 8192),
    (42, 522240, 8704),
    (50, 589824, 22080),
    (51, 983040, 36864),
    (52, 2073600, 36864),
    (60, 4177920, 139264),
    (61, 8355840, 139264),
    (62, 16711680, 139264),
];

pub(crate) fn level_idc(mbw: usize, mbh: usize, fps: f64) -> u8 {
    let fs = (mbw * mbh) as u64;
    let mbps = (fs as f64 * fps).ceil() as u64;
    for &(l, m, f) in LEVELS.iter() {
        if mbps <= m && fs <= f && (mbw * mbw) as u64 <= 8 * f && (mbh * mbh) as u64 <= 8 * f {
            return l;
        }
    }
    62
}

/// Max |vertical MV| in full pels for a level.
pub(crate) fn max_mv_y(level: u8) -> i32 {
    match level {
        0..=10 => 63,
        11..=20 => 127,
        21..=30 => 255,
        _ => 511,
    }
}

fn timing(fps: f64) -> (u32, u32) {
    let r = fps.round();
    if (fps - r).abs() < 1e-6 && r >= 1.0 {
        (1, 2 * r as u32)
    } else {
        (1001, 2 * (fps * 1001.0).round() as u32)
    }
}

pub(crate) fn write_sps(w: &mut BitWriter, s: &SeqInfo, level: u8) {
    if s.cabac {
        w.put(8, 77); // Main
        w.put(8, 0b0100_0000); // constraint_set1
    } else {
        w.put(8, 66); // Baseline
        w.put(8, 0b1100_0000); // constraint_set0 + set1: Constrained Baseline
    }
    w.put(8, level as u32);
    w.ue(0); // sps id
    w.ue(LOG2_MAX_FRAME_NUM - 4);
    w.ue(2); // pic_order_cnt_type
    w.ue(1); // max_num_ref_frames
    w.bit(false); // gaps
    w.ue(s.mbw as u32 - 1);
    w.ue(s.mbh as u32 - 1);
    w.bit(true); // frame_mbs_only
    w.bit(true); // direct_8x8_inference
    let cr = (s.mbw as u32 * 16 - s.width) / 2;
    let cb = (s.mbh as u32 * 16 - s.height) / 2;
    if cr > 0 || cb > 0 {
        w.bit(true);
        w.ue(0);
        w.ue(cr);
        w.ue(0);
        w.ue(cb);
    } else {
        w.bit(false);
    }
    // VUI
    w.bit(true);
    w.bit(false); // aspect_ratio_info
    w.bit(false); // overscan
    w.bit(true); // video_signal_type_present
    w.put(3, 5); // unspecified video format
    w.bit(s.full_range);
    w.bit(true); // colour_description_present
    w.put(8, 1); // BT.709 primaries
    w.put(8, 1); // transfer
    w.put(8, 1); // matrix
    w.bit(false); // chroma_loc_info
    let (tick, scale) = timing(s.fps);
    w.bit(true);
    w.put(32, tick);
    w.put(32, scale);
    w.bit(true); // fixed_frame_rate
    w.bit(false); // nal_hrd
    w.bit(false); // vcl_hrd
    w.bit(false); // pic_struct_present
    w.bit(true); // bitstream_restriction
    w.bit(true); // motion_vectors_over_pic_boundaries
    w.ue(0); // max_bytes_per_pic_denom
    w.ue(0); // max_bits_per_mb_denom
    w.ue(13); // log2_max_mv_length_horizontal
    w.ue(11); // log2_max_mv_length_vertical
    w.ue(0); // max_num_reorder_frames
    w.ue(1); // max_dec_frame_buffering
    w.trailing();
}

pub(crate) fn write_pps(w: &mut BitWriter, qp: u8, cabac: bool) {
    w.ue(0);
    w.ue(0);
    w.bit(cabac); // entropy_coding_mode_flag
    w.bit(false); // bottom_field_pic_order
    w.ue(0); // slice groups
    w.ue(0); // num_ref_idx_l0_default_active_minus1
    w.ue(0);
    w.bit(false); // weighted_pred
    w.put(2, 0);
    w.se(qp as i32 - 26);
    w.se(0);
    w.se(0); // chroma_qp_index_offset
    w.bit(true); // deblocking_filter_control_present
    w.bit(false); // constrained_intra_pred
    w.bit(false); // redundant_pic_cnt
    w.trailing();
}

pub(crate) struct SliceHdr {
    pub first_mb: u32,
    pub idr: bool,
    pub frame_num: u32,
    pub idr_id: u32,
    pub deblock: bool,
    pub cabac: bool,
}

pub(crate) fn write_slice_header(w: &mut BitWriter, h: &SliceHdr) {
    w.ue(h.first_mb);
    w.ue(if h.idr { 7 } else { 5 }); // I / P, all slices of the picture alike
    w.ue(0);
    w.put(LOG2_MAX_FRAME_NUM, h.frame_num);
    if h.idr {
        w.ue(h.idr_id);
    } else {
        w.bit(false); // num_ref_idx_active_override
        w.bit(false); // ref_pic_list_modification_flag_l0
    }
    // dec_ref_pic_marking (nal_ref_idc != 0)
    if h.idr {
        w.bit(false);
        w.bit(false);
    } else {
        w.bit(false);
    }
    if h.cabac && !h.idr {
        w.ue(0); // cabac_init_idc
    }
    w.se(0); // slice_qp_delta
    if h.deblock {
        w.ue(0);
        w.se(0);
        w.se(0);
    } else {
        w.ue(1);
    }
}
