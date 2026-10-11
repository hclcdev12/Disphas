//! QR code of the attestation, as SVG.

use qrcode::{Color, EcLevel, QrCode};

use crate::ReportError;

/// Quiet zone around the code, in modules (ISO/IEC 18004 asks for 4).
const QUIET_ZONE: usize = 4;

/// Printed width of the code with its quiet zone, in millimetres: the QR
/// image width in `certificate.typ`, to keep in sync.
#[cfg(test)]
pub(crate) const PRINTED_MM: f64 = 55.0;

/// Smallest printed module, in millimetres: about the module of a typical
/// certificate at the former 4.4 cm (0.404 mm), readable when printed
/// (2026-10-11).
#[cfg(test)]
pub(crate) const MIN_MODULE_MM: f64 = 0.40;

/// Printed size of one module of the code for `text`, in millimetres.
#[cfg(test)]
pub(crate) fn printed_module_mm(text: &str) -> Result<f64, ReportError> {
    let (width, _) = modules(text)?;
    Ok(PRINTED_MM / (width + 2 * QUIET_ZONE) as f64)
}

/// Dark modules of the QR code for `text`, row by row, without quiet zone.
/// Error correction level M: about 15 % of the code can be damaged.
pub(crate) fn modules(text: &str) -> Result<(usize, Vec<bool>), ReportError> {
    let code = QrCode::with_error_correction_level(text.as_bytes(), EcLevel::M)
        .map_err(|error| ReportError::Qr(error.to_string()))?;
    let width = code.width();
    let dark = code
        .to_colors()
        .into_iter()
        .map(|color| color == Color::Dark)
        .collect();
    Ok((width, dark))
}

/// SVG of the QR code for `text`, one unit per module, quiet zone included.
pub(crate) fn svg(text: &str) -> Result<String, ReportError> {
    let (width, dark) = modules(text)?;
    let size = width + 2 * QUIET_ZONE;
    let mut path = String::new();
    for (index, _) in dark.iter().enumerate().filter(|(_, dark)| **dark) {
        let x = index % width + QUIET_ZONE;
        let y = index / width + QUIET_ZONE;
        path.push_str(&format!("M{x} {y}h1v1h-1z"));
    }
    Ok(format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {size} {size}" shape-rendering="crispEdges"><rect width="{size}" height="{size}" fill="#ffffff"/><path d="{path}" fill="#000000"/></svg>"##
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_back_to_the_same_text() {
        let text = "OBV1:NCFOXN%TS3DH0RGPJB/IB-OM7533*8PKF$ULLB*ID2E9:/O";
        let (width, dark) = modules(text).expect("encodable");
        let scale = 4;
        let side = (width + 2 * QUIET_ZONE) * scale;
        let mut image = rqrr::PreparedImage::prepare_from_greyscale(side, side, |x, y| {
            let (mx, my) = (x / scale, y / scale);
            let inside = (QUIET_ZONE..QUIET_ZONE + width).contains(&mx)
                && (QUIET_ZONE..QUIET_ZONE + width).contains(&my);
            if inside && dark[(my - QUIET_ZONE) * width + (mx - QUIET_ZONE)] {
                0
            } else {
                255
            }
        });
        let grids = image.detect_grids();
        assert_eq!(grids.len(), 1);
        let (_, decoded) = grids[0].decode().expect("decodable");
        assert_eq!(decoded, text);
    }

    #[test]
    fn svg_is_square_with_a_quiet_zone() {
        let svg = svg("OBV1:ABC").expect("encodable");
        let (width, _) = modules("OBV1:ABC").expect("encodable");
        let size = width + 2 * QUIET_ZONE;
        assert!(svg.contains(&format!("viewBox=\"0 0 {size} {size}\"")));
    }
}
