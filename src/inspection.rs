//! ICC inspection and smoke tests using the established LittleCMS 2 engine.
//! This is deliberately separate from strict LUT channel-bound validation.
use lcms2::{Profile, PixelFormat, Transform, Intent, Tag, TagSignature, CIEXYZ, CIEXYZExt, ProfileClassSignature};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct IntentResult {
    pub name: &'static str,
    pub result: Result<[f64; 4], String>,
}
/// Stored media white point, not the ICC header illuminant or a new measurement.
#[derive(Debug, Clone)]
pub struct MediaWhite {
    pub xyz: [f64;3],
    pub lab: [f64;3],
    pub srgb: Option<[u8;3]>,
}
fn media_white(profile: &Profile) -> Option<MediaWhite> {
    let Tag::CIEXYZ(value) = profile.read_tag(TagSignature::MediaWhitePointTag) else { return None };
    let xyz = [value.X, value.Y, value.Z];
    if xyz.iter().any(|v| !v.is_finite() || *v < 0.) || xyz[1] <= 0. { return None; }
    let lab = value.to_lab(&CIEXYZ{X:0.9642,Y:1.,Z:0.8249});
    let lab = [lab.L,lab.a,lab.b];
    if lab.iter().any(|v| !v.is_finite()) { return None; }
    // Approximate display swatch: D50 XYZ through the standard sRGB profile.
    // Out-of-display-gamut values can clip; numeric tag values remain unchanged.
    let source=Profile::new_xyz();
    let display=Profile::new_srgb();
    let srgb=Transform::<[f64;3],[u8;3]>::new(&source,PixelFormat::XYZ_DBL,&display,PixelFormat::RGB_8,Intent::RelativeColorimetric).ok().map(|t|{
        let mut pixel=[[0u8;3];1];t.transform_pixels(&[xyz],&mut pixel);pixel[0]
    });
    Some(MediaWhite{xyz,lab,srgb})
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
    pub media_white: Option<MediaWhite>,
    pub is_output: bool,
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
    let media_white=media_white(&profile);
    let is_output=profile.device_class()==ProfileClassSignature::OutputClass;
    Ok(Inspection{
        version:profile.version(),device_class:format!("{:?}",profile.device_class()),
        color_space,pcs:format!("{:?}",profile.pcs()),size,media_white,is_output,
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
    #[test] fn stored_substrate_white_is_not_header_d50() {
        let mut p=crate::synthetic_cmyk_v4(true);
        let mut tag=b"XYZ \0\0\0\0".to_vec();
        let expected=[0.82,0.87,0.65];
        for x in expected { crate::put32(&mut tag,(x*65536_f64).round() as u32); }
        p.tags.insert(*b"wtpt",tag);
        let d=tempfile::tempdir().unwrap();let path=d.path().join("paper.icc");
        p.save_new(&path).unwrap();
        let report=inspect(&path).unwrap();
        assert!(report.is_output);
        let w=report.media_white.unwrap();
        for (x,y) in w.xyz.into_iter().zip(expected){assert!((x-y).abs()<0.00002);}
        assert!(w.lab[0]<100.);assert!(w.srgb.is_some());
    }
    #[test] fn missing_white_does_not_fabricate_d50() {
        let mut p=crate::synthetic_cmyk_v4(true);
        p.tags.remove(b"wtpt");
        let native=Profile::new_icc(&p.bytes().unwrap()).unwrap();
        assert!(media_white(&native).is_none());
    }
    #[test] fn rejects_invalid_profile() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.icc");
        std::fs::write(&path,b"not ICC").unwrap();
        assert!(inspect(&path).is_err());
    }
}
