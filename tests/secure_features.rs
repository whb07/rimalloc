use mechanical_mimalloc::raw::{internal, types};

#[test]
fn secure_feature_constants_follow_c_mode_rules() {
    if cfg!(feature = "secure") {
        assert_eq!(internal::MI_SECURE, 4);
        assert_eq!(types::MI_SECURE, 4);
        assert_eq!(types::MI_PADDING, 1);
        assert_eq!(types::MI_PADDING_CHECK, 1);
        assert_eq!(types::MI_ENCODE_FREELIST, 1);
    } else {
        assert_eq!(internal::MI_SECURE, 0);
        assert_eq!(types::MI_SECURE, 0);
    }
}
