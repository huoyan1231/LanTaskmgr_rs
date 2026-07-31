//! 获取本机用于「手机访问」的局域网 IPv4 地址。
//!
//! 原程序直接跑 `ipconfig` 并正则抓第一个 IPv4。
//! 这里用 local-ip-address 枚举网卡，挑出「非回环、非虚拟专用（VMware 等）」的地址，
//! 优先选主网卡。仍保留「按可读名字排序」的语义。

use local_ip_address::list_afinet_netifas;

/// 返回 [(ip, 网卡描述)]，按「更适合手机访问」排序。
pub fn lan_addresses() -> Vec<(String, String)> {
    let mut out = Vec::new();

    if let Ok(netifs) = list_afinet_netifas() {
        for (name, ip) in netifs {
            if let std::net::IpAddr::V4(v4) = ip {
                if v4.is_loopback() || is_link_local(&v4) {
                    continue;
                }
                let desc = friendly_name(&name);
                out.push((v4.to_string(), desc));
            }
        }
    }

    // 虚拟网卡往后放
    out.sort_by(|(_, a), (_, b)| is_virtual(a).cmp(&is_virtual(b)));
    out
}

/// 挑一个「最可能是主网卡」的地址。
#[allow(dead_code)]
pub fn primary_lan() -> Option<(String, String)> {
    lan_addresses().into_iter().next()
}

fn is_link_local(ip: &std::net::Ipv4Addr) -> bool {
    ip.octets()[0] == 169 && ip.octets()[1] == 254
}

fn is_virtual(desc: &str) -> bool {
    let d = desc.to_ascii_lowercase();
    d.contains("vmware")
        || d.contains("virtual")
        || d.contains("vethernet")
        || d.contains("hyper-v")
        || d.contains("loopback")
        || d.contains("bluetooth")
}

fn friendly_name(iface: &str) -> String {
    #[cfg(windows)]
    {
        windows_friendly_name(iface).unwrap_or_else(|| iface.to_string())
    }
    #[cfg(not(windows))]
    {
        iface.to_string()
    }
}

#[cfg(windows)]
fn windows_friendly_name(guid: &str) -> Option<String> {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    let base = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(r"SYSTEM\CurrentControlSet\Control\Network\{4d36e972-e325-11ce-bfc1-08002be10318}");

    if let Ok(base) = base {
        for sub in base.enum_keys().filter_map(|k| k.ok()) {
            if let Ok(dev) = base.open_subkey(&sub) {
                if let Ok(conn) = dev.open_subkey("Connection") {
                    if let Ok(g) = conn.get_value::<String, _>("GUID") {
                        if g.eq_ignore_ascii_case(guid) {
                            if let Ok(name) = conn.get_value::<String, _>("Name") {
                                return Some(name);
                            }
                        }
                    }
                }
            }
        }
    }

    // 退化：从 Tcpip\Interfaces 里找 DhcpConnName 与 Name
    let tcpipparams = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(r"SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces");
    if let Ok(tcpipparams) = tcpipparams {
        for sub in tcpipparams.enum_keys().filter_map(|k| k.ok()) {
            if let Ok(iface_key) = tcpipparams.open_subkey(&sub) {
                if let Ok(raw) = iface_key.get_raw_value("DhcpConnName") {
                    let dhcp = String::from_utf16_lossy(
                        &raw.bytes
                            .chunks(2)
                            .map(|c| u16::from_ne_bytes([c[0], *c.get(1).unwrap_or(&0)]))
                            .collect::<Vec<u16>>(),
                    )
                    .trim_end()
                    .to_string();
                    if dhcp.eq_ignore_ascii_case(guid) {
                        if let Ok(raw) = iface_key.get_raw_value("Name") {
                            let name = String::from_utf16_lossy(
                                &raw.bytes
                                    .chunks(2)
                                    .map(|c| u16::from_ne_bytes([c[0], *c.get(1).unwrap_or(&0)]))
                                    .collect::<Vec<u16>>(),
                            );
                            let name = name.trim_end().to_string();
                            if !name.is_empty() {
                                return Some(name);
                            }
                        }
                    }
                }
            }
        }
    }
    None
}
