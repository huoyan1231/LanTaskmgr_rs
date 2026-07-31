//! 把一段文本（这里是访问链接）生成 SVG 二维码字符串。
//!
//! 原程序用 Gma.QrCodeNet 这个第三方库在桌面 UI 里画二维码。
//! 这里用 qrcode 库自己拼一个 SVG，丢进桌面 UI 的 <img> 里，简单可控。

use qrcode::QrCode;

pub fn svg(text: &str) -> String {
    let code = match QrCode::new(text.as_bytes()) {
        Ok(c) => c,
        Err(_) => return String::new(),
    };
    let size = code.width();
    let scale = 8u32;
    let dim = size as u32 * scale;

    let mut rects = String::new();
    for y in 0..size {
        for x in 0..size {
            if code[(x, y)] == qrcode::Color::Dark {
                rects.push_str(&format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/>",
                    x as u32 * scale,
                    y as u32 * scale,
                    scale,
                    scale
                ));
            }
        }
    }

    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{dim}\" height=\"{dim}\" \
         viewBox=\"0 0 {dim} {dim}\" shape-rendering=\"crispEdges\">\
         <rect width=\"100%\" height=\"100%\" fill=\"#ffffff\"/>\
         <g fill=\"#000000\">{rects}</g></svg>"
    )
}
