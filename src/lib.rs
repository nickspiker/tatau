//! tatau — every number a person reads, in the base they chose.
//!
//! Numbers live in binary; the base is chosen only where text is drawn, and the person chooses it. Four bases:
//!
//! - **Arabic** — decimal, with unit words: the ledger world's notation.
//! - **Hex** — linear counts in hexadecimal, for coders: ages are seconds, sizes are bits, a clock is seconds since midnight, a date is the raw stamp.
//! - **DMS**, dozenal magnitude scale — a pure logarithm: the count of doublings, in dozenal. A byte is Ter (three doublings of a bit), a megabyte Zila Stelor. Time's unit is one oscillation of the hydrogen line, so a second is a landmark on the ladder, not a unit.
//! - **DLS**, dozenal linear scale — the value itself in dozenal, signed with an arrow and followed by its SI unit: half a second is `↑Zil·Lun Zil Zil s` at three significant digits, two seconds `↑Zilor·Zil Zil s`.
//!
//! Time of day and dates read the same in DMS and DLS: a share of the day after a radix point (`·Tera Lunor` is 9:20), and `year month day weekday ·share` (months zero-indexed, weekdays as words because seven is coprime to twelve).
//!
//! Two styles for the digits, by surface: **glyphs** where the font is ours (the Oxanium `+glyphs` face draws the twelve dozenal digits at G#10..G#1B), **words** everywhere else — terminals, logs, anything read aloud — `Zil` through `Stelor`. The radix point is `·` and the sign arrows `↑` `↓` in both.
//!
//! Integer arithmetic throughout (no floats, no rounding drift: a third is exactly `·Tera`), output through `core::fmt::Write`, no allocation, no dependencies — so a kernel can use it as freely as an app. The conventions come from Photon (Nick, 2026-09 and 10), where the first three bases were worked out; DLS is the fourth (2026-10-22).

#![no_std]

use core::fmt::{self, Write};
use core::sync::atomic::{AtomicU8, Ordering};

/// The four bases. The discriminant is what a settings store keeps (`display.base`): the radix for the first three, so Photon's stored 10/12/16 read unchanged, and G#8C (twelve with the linear bit) for DLS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Base {
    Arabic = 10,
    Dms = 12,
    Hex = 16,
    Dls = 0x8C,
}

impl Base {
    pub fn from_code(c: u8) -> Option<Base> {
        match c {
            10 => Some(Base::Arabic),
            12 => Some(Base::Dms),
            16 => Some(Base::Hex),
            0x8C => Some(Base::Dls),
            _ => None,
        }
    }
    pub fn code(self) -> u8 {
        self as u8
    }
    /// Whether digits are dozenal.
    pub fn dozenal(self) -> bool {
        matches!(self, Base::Dms | Base::Dls)
    }
}

/// How dozenal digits are drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// The Oxanium `+glyphs` face's digit characters, G#10..G#1B. Only where the font is ours.
    Glyphs,
    /// Digit words, space-separated: for terminals, logs and reading aloud.
    Words,
}

/// The dozenal digit names, 0 to 11.
pub const NAMES: [&str; 12] = ["Zil", "Zila", "Zilor", "Ter", "Tera", "Teror", "Lun", "Luna", "Lunor", "Stel", "Stela", "Stelor"];
/// Where the `+glyphs` face puts dozenal digit 0; digit d is this plus d.
pub const GLYPH_ZERO: u32 = 0x10;
/// The radix point.
pub const POINT: char = '\u{00B7}';
/// The sign of a signed or linear dozenal value, always shown; zero takes the up arrow so a column aligns.
pub const UP: char = '\u{2191}';
pub const DOWN: char = '\u{2193}';

/// One Eagle second is exactly this many periods of the hydrogen line (vsf's Eagle time defines it the same way): DMS counts time in these.
pub const OSC_PER_SECOND: u64 = 1_420_407_826;

/// The process-wide choice (default: DMS, the house base), for callers that do not carry their own. Set it from the settings store at start.
static BASE: AtomicU8 = AtomicU8::new(Base::Dms as u8);

pub fn base() -> Base {
    Base::from_code(BASE.load(Ordering::Relaxed)).unwrap_or(Base::Dms)
}
pub fn set_base(b: Base) {
    BASE.store(b.code(), Ordering::Relaxed);
}

/// What a formatter needs to know: the base, the digit style, and for DLS how many significant digits.
#[derive(Clone, Copy, Debug)]
pub struct Fmt {
    pub base: Base,
    pub style: Style,
    /// DLS significant digits (leading zeros after the point do not count).
    pub sig: usize,
}

impl Fmt {
    /// The process-wide base, in `style`, three significant digits.
    pub fn current(style: Style) -> Fmt {
        Fmt { base: base(), style, sig: 3 }
    }
    pub fn new(base: Base, style: Style) -> Fmt {
        Fmt { base, style, sig: 3 }
    }
}

/// Write dozenal digits: words are separated by spaces except across the radix point (`Zil·Lun`, `Tera Lunor`); glyphs are packed. `point` is how many digits stand before the point (None: no point).
fn dozenal(w: &mut impl Write, style: Style, digits: &[u8], point: Option<usize>) -> fmt::Result {
    for (i, &d) in digits.iter().enumerate() {
        if point == Some(i) {
            w.write_char(POINT)?;
        } else if i > 0 && style == Style::Words {
            w.write_char(' ')?;
        }
        match style {
            Style::Glyphs => w.write_char(char::from_u32(GLYPH_ZERO + d as u32).unwrap_or('?'))?,
            Style::Words => w.write_str(NAMES[d as usize])?,
        }
    }
    if point == Some(digits.len()) {
        w.write_char(POINT)?;
    }
    Ok(())
}

/// The dozenal digits of `n`, most significant first, into `buf`; returns how many.
fn digits12(mut n: u128, buf: &mut [u8; 64]) -> usize {
    let mut tmp = [0u8; 64];
    let mut k = 0;
    loop {
        tmp[k] = (n % 12) as u8;
        k += 1;
        n /= 12;
        if n == 0 || k == 64 {
            break;
        }
    }
    for i in 0..k {
        buf[i] = tmp[k - 1 - i];
    }
    k
}

/// A whole number in dozenal, no sign and no point.
pub fn dozenal_whole(w: &mut impl Write, style: Style, n: u64) -> fmt::Result {
    let mut buf = [0u8; 64];
    let k = digits12(n as u128, &mut buf);
    dozenal(w, style, &buf[..k], None)
}

/// The linear value `num / den` in dozenal at `sig` significant digits (truncated, as a clock or an odometer reads), signed with an arrow: the DLS number. The whole part is always written (Zil when it is nothing); fraction digits follow until `sig` significant digits are out, leading zeros after the point not counting.
pub fn dls_value(w: &mut impl Write, style: Style, negative: bool, num: u128, den: u128, sig: usize) -> fmt::Result {
    w.write_char(if negative { DOWN } else { UP })?;
    let den = den.max(1);
    let whole = num / den;
    let mut rem = num % den;
    let mut buf = [0u8; 64];
    let k = digits12(whole, &mut buf);
    let mut counted = if whole == 0 { 0 } else { k };
    let mut n = k;
    let mut point = None;
    let mut guard = 0;
    while counted < sig.max(1) && n < 64 && guard < 40 {
        if point.is_none() {
            point = Some(k);
        }
        rem *= 12;
        let d = (rem / den) as u8;
        rem %= den;
        buf[n] = d;
        n += 1;
        if counted > 0 || d != 0 {
            counted += 1;
        }
        guard += 1;
    }
    dozenal(w, style, &buf[..n], point)
}

/// floor(log2(x)) and `frac` dozenal digits of the fraction of a doubling, for x = num/den >= 1, in integer arithmetic: the mantissa is held in Q62 and raised to the twelfth power once per digit (the twelfth power of a number in [1, 2) lies in [1, 4096), and its bit length less one is the next digit). Each digit is a twelfth of a doubling, then a gross-th; two digits resolve about half a percent.
pub fn log2_dozenal(num: u128, den: u128, frac: usize, digits: &mut [u8; 64]) -> Option<(u32, usize)> {
    if num < den || den == 0 {
        return None;
    }
    let mut q = num / den;
    let mut whole = 0u32;
    while q > 1 {
        q >>= 1;
        whole += 1;
    }
    // Mantissa m = num / (den * 2^whole) in [1, 2), Q62.
    const ONE: u128 = 1 << 62;
    let m_num = num << 62;
    let m_den = den << whole;
    let mut m = m_num / m_den;
    let mut n = 0;
    for _ in 0..frac.min(16) {
        // m^12 by squaring: m^2, m^4, m^8, then m^8 * m^4. Values stay below 2^74 in Q62, renormalised by a power of two after each product to stay in range.
        let mut e = 0u32; // accumulated power of two pulled out
        let mul = |a: u128, b: u128| -> u128 { (a >> 31) * (b >> 31) };
        let norm = |v: &mut u128, e: &mut u32| {
            while *v >= 2 * ONE {
                *v >>= 1;
                *e += 1;
            }
        };
        let mut m2 = mul(m, m);
        norm(&mut m2, &mut e);
        let e2 = e;
        let mut m4 = mul(m2, m2);
        let mut e4 = 2 * e2;
        norm(&mut m4, &mut e4);
        let mut m8 = mul(m4, m4);
        let mut e8 = 2 * e4;
        norm(&mut m8, &mut e8);
        let mut m12 = mul(m8, m4);
        let mut e12 = e8 + e4;
        norm(&mut m12, &mut e12);
        // m^12 = m12 * 2^e12 with m12 in [1, 2): the digit is e12 (0..11), the next mantissa m12.
        digits[n] = e12.min(11) as u8;
        n += 1;
        m = m12;
    }
    Some((whole, n))
}

/// A DMS doubling count: `whole` in dozenal, then `frac` digits after the point when there are any.
fn dms_digits(w: &mut impl Write, style: Style, whole: u32, frac: &[u8]) -> fmt::Result {
    let mut buf = [0u8; 64];
    let k = digits12(whole as u128, &mut buf);
    let n = k + frac.len();
    buf[k..n].copy_from_slice(frac);
    dozenal(w, style, &buf[..n], if frac.is_empty() { None } else { Some(k) })
}

/// What a formatter writes when DMS has nothing to take the logarithm of: zero has no doublings. The caller writes its own word for it ("now", "empty").
#[derive(Debug, PartialEq, Eq)]
pub enum Out {
    Written,
    /// A DMS value of zero: nothing was written.
    Nothing,
}

/// A COUNT — peers, messages, files, lines. Arabic decimal; hex linear; DMS the doubling count with `frac` digits (one keeps two and three apart: Zila and Zila·Luna); DLS the count itself.
pub fn count(w: &mut impl Write, f: Fmt, n: u64, frac: usize) -> Result<Out, fmt::Error> {
    match f.base {
        Base::Arabic => write!(w, "{n}")?,
        Base::Hex => write!(w, "{n:X}")?,
        Base::Dms => {
            let mut d = [0u8; 64];
            let Some((whole, k)) = log2_dozenal(n as u128, 1, frac, &mut d) else { return Ok(Out::Nothing) };
            dms_digits(w, f.style, whole, &d[..k])?;
        }
        Base::Dls => dls_value(w, f.style, false, n as u128, 1, f.sig.max(digits_needed(n)))?,
    }
    Ok(Out::Written)
}

/// How many dozenal digits a whole count needs (a count in DLS shows all of them, no point).
fn digits_needed(n: u64) -> usize {
    let mut buf = [0u8; 64];
    digits12(n as u128, &mut buf)
}

/// A DURATION in nanoseconds. Arabic in ms below a second, else seconds; hex seconds with three hex fraction digits (a quarter-millisecond step), bare; DMS the doublings of hydrogen-line oscillations, bare (a second is about Zilor Stelor Lun... of them); DLS seconds, `↑Zil·Lun Zil Zil s`.
pub fn duration_ns(w: &mut impl Write, f: Fmt, ns: u64, frac: usize) -> Result<Out, fmt::Error> {
    match f.base {
        Base::Arabic => {
            if ns < 1_000_000_000 {
                write!(w, "{} ms", ns / 1_000_000)?
            } else {
                let cs = ns / 10_000_000;
                write!(w, "{}.{:02} s", cs / 100, cs % 100)?
            }
        }
        Base::Hex => {
            let whole = ns / 1_000_000_000;
            let frac16 = ((ns % 1_000_000_000) as u128 * 4096 / 1_000_000_000) as u64;
            write!(w, "{whole:X}.{frac16:03X}")?
        }
        Base::Dms => {
            let osc = ns as u128 * OSC_PER_SECOND as u128 / 1_000_000_000;
            let mut d = [0u8; 64];
            let Some((whole, k)) = log2_dozenal(osc, 1, frac, &mut d) else { return Ok(Out::Nothing) };
            dms_digits(w, f.style, whole, &d[..k])?;
        }
        Base::Dls => {
            dls_value(w, f.style, false, ns as u128, 1_000_000_000, f.sig)?;
            w.write_str(" s")?;
        }
    }
    Ok(Out::Written)
}

/// A SIZE, counted in BITS everywhere a base is not arabic ("I say bits"). Arabic bytes with binary prefixes; hex the bit count; DMS doublings of a bit (a byte is Ter, a kilobyte Zila Zila); DLS the bit count with its unit, `b`.
pub fn size_bits(w: &mut impl Write, f: Fmt, bits: u64, frac: usize) -> Result<Out, fmt::Error> {
    match f.base {
        Base::Arabic => {
            let bytes = bits / 8;
            const P: [&str; 7] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"];
            let mut i = 0;
            let mut v = bytes;
            while v >= 1024 && i < 6 {
                v /= 1024;
                i += 1;
            }
            write!(w, "{v} {}", P[i])?
        }
        Base::Hex => write!(w, "{bits:X}")?,
        Base::Dms => {
            let mut d = [0u8; 64];
            let Some((whole, k)) = log2_dozenal(bits as u128, 1, frac, &mut d) else { return Ok(Out::Nothing) };
            dms_digits(w, f.style, whole, &d[..k])?;
        }
        Base::Dls => {
            dls_value(w, f.style, false, bits as u128, 1, f.sig.max(digits_needed(bits)))?;
            w.write_str(" b")?;
        }
    }
    Ok(Out::Written)
}

/// A SHARE of a whole, `part / whole`: a radix point and the digits after it, computed in integer base twelve so a third is exactly `·Tera`. Arabic a percentage; hex three hex fraction digits. The whole itself reads as one, not as the largest fraction.
pub fn share(w: &mut impl Write, f: Fmt, part: u64, whole: u64, width: usize) -> fmt::Result {
    let whole = whole.max(1);
    let part = part.min(whole);
    match f.base {
        Base::Arabic => write!(w, "{}%", part as u128 * 100 / whole as u128),
        Base::Hex => {
            if part == whole {
                return w.write_str("1");
            }
            write!(w, ".{:03X}", part as u128 * 4096 / whole as u128)
        }
        Base::Dms | Base::Dls => {
            if part == whole {
                return dozenal(w, f.style, &[1], None);
            }
            let mut buf = [0u8; 64];
            let mut rem = part as u128;
            let mut n = 0;
            let mut counted = 0;
            while counted < width.max(1) && n < 32 {
                rem *= 12;
                let d = (rem / whole as u128) as u8;
                rem %= whole as u128;
                buf[n] = d;
                n += 1;
                if counted > 0 || d != 0 {
                    counted += 1;
                }
            }
            dozenal(w, f.style, &buf[..n], Some(0))
        }
    }
}

/// TIME OF DAY from the time since local midnight. Arabic `HH:MM:SS`; hex seconds since midnight, linear; DMS and DLS a FIXED-width share of the day (`·Tera Lunor` is 9:20 at two digits; a clock's trailing Zil is a position, so it is kept).
pub fn time_of_day(w: &mut impl Write, f: Fmt, ns_since_midnight: u64, width: usize) -> fmt::Result {
    const DAY: u128 = 86_400_000_000_000;
    let t = ns_since_midnight as u128 % DAY;
    match f.base {
        Base::Arabic => {
            let s = (t / 1_000_000_000) as u64;
            write!(w, "{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
        }
        Base::Hex => write!(w, "{:X}", t / 1_000_000_000),
        Base::Dms | Base::Dls => {
            let width = width.clamp(1, 20);
            let mut v = t * 12u128.pow(width as u32) / DAY;
            let mut buf = [0u8; 64];
            for i in (0..width).rev() {
                buf[i] = (v % 12) as u8;
                v /= 12;
            }
            dozenal(w, f.style, &buf[..width], Some(0))
        }
    }
}

/// Weekday words, Monday first (seven is coprime to twelve, so weekdays are never digits).
pub const WEEKDAYS: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];

/// A civil date (proleptic Gregorian) from days since 1970-01-01: (year, month 0..11, day 1..31, weekday 0 = Monday). Integer only (Howard Hinnant's days-to-civil).
pub fn civil_from_days(days: i64) -> (i64, u32, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
    let weekday = (days + 3).rem_euclid(7) as u32; // 1970-01-01 was a Thursday
    (y, m - 1, d, weekday)
}

/// A DATE AND TIME from local seconds since 1970-01-01 (the caller applies its zone) and the nanoseconds within the second. Arabic `Mon 2026-10-22 09:20:00`; hex the raw Eagle stamp (oscillations since the Eagle epoch), no calendar and no zone; DMS and DLS `year month day weekday ·share` with four share digits (four-second steps).
pub fn date_time(w: &mut impl Write, f: Fmt, local_unix_secs: i64, ns: u32, eagle_osc: i64) -> fmt::Result {
    let days = local_unix_secs.div_euclid(86_400);
    let sod = local_unix_secs.rem_euclid(86_400) as u64;
    let (y, m0, d, wd) = civil_from_days(days);
    match f.base {
        Base::Hex => write!(w, "{:X}", eagle_osc.max(0)),
        Base::Arabic => write!(w, "{} {:04}-{:02}-{:02} {:02}:{:02}:{:02}", &WEEKDAYS[wd as usize][..3], y, m0 + 1, d, sod / 3600, sod / 60 % 60, sod % 60),
        Base::Dms | Base::Dls => {
            dozenal_whole(w, f.style, y.max(0) as u64)?;
            w.write_char(' ')?;
            dozenal_whole(w, f.style, m0 as u64)?;
            w.write_char(' ')?;
            dozenal_whole(w, f.style, d as u64)?;
            write!(w, " {} ", WEEKDAYS[wd as usize])?;
            time_of_day(w, f, sod * 1_000_000_000 + ns as u64, 4)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate std;
    use std::string::String;

    fn words(base: Base) -> Fmt {
        Fmt::new(base, Style::Words)
    }
    fn s(f: impl FnOnce(&mut String) -> fmt::Result) -> String {
        let mut o = String::new();
        f(&mut o).unwrap();
        o
    }

    /// The owner's own DLS examples (2026-10-22), at the significant-digit counts that produce them, and at the default three.
    #[test]
    fn dls_durations() {
        let mut f = words(Base::Dls);
        f.sig = 1;
        assert_eq!(s(|o| duration_ns(o, f, 500_000_000, 0).map(|_| ())), "↑Zil·Lun s");
        assert_eq!(s(|o| duration_ns(o, f, 2_000_000_000, 0).map(|_| ())), "↑Zilor s");
        f.sig = 3;
        assert_eq!(s(|o| duration_ns(o, f, 500_000_000, 0).map(|_| ())), "↑Zil·Lun Zil Zil s");
        assert_eq!(s(|o| duration_ns(o, f, 2_000_000_000, 0).map(|_| ())), "↑Zilor·Zil Zil s");
    }

    /// A third is exactly ·Tera (four twelfths) — the dozenal argument, kept exact by integer arithmetic.
    #[test]
    fn shares_are_exact() {
        assert_eq!(s(|o| share(o, words(Base::Dms), 1, 3, 1)), "·Tera");
        assert_eq!(s(|o| share(o, words(Base::Dms), 1, 2, 1)), "·Lun");
        assert_eq!(s(|o| share(o, words(Base::Dms), 3, 3, 1)), "Zila");
    }

    /// Photon's anchors: three files is Zila·Luna (log2 3 = 1.585, seven twelfths of a doubling); a byte is Ter doublings of a bit; a kilobyte Zila Zila.
    #[test]
    fn dms_magnitudes() {
        assert_eq!(s(|o| count(o, words(Base::Dms), 3, 1).map(|_| ())), "Zila·Luna");
        assert_eq!(s(|o| count(o, words(Base::Dms), 1, 0).map(|_| ())), "Zil");
        assert_eq!(s(|o| size_bits(o, words(Base::Dms), 8, 0).map(|_| ())), "Ter");
        assert_eq!(s(|o| size_bits(o, words(Base::Dms), 8 * 1024, 0).map(|_| ())), "Zila Zila");
        assert_eq!(count(&mut String::new(), words(Base::Dms), 0, 0).unwrap(), Out::Nothing);
    }

    /// 9:20 is ·Tera Lunor (Photon's clock test); noon ·Lun.
    #[test]
    fn day_share() {
        let t920 = (9 * 3600 + 20 * 60) * 1_000_000_000u64;
        assert_eq!(s(|o| time_of_day(o, words(Base::Dls), t920, 2)), "·Tera Lunor");
        assert_eq!(s(|o| time_of_day(o, words(Base::Dms), 43_200_000_000_000, 1)), "·Lun");
        assert_eq!(s(|o| time_of_day(o, words(Base::Arabic), t920, 2)), "09:20:00");
        assert_eq!(s(|o| time_of_day(o, words(Base::Hex), t920, 2)), "8340");
    }

    /// Glyphs are the +glyphs face's code points, packed.
    #[test]
    fn glyphs() {
        assert_eq!(s(|o| dozenal_whole(o, Style::Glyphs, 13)), "\u{11}\u{11}");
    }

    #[test]
    fn dates() {
        // 2026-10-22 was a Thursday.
        let days = 20_748;
        assert_eq!(civil_from_days(days), (2026, 9, 22, 3));
        assert_eq!(s(|o| date_time(o, words(Base::Arabic), days * 86_400 + 9 * 3600 + 20 * 60, 0, 0)), "Thu 2026-10-22 09:20:00");
    }

    #[test]
    fn other_bases() {
        assert_eq!(s(|o| duration_ns(o, words(Base::Arabic), 562_000_000, 0).map(|_| ())), "562 ms");
        assert_eq!(s(|o| duration_ns(o, words(Base::Hex), 500_000_000, 0).map(|_| ())), "0.800");
        assert_eq!(s(|o| size_bits(o, words(Base::Arabic), 8 * 4096, 0).map(|_| ())), "4 KiB");
        assert_eq!(s(|o| size_bits(o, words(Base::Dls), 8, 0).map(|_| ())), "↑Lunor·Zil Zil b");
    }
}
