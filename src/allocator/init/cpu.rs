#[cfg(all(
    not(miri),
    any(target_arch = "x86", target_arch = "x86_64"),
    not(target_env = "msvc")
))]
fn mi_cpuid(regs4: *mut u32, level: u32, sublevel: u32) -> bool {
    let regs = {
        #[cfg(target_arch = "x86")]
        {
            // Mirrors the GNU C path while preserving the sublevel input.
            // SAFETY: random seeding/reseeding and CPU feature publication use process-local init witnesses.
            unsafe { core::arch::x86::__cpuid_count(level, sublevel) }
        }
        #[cfg(target_arch = "x86_64")]
        {
            // Mirrors the GNU C path while preserving the sublevel input.
            // SAFETY: random seeding/reseeding and CPU feature publication use process-local init witnesses.
            unsafe { core::arch::x86_64::__cpuid_count(level, sublevel) }
        }
    };
    // SAFETY: random seeding/reseeding and CPU feature publication use process-local init witnesses.
    unsafe {
        regs4.add(0).write(regs.eax);
        regs4.add(1).write(regs.ebx);
        regs4.add(2).write(regs.ecx);
        regs4.add(3).write(regs.edx);
    }
    true
}

#[cfg(all(
    not(miri),
    any(target_arch = "x86", target_arch = "x86_64"),
    target_env = "msvc"
))]
fn mi_cpuid(regs4: *mut u32, level: u32, sublevel: u32) -> bool {
    let regs = {
        #[cfg(target_arch = "x86")]
        {
            // SAFETY: random seeding/reseeding and CPU feature publication use process-local init witnesses.
            unsafe { core::arch::x86::__cpuid_count(level, sublevel) }
        }
        #[cfg(target_arch = "x86_64")]
        {
            // SAFETY: random seeding/reseeding and CPU feature publication use process-local init witnesses.
            unsafe { core::arch::x86_64::__cpuid_count(level, sublevel) }
        }
    };
    // SAFETY: random seeding/reseeding and CPU feature publication use process-local init witnesses.
    unsafe {
        regs4.add(0).write(regs.eax);
        regs4.add(1).write(regs.ebx);
        regs4.add(2).write(regs.ecx);
        regs4.add(3).write(regs.edx);
    }
    true
}

#[cfg(any(miri, not(any(target_arch = "x86", target_arch = "x86_64"))))]
fn mi_cpuid(_regs4: *mut u32, _level: u32, _sublevel: u32) -> bool {
    false
}

pub(super) fn mi_detect_cpu_features() {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        let mut amd = false;
        let mut fsrm = false;
        let mut fsrs = false;
        let mut cpu_info = [0u32; 4];
        if mi_cpuid(cpu_info.as_mut_ptr(), 0, 0) {
            amd = cpu_info[2] == 0x444d_4163;
        }
        if mi_cpuid(cpu_info.as_mut_ptr(), 7, 0) {
            fsrm = (cpu_info[3] & (1 << 4)) != 0;
        }
        if mi_cpuid(cpu_info.as_mut_ptr(), 7, 1) {
            fsrs = (cpu_info[1] & (1 << 11)) != 0;
        }
        if mi_cpuid(cpu_info.as_mut_ptr(), 1, 0) {
            unsafe { super::_mi_cpu_has_popcnt = (cpu_info[2] & (1 << 23)) != 0 }; // SAFETY: random seeding/reseeding and CPU feature publication use process-local init witnesses.
        }
        if fsrs || (amd && fsrm) {
            unsafe { super::_mi_cpu_stosb_max = 127 }; // SAFETY: random seeding/reseeding and CPU feature publication use process-local init witnesses.
        }
    }

    #[cfg(target_arch = "aarch64")]
    // SAFETY: random seeding/reseeding and CPU feature publication use process-local init witnesses.
    unsafe {
        super::_mi_cpu_has_popcnt = true;
    }
}
