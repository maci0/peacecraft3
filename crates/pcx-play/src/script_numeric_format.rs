//! Selected native real formatting; integer decimal rendering surrounds the
//! same source-qualified arithmetic used by script expressions.
use super::{Binding as NumericBinding, Error, Problem, at, call, word};
use crate::camera_margins::SourceWindow;
use crate::neutral_player::{RuleInputBudget, Sha256Digest, sha256};
use crate::neutral_rule_input::Pe;
use crate::script_random::real;

#[derive(Clone, Debug)]
pub struct Binding {
    pub image_sha256: Sha256Digest,
    pub r2s: SourceWindow,
    pub r2sw: SourceWindow,
    pub body: SourceWindow,
    pub floor: SourceWindow,
    pub truncate: SourceWindow,
    pub rounding: SourceWindow,
    pub large_limit: SourceWindow,
    pub format: SourceWindow,
    pub integer_format: SourceWindow,
}

#[derive(Debug)]
pub(super) struct Formatter {
    default_width: i32,
    default_precision: i32,
    negative_precision: i32,
    maximum_precision: i32,
    radix: i32,
    capacity: usize,
    rounding: u32,
    large_threshold: u32,
    large_factor: u32,
    large_limit: u32,
    minimum_width: i32,
    minimum_fraction: i32,
    trim_minimum: i32,
    trim_divisor: i32,
    sign: char,
    separator: char,
    zero: char,
    infinity: String,
}

impl Formatter {
    pub(super) fn read(
        image: &[u8],
        binding: &Binding,
        numeric: &NumericBinding,
        budget: RuleInputBudget,
    ) -> Result<Self, Error> {
        let fail = |problem| Error {
            source_context: numeric.origin.source.clone(),
            problem,
        };
        let windows = [
            (&binding.r2s, true),
            (&binding.r2sw, true),
            (&binding.body, true),
            (&binding.floor, true),
            (&binding.truncate, true),
            (&binding.rounding, false),
            (&binding.large_limit, false),
            (&binding.format, false),
            (&binding.integer_format, false),
        ];
        if image.len() > budget.image_bytes
            || windows.iter().map(|(w, _)| u64::from(w.len)).sum::<u64>()
                > budget.profile_bytes as u64
        {
            return Err(fail(Problem::Budget));
        }
        if binding.image_sha256 != numeric.image_sha256 || sha256(image) != binding.image_sha256 {
            return Err(fail(Problem::Identity));
        }
        for (index, (left, _)) in windows.iter().enumerate() {
            let end = left
                .va
                .checked_add(left.len)
                .ok_or_else(|| fail(Problem::Shape))?;
            if windows[index + 1..].iter().any(|(right, _)| {
                right
                    .va
                    .checked_add(right.len)
                    .is_none_or(|r| left.va < r && right.va < end)
            }) {
                return Err(fail(Problem::Shape));
            }
        }
        let pe = Pe::parse(image, budget).map_err(|e| fail(Problem::Image(e)))?;
        let open = |window, code| pe.window(window, code).map_err(|e| fail(Problem::Image(e)));
        let native = open(&binding.r2s, true)?;
        let wide = open(&binding.r2sw, true)?;
        let body = open(&binding.body, true)?;
        let floor = open(&binding.floor, true)?;
        let truncate = open(&binding.truncate, true)?;
        let rounding = open(&binding.rounding, false)?;
        let limit = open(&binding.large_limit, false)?;
        let format = open(&binding.format, false)?;
        let integer_format = open(&binding.integer_format, false)?;
        let decode = (|| {
            if native.len() != 104
                || wide.len() != 106
                || body.len() != 0x2b0
                || floor.len() != 152
                || truncate.len() != 80
                || rounding.len() != 4
                || limit.len() != 4
                || format.len() != 9
                || integer_format != b"%d\0"
                || !at(native, 0, &[0x55, 0x8b, 0xec, 0x81, 0xec])
                || !at(wide, 0, &[0x55, 0x8b, 0xec, 0x81, 0xec])
                || !at(native, 0x10, &[0x89, 0x45, 0xfc])
                || !at(wide, 0x10, &[0x89, 0x45, 0xfc])
                || !at(native, 0x16, &[0x8d, 0x8d])
                || !at(wide, 0x19, &[0x8d, 0x8d])
                || !at(native, 0x1c, &[0x6a])
                || !at(native, 0x1e, &[0x6a])
                || !at(wide, 0x13, &[0xff, 0x75, 0x10])
                || !at(wide, 0x1f, &[0xff, 0x75, 0x0c])
                || call(native, binding.r2s.va, 0x22) != Some(binding.body.va)
                || call(wide, binding.r2sw.va, 0x24) != Some(binding.body.va)
                || !at(body, 0, &[0x55, 0x8b, 0xec, 0x8b, 0x55, 8])
                || !at(body, 0x18, &[0x85, 0xc8, 0x74, 4, 0xc6, 3])
                || !at(body, 0x2f, &[0x81, 0xe1, 0, 0, 0x80, 0x7f, 0x81, 0xf9])
                || !at(body, 0x46, &[0x0f, 0x2f, 5])
                || word(body, 0x49) != Some(binding.large_limit.va)
                || !at(body, 0x51, &[0xc7, 3])
                || !at(body, 0x6b, &[0xc7, 0x45, 0xf4])
                || !at(body, 0xd8, &[0xbe])
                || !at(body, 0xdd, &[0xc6, 3])
                || !at(body, 0x110, &[0xbb])
                || !at(body, 0x16d, &[0xb9])
                || !at(body, 0x177, &[0x8d, 0x51])
                || !at(body, 0x17f, &[0x8d, 0x72])
                || !at(body, 0x1f1, &[0x68])
                || word(body, 0x1f2) != Some(binding.rounding.va)
                || !at(body, 0x287, &[0xb9])
                || !at(body, 0x299, &[0x68])
                || word(body, 0x29a) != Some(binding.format.va)
                || !at(body, 0xa8, &[0x68])
                || word(body, 0xa9) != Some(binding.integer_format.va)
                || !at(
                    floor,
                    0,
                    &[0x55, 0x8b, 0xec, 0x8b, 0x45, 0x0c, 0x56, 0x8b, 0x10],
                )
                || !at(floor, 0x11, &[0x83, 0xee, 0x7f])
                || !at(floor, 0x35, &[0x83, 0xfe, 0x17])
                || !at(
                    truncate,
                    0,
                    &[0x55, 0x8b, 0xec, 0x8b, 0x45, 8, 0x56, 0x8b, 0x30],
                )
                || !at(truncate, 0x11, &[0x83, 0xf8, 0x7f])
                || format.get(..3) != Some(b"%*d")
                || format.get(4..) != Some(b"%0*d\0")
            {
                return None;
            }
            for (offset, target) in [
                (0x7b, numeric.math_mul.va),
                (0xa2, binding.truncate.va),
                (0x1c8, numeric.math_sub.va),
                (0x1de, numeric.math_i32_to_real.va),
                (0x1ec, numeric.math_mul.va),
                (0x1fe, numeric.math_add.va),
                (0x208, binding.floor.va),
            ] {
                if call(body, binding.body.va, offset) != Some(target) {
                    return None;
                }
            }
            // The scale loop derives both its radix and neutral element from these operands.
            let maximum_precision = i32::try_from(word(body, 0x16e)?).ok()?;
            let radix = maximum_precision.checked_add(i32::from(*body.get(0x179)? as i8))?;
            let identity = radix.checked_add(i32::from(*body.get(0x181)? as i8))?;
            let negative_precision = i32::try_from(word(body, 0x111)?).ok()?;
            let default_precision = i32::from(*native.get(0x1d)? as i8);
            let default_width = i32::from(*native.get(0x1f)? as i8);
            let displacement = word(native, 0x18)? as i32;
            let wide_displacement = word(wide, 0x1b)? as i32;
            let capacity =
                usize::try_from(i32::from(native[0x12] as i8).checked_sub(displacement)?).ok()?;
            if displacement != wide_displacement
                || word(native, 5)? != displacement.unsigned_abs()
                || word(wide, 5)? != displacement.unsigned_abs()
                || identity != 1
                || radix < 2
                || maximum_precision < 0
                || negative_precision < 0
                || default_precision < 0
                || capacity == 0
                || capacity > budget.profile_bytes
            {
                return None;
            }
            let mut scale = identity;
            // Bound profile work before walking a supplied precision cap.
            if usize::try_from(maximum_precision).ok()? > capacity {
                return None;
            }
            for _ in 0..maximum_precision {
                scale = scale.checked_mul(radix)?;
            }
            if !at(body, 0x25a, &[0x83, 0xfb])
                || !at(body, 0x260, &[0xb8])
                || !at(body, 0x269, &[0xc1, 0xfa])
                || !at(body, 0x273, &[0x8d, 0x0c])
                || !at(body, 0x276, &[0x03, 0xc9])
                || !at(body, 0x27f, &[0x83, 0xfb])
            {
                return None;
            }
            let trim_minimum = i32::from(body[0x25c]);
            let trim_divisor =
                (1i32.checked_shl(u32::from(body[0x275] >> 6))? + 1).checked_mul(2)?;
            let shift = u32::from(body[0x26b]);
            if body[0x281] != body[0x25c]
                || shift >= 31
                || u64::from(word(body, 0x261)?)
                    != (1u64.checked_shl(32 + shift)? / trim_divisor as u64) + 1
            {
                return None;
            }
            let minimum_width = i32::try_from(word(body, 0x288)?).ok()?;
            let minimum_fraction = i32::try_from(word(body, 0xd9)?).ok()?;
            let rounding = word(rounding, 0)?;
            let large_limit = word(limit, 0)?;
            let large_threshold = word(body, 0x37)?;
            let large_factor = word(body, 0x6e)?;
            let sign = body[0x1e];
            let separator = format[3];
            let zero = body[0xc3];
            let infinity_bytes = body.get(0x53..0x57)?;
            let end = infinity_bytes.iter().position(|b| *b == 0)?;
            let infinity = std::str::from_utf8(&infinity_bytes[..end])
                .ok()?
                .to_string();
            if minimum_width < 1
                || minimum_fraction < 1
                || separator != body[0xdf]
                || ![sign, separator, zero]
                    .into_iter()
                    .all(|b| b.is_ascii_graphic())
                || body.get(0xc3..0xc7)?.iter().any(|b| *b != zero)
                || !f32::from_bits(rounding).is_finite()
                || !(0.0..1.0).contains(&f32::from_bits(rounding))
                || !(0.0..1.0).contains(&f32::from_bits(large_factor))
                || large_factor == 0
                || large_threshold != ((127 + 31) << 23)
                || large_limit < large_threshold
                || large_limit >= f32::INFINITY.to_bits()
                || infinity.is_empty()
            {
                return None;
            }
            Some(Self {
                default_width,
                default_precision,
                negative_precision,
                maximum_precision,
                radix,
                capacity,
                rounding,
                large_threshold,
                large_factor,
                large_limit,
                minimum_width,
                minimum_fraction,
                trim_minimum,
                trim_divisor,
                sign: char::from(sign),
                separator: char::from(separator),
                zero: char::from(zero),
                infinity,
            })
        })();
        decode.ok_or_else(|| fail(Problem::Shape))
    }

    pub(super) fn render(
        &self,
        value: f32,
        width_precision: Option<(i32, i32)>,
    ) -> Result<String, String> {
        let (width, requested) =
            width_precision.unwrap_or((self.default_width, self.default_precision));
        if value.is_nan() {
            return Err("unsupported real formatter NaN input".into());
        }
        let mut magnitude = value.to_bits();
        let negative = magnitude >> 31 != 0 && real::abs(magnitude) != 0;
        magnitude = real::abs(magnitude);
        let sign = usize::from(negative);
        if magnitude >= self.large_threshold {
            if magnitude >= self.large_limit {
                let mut out = self.allocate(
                    sign.checked_add(self.infinity.len())
                        .ok_or("formatter length overflow")?,
                )?;
                if negative {
                    out.push(self.sign);
                }
                out.push_str(&self.infinity);
                return Ok(out);
            }
            let mut zeros = 0usize;
            while magnitude >= self.large_threshold {
                zeros = zeros
                    .checked_add(1)
                    .filter(|n| *n < self.capacity)
                    .ok_or("formatter scale exceeds output capacity")?;
                magnitude = real::mul(magnitude, self.large_factor)
                    .map_err(|_| "unsupported formatter scaling")?;
            }
            let whole = real::truncate(magnitude)
                .map_err(|_| "unsupported formatter integer")?
                .to_string();
            let digits = usize::try_from(requested.max(self.minimum_fraction))
                .map_err(|_| "formatter precision overflow")?;
            let bytes = sign
                .checked_add(whole.len())
                .and_then(|n| n.checked_add(zeros))
                .and_then(|n| n.checked_add(1))
                .and_then(|n| n.checked_add(digits))
                .ok_or("formatter length overflow")?;
            let mut out = self.allocate(bytes)?;
            if negative {
                out.push(self.sign);
            }
            out.push_str(&whole);
            out.extend(std::iter::repeat_n(self.zero, zeros));
            out.push(self.separator);
            out.extend(std::iter::repeat_n(self.zero, digits));
            return Ok(out);
        }
        let trim = requested < 0;
        let mut precision = (if trim {
            self.negative_precision
        } else {
            requested
        })
        .min(self.maximum_precision);
        let scale = self
            .radix
            .checked_pow(precision as u32)
            .ok_or("formatter scale overflow")?;
        let mut whole = if magnitude & (0xff << 23) == 0 {
            0
        } else {
            real::truncate(magnitude).map_err(|_| "unsupported formatter integer")?
        };
        let fraction = if whole == 0 {
            magnitude
        } else {
            real::sub(
                magnitude,
                real::from_i32(whole).map_err(|_| "unsupported formatter integer")?,
            )
            .map_err(|_| "unsupported formatter fraction")?
        };
        // A subnormal remains below an integer after the admitted scale and rounding bias.
        let scaled = if fraction & (0xff << 23) == 0 {
            0
        } else {
            real::mul(
                fraction,
                real::from_i32(scale).map_err(|_| "unsupported formatter scale")?,
            )
            .map_err(|_| "unsupported formatter multiplication")?
        };
        let rounded =
            real::add(scaled, self.rounding).map_err(|_| "unsupported formatter rounding")?;
        // Both operands are nonnegative; floor followed by native truncation has this result.
        let mut fractional =
            real::truncate(rounded).map_err(|_| "unsupported formatter integer")?;
        if fractional >= scale {
            whole = whole.checked_add(1).ok_or("formatter integer overflow")?;
            fractional -= scale;
        }
        while trim && precision > self.trim_minimum && fractional % self.trim_divisor == 0 {
            precision -= 1;
            fractional /= self.trim_divisor;
        }
        let whole = whole.to_string();
        let fraction = fractional.to_string();
        let field = usize::try_from(width.wrapping_sub(precision).max(self.minimum_width))
            .map_err(|_| "formatter width overflow")?;
        let field = field.max(whole.len());
        let digits = (precision as usize).max(fraction.len());
        let bytes = sign
            .checked_add(field)
            .and_then(|n| n.checked_add(1))
            .and_then(|n| n.checked_add(digits))
            .ok_or("formatter length overflow")?;
        let mut out = self.allocate(bytes)?;
        if negative {
            out.push(self.sign);
        }
        out.extend(std::iter::repeat_n(' ', field - whole.len()));
        out.push_str(&whole);
        out.push(self.separator);
        out.extend(std::iter::repeat_n('0', digits - fraction.len()));
        out.push_str(&fraction);
        Ok(out)
    }
    fn allocate(&self, bytes: usize) -> Result<String, String> {
        if bytes >= self.capacity {
            return Err("real formatter output exceeds selected native buffer".into());
        }
        let mut out = String::new();
        out.try_reserve_exact(bytes)
            .map_err(|_| "real formatter output allocation failed")?;
        Ok(out)
    }
}

#[cfg(test)]
#[path = "script_numeric_format_tests.rs"]
mod tests;
