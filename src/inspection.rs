//! ICC inspection and smoke tests using the established LittleCMS 2 engine.
//! This is deliberately separate from strict LUT channel-bound validation.
use lcms2::{Profile, PixelFormat, Transform, Intent};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct IntentResult {
    pub name: &'static str,
    pub result: Result<[f64; 4], String>,
}
#[derive(Debug, Clone)]
pub struct Inspection {
    pub version: f64,
    pub device_class: String,
    pub color_space: String,
    pub pcs: String,
    pub size: u64,
    pub tags: Vec<String>,
    pub tac: Option<f64>,
    pub intents: Vec<IntentResult>,
}
pub fn inspect(path: &Path) -> Result<Inspection, String> {
    let size = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    if size > 128 * 1024 * 1024 { return Err("ICC exceeds 128 MiB".into()); }
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    let profile = Profile::new_icc(&data).map_err(|e| format!("LittleCMS: {e}"))?;
    let color_space = format!("{:?}", profile.color_space());
    let mut intents = Vec::new();
    let mut tac = None;
    if profile.color_space() == lcms2::ColorSpaceSignature::CmykData {
        let estimate = profile.detect_tac();
        if estimate.is_finite() && estimate > 0.0 && estimate <= 400.0 {
            tac = Some(estimate);
        }
        let srgb = Profile::new_srgb();
        let mut samples = Vec::<[u8;3]>::with_capacity(729);
        for r in 0..=8 { for g in 0..=8 { for b in 0..=8 {
            samples.push([(r*255/8) as u8,(g*255/8) as u8,(b*255/8) as u8]);
        }}}
        for (name,intent) in [("Perceptual",Intent::Perceptual),("Relative colorimetric",Intent::RelativeColorimetric),("Saturation",Intent::Saturation),("Absolute colorimetric",Intent::AbsoluteColorimetric)] {
            let result = (|| -> Result<[f64;4],String> {
                let to_device: Transform<[u8;3],[u16;4]> = Transform::new(&srgb,PixelFormat::RGB_8,&profile,PixelFormat::CMYK_16,intent).map_err(|e|e.to_string())?;
                let to_rgb: Transform<[u16;4],[u8;3]> = Transform::new(&profile,PixelFormat::CMYK_16,&srgb,PixelFormat::RGB_8,intent).map_err(|e|e.to_string())?;
                let mut device = vec![[0u16;4];samples.len()];
                let mut roundtrip = vec![[0u8;3];samples.len()];
                to_device.transform_pixels(&samples,&mut device);
                to_rgb.transform_pixels(&device,&mut roundtrip);
                let mut max = [0.0_f64;4];
                for pixel in device { for j in 0..4 { max[j] = max[j].max(pixel[j] as f64 * 100.0 / 65535.0); }}
                Ok(max)
            })();
            intents.push(IntentResult{name,result});
        }
    }
    Ok(Inspection{
        version:profile.version(),device_class:format!("{:?}",profile.device_class()),
        color_space,pcs:format!("{:?}",profile.pcs()),size,
        tags:profile.tag_signatures().iter().map(|s|format!("{s:?}")).collect(),tac,intents,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn inspects_srgb() {
        let p = Profile::new_srgb();
        let data = p.icc().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("srgb.icc");
        std::fs::write(&path,data).unwrap();
        let result = inspect(&path).unwrap();
        assert!(result.version >= 2.);
        assert!(!result.tags.is_empty());
        assert!(result.intents.is_empty());
    }
    #[test] fn rejects_invalid_profile() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.icc");
        std::fs::write(&path,b"not ICC").unwrap();
        assert!(inspect(&path).is_err());
    }
}
