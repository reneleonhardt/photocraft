use super::*;
use photocraft_psd::{ColorMode, Compression, Version, testgen};

#[test]
fn mask_presence_flags_and_optional_fields() {
    let file = testgen::layered(Version::Psd, ColorMode::Rgb, 16, Compression::Raw);
    let layer = file.layers().iter().position(|l| l.layer_mask().is_some_and(|m| m.real.is_some())).unwrap();
    let doc = Document::parse(&file.to_bytes().unwrap()).unwrap();
    let mut mask = Mask::default();
    // SAFETY: the document is live and the output is aligned writable storage.
    unsafe {
        assert_eq!(pc_psd_layer_mask(&doc, layer, &mut mask, ptr::null_mut()), OK);
    }
    assert_eq!((mask.present, mask.parameters, mask.user_density, mask.has_real, mask.real_background), (1, 3, 200, 1, 255));
    assert_eq!(mask.user_feather, 1.5);
    assert_eq!(mask.real_bounds, [0, 0, 3, 3]);
}

#[test]
fn bad_arguments_errors_and_panics_are_reported_and_outputs_reset() {
    let mut doc = ptr::null_mut();
    let mut e = Error { message: [0; 256] };
    let mut h = Header::default();
    // SAFETY: live storage obeys the contract; invalid pointers are rejected before dereference.
    unsafe {
        assert_eq!(pc_psd_parse(ptr::null(), 1, &mut doc, &mut e), INVALID_ARGUMENT);
        assert!(doc.is_null());
        assert_eq!(pc_psd_parse(ptr::null(), 0, &mut doc, &mut e), ERROR);
        assert_eq!(pc_psd_parse(b"x".as_ptr(), usize::MAX, &mut doc, ptr::null_mut()), INVALID_ARGUMENT);
        assert_eq!(pc_psd_parse(b"x".as_ptr(), 1, ptr::null_mut(), &mut e), INVALID_ARGUMENT);
        assert_eq!(pc_psd_header(ptr::null(), &mut h, &mut e), INVALID_ARGUMENT);
        let mut floats = FloatImage { width: 99, ..Default::default() };
        assert_eq!(pc_psd_merged_pixels_f32(ptr::null(), &mut floats, &mut e), INVALID_ARGUMENT);
        assert_eq!(floats.width, 0);
        assert!(floats.data.data.is_null());
        assert_eq!(pc_psd_header(ptr::null(), ptr::without_provenance_mut(1), &mut e), INVALID_ARGUMENT);
        assert_eq!(run(&mut h, &mut e, || panic!("synthetic boundary panic")), PANIC);
        assert_eq!(h, Header::default());
        assert_ne!(e.message[0], 0);
        assert_eq!(run(&mut h, &mut e, || Err(Failure(ERROR, "é".repeat(200)))), ERROR);
        let message = std::ffi::CStr::from_ptr(e.message.as_ptr()).to_str().unwrap();
        assert_eq!(message.len(), 254);
        assert_eq!(run(&mut h, &mut e, || Ok(Header::default())), OK);
        assert_eq!(e.message[0], 0);
        let mut file = testgen::merged_only(Version::Psd, ColorMode::Rgb, 8, Compression::Raw, 1, 1);
        file.resources.retain(|r| r.id != 1039);
        let empty = file.to_bytes().unwrap();
        assert_eq!(pc_psd_parse(empty.as_ptr(), empty.len(), &mut doc, &mut e), OK);
        let mut out = Buffer::default();
        assert_eq!(pc_psd_icc_profile(doc, &mut out, &mut e), OK);
        assert!(out.data.is_null());
        assert_eq!(out.len, 0);
        assert_eq!(pc_psd_layer_name(doc, usize::MAX, &mut out, &mut e), ERROR);
        assert!(out.data.is_null());
        pc_psd_buffer_free(&mut out);
        pc_psd_buffer_free(ptr::null_mut());
        out = buffer(vec![1, 2, 3]);
        assert_eq!(std::slice::from_raw_parts(out.data, out.len), [1, 2, 3]);
        pc_psd_buffer_free(&mut out);
        pc_psd_buffer_free(&mut out);
        assert!(out.data.is_null() && out.len == 0);
        let mut tree = buffer(vec![TreeNode { index: 1, parent: usize::MAX, divider: usize::MAX, is_group: 0 }]);
        assert_eq!((*tree.data).index, 1);
        pc_psd_tree_free(&mut tree);
        pc_psd_tree_free(&mut tree);
        pc_psd_tree_free(ptr::null_mut());
        assert!(tree.data.is_null() && tree.len == 0);
        pc_psd_free(doc);
        pc_psd_free(ptr::null_mut());
    }
}
