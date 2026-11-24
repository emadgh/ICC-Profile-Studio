//! ICC DeviceLink generation and profile-level numeric validation.
use std::path::Path;
use lcms2::{Profile,Transform,PixelFormat,Intent,Flags};
type R<T>=Result<T,String>;
fn err(e:impl std::fmt::Display)->String{e.to_string()}
#[derive(Debug,Clone)]
pub struct DeviceLinkReport{pub sample_count:usize,pub max_channel_difference:f64,pub peak_tac:f64,pub maxima:[f64;4]}
pub fn device_link(reference:&Path,candidate:&Path,destination:&Path,intent:Intent)->R<DeviceLinkReport>{
 let a=crate::comparison::open_printer(reference)?;let b=crate::comparison::open_printer(candidate)?;
 let transform:Transform<[u16;4],[u16;4]>=Transform::new(&a,PixelFormat::CMYK_16,&b,PixelFormat::CMYK_16,intent).map_err(err)?;
 let link=Profile::new_device_link(&transform,4.3,Flags::HIGHRES_PRECALC).map_err(err)?;
 let bytes=link.icc().map_err(err)?;
 if bytes.len()<128||&bytes[12..16]!=b"link"||&bytes[16..24]!=b"CMYKCMYK"{return Err("Unexpected DeviceLink class or color spaces".into())}
 let check=Profile::new_icc(&bytes).map_err(err)?;
 let direct:Transform<[u16;4],[u16;4]>=Transform::new_multiprofile(&[&check],PixelFormat::CMYK_16,PixelFormat::CMYK_16,intent,Flags::NO_OPTIMIZE).map_err(err)?;
 let mut samples=Vec::new();
 for i in 0..9usize.pow(4){let mut q=i;let mut v=[0u16;4];for j in (0..4).rev(){v[j]=((q%9)*65535/8) as u16;q/=9;}samples.push(v);}
 let mut expected=vec![[0u16;4];samples.len()];let mut actual=expected.clone();
 transform.transform_pixels(&samples,&mut expected);direct.transform_pixels(&samples,&mut actual);
 let mut report=DeviceLinkReport{sample_count:samples.len(),max_channel_difference:0.,peak_tac:0.,maxima:[0.;4]};
 for (a,b) in expected.iter().zip(actual){
  let total=b.iter().map(|&v|100.*v as f64/65535.).sum::<f64>();report.peak_tac=report.peak_tac.max(total);
  for j in 0..4{report.maxima[j]=report.maxima[j].max(100.*b[j] as f64/65535.);report.max_channel_difference=report.max_channel_difference.max(100.*a[j].abs_diff(b[j]) as f64/65535.);}
 }
 let mut staged=tempfile::NamedTempFile::new_in(destination.parent().unwrap_or(Path::new("."))).map_err(err)?;
 use std::io::Write;staged.write_all(&bytes).map_err(err)?;staged.as_file().sync_all().map_err(err)?;
 staged.persist_noclobber(destination).map_err(err)?;Ok(report)
}

#[cfg(test)]mod tests{
 use super::*;
 #[test]fn native_device_link_samples(){
 let d=tempfile::tempdir().unwrap();let a=d.path().join("a.icc");let b=d.path().join("b.icc");let out=d.path().join("link.icc");
 crate::synthetic_cmyk_v4(false).save_new(&a).unwrap();crate::synthetic_cmyk_v4(true).save_new(&b).unwrap();
 let r=device_link(&a,&b,&out,Intent::RelativeColorimetric).unwrap();
 assert_eq!(r.sample_count,6561);assert!(r.maxima[3]<=0.01);assert!(r.max_channel_difference.is_finite());
 assert!(device_link(&a,&b,&out,Intent::RelativeColorimetric).is_err());
 }
}
