//! Read-only AMDGPU counters for the adapter selected by wgpu.

#[cfg(target_os = "linux")]
pub(super) fn vram(pci_address: &str) -> Option<(u64, u64)> {
    use std::io::Read;
    if !valid_pci_address(pci_address) {
        return None;
    }
    let device =
        std::path::Path::new("/sys/bus/pci/devices").join(pci_address.to_ascii_lowercase());
    let read = |name: &str| -> Option<String> {
        let mut value = String::new();
        std::fs::File::open(device.join(name))
            .ok()?
            .take(65)
            .read_to_string(&mut value)
            .ok()?;
        (value.len() <= 64).then_some(value)
    };
    totals(&read("mem_info_vram_used")?, &read("mem_info_vram_total")?)
}

#[cfg(not(target_os = "linux"))]
pub(super) fn vram(_pci_address: &str) -> Option<(u64, u64)> {
    None
}

#[cfg(any(target_os = "linux", test))]
fn valid_pci_address(value: &str) -> bool {
    value.len() == 12
        && value.bytes().enumerate().all(|(index, byte)| match index {
            4 | 7 => byte == b':',
            10 => byte == b'.',
            _ => byte.is_ascii_hexdigit(),
        })
}

#[cfg(any(target_os = "linux", test))]
fn totals(used: &str, total: &str) -> Option<(u64, u64)> {
    let parse = |value: &str| {
        let value = value.trim();
        (!value.is_empty() && value.len() <= 64 && value.bytes().all(|byte| byte.is_ascii_digit()))
            .then(|| value.parse::<u64>().ok())
            .flatten()
    };
    let (used, total) = (parse(used)?, parse(total)?);
    (total > 0 && used <= total).then_some((used, total))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_device_addresses_and_counter_values() {
        assert!(valid_pci_address("0123:ab:1f.7"));
        for value in ["", "../../passwd", "0123/ab/1f.7", "0123:ab:zz.7"] {
            assert!(!valid_pci_address(value));
        }
        assert_eq!(totals("4096\n", "8192\n"), Some((4096, 8192)));
        assert_eq!(totals("0", "8192"), Some((0, 8192)));
        for (used, total) in [
            ("1", "0"),
            ("9", "8"),
            ("-1", "8"),
            ("1 MiB", "8"),
            ("18446744073709551616", "8"),
            ("", "8"),
        ] {
            assert_eq!(totals(used, total), None);
        }
    }
}
