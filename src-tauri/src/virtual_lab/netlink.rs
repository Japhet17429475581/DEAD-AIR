//! Minimal, bounded use of the public Linux generic-netlink ABI. No frame
//! injection protocol is exposed. These sockets create owned hwsim radios and
//! move one owned wiphy into an already-owned network namespace.
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

pub struct GenericNetlink {
    fd: OwnedFd,
    sequence: u32,
}
fn align(n: usize) -> usize {
    (n + 3) & !3
}
pub fn attribute(kind: u16, data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(align(data.len() + 4));
    out.extend(((data.len() + 4) as u16).to_ne_bytes());
    out.extend(kind.to_ne_bytes());
    out.extend(data);
    out.resize(align(out.len()), 0);
    out
}
pub fn attributes(data: &[u8]) -> Result<Vec<(u16, &[u8])>, String> {
    let mut result = vec![];
    let mut offset = 0;
    while offset < data.len() {
        if data.len() - offset < 4 {
            return Err("Truncated netlink attribute".into());
        }
        let size = u16::from_ne_bytes(data[offset..offset + 2].try_into().unwrap()) as usize;
        let kind = u16::from_ne_bytes(data[offset + 2..offset + 4].try_into().unwrap()) & 0x3fff;
        if size < 4 || size > data.len() - offset {
            return Err("Invalid netlink attribute bounds".into());
        }
        result.push((kind, &data[offset + 4..offset + size]));
        offset += align(size);
        if offset > data.len() {
            return Err("Truncated netlink padding".into());
        }
    }
    Ok(result)
}
impl GenericNetlink {
    pub fn open() -> Result<Self, String> {
        let raw = unsafe {
            libc::socket(
                libc::AF_NETLINK,
                libc::SOCK_RAW | libc::SOCK_CLOEXEC,
                libc::NETLINK_GENERIC,
            )
        };
        if raw < 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let fd = unsafe { OwnedFd::from_raw_fd(raw) };
        // Explicit high random IDs avoid PID/port collisions across netns and
        // old hwsim kernels' socket-close cleanup keyed by port ID.
        let mut bound = false;
        for _ in 0..16 {
            let mut address: libc::sockaddr_nl = unsafe { std::mem::zeroed() };
            address.nl_family = libc::AF_NETLINK as u16;
            address.nl_pid =
                u32::from_ne_bytes(uuid::Uuid::new_v4().as_bytes()[..4].try_into().unwrap())
                    | 0x4000_0000;
            if unsafe {
                libc::bind(
                    raw,
                    (&address as *const libc::sockaddr_nl).cast(),
                    std::mem::size_of_val(&address) as u32,
                )
            } == 0
            {
                bound = true;
                break;
            }
        }
        if !bound {
            return Err("Could not reserve a lab netlink port".into());
        }
        let timeout = libc::timeval {
            tv_sec: 4,
            tv_usec: 0,
        };
        if unsafe {
            libc::setsockopt(
                raw,
                libc::SOL_SOCKET,
                libc::SO_RCVTIMEO,
                (&timeout as *const libc::timeval).cast(),
                std::mem::size_of_val(&timeout) as u32,
            )
        } != 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(Self { fd, sequence: 0 })
    }
    pub fn request(
        &mut self,
        family: u16,
        command: u8,
        attrs: &[u8],
        dump: bool,
    ) -> Result<(i32, Vec<Vec<u8>>), String> {
        self.sequence = self.sequence.wrapping_add(1);
        let mut request = vec![];
        request.extend(((20 + attrs.len()) as u32).to_ne_bytes());
        request.extend(family.to_ne_bytes());
        request.extend((if dump { 0x301u16 } else { 5u16 }).to_ne_bytes());
        request.extend(self.sequence.to_ne_bytes());
        request.extend(0u32.to_ne_bytes());
        request.extend([command, 1, 0, 0]);
        request.extend(attrs);
        let mut kernel: libc::sockaddr_nl = unsafe { std::mem::zeroed() };
        kernel.nl_family = libc::AF_NETLINK as u16;
        let sent = unsafe {
            libc::sendto(
                self.fd.as_raw_fd(),
                request.as_ptr().cast(),
                request.len(),
                0,
                (&kernel as *const libc::sockaddr_nl).cast(),
                std::mem::size_of_val(&kernel) as u32,
            )
        };
        if sent != request.len() as isize {
            return Err(format!(
                "Lab netlink send: {}",
                std::io::Error::last_os_error()
            ));
        }
        let mut responses = vec![];
        let mut total = 0;
        for _ in 0..128 {
            let mut buffer = [0u8; 65536];
            let mut sender: libc::sockaddr_nl = unsafe { std::mem::zeroed() };
            let mut sender_size = std::mem::size_of_val(&sender) as u32;
            let read = unsafe {
                libc::recvfrom(
                    self.fd.as_raw_fd(),
                    buffer.as_mut_ptr().cast(),
                    buffer.len(),
                    0,
                    (&mut sender as *mut libc::sockaddr_nl).cast(),
                    &mut sender_size,
                )
            };
            if read < 0 {
                return Err(format!(
                    "Lab netlink receive: {}",
                    std::io::Error::last_os_error()
                ));
            }
            if sender.nl_pid != 0 {
                continue;
            }
            let data = &buffer[..read as usize];
            let mut offset = 0;
            while offset + 16 <= data.len() {
                let size =
                    u32::from_ne_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
                if size < 16 || size > data.len() - offset {
                    return Err("Invalid netlink message bounds".into());
                }
                let kind = u16::from_ne_bytes(data[offset + 4..offset + 6].try_into().unwrap());
                let sequence =
                    u32::from_ne_bytes(data[offset + 8..offset + 12].try_into().unwrap());
                let payload = &data[offset + 16..offset + size];
                if sequence == self.sequence {
                    if kind == 2 {
                        if payload.len() < 4 {
                            return Err("Truncated netlink acknowledgement".into());
                        }
                        let code = i32::from_ne_bytes(payload[..4].try_into().unwrap());
                        if code < 0 {
                            return Err(format!(
                                "Kernel rejected virtual-radio operation: {}",
                                std::io::Error::from_raw_os_error(-code)
                            ));
                        }
                        return Ok((code, responses));
                    }
                    if kind == 3 {
                        return Ok((0, responses));
                    }
                    if kind == family && payload.len() >= 4 {
                        total += payload.len();
                        if total > 512 * 1024 {
                            return Err("Lab netlink response limit exceeded".into());
                        }
                        responses.push(payload[4..].to_vec());
                    }
                }
                offset += align(size);
            }
        }
        Err("Lab netlink response limit exceeded".into())
    }
    pub fn family(&mut self, name: &str) -> Result<u16, String> {
        let mut name = name.as_bytes().to_vec();
        name.push(0);
        let (_, responses) = self.request(16, 3, &attribute(2, &name), false)?;
        for response in responses {
            for (kind, value) in attributes(&response)? {
                if kind == 1 && value.len() == 2 {
                    return Ok(u16::from_ne_bytes(value.try_into().unwrap()));
                }
            }
        }
        Err("Requested kernel netlink family is unavailable".into())
    }
    pub fn new_radio(&mut self, family: u16, name: &str, mac: [u8; 6]) -> Result<(), String> {
        let mut bytes = name.as_bytes().to_vec();
        bytes.push(0);
        let mut attrs = attribute(17, &bytes);
        attrs.extend(attribute(16, &[]));
        attrs.extend(attribute(18, &[]));
        attrs.extend(attribute(22, &mac));
        self.request(family, 4, &attrs, false)?;
        Ok(())
    }
    pub fn wiphy_index(&mut self, family: u16, name: &str) -> Result<u32, String> {
        let (_, responses) = self.request(family, 1, &[], true)?;
        for response in responses {
            let attrs = attributes(&response)?;
            let candidate = attrs.iter().find(|(k, _)| *k == 2).map(|(_, v)| *v);
            if candidate
                .is_some_and(|value| value.strip_suffix(&[0]).unwrap_or(value) == name.as_bytes())
            {
                if let Some((_, index)) = attrs.iter().find(|(k, v)| *k == 1 && v.len() == 4) {
                    return Ok(u32::from_ne_bytes((*index).try_into().unwrap()));
                }
            }
        }
        Err(format!(
            "Owned virtual radio {name} was not found in the lab namespace"
        ))
    }
    pub fn move_wiphy(&mut self, family: u16, index: u32, namespace_fd: i32) -> Result<(), String> {
        let mut attrs = attribute(1, &index.to_ne_bytes());
        attrs.extend(attribute(219, &namespace_fd.to_ne_bytes()));
        // NL80211_CMD_SET_WIPHY_NETNS (49), not SET_WIPHY (2).
        self.request(family, 49, &attrs, false)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_truncated_and_oversized_attributes() {
        assert!(attributes(&[3, 0, 1, 0]).is_err());
        assert!(attributes(&[20, 0, 1, 0]).is_err());
        assert!(attributes(&[4]).is_err());
    }
    #[test]
    fn attributes_round_trip_alignment_and_flags() {
        let mut data = attribute(17, b"test\0");
        data.extend(attribute(16, &[]));
        let parsed = attributes(&data).unwrap();
        assert_eq!(
            parsed,
            vec![(17, b"test\0".as_slice()), (16, b"".as_slice())]
        );
    }
}
