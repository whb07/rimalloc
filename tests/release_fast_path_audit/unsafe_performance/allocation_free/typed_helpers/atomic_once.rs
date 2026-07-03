use super::*;

pub(super) fn assert_atomic_once_boundaries(atomic_rs: &str, atomic_types: &str) {
    assert!(
        source_has_inline_never_function(
            atomic_rs,
            "pub unsafe extern \"C\" fn _mi_atomic_once_enter",
        ) && source_has_inline_never_function(
            atomic_rs,
            "pub unsafe extern \"C\" fn _mi_atomic_once_release",
        ) && atomic_types.contains("pub(crate) struct OnceGuard<'a>")
            && atomic_types.contains("pub(crate) struct OnceStatePtr(*mut usize)")
            && atomic_types.contains("pub(crate) fn from_raw(state: *mut usize) -> Self")
            && atomic_types
                .contains("fn as_once_ptr(self) -> *mut crate::atomic::mi_atomic_once_t")
            && atomic_types
                .contains("pub(crate) unsafe fn try_enter(once: OnceStatePtr) -> Option<Self>")
            && atomic_types.contains("impl Drop for OnceGuard<'_>")
            && atomic_types.contains("crate::atomic::_mi_atomic_once_release(")
            && !atomic_types.contains("pub(crate) struct OnceStatePtr(pub(crate)")
            && !atomic_types.contains("once.0.cast::<crate::atomic::mi_atomic_once_t>()")
            && atomic_rs.contains("_once: crate::core_types::OnceGuard<'static>")
            && atomic_rs.contains("crate::core_types::OnceStatePtr::from_raw")
            && atomic_rs.contains("crate::core_types::OnceGuard::try_enter(state)")
            && !atomic_rs.contains("once: *mut mi_atomic_once_t,\n}")
            && !atomic_rs.contains("_mi_atomic_once_release(self.once);"),
        "atomic once should keep C's enter/release helper boundaries and release through typed OnceGuard Drop"
    );
}
