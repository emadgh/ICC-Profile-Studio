//! Native 16-bit CMYK / nCLR output sampling. No assumption that channel 4
//! is black. LittleCMS pixel format uses PT_MCH2..PT_MCH15 for nCLR.
use lcms2::{Profile,PixelFormat,Transform,Intent,ProfileClassSignature};
use crate::R;
#[derive(Clone,Copy,Debug)]
pub struct Format{pub channels:usize,pub pixel:PixelFormat}
pub fn format(signature:&[u8])->R<Format>{
 if signature==b"CMYK"{return Ok(Format{channels:4,pixel:PixelFormat::CMYK_16})}
 if signature.len()!=4||&signature[1..]!=b"CLR"{return Err("Requires CMYK or 2–15 channel nCLR printer color space".into())}
 let n=(signature[0] as char).to_digit(16).filter(|n|(2..=15).contains(n)).ok_or("Invalid nCLR count")? as usize;
 let pixel=PixelFormat(((14+n as u32)<<16)|((n as u32)<<3)|2);
 if pixel.channels()!=n||pixel.bytes_per_channel()!=2{return Err("Invalid native pixel format".into())}
 Ok(Format{channels:n,pixel})
}
pub fn for_profile(profile:&Profile)->R<Format>{
 if profile.device_class()!=ProfileClassSignature::OutputClass{return Err("Requires output/printer ICC".into())}
 format(&(profile.color_space() as u32).to_be_bytes())
}
pub struct Roundtrip{
 format:Format,
 separation:Transform<[f64;3],u8>,
 appearance:Transform<u8,[f64;3]>,
}
impl Roundtrip{
 pub fn new(profile:&Profile,intent:Intent)->R<Self>{
  let format=for_profile(profile)?;let xyz=Profile::new_xyz();
  let separation=Transform::new(&xyz,PixelFormat::XYZ_DBL,profile,format.pixel,intent).map_err(|e|e.to_string())?;
  let appearance=Transform::new(profile,format.pixel,&xyz,PixelFormat::XYZ_DBL,intent).map_err(|e|e.to_string())?;
  Ok(Self{format,separation,appearance})
 }
 pub fn apply(&self,xyz:&[[f64;3]])->R<Vec<[f64;3]>>{
  let count=xyz.len().checked_mul(self.format.channels*2).ok_or("Device buffer overflow")?;
  let mut device=vec![0u8;count];let mut result=vec![[0.;3];xyz.len()];
  self.separation.transform_pixels(xyz,&mut device);self.appearance.transform_pixels(&device,&mut result);
  if result.iter().flatten().any(|v|!v.is_finite()){return Err("Non-finite native PCS sample".into())}
  Ok(result)
 }
}
pub fn sample_rgb(profile:&Profile,samples:&[[u8;3]],intent:Intent)->R<Vec<f64>>{
 let f=for_profile(profile)?;let rgb=Profile::new_srgb();
 let to:Transform<[u8;3],u8>=Transform::new(&rgb,PixelFormat::RGB_8,profile,f.pixel,intent).map_err(|e|e.to_string())?;
 let from:Transform<u8,[u8;3]>=Transform::new(profile,f.pixel,&rgb,PixelFormat::RGB_8,intent).map_err(|e|e.to_string())?;
 let mut device=vec![0u8;samples.len().checked_mul(f.channels*2).ok_or("Device buffer overflow")?];let mut roundtrip=vec![[0;3];samples.len()];
 to.transform_pixels(samples,&mut device);from.transform_pixels(&device,&mut roundtrip);
 let mut maxima=vec![0f64;f.channels];
 for p in device.chunks_exact(f.channels*2){for (j,v) in p.chunks_exact(2).enumerate(){maxima[j]=maxima[j].max(u16::from_ne_bytes([v[0],v[1]]) as f64*100./65535.);}}
 Ok(maxima)
}
/// Simple-LUT mathematical channel and TAC bounds for dynamic channel count.
/// The sampled post-curve extrema are evaluated by the strict parser; unknown
/// structures are refused, rather than extrapolating from native samples.
#[derive(Clone,Debug)]
pub struct Bounds{pub channels:Vec<f64>,pub tac:f64,pub exact_node_tac:bool}
pub fn bounds(p:&crate::Profile)->R<Bounds>{
 if &p.header[12..16]!=b"prtr"||p.tags.keys().any(|k|k.starts_with(b"B2D")){return Err("Unsupported output profile or alternate B2D".into())}
 let n=format(&p.header[16..20])?.channels;
 let mut b=Bounds{channels:vec![0.;n],tac:0.,exact_node_tac:true};
 for k in [b"B2A0",b"B2A1",b"B2A2"]{
  let l=crate::Lut::parse(p.tags.get(k).ok_or("Missing B2A intent")?)?;
  if l.inputs!=3||l.outputs!=n{return Err("Unexpected output LUT dimensions".into())}
  for j in 0..n{
   let (mut lo,mut hi)=(1f64,0f64);
   for v in l.clut.chunks_exact(n){lo=lo.min(v[j]);hi=hi.max(v[j]);}
   b.channels[j]=b.channels[j].max(crate::validation::interval_peak(&l.out_curves[j],lo,hi));
  }
  if l.out_curves.iter().all(|v|v.len()==2){
   let t=l.clut.chunks_exact(n).map(|v|(0..n).map(|j|crate::interp(&l.out_curves[j],v[j])).sum::<f64>()).fold(0.,f64::max);
   b.tac=b.tac.max(t);
  }else{
   b.exact_node_tac=false;
   // A valid global bound, potentially more conservative than per-cell
   // bounds. Do not call it a sampled or attainable maximum.
   let t=(0..n).map(|j|{
    let (mut lo,mut hi)=(1f64,0f64);for v in l.clut.chunks_exact(n){lo=lo.min(v[j]);hi=hi.max(v[j]);}
    crate::validation::interval_peak(&l.out_curves[j],lo,hi)
   }).sum::<f64>();b.tac=b.tac.max(t);
  }
 }
 Ok(b)
}
#[cfg(test)]
pub(crate) fn fixture(n:usize)->crate::Profile{
 let mut p=crate::synthetic_cmyk_v4(true);
 let sig=format!("{n:X}CLR");p.header[16..20].copy_from_slice(sig.as_bytes());
 let mut forward=Vec::new();
 for i in 0..1usize<<n{
  let ink=|j:usize|((i>>(n-1-j))&1) as f64;
  let sum=(0..n).map(ink).sum::<f64>();
  forward.extend([1.-0.75*sum/n as f64,0.5+0.12*(ink(1)-ink(0)),0.5+0.10*(ink(2.min(n-1))-ink(0))]);
 }
 let a=crate::Lut{inputs:n,outputs:3,grid:2,in_curves:vec![vec![0.,1.];n],out_curves:vec![vec![0.,1.];3],clut:forward};
 let mut reverse=Vec::new();
 for l in 0..2{for a in 0..2{for b in 0..2{
  for j in 0..n{reverse.push(match j{0=>0.30*(1-l) as f64,1=>0.20*a as f64,2=>0.20*b as f64,_ if j==n-1=>0.,_=>0.08*(1-l) as f64});}
 }}}
 let b=crate::Lut{inputs:3,outputs:n,grid:2,in_curves:vec![vec![0.,1.];3],out_curves:vec![vec![0.,1.];n],clut:reverse};
 for k in [b"A2B0",b"A2B1",b"A2B2"]{p.tags.insert(*k,crate::editing::encode(&a,b"mAB ").unwrap());}
 for k in [b"B2A0",b"B2A1",b"B2A2"]{p.tags.insert(*k,crate::editing::encode(&b,b"mBA ").unwrap());}
 p
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn format_counts(){for n in 2..=15{let f=format(format!("{n:X}CLR").as_bytes()).unwrap();assert_eq!(f.channels,n);assert_eq!(f.pixel.bytes_per_pixel(),n*2);}assert!(format(b"RGB ").is_err());assert!(format(b"GRAY").is_err());}
 #[test]fn six_channel_native_and_bounds(){
  let p=fixture(6);let native=Profile::new_icc(&p.bytes().unwrap()).unwrap();
  let r=Roundtrip::new(&native,Intent::RelativeColorimetric).unwrap().apply(&[[0.2,0.3,0.2],[0.8,0.9,0.7]]).unwrap();assert_eq!(r.len(),2);assert_ne!(r[0],r[1]);
  let maxima=sample_rgb(&native,&[[0,0,0],[255,255,255],[90,20,120]],Intent::RelativeColorimetric).unwrap();assert_eq!(maxima.len(),6);assert_eq!(maxima[5],0.);
  let b=bounds(&p).unwrap();assert_eq!(b.channels.len(),6);assert_eq!(b.channels[5],0.);assert!(b.exact_node_tac);assert!(b.tac<=1.);
 }
}
