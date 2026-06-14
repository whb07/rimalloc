use mechanical_mimalloc::bitmap::{
    mi_bbitmap_init, mi_bbitmap_is_setN, mi_bbitmap_setN, mi_bbitmap_t,
    mi_bbitmap_try_find_and_clearN,
};

#[test]
fn bbitmap_find_and_clear_claims_one_range() {
    let mut bitmap = mi_bbitmap_t::default();
    unsafe {
        mi_bbitmap_init(&mut bitmap, 1024, true);
        assert!(mi_bbitmap_setN(&mut bitmap, 0, 4));

        let mut idx = usize::MAX;
        assert!(mi_bbitmap_try_find_and_clearN(&mut bitmap, 0, 1, &mut idx));
        assert_eq!(idx, 0);

        let mut remaining = 0;
        for i in 0..4 {
            if mi_bbitmap_is_setN(&mut bitmap, i, 1) {
                remaining += 1;
            }
        }
        assert_eq!(remaining, 3);
    }
}
