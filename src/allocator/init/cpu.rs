#[cfg(all(
    not(miri),
    any(target_arch = "x86", target_arch = "x86_64"),
    not(target_env = "msvc")
))]
fn mi_cpuid(regs4: &mut [u32; 4], level: u32, sublevel: u32) -> bool {
    let regs = {
        #[cfg(target_arch = "x86")]
        {
            // Mirrors the GNU C path while preserving the sublevel input.
            // SAFETY: this cfg is a non-Miri x86 target whose supported CPU
            // baseline can execute CPUID; arbitrary leaf/subleaf execution is
            // defined and accesses no memory, and the caller interprets the bits.
            unsafe { core::arch::x86::__cpuid_count(level, sublevel) }
        }
        #[cfg(target_arch = "x86_64")]
        {
            // Mirrors the GNU C path while preserving the sublevel input.
            // SAFETY: this cfg is a non-Miri x86-64 target, where CPUID is
            // available; arbitrary leaf/subleaf execution is defined and accesses
            // no memory, and the caller interprets the returned bits.
            unsafe { core::arch::x86_64::__cpuid_count(level, sublevel) }
        }
    };
    *regs4 = [regs.eax, regs.ebx, regs.ecx, regs.edx];
    true
}

#[cfg(all(
    not(miri),
    any(target_arch = "x86", target_arch = "x86_64"),
    target_env = "msvc"
))]
fn mi_cpuid(regs4: &mut [u32; 4], level: u32, sublevel: u32) -> bool {
    let regs = {
        #[cfg(target_arch = "x86")]
        {
            // SAFETY: this cfg is a non-Miri x86 target whose supported CPU
            // baseline can execute CPUID; arbitrary leaf/subleaf execution is
            // defined and accesses no memory, and the caller interprets the bits.
            unsafe { core::arch::x86::__cpuid_count(level, sublevel) }
        }
        #[cfg(target_arch = "x86_64")]
        {
            // SAFETY: this cfg is a non-Miri x86-64 target, where CPUID is
            // available; arbitrary leaf/subleaf execution is defined and accesses
            // no memory, and the caller interprets the returned bits.
            unsafe { core::arch::x86_64::__cpuid_count(level, sublevel) }
        }
    };
    *regs4 = [regs.eax, regs.ebx, regs.ecx, regs.edx];
    true
}

#[cfg(any(miri, not(any(target_arch = "x86", target_arch = "x86_64"))))]
fn mi_cpuid(_regs4: &mut [u32; 4], _level: u32, _sublevel: u32) -> bool {
    false
}

pub(super) fn mi_detect_cpu_features() {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        let mut amd = false;
        let mut fsrm = false;
        let mut fsrs = false;
        let mut cpu_info = [0u32; 4];
        if mi_cpuid(&mut cpu_info, 0, 0) {
            amd = cpu_info[2] == 0x444d_4163;
        }
        if mi_cpuid(&mut cpu_info, 7, 0) {
            fsrm = (cpu_info[3] & (1 << 4)) != 0;
        }
        if mi_cpuid(&mut cpu_info, 7, 1) {
            fsrs = (cpu_info[1] & (1 << 11)) != 0;
        }
        if mi_cpuid(&mut cpu_info, 1, 0) {
            // SAFETY: process initialization holds the one-time init witness;
            // this feature flag is published before allocator readers proceed.
            unsafe { super::_mi_cpu_has_popcnt = (cpu_info[2] & (1 << 23)) != 0 };
        }
        if fsrs || (amd && fsrm) {
            // SAFETY: process initialization holds the one-time init witness;
            // this threshold is published before allocator readers proceed.
            unsafe { super::_mi_cpu_stosb_max = 127 };
        }
    }

    #[cfg(target_arch = "aarch64")]
    // SAFETY: `mi_process_init_inner` calls this under its one-time process-init
    // witness, before allocator readers proceed, so this AArch64 baseline
    // feature publication has unique write access and is frozen afterward.
    unsafe {
        super::_mi_cpu_has_popcnt = true;
    }
}

#[cfg(test)]
mod tests {
    use super::mi_cpuid;

    #[test]
    fn cpuid_projects_registers_into_the_fixed_array() {
        let mut actual = [u32::MAX; 4];

        #[cfg(all(not(miri), any(target_arch = "x86", target_arch = "x86_64")))]
        {
            assert!(mi_cpuid(&mut actual, 0, 0));
            let expected = {
                #[cfg(target_arch = "x86")]
                {
                    // SAFETY: CPUID level zero is supported on the test's x86 target.
                    unsafe { core::arch::x86::__cpuid_count(0, 0) }
                }
                #[cfg(target_arch = "x86_64")]
                {
                    // SAFETY: CPUID level zero is supported on the test's x86-64 target.
                    unsafe { core::arch::x86_64::__cpuid_count(0, 0) }
                }
            };
            assert_eq!(
                actual,
                [expected.eax, expected.ebx, expected.ecx, expected.edx]
            );
        }

        #[cfg(any(miri, not(any(target_arch = "x86", target_arch = "x86_64"))))]
        {
            assert!(!mi_cpuid(&mut actual, 0, 0));
            assert_eq!(actual, [u32::MAX; 4]);
        }
    }
}
