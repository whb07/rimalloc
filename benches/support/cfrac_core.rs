use super::BenchAllocator;
use std::ptr;

const BASE_BITS: usize = 16;
const BASE: u32 = 1 << BASE_BITS;
const BASE_MASK: u32 = BASE - 1;

pub struct Big {
    alloc: BenchAllocator,
    ptr: *mut u16,
    len: usize,
    cap: usize,
    neg: bool,
}

impl Big {
    pub unsafe fn zero(alloc: BenchAllocator) -> Self {
        unsafe {
            let mut value = Self::with_capacity(alloc, 1);
            value.ptr.write(0);
            value.len = 1;
            value
        }
    }

    pub unsafe fn one(alloc: BenchAllocator) -> Self {
        unsafe { Self::from_u64(alloc, 1) }
    }

    pub unsafe fn from_u64(alloc: BenchAllocator, mut value: u64) -> Self {
        unsafe {
            let mut out = Self::with_capacity(alloc, 4);
            while value != 0 {
                out.push_digit((value & BASE_MASK as u64) as u16);
                value >>= BASE_BITS;
            }
            if out.len == 0 {
                out.push_digit(0);
            }
            out
        }
    }

    pub unsafe fn from_decimal(alloc: BenchAllocator, text: &str) -> Self {
        unsafe {
            let mut out = Self::zero(alloc);
            for byte in text.bytes() {
                assert!(byte.is_ascii_digit(), "invalid cfrac decimal input");
                out.mul_small_assign(10);
                out.add_small_assign((byte - b'0') as u32);
            }
            out.normalize();
            out
        }
    }

    pub fn is_zero(&self) -> bool {
        self.len == 1 && unsafe { *self.ptr == 0 }
    }

    pub fn is_one(&self) -> bool {
        !self.neg && self.len == 1 && unsafe { *self.ptr == 1 }
    }

    pub fn is_odd(&self) -> bool {
        unsafe { (*self.ptr & 1) != 0 }
    }

    pub fn allocator(&self) -> BenchAllocator {
        self.alloc
    }

    pub unsafe fn cmp(&self, rhs: &Self) -> std::cmp::Ordering {
        unsafe {
            match (self.neg, rhs.neg) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                (false, false) => self.cmp_abs(rhs),
                (true, true) => self.cmp_abs(rhs).reverse(),
            }
        }
    }

    pub unsafe fn cmp_u32(&self, rhs: u32) -> std::cmp::Ordering {
        unsafe {
            let rhs = Self::from_u64(self.alloc, rhs as u64);
            self.cmp(&rhs)
        }
    }

    pub unsafe fn clone_big(&self) -> Self {
        unsafe {
            let mut out = Self::with_capacity(self.alloc, self.len);
            ptr::copy_nonoverlapping(self.ptr, out.ptr, self.len);
            out.len = self.len;
            out.neg = self.neg;
            out
        }
    }

    pub unsafe fn abs(&self) -> Self {
        unsafe {
            let mut out = self.clone_big();
            out.neg = false;
            out
        }
    }

    pub unsafe fn negated(&self) -> Self {
        unsafe {
            let mut out = self.clone_big();
            if !out.is_zero() {
                out.neg = !out.neg;
            }
            out
        }
    }

    pub unsafe fn add(&self, rhs: &Self) -> Self {
        unsafe {
            assert_eq!(self.alloc.name(), rhs.alloc.name());
            if self.neg == rhs.neg {
                let mut out = self.add_abs(rhs);
                out.neg = self.neg && !out.is_zero();
                out
            } else {
                match self.cmp_abs(rhs) {
                    std::cmp::Ordering::Greater => {
                        let mut out = self.sub_abs(rhs);
                        out.neg = self.neg && !out.is_zero();
                        out
                    }
                    std::cmp::Ordering::Less => {
                        let mut out = rhs.sub_abs(self);
                        out.neg = rhs.neg && !out.is_zero();
                        out
                    }
                    std::cmp::Ordering::Equal => Self::zero(self.alloc),
                }
            }
        }
    }

    pub unsafe fn sub(&self, rhs: &Self) -> Self {
        unsafe {
            let neg_rhs = rhs.negated();
            self.add(&neg_rhs)
        }
    }

    pub unsafe fn mul(&self, rhs: &Self) -> Self {
        unsafe {
            assert_eq!(self.alloc.name(), rhs.alloc.name());
            let mut out = Self::with_capacity(self.alloc, self.len + rhs.len);
            for _ in 0..(self.len + rhs.len) {
                out.push_digit(0);
            }
            for i in 0..self.len {
                let mut carry = 0u32;
                let lhs = self.digit(i) as u32;
                for j in 0..rhs.len {
                    let idx = i + j;
                    let cur = out.digit(idx) as u32 + lhs * rhs.digit(j) as u32 + carry;
                    out.set_digit(idx, (cur & BASE_MASK) as u16);
                    carry = cur >> BASE_BITS;
                }
                let mut idx = i + rhs.len;
                while carry != 0 {
                    let cur = out.digit(idx) as u32 + carry;
                    out.set_digit(idx, (cur & BASE_MASK) as u16);
                    carry = cur >> BASE_BITS;
                    idx += 1;
                }
            }
            out.neg = self.neg ^ rhs.neg;
            out.normalize();
            out
        }
    }

    pub unsafe fn div_rem(&self, rhs: &Self) -> (Self, Self) {
        unsafe {
            assert!(!rhs.is_zero(), "cfrac division by zero");
            assert_eq!(self.alloc.name(), rhs.alloc.name());
            if self.cmp_abs(rhs) == std::cmp::Ordering::Less {
                return (Self::zero(self.alloc), self.clone_big());
            }
            let mut quotient = Self::zero(self.alloc);
            let mut remainder = Self::zero(self.alloc);
            let bits = self.bit_len();
            for bit in (0..bits).rev() {
                remainder.shl1_assign();
                if self.bit(bit) {
                    remainder.add_small_assign(1);
                }
                if remainder.cmp_abs(rhs) != std::cmp::Ordering::Less {
                    let next = remainder.sub_abs(rhs);
                    remainder = next;
                    quotient.set_bit(bit);
                }
            }
            quotient.neg = self.neg ^ rhs.neg;
            quotient.normalize();
            remainder.neg = self.neg && !remainder.is_zero();
            (quotient, remainder)
        }
    }

    pub unsafe fn div_small(&self, divisor: u32) -> (Self, u32) {
        unsafe {
            assert!(divisor != 0);
            let mut out = Self::with_capacity(self.alloc, self.len);
            for _ in 0..self.len {
                out.push_digit(0);
            }
            let mut rem = 0u32;
            for i in (0..self.len).rev() {
                let cur = (rem << BASE_BITS) | self.digit(i) as u32;
                out.set_digit(i, (cur / divisor) as u16);
                rem = cur % divisor;
            }
            out.neg = self.neg;
            out.normalize();
            (out, rem)
        }
    }

    pub unsafe fn mod_small(&self, divisor: u32) -> u32 {
        unsafe {
            let mut rem = 0u32;
            for i in (0..self.len).rev() {
                let cur = (rem << BASE_BITS) | self.digit(i) as u32;
                rem = cur % divisor;
            }
            rem
        }
    }

    pub unsafe fn modulo(&self, rhs: &Self) -> Self {
        unsafe {
            let (_, rem) = self.div_rem(rhs);
            rem
        }
    }

    pub unsafe fn gcd(mut a: Self, mut b: Self) -> Self {
        unsafe {
            a.neg = false;
            b.neg = false;
            while !b.is_zero() {
                let r = a.modulo(&b);
                a = b;
                b = r;
            }
            a
        }
    }

    pub unsafe fn powmod(base: &Self, exp: &Self, modulus: &Self) -> Self {
        unsafe {
            let mut result = Self::one(base.alloc);
            let mut b = base.modulo(modulus);
            let mut e = exp.clone_big();
            while !e.is_zero() {
                if e.is_odd() {
                    result = result.mul(&b).modulo(modulus);
                }
                e = e.div_small(2).0;
                if !e.is_zero() {
                    b = b.mul(&b).modulo(modulus);
                }
            }
            result
        }
    }

    pub unsafe fn sqrt_floor(&self) -> Self {
        unsafe {
            assert!(!self.neg, "cfrac sqrt of negative precision");
            if self.is_zero() {
                return Self::zero(self.alloc);
            }
            let mut low = Self::zero(self.alloc);
            let mut high = Self::one(self.alloc);
            let target = self.clone_big();
            while high.mul(&high).cmp_abs(&target) != std::cmp::Ordering::Greater {
                high.shl1_assign();
            }
            while low.add(&Self::one(self.alloc)).cmp_abs(&high) == std::cmp::Ordering::Less {
                let mid = low.add(&high).div_small(2).0;
                let square = mid.mul(&mid);
                if square.cmp_abs(&target) == std::cmp::Ordering::Greater {
                    high = mid;
                } else {
                    low = mid;
                }
            }
            low
        }
    }

    pub unsafe fn to_decimal(&self) -> String {
        unsafe {
            if self.is_zero() {
                return "0".to_owned();
            }
            let mut cur = self.abs();
            let mut parts = Vec::new();
            while !cur.is_zero() {
                let (q, rem) = cur.div_small(10);
                parts.push((b'0' + rem as u8) as char);
                cur = q;
            }
            if self.neg {
                parts.push('-');
            }
            parts.iter().rev().collect()
        }
    }

    pub fn to_f64(&self) -> f64 {
        let mut out = 0.0;
        for i in (0..self.len).rev() {
            out = out * BASE as f64 + unsafe { self.digit(i) as f64 };
        }
        if self.neg { -out } else { out }
    }

    unsafe fn with_capacity(alloc: BenchAllocator, cap: usize) -> Self {
        unsafe {
            let cap = cap.max(1);
            let bytes = cap * std::mem::size_of::<u16>();
            let ptr = alloc.malloc(bytes).cast::<u16>();
            assert!(!ptr.is_null(), "allocation failed in cfrac precision");
            Self {
                alloc,
                ptr,
                len: 0,
                cap,
                neg: false,
            }
        }
    }

    unsafe fn reserve(&mut self, needed: usize) {
        unsafe {
            if needed <= self.cap {
                return;
            }
            let new_cap = needed.next_power_of_two();
            let bytes = new_cap * std::mem::size_of::<u16>();
            let new_ptr = self.alloc.malloc(bytes).cast::<u16>();
            assert!(!new_ptr.is_null(), "allocation failed in cfrac precision");
            ptr::copy_nonoverlapping(self.ptr, new_ptr, self.len);
            self.alloc.free(self.ptr.cast());
            self.ptr = new_ptr;
            self.cap = new_cap;
        }
    }

    unsafe fn push_digit(&mut self, digit: u16) {
        unsafe {
            self.reserve(self.len + 1);
            self.ptr.add(self.len).write(digit);
            self.len += 1;
        }
    }

    unsafe fn digit(&self, index: usize) -> u16 {
        unsafe {
            if index < self.len {
                *self.ptr.add(index)
            } else {
                0
            }
        }
    }

    unsafe fn set_digit(&mut self, index: usize, digit: u16) {
        unsafe {
            self.reserve(index + 1);
            if index >= self.len {
                for i in self.len..=index {
                    self.ptr.add(i).write(0);
                }
                self.len = index + 1;
            }
            *self.ptr.add(index) = digit;
        }
    }

    unsafe fn normalize(&mut self) {
        unsafe {
            while self.len > 1 && self.digit(self.len - 1) == 0 {
                self.len -= 1;
            }
            if self.len == 0 {
                self.push_digit(0);
            }
            if self.is_zero() {
                self.neg = false;
            }
        }
    }

    unsafe fn cmp_abs(&self, rhs: &Self) -> std::cmp::Ordering {
        unsafe {
            if self.len != rhs.len {
                return self.len.cmp(&rhs.len);
            }
            for i in (0..self.len).rev() {
                let lhs = self.digit(i);
                let rhs = rhs.digit(i);
                if lhs != rhs {
                    return lhs.cmp(&rhs);
                }
            }
            std::cmp::Ordering::Equal
        }
    }

    unsafe fn add_abs(&self, rhs: &Self) -> Self {
        unsafe {
            let len = self.len.max(rhs.len);
            let mut out = Self::with_capacity(self.alloc, len + 1);
            let mut carry = 0u32;
            for i in 0..len {
                let sum = self.digit(i) as u32 + rhs.digit(i) as u32 + carry;
                out.push_digit((sum & BASE_MASK) as u16);
                carry = sum >> BASE_BITS;
            }
            if carry != 0 {
                out.push_digit(carry as u16);
            }
            out
        }
    }

    unsafe fn sub_abs(&self, rhs: &Self) -> Self {
        unsafe {
            assert!(self.cmp_abs(rhs) != std::cmp::Ordering::Less);
            let mut out = Self::with_capacity(self.alloc, self.len);
            let mut borrow = 0i32;
            for i in 0..self.len {
                let mut diff = self.digit(i) as i32 - rhs.digit(i) as i32 - borrow;
                if diff < 0 {
                    diff += BASE as i32;
                    borrow = 1;
                } else {
                    borrow = 0;
                }
                out.push_digit(diff as u16);
            }
            out.normalize();
            out
        }
    }

    unsafe fn mul_small_assign(&mut self, factor: u32) {
        unsafe {
            let mut carry = 0u32;
            for i in 0..self.len {
                let cur = self.digit(i) as u32 * factor + carry;
                self.set_digit(i, (cur & BASE_MASK) as u16);
                carry = cur >> BASE_BITS;
            }
            while carry != 0 {
                self.push_digit((carry & BASE_MASK) as u16);
                carry >>= BASE_BITS;
            }
        }
    }

    unsafe fn add_small_assign(&mut self, value: u32) {
        unsafe {
            let mut carry = value;
            let mut index = 0;
            while carry != 0 {
                let cur = self.digit(index) as u32 + carry;
                self.set_digit(index, (cur & BASE_MASK) as u16);
                carry = cur >> BASE_BITS;
                index += 1;
            }
        }
    }

    unsafe fn shl1_assign(&mut self) {
        unsafe {
            let mut carry = 0u32;
            for i in 0..self.len {
                let cur = (self.digit(i) as u32) * 2 + carry;
                self.set_digit(i, (cur & BASE_MASK) as u16);
                carry = cur >> BASE_BITS;
            }
            if carry != 0 {
                self.push_digit(carry as u16);
            }
        }
    }

    unsafe fn set_bit(&mut self, bit: usize) {
        unsafe {
            let digit = bit / BASE_BITS;
            let offset = bit % BASE_BITS;
            let value = self.digit(digit) | (1u16 << offset);
            self.set_digit(digit, value);
        }
    }

    unsafe fn bit(&self, bit: usize) -> bool {
        unsafe {
            let digit = bit / BASE_BITS;
            let offset = bit % BASE_BITS;
            (self.digit(digit) & (1u16 << offset)) != 0
        }
    }

    fn bit_len(&self) -> usize {
        let top = unsafe { self.digit(self.len - 1) };
        (self.len - 1) * BASE_BITS + (u16::BITS as usize - top.leading_zeros() as usize)
    }
}

impl Drop for Big {
    fn drop(&mut self) {
        unsafe { self.alloc.free(self.ptr.cast()) }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn decimal_roundtrip_and_basic_arithmetic() {
        unsafe {
            extern "C" fn malloc(size: usize) -> *mut std::ffi::c_void {
                unsafe {
                    std::alloc::alloc(std::alloc::Layout::from_size_align(size, 16).unwrap()).cast()
                }
            }
            extern "C" fn calloc(count: usize, size: usize) -> *mut std::ffi::c_void {
                let total = count * size;
                unsafe {
                    let ptr = std::alloc::alloc_zeroed(
                        std::alloc::Layout::from_size_align(total, 16).unwrap(),
                    );
                    ptr.cast()
                }
            }
            extern "C" fn realloc(
                _ptr: *mut std::ffi::c_void,
                _size: usize,
            ) -> *mut std::ffi::c_void {
                unimplemented!()
            }
            extern "C" fn free(_ptr: *mut std::ffi::c_void) {}
            extern "C" fn new_array(size: usize) -> *mut std::ffi::c_void {
                malloc(size)
            }
            extern "C" fn delete_array(_ptr: *mut std::ffi::c_void) {}
            extern "C" fn delete_array_sized(_ptr: *mut std::ffi::c_void, _size: usize) {}
            extern "C" fn memalign(_alignment: usize, size: usize) -> *mut std::ffi::c_void {
                malloc(size)
            }
            static ALLOC: super::super::DylibAllocator = super::super::DylibAllocator {
                malloc,
                calloc,
                realloc,
                free,
                memalign,
                new_array,
                delete_array,
                delete_array_sized,
            };
            let alloc = super::BenchAllocator::Rust(&ALLOC);
            let a = super::Big::from_decimal(alloc, "17545186520507317056371138836327483792789528");
            assert_eq!(
                a.to_decimal(),
                "17545186520507317056371138836327483792789528"
            );
            let b = super::Big::from_u64(alloc, 856070387728264);
            let c = super::Big::from_decimal(alloc, "20495027946319472471219512627");
            assert_eq!(b.mul(&c).to_decimal(), a.to_decimal());
            let (q, r) = a.div_rem(&b);
            assert!(r.is_zero());
            assert_eq!(q.to_decimal(), c.to_decimal());
        }
    }
}
